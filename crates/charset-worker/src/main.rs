//! The `charset` VGI worker.
//!
//! A standalone binary that DuckDB launches and talks to over Apache Arrow IPC
//! (`ATTACH 'charset' (TYPE vgi, LOCATION '…')`). It brings character-encoding
//! detection and UTF-8 transcoding (mojibake repair) to SQL under the catalog
//! `charset`, schema `main`:
//!
//! ```sql
//! ATTACH 'charset' (TYPE vgi, LOCATION './target/release/charset-worker');
//! SET search_path = 'charset.main';
//!
//! SELECT detect_encoding('\x63\x61\x66\xE9'::BLOB);    -- 'windows-1252'
//! SELECT detect_confidence('caf\xE9'::BLOB);           -- 1.0
//! SELECT to_utf8('\x63\x61\x66\xE9'::BLOB);            -- 'café'
//! SELECT to_utf8_from('\x93\xFA'::BLOB, 'shift_jis');  -- explicit decode
//! SELECT transcode('café', 'windows-1252');            -- BLOB export
//! SELECT fix_mojibake('CafÃ©');                         -- 'Café'
//! SELECT is_valid_utf8('café'::BLOB);                  -- true
//! SELECT * FROM supported_encodings();                 -- discovery
//! ```
//!
//! The pure detection/transcoding engine lives in `charset.rs`; the `scalar/`
//! and `table/` modules are thin Arrow adapters over it.

mod arrow_io;
mod charset;
mod meta;
mod scalar;
mod table;

use vgi::catalog::{CatSchema, CatalogModel};
use vgi::Worker;

