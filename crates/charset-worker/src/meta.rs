//! Shared helpers for the per-object discovery/description metadata that the
//! `vgi-lint` strict profile expects on **every** function and table.
//!
//! Each function/table surfaces these in its `FunctionMetadata.tags`:
//! - `vgi.title` (VGI124)        — human-friendly display name
//! - `vgi.doc_llm` (VGI112)      — Markdown narrative aimed at LLMs/agents
//! - `vgi.doc_md` (VGI113)       — Markdown narrative aimed at human docs
//! - `vgi.keywords` (VGI126/VGI138) — a JSON array of search terms/synonyms
//!
//! Per-object `vgi.source_url` links are intentionally omitted: provenance lives
//! once on the catalog object (VGI139), so repeating a blob URL on every function
//! is redundant. The catalog's `source_url` field is the single source of truth.

/// Serialize a comma-separated list of keywords into the JSON-array form that
/// the metadata linter expects for `vgi.keywords` (VGI138): each trimmed,
/// non-empty term becomes one element of a JSON string array, e.g.
/// `keywords_json("a, b")` → `["a","b"]`. JSON-escaping is handled so quotes or
/// backslashes in a keyword cannot break the array.
pub fn keywords_json(comma_separated: &str) -> String {
    let items: Vec<String> = comma_separated
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|kw| format!("\"{}\"", json_escape(kw)))
        .collect();
    format!("[{}]", items.join(","))
}

/// JSON-escape a string for embedding as a JSON string literal — enough for the
/// characters our metadata carries (quotes, backslashes, control chars). The SQL
/// in an example query contains backslash byte-escapes (`'\xE9'`), so escaping
/// backslashes correctly is load-bearing.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Serialize `(description, sql)` pairs into the described-example JSON list the
/// metadata linter requires for `vgi.example_queries` (VGI515): a JSON array of
/// `{"description": "...", "sql": "..."}` objects so every example carries a
/// human-readable description (and is executed + coverage-checked).
pub fn example_queries_json(examples: &[(&str, &str)]) -> String {
    let items: Vec<String> = examples
        .iter()
        .map(|(description, sql)| {
            format!(
                "{{\"description\":\"{}\",\"sql\":\"{}\"}}",
                json_escape(description),
                json_escape(sql)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// Build the standard per-object discovery/description tags.
///
/// `keywords` is supplied as a convenient comma-separated string and serialized
/// to the required JSON-array form for `vgi.keywords`. `category` names one of
/// the schema's `vgi.categories` (VGI413), surfaced as `vgi.category` so the
/// object joins the worker's navigation/listing sections. `examples` is a slice
/// of `(description, sql)` pairs serialized into the `vgi.example_queries` tag
/// (VGI515) — each example thus carries a description and is executed +
/// coverage-checked; pass `&[]` for objects whose examples live elsewhere.
pub fn object_tags(
    title: &str,
    doc_llm: &str,
    doc_md: &str,
    keywords: &str,
    category: &str,
    examples: &[(&str, &str)],
) -> Vec<(String, String)> {
    let mut tags = vec![
        ("vgi.title".to_string(), title.to_string()),
        ("vgi.doc_llm".to_string(), doc_llm.to_string()),
        ("vgi.doc_md".to_string(), doc_md.to_string()),
        ("vgi.keywords".to_string(), keywords_json(keywords)),
        ("vgi.category".to_string(), category.to_string()),
    ];
    if !examples.is_empty() {
        tags.push((
            "vgi.example_queries".to_string(),
            example_queries_json(examples),
        ));
    }
    tags
}
