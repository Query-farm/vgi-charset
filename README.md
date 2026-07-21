<p align="center">
  <img src="https://raw.githubusercontent.com/Query-farm/vgi/main/docs/vgi-logo.png" alt="Vector Gateway Interface (VGI)" width="320">
</p>

<p align="center"><em>A <a href="https://query.farm">Query.Farm</a> VGI worker for DuckDB.</em></p>

# Detect Text Encoding & Transcode to UTF-8 (Fix Mojibake) in DuckDB

> **vgi-charset** · a [Query.Farm](https://query.farm) VGI worker

A [VGI](https://query.farm) worker that brings **character-encoding detection**
and **UTF-8 transcoding** (mojibake repair) to DuckDB over Apache Arrow.

```sql
LOAD vgi;
ATTACH 'charset' (TYPE vgi, LOCATION './target/release/charset-worker');
SET search_path = 'charset.main';

SELECT detect_encoding('\x63\x61\x66\xE9'::BLOB);    -- 'windows-1252'
SELECT detect_confidence('\x63\x61\x66\xE9'::BLOB);  -- 1.0
SELECT to_utf8('\x63\x61\x66\xE9'::BLOB);            -- 'café'
SELECT to_utf8_from('\x93\xFA\x96\x7B\x8C\xEA'::BLOB, 'shift_jis');  -- '日本語'
SELECT transcode('café', 'windows-1252');            -- BLOB \x63\x61\x66\xE9
SELECT fix_mojibake('CafÃ©');                         -- 'Café'
SELECT is_valid_utf8('café'::BLOB);                  -- true
SELECT * FROM supported_encodings();                 -- discovery: label
```

## What it does

Dirty text data arrives in every legacy encoding imaginable — Windows-1252 smart
quotes, Shift_JIS, GBK, Latin-1 — and frequently *double-encoded* (UTF-8 read as
Latin-1 then re-stored as UTF-8, so `é` shows up as `Ã©`). This worker detects
the encoding of raw bytes and normalizes everything to UTF-8, with an explicit
escape hatch (`to_utf8_from`) when you already know the codec, an exporter
(`transcode`) for writing legacy bytes back out, and a `fix_mojibake` repair for
the classic double-encoding case.

## Detection and decoding libraries

| Concern | Crate | License | Notes |
| --- | --- | --- | --- |
| Encoding **detection** | [`chardetng`](https://docs.rs/chardetng) | Apache-2.0 / MIT (Mozilla) | The heuristic Firefox uses for legacy/unlabelled text. Pure Rust, no native deps. Returns a single best-guess encoding (no probability). |
| **Decode / encode** | [`encoding_rs`](https://docs.rs/encoding_rs) | Apache-2.0 / MIT (Mozilla) | The WHATWG Encoding Standard codec library Firefox uses. Maps every web-platform label to a codec and does the lossy U+FFFD / HTML-numeric-reference handling. |

Both are MSRV-friendly (build clean on Rust 1.86) and pure Rust.

**Detection order:** `detect_encoding` checks for a Unicode **BOM** first
(UTF-8 / UTF-16LE / UTF-16BE) — an explicit, unambiguous declaration — and only
falls back to the `chardetng` heuristic when there is no BOM.

**Confidence proxy:** `chardetng` exposes no probability, so `detect_confidence`
derives a value in `[0, 1]` from the *decode result* (the property callers
actually care about): `1.0` when the bytes decode with **no** U+FFFD replacements
(lossless) or a BOM was present, otherwise `1 − replaced/total` scaled by the
fraction of decoded characters that came out as U+FFFD, and `0.0` for empty
input.

## Function surface

Scalars (positional-only):

| Function | Signature | Notes |
| --- | --- | --- |
| `detect_encoding` | `detect_encoding(bytes BLOB) -> VARCHAR` | Detected encoding label; **NULL** for empty/NULL. BOM check, then `chardetng`. |
| `detect_confidence` | `detect_confidence(bytes BLOB) -> DOUBLE` | `[0,1]` proxy from decode losslessness; NULL for empty/NULL. |
| `to_utf8` | `to_utf8(bytes BLOB) -> VARCHAR` | Detect + decode to UTF-8 (U+FFFD for undecodable bytes); NULL for empty/NULL. |
| `to_utf8_from` | `to_utf8_from(bytes BLOB, encoding VARCHAR) -> VARCHAR` | Decode with an **explicit** label (no detection). **ERROR** on unknown label; NULL for NULL bytes. |
| `transcode` | `transcode(text VARCHAR, to_encoding VARCHAR) -> BLOB` | Encode a UTF-8 string into the named encoding (export). **ERROR** on unknown label; NULL for NULL. |
| `is_valid_utf8` | `is_valid_utf8(bytes BLOB) -> BOOLEAN` | Already valid UTF-8? NULL for NULL. |
| `fix_mojibake` | `fix_mojibake(text VARCHAR) -> VARCHAR` | Repair double-encoded mojibake; no-ops when it can't improve. NULL for NULL. |

The worker build version is exposed as the catalog's `implementation_version` (read it
from `vgi_catalogs()`), not as a scalar function.

Table function:

| Function | Columns |
| --- | --- |
| `supported_encodings()` | `label VARCHAR` — every encoding label the worker accepts (the `encoding_rs` set) |

### NULL-vs-error policy

**Empty / NULL input → NULL** everywhere (nothing to detect or decode). An
**unknown encoding label** passed to `to_utf8_from` / `transcode` is a caller bug
— the named codec doesn't exist — so it raises a DuckDB **ERROR**. Undecodable
*bytes* within a known encoding are not an error: they decode to U+FFFD.

### `transcode` unmappable characters

`transcode` uses `encoding_rs`'s encoder. Characters the target encoding cannot
represent are emitted as **HTML numeric character references** (e.g. `&#1234;`)
for the legacy single-/multi-byte encodings — exactly what a browser does when
form-submitting in a legacy encoding. The UTF-8 / UTF-16 encoders are lossless.

### The mojibake-fix heuristic

`fix_mojibake` repairs the classic case where UTF-8 bytes were decoded as
Latin-1 / Windows-1252 and then re-stored as UTF-8 (so `é → Ã©`, `" → â€œ`):

1. Re-encode the input string as **Windows-1252**. If any character isn't
   representable, the text isn't 1252-mojibake → **no-op**.
2. Decode those bytes as **UTF-8**. If invalid, **no-op**.
3. Accept the repair only if it **strictly reduced** the count of tell-tale
   mojibake marker characters (`Ã Â â € ™ … “ ” ‘ ’`). Otherwise **no-op**.

The strict-improvement gate stops it from mangling text that was never mojibake.

## Development

```sh
make test       # cargo unit/integration tests + SQL E2E
make test-unit  # cargo test --workspace
make test-sql   # build release worker + DuckDB sqllogictest suite (haybarn-unittest)
make lint       # clippy (deny warnings) + rustfmt --check
make fmt        # rustfmt the workspace
```

The SQL E2E suite uses [`haybarn-unittest`](https://query.farm)
(`uv tool install haybarn-unittest`).

## License

MIT — see [LICENSE](LICENSE). Bundles `chardetng` and `encoding_rs` (both
Apache-2.0 / MIT).

---

## Authorship & License

Written by [Query.Farm](https://query.farm).

Copyright 2026 Query Farm LLC - https://query.farm

