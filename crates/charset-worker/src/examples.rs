//! Composed example queries — each combines two or more of the worker's own
//! objects in the kind of query a real user writes (CTEs, joins against the
//! `supported_encodings` table, one function's output feeding another).
//!
//! Each SQL is defined once here and referenced from both the schema-level
//! `vgi.example_queries` list (`lib.rs`) and the example list of the object that
//! drives the query, so the copies can't drift. All are offline and
//! deterministic: the legacy bytes are produced inline with `transcode`, so no
//! BLOB fixtures are needed.

/// Triage a column of unlabelled legacy bytes: detect, score, test and decode
/// each row. Driven by `detect_encoding`.
pub const TRIAGE_MIXED_BYTES: (&str, &str) = (
    "Triage a column of unlabelled bytes in mixed legacy encodings (produced here with \
     transcode): detect each row's encoding and confidence, flag which rows are already UTF-8, \
     and decode every row to UTF-8 text.",
    "WITH raw AS (SELECT id, charset.main.transcode(txt, enc) AS bytes FROM (VALUES \
     (1, 'Grüße aus München', 'windows-1252'), (2, 'Привет, как дела?', 'windows-1251'), \
     (3, 'こんにちは世界', 'shift_jis'), (4, 'naïve café', 'utf-8')) AS t(id, txt, enc)) \
     SELECT id, charset.main.detect_encoding(bytes) AS detected, \
     charset.main.detect_confidence(bytes) AS confidence, \
     charset.main.is_valid_utf8(bytes) AS already_utf8, charset.main.to_utf8(bytes) AS text \
     FROM raw ORDER BY id",
);

/// Which encodings can store a string losslessly: every supported label,
/// encode then decode, keep the exact round-trips. Driven by the
/// `supported_encodings` table.
pub const LOSSLESS_ENCODINGS: (&str, &str) = (
    "List the supported encodings that can store 'naïve café' losslessly: encode it into each \
     label with transcode, decode it back with to_utf8_from, and keep the labels whose round \
     trip is exact (the decode-only 'replacement' encoding has no encoder, so it is skipped).",
    "SELECT label FROM charset.main.supported_encodings WHERE label <> 'replacement' \
     AND charset.main.to_utf8_from(charset.main.transcode('naïve café', label), label) \
     = 'naïve café' ORDER BY label",
);

/// Pre-export audit: which names would be mangled by a windows-1252 export.
/// Driven by `transcode`.
pub const EXPORT_AUDIT_1252: (&str, &str) = (
    "Before exporting names to a windows-1252 system, flag the ones that would not survive: \
     encode each with transcode, decode the bytes back with to_utf8_from, and compare with \
     the original.",
    "SELECT name, charset.main.to_utf8_from(charset.main.transcode(name, 'windows-1252'), \
     'windows-1252') = name AS fits_1252 FROM (VALUES ('Café Müller'), ('Łódź'), ('東京'), \
     ('Smith')) AS t(name) ORDER BY name",
);

/// Reproduce the classic UTF-8-read-as-windows-1252 mojibake, then repair it.
/// Driven by `fix_mojibake`.
pub const MOJIBAKE_ROUND_TRIP: (&str, &str) = (
    "Reproduce double-encoding mojibake (UTF-8 bytes from transcode misread as windows-1252 by \
     to_utf8_from) and show fix_mojibake restoring the original text.",
    "SELECT original, charset.main.to_utf8_from(charset.main.transcode(original, 'utf-8'), \
     'windows-1252') AS garbled, charset.main.fix_mojibake(garbled) AS repaired FROM (VALUES \
     ('Café'), ('Ünïcödé'), ('“smart quotes” — dash')) AS t(original) ORDER BY original",
);

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, &str); 4] = [
        TRIAGE_MIXED_BYTES,
        LOSSLESS_ENCODINGS,
        EXPORT_AUDIT_1252,
        MOJIBAKE_ROUND_TRIP,
    ];

    #[test]
    fn composed_examples_are_published_at_schema_level() {
        let model = crate::catalog_metadata("charset");
        let schema_examples = &model.schemas[0]
            .tags
            .iter()
            .find(|(k, _)| k == "vgi.example_queries")
            .expect("schema example list")
            .1;
        for (_, sql) in ALL {
            // None of the composed SQL needs JSON escaping, so the serialized
            // list contains it verbatim.
            assert!(
                schema_examples.contains(sql),
                "missing from schema list: {sql}"
            );
            // VGI510 stable output, VGI514 no bare SELECT *.
            assert!(sql.contains(" ORDER BY "), "{sql}");
            assert!(!sql.contains("SELECT *"), "{sql}");
        }
    }
}
