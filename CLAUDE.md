# CLAUDE.md — vgi-charset

Contributor/agent notes. User-facing docs live in `README.md`; this is the
"how it's built and where the sharp edges are" companion.

## What this is

A [VGI](https://query.farm) worker (Rust, compiled binary) exposing
**character-encoding detection** and **UTF-8 transcoding** (mojibake repair) to
DuckDB/SQL over Arrow IPC. Built on the `vgi` crate (crates.io), modeled on
`vgi-units` / `vgi-image` / `vgi-ioc`. Catalog name `charset` (single `main`
schema).

Detection is [`chardetng`](https://docs.rs/chardetng) (the Firefox heuristic);
decode/encode is [`encoding_rs`](https://docs.rs/encoding_rs) (the WHATWG codec
library Firefox uses). Both Apache-2.0/MIT, pure Rust, no native deps, clean on
Rust 1.86 (the workspace MSRV).

## Layout

```
Cargo.toml                          workspace; pins vgi = "0.5.0", arrow 58, chardetng, encoding_rs
crates/charset-worker/
  src/main.rs                       Worker::new(); registers scalars + table
  src/lib.rs                        lib target re-exporting `charset` for integration tests
  src/charset.rs                    PURE engine (no Arrow): detect/decode/encode/fix_mojibake + unit tests
  src/arrow_io.rs                   BLOB + VARCHAR cell reads + in-process scalar test harness
  src/scalar/{detect,decode,encode,version,mod}.rs   thin Arrow scalar adapters
  src/table/{supported,mod}.rs      thin Arrow table-producer adapter
  tests/transcode.rs                integration tests against known byte sequences
test/sql/*.test                     haybarn-unittest sqllogictest — authoritative E2E
Makefile                            test / test-unit / test-sql / lint / fmt / build / clean
```

Pattern: keep computation in `charset.rs` (pure, unit-tested), keep Arrow
marshalling in `arrow_io.rs` + `scalar/*.rs` + `table/*.rs` (thin, harness-tested).

## The pieces

- **Detection** (`detect`): BOM check (`Encoding::for_bom`) first, then
  `chardetng::EncodingDetector` fed the whole buffer (`feed(bytes, true)`,
  `guess(None, true)`). Empty input → `None` (NULL at the SQL boundary).
- **Confidence** (`confidence`): no probability from chardetng; we proxy it from
  the decode — `1.0` if lossless or BOM, else `1 − replaced/total` (U+FFFD
  fraction). See `charset.rs` module docs.
- **Decode** (`to_utf8`, `to_utf8_from`): `Encoding::decode`. Unknown explicit
  label → `Err` → DuckDB ERROR; undecodable bytes → U+FFFD (not an error).
- **Encode** (`transcode`): `Encoding::encode` → BLOB; unmappable chars become
  HTML numeric refs per encoding_rs.
- **fix_mojibake**: re-encode as Windows-1252, re-decode as UTF-8, accept only on
  strict reduction of mojibake markers; no-op otherwise. See `charset.rs`.

## NULL-vs-error policy

Empty/NULL input → NULL everywhere. Unknown encoding **label** (a named codec
that doesn't exist) → DuckDB ERROR. Undecodable **bytes** within a known encoding
→ U+FFFD, never an error. Functions never panic; all input is bounded.

## Sharp edges

1. **`haybarn-unittest` skips `require vgi`** — `.test` files use explicit
   `statement ok` + `LOAD vgi;`. Functions live under the `charset` catalog, so
   each file does `SET search_path = 'charset.main'`, then `USE memory` before
   `DETACH charset`. Determinism: exact assertions over `'\x..'::BLOB` literals.
2. **BLOB literals** in tests are `'\x63\x61\x66\xE9'::BLOB` (windows-1252
   "café"). UTF-16 BOM tests use `\xFF\xFE…` (LE) / `\xFE\xFF…` (BE).
3. **Scalars are positional-only**; `to_utf8_from(bytes, encoding)` and
   `transcode(text, to_encoding)` read a second positional column (`ArgSpec`
   index 1), no named/optional args.
4. **STRUCT returns** would need explicit Arrow `DataType` matching bind↔process
   — this worker returns only scalar types (VARCHAR/DOUBLE/BOOLEAN/BLOB), so no
   STRUCT plumbing is needed here.

## Testing

- Pure-engine + Arrow-boundary unit tests live in each module (`#[cfg(test)]`),
  driven via the in-process harness in `arrow_io::test_support`.
- `tests/transcode.rs` hits the library surface with known byte sequences.
- `test/sql/*.test` is the authoritative E2E (run with `make test-sql`).