/// Worker version string, surfaced to discovery as the catalog's
/// `implementation_version` (readable from `vgi_catalogs()` without spending a
/// query, and it can't drift from the running build).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Catalog + schema metadata (description, provenance, support, license)
/// surfaced to DuckDB and the `vgi-lint` metadata-quality linter. The function
/// objects themselves are served from the registered scalars/table; this only
/// adds catalog/schema-level comments and tags.
fn catalog_metadata(name: &str) -> CatalogModel {
    CatalogModel {
        name: name.to_string(),
        comment: Some(
            "Character-encoding detection and UTF-8 transcoding (mojibake repair).".to_string(),
        ),
        tags: vec![
            (
                "vgi.title".to_string(),
                "Character Encoding Detection & Transcoding".to_string(),
            ),
            (
                "vgi.keywords".to_string(),
                meta::keywords_json(
                    "charset, character encoding, encoding detection, transcode, transcoding, \
                     UTF-8, mojibake, mojibake repair, decode, encode, windows-1252, latin-1, \
                     shift_jis, chardetng, encoding_rs, BOM, garbled text",
                ),
            ),
            (
                "vgi.doc_llm".to_string(),
                "Detect the character encoding of raw text bytes (BOM check plus the Firefox \
                 chardetng heuristic), decode legacy/unlabelled bytes to UTF-8 (auto-detected or \
                 with an explicit codec label like 'shift_jis'), encode UTF-8 back into a legacy \
                 encoding's bytes, repair double-encoded mojibake such as 'CafÃ©' -> 'Café', test \
                 whether bytes are already valid UTF-8, and list every supported encoding label. \
                 Use for cleaning up imported text, fixing garbled characters, and normalising \
                 mixed-encoding data to UTF-8 in SQL. Detection is heuristic, so pair \
                 detect_encoding with detect_confidence before trusting a guess on short input."
                    .to_string(),
            ),
            (
                "vgi.doc_md".to_string(),
                "# charset — detect text encoding and transcode to UTF-8 in SQL\n\n\
                 Detect the character encoding of raw bytes and transcode legacy or \
                 mojibake-garbled text into clean UTF-8 directly in DuckDB SQL — no Python \
                 preprocessing step and no manual `iconv` passes.\n\n\
                 ## What it does\n\n\
                 `charset` is a [VGI](https://query.farm) worker that adds character-encoding \
                 detection and UTF-8 transcoding to DuckDB over Apache Arrow. It is built for \
                 data engineers and analysts wrangling text of unknown or mixed provenance: CSV \
                 and log dumps in legacy Windows code pages, scraped HTML, email archives, and \
                 columns that arrived double-encoded — the classic `CafÃ©`-instead-of-`Café` \
                 mojibake. Rather than shelling out to external tools, you detect, repair, and \
                 normalize encodings inline with ordinary SQL queries.\n\n\
                 ## How it works\n\n\
                 Encoding detection is powered by Mozilla's \
                 [`chardetng`](https://github.com/hsivonen/chardetng) \
                 ([docs](https://docs.rs/chardetng)), the same heuristic Firefox applies to \
                 unlabelled legacy text. Decoding and encoding use \
                 [`encoding_rs`](https://github.com/hsivonen/encoding_rs) \
                 ([docs](https://docs.rs/encoding_rs)), the pure-Rust implementation of the \
                 [WHATWG Encoding Standard](https://encoding.spec.whatwg.org/) that Firefox \
                 ships for every web-platform codec. A byte-order-mark (BOM) check runs first; \
                 undecodable bytes within a known encoding become the U+FFFD replacement \
                 character rather than raising an error, while an unknown encoding *label* is \
                 rejected so typos surface immediately.\n\n\
                 ## When to reach for it\n\n\
                 Reach for `charset` whenever a text column's bytes are of unknown, mixed, or \
                 legacy provenance: sniff the likely encoding and score how trustworthy that \
                 guess is, decode legacy bytes to UTF-8 either automatically or with an explicit \
                 codec label, re-encode UTF-8 back into a legacy codec for export, repair \
                 double-encoded mojibake, and check whether bytes are already clean UTF-8. \
                 Because detection is heuristic, confidence-check short or ambiguous samples \
                 before relying on a result. Empty or NULL input yields NULL everywhere."
                    .to_string(),
            ),
            ("vgi.author".to_string(), "Query.Farm".to_string()),
            (
                "vgi.copyright".to_string(),
                "Copyright 2026 Query Farm LLC - https://query.farm".to_string(),
            ),
            ("vgi.license".to_string(), "MIT".to_string()),
            (
                "vgi.support_contact".to_string(),
                "https://github.com/Query-farm/vgi-charset/issues".to_string(),
            ),
            (
                "vgi.support_policy_url".to_string(),
                "https://github.com/Query-farm/vgi-charset/blob/main/README.md".to_string(),
            ),
            // VGI152/VGI920: analyst tasks that `vgi-lint simulate` runs to measure
            // how well an agent can actually use this worker. Kept BLOB-literal-free
            // (nested transcode/casts) so they are robust across DuckDB versions.
            (
                "vgi.agent_test_tasks".to_string(),
                "[\
                 {\"name\":\"detect-utf8\",\
                 \"prompt\":\"What encoding does this worker detect for the UTF-8 byte \
                 representation of the text 'café'? Return only the single detected encoding \
                 label as one column.\",\
                 \"reference_sql\":\"SELECT charset.main.detect_encoding(charset.main.transcode('café', 'utf-8'));\",\
                 \"ignore_column_names\":true},\
                 {\"name\":\"detect-windows-1252\",\
                 \"prompt\":\"What encoding does this worker detect for the windows-1252 byte \
                 representation of the text 'café'? Return only the single detected encoding \
                 label as one column.\",\
                 \"reference_sql\":\"SELECT charset.main.detect_encoding(charset.main.transcode('café', 'windows-1252'));\",\
                 \"ignore_column_names\":true},\
                 {\"name\":\"decode-legacy\",\
                 \"prompt\":\"Take the windows-1252 byte encoding of 'café' and decode it back into \
                 UTF-8 text.\",\
                 \"reference_sql\":\"SELECT charset.main.to_utf8(charset.main.transcode('café', 'windows-1252'));\",\
                 \"ignore_column_names\":true},\
                 {\"name\":\"fix-mojibake\",\
                 \"prompt\":\"Repair the double-encoded mojibake string 'CafÃ©' into correct \
                 UTF-8.\",\
                 \"reference_sql\":\"SELECT charset.main.fix_mojibake('CafÃ©');\",\
                 \"ignore_column_names\":true},\
                 {\"name\":\"is-valid-utf8\",\
                 \"prompt\":\"Treating the ASCII text 'hello' as raw bytes, does this worker \
                 consider them valid UTF-8? Return only the single boolean result as one \
                 column.\",\
                 \"reference_sql\":\"SELECT charset.main.is_valid_utf8('hello'::BLOB);\",\
                 \"ignore_column_names\":true},\
                 {\"name\":\"count-encodings\",\
                 \"prompt\":\"How many distinct character encodings does this worker support?\",\
                 \"reference_sql\":\"SELECT count(*) FROM charset.main.supported_encodings();\",\
                 \"ignore_column_names\":true},\
                 {\"name\":\"confidence-threshold\",\
                 \"prompt\":\"Does this worker report full confidence — a score of exactly 1.0 — \
                 for the UTF-8 byte representation of the text 'hello'? Return only a single \
                 boolean value as one column.\",\
                 \"reference_sql\":\"SELECT charset.main.detect_confidence(charset.main.transcode('hello', 'utf-8')) = 1.0;\",\
                 \"ignore_column_names\":true},\
                 {\"name\":\"decode-explicit-label\",\
                 \"prompt\":\"Using the explicit-codec decode function (not auto-detection), decode \
                 the windows-1252 byte representation of 'café' by naming the codec \
                 'windows-1252'. Return the decoded text.\",\
                 \"reference_sql\":\"SELECT charset.main.to_utf8_from(charset.main.transcode('café', 'windows-1252'), 'windows-1252');\",\
                 \"ignore_column_names\":true}\
                 ]"
                    .to_string(),
            ),
        ],
        source_url: Some("https://github.com/Query-farm/vgi-charset".to_string()),
        // Worker build version, surfaced via `vgi_catalogs().implementation_version`
        // so an agent can read it from discovery without a dedicated function call
        // (replaces the retired `charset_version()` scalar — VGI328).
        implementation_version: Some(version().to_string()),
        schemas: vec![CatSchema {
            name: "main".to_string(),
            comment: Some(
                "Character-encoding detection and UTF-8 transcoding functions.".to_string(),
            ),
            tags: vec![
                ("vgi.title".to_string(), "Charset — main".to_string()),
                (
                    "vgi.keywords".to_string(),
                    meta::keywords_json(
                        "charset, character encoding, detect_encoding, detect_confidence, \
                         is_valid_utf8, to_utf8, to_utf8_from, transcode, fix_mojibake, \
                         supported_encodings, mojibake, UTF-8, decode, encode, transcoding",
                    ),
                ),
                // VGI123 classifying tags (bare keys: domain/category/topic) for faceting.
                ("domain".to_string(), "text-processing".to_string()),
                ("category".to_string(), "character-encoding".to_string()),
                (
                    "topic".to_string(),
                    "encoding-detection-and-transcoding".to_string(),
                ),
                // VGI413 category registry: each object carries a `vgi.category`
                // (set via meta::object_tags) naming one of these, in listing order.
                (
                    "vgi.categories".to_string(),
                    "[\
                     {\"name\":\"Detection\",\"description\":\"Identify the character encoding \
                     of raw bytes, score confidence in the guess, and test for valid UTF-8.\"},\
                     {\"name\":\"Decoding\",\"description\":\"Turn legacy or unlabelled bytes \
                     into clean UTF-8, auto-detected or with an explicit codec label.\"},\
                     {\"name\":\"Encoding & Repair\",\"description\":\"Encode UTF-8 back into a \
                     legacy codec's bytes and repair double-encoded mojibake.\"},\
                     {\"name\":\"Discovery\",\"description\":\"Enumerate the supported encoding \
                     labels the worker accepts.\"}\
                     ]"
                        .to_string(),
                ),
                (
                    "vgi.doc_llm".to_string(),
                    "The `main` schema of the charset worker. It exposes character-encoding \
                     functions: detect the encoding of text bytes, score detection confidence, \
                     test for valid UTF-8, decode bytes to UTF-8 (auto-detected or with an \
                     explicit label), encode UTF-8 into a legacy encoding, repair double-encoded \
                     mojibake, and enumerate supported encodings. All functions are catalog- \
                     qualified as charset.main.<fn>(...) and operate row-wise over Arrow."
                        .to_string(),
                ),
                (
                    "vgi.doc_md".to_string(),
                    "## charset.main\n\n\
                     Character-encoding detection and UTF-8 transcoding over Apache Arrow. \
                     The `main` schema groups its functions into detection (identify an \
                     encoding and score confidence), decoding (turn legacy or unlabelled bytes \
                     into clean UTF-8), encoding and repair (write UTF-8 back into a legacy \
                     codec and fix double-encoded mojibake), and discovery (enumerate supported \
                     encodings). Every function is catalog-qualified as \
                     `charset.main.<fn>(...)` and operates row-wise.\n\n\
                     ### Usage\n\n\
                     ```sql\n\
                     SELECT charset.main.to_utf8('\\x63\\x61\\x66\\xE9'::BLOB); -- 'café'\n\
                     ```"
                    .to_string(),
                ),
                // VGI506/VGI515 representative example queries for the schema, each
                // a described {"description","sql"} object.
                (
                    "vgi.example_queries".to_string(),
                    meta::example_queries_json(&[
                        (
                            "Detect the character encoding of windows-1252 bytes for \"café\".",
                            "SELECT charset.main.detect_encoding('\\x63\\x61\\x66\\xE9'::BLOB);",
                        ),
                        (
                            "Auto-detect and decode windows-1252 bytes to a UTF-8 string.",
                            "SELECT charset.main.to_utf8('\\x63\\x61\\x66\\xE9'::BLOB);",
                        ),
                        (
                            "Decode Shift-JIS bytes to UTF-8 with an explicit codec label.",
                            "SELECT charset.main.to_utf8_from('\\x93\\xFA\\x96\\x7B'::BLOB, \
                             'shift_jis');",
                        ),
                        (
                            "Encode a UTF-8 string into windows-1252 bytes for export.",
                            "SELECT charset.main.transcode('café', 'windows-1252');",
                        ),
                        (
                            "Repair double-encoded mojibake back to clean UTF-8.",
                            "SELECT charset.main.fix_mojibake('CafÃ©');",
                        ),
                        (
                            "Check whether bytes are already valid UTF-8.",
                            "SELECT charset.main.is_valid_utf8('\\x63\\x61\\x66\\xC3\\xA9'::BLOB);",
                        ),
                        (
                            "List the first few supported encoding labels.",
                            "SELECT * FROM charset.main.supported_encodings() LIMIT 5;",
                        ),
                    ]),
                ),
            ],
            views: Vec::new(),
            macros: Vec::new(),
            // Expose the parameterless `supported_encodings` scan as a regular
            // table (VGI311) so `SELECT * FROM charset.main.supported_encodings`
            // works without parentheses. `with_function` auto-registers the
            // backing scan function via `set_catalog`.
            tables: vec![table::supported_encodings_table()],
        }],
        ..Default::default()
    }
}

fn main() {
    // Logs MUST go to stderr — stdout is the Arrow-IPC channel.
    let _ = env_logger::Builder::from_env(env_logger::Env::default().filter_or("VGI_LOG", "info"))
        .format_timestamp_millis()
        .try_init();

    // The catalog name DuckDB sees in `ATTACH 'charset' (TYPE vgi, …)`. Default
    // to `charset`, but honor an explicit override so a test harness can rename.
    if std::env::var_os("VGI_WORKER_CATALOG_NAME").is_none() {
        std::env::set_var("VGI_WORKER_CATALOG_NAME", "charset");
    }
    let catalog_name =
        std::env::var("VGI_WORKER_CATALOG_NAME").unwrap_or_else(|_| "charset".to_string());

    let mut worker = Worker::new();
    scalar::register(&mut worker);
    // The `supported_encodings` table function is auto-registered by
    // `set_catalog` via the `CatTable::with_function` entry in
    // `catalog_metadata`, so no separate `register_table` call is needed.
    worker.set_catalog(catalog_metadata(&catalog_name));
    worker.run();
}
