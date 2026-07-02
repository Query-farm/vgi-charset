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
        .map(|kw| {
            let escaped = kw.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{escaped}\"")
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// Build the standard per-object discovery/description tags.
///
/// `keywords` is supplied as a convenient comma-separated string and serialized
/// to the required JSON-array form for `vgi.keywords`. `category` names one of
/// the schema's `vgi.categories` (VGI413), surfaced as `vgi.category` so the
/// object joins the worker's navigation/listing sections.
pub fn object_tags(
    title: &str,
    doc_llm: &str,
    doc_md: &str,
    keywords: &str,
    category: &str,
) -> Vec<(String, String)> {
    vec![
        ("vgi.title".to_string(), title.to_string()),
        ("vgi.doc_llm".to_string(), doc_llm.to_string()),
        ("vgi.doc_md".to_string(), doc_md.to_string()),
        ("vgi.keywords".to_string(), keywords_json(keywords)),
        ("vgi.category".to_string(), category.to_string()),
    ]
}
