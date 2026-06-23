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

use vgi::Worker;

/// Worker version string, surfaced by `charset_version()`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
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

    let mut worker = Worker::new();
    scalar::register(&mut worker);
    table::register(&mut worker);
    worker.run();
}
