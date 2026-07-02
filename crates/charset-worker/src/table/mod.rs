//! Table functions exposed by the charset worker, registered under `charset.main`.

mod supported;

use std::sync::Arc;

use vgi::catalog::CatTable;

/// Build the catalog `CatTable` that exposes `supported_encodings` as a regular
/// table (`SELECT * FROM charset.main.supported_encodings`, no parentheses —
/// VGI311), backed by the [`supported::SupportedEncodings`] scan function.
///
/// `CatTable::with_function` stores the function instance, so
/// [`vgi::Worker::set_catalog`] auto-registers it into the dispatch table; no
/// separate `register_table` call is needed. The table carries the same
/// discovery tags, example queries, a documented column, and a primary key as
/// any well-documented catalog object so it lints clean on its own.
pub fn supported_encodings_table() -> CatTable {
    let mut t = CatTable::with_function(
        "supported_encodings",
        supported::output_schema(),
        Arc::new(supported::SupportedEncodings),
        Some(
            "Every encoding label the worker accepts (the encoding_rs / WHATWG set of canonical \
             encoding names)."
                .to_string(),
        ),
        Some(crate::charset::supported_encodings().len() as i64),
    );
    // `label` (column 0) is the unique row identity — declare it the primary key
    // and the table's NOT NULL/unique constraint.
    t.primary_key = vec![vec![0]];
    t.not_null = vec![0];
    t.unique = vec![vec![0]];
    t.tags = vec![
        (
            "vgi.title".to_string(),
            "Supported Encodings Catalog".to_string(),
        ),
        (
            "vgi.doc_llm".to_string(),
            "Every encoding label the worker accepts — the encoding_rs / WHATWG set of canonical \
             encoding names. Query it to discover which labels are valid inputs to to_utf8_from \
             and transcode."
                .to_string(),
        ),
        (
            "vgi.doc_md".to_string(),
            "# supported_encodings\n\nThe discovery table of every encoding label the worker \
             accepts. One row per encoding, with a single `label` column (e.g. `UTF-8`, \
             `windows-1252`, `Shift_JIS`). Use it to find valid encoding inputs for `to_utf8_from` \
             and `transcode`."
                .to_string(),
        ),
        (
            "vgi.keywords".to_string(),
            crate::meta::keywords_json(
                "supported encodings, list encodings, available codecs, encoding catalog, \
                 discovery, what encodings, WHATWG, encoding_rs, labels",
            ),
        ),
        ("domain".to_string(), "text-processing".to_string()),
        ("category".to_string(), "discovery".to_string()),
        ("topic".to_string(), "encoding-catalog".to_string()),
        // VGI413: name one of the schema's `vgi.categories`.
        ("vgi.category".to_string(), "Discovery".to_string()),
        (
            "vgi.example_queries".to_string(),
            r#"[
  {
    "description": "List the first few supported encoding labels.",
    "sql": "SELECT label FROM charset.main.supported_encodings ORDER BY label LIMIT 5"
  },
  {
    "description": "Count how many encoding labels the worker accepts.",
    "sql": "SELECT count(*) AS n FROM charset.main.supported_encodings"
  }
]"#
            .to_string(),
        ),
    ];
    t
}
