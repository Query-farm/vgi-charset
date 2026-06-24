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
mod scalar;
mod table;

use vgi::catalog::{CatSchema, CatalogModel};
use vgi::Worker;

/// Worker version string, surfaced by `charset_version()`.
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
                "vgi.description_llm".to_string(),
                "Detect the character encoding of raw text bytes (BOM check plus the Firefox \
                 chardetng heuristic), decode legacy/unlabelled bytes to UTF-8 (auto-detected or \
                 with an explicit codec label like 'shift_jis'), encode UTF-8 back into a legacy \
                 encoding's bytes, repair double-encoded mojibake such as 'CafÃ©' -> 'Café', test \
                 whether bytes are already valid UTF-8, and list every supported encoding label. \
                 Use for cleaning up imported text, fixing garbled characters, and normalising \
                 mixed-encoding data to UTF-8 in SQL."
                    .to_string(),
            ),
            (
                "vgi.description_md".to_string(),
                "# charset\n\nCharacter-encoding detection and UTF-8 transcoding over Apache \
                 Arrow, powered by Mozilla's `chardetng` (detection) and `encoding_rs` (the \
                 WHATWG codec library).\n\nScalars: `detect_encoding`, `detect_confidence`, \
                 `is_valid_utf8`, `to_utf8`, `to_utf8_from`, `transcode`, `fix_mojibake`, \
                 `charset_version`. Table: `supported_encodings`."
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
        ],
        source_url: Some("https://github.com/Query-farm/vgi-charset".to_string()),
        schemas: vec![CatSchema {
            name: "main".to_string(),
            comment: Some(
                "Character-encoding detection and UTF-8 transcoding functions.".to_string(),
            ),
            tags: vec![
                (
                    "vgi.description_llm".to_string(),
                    "Character-encoding functions: detect the encoding of text bytes, decode bytes \
                     to UTF-8 (auto-detected or with an explicit label), encode UTF-8 into a \
                     legacy encoding, repair double-encoded mojibake, test for valid UTF-8, and \
                     enumerate supported encodings."
                        .to_string(),
                ),
                (
                    "vgi.description_md".to_string(),
                    "Character-encoding detection and UTF-8 transcoding functions over Apache \
                     Arrow."
                        .to_string(),
                ),
            ],
            views: Vec::new(),
            macros: Vec::new(),
            tables: Vec::new(),
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
    table::register(&mut worker);
    worker.set_catalog(catalog_metadata(&catalog_name));
    worker.run();
}
