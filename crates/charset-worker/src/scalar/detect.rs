//! Detection scalars over BLOB input:
//!
//! - `detect_encoding(bytes) -> VARCHAR` — the detected encoding label.
//! - `detect_confidence(bytes) -> DOUBLE` — a `[0,1]` confidence proxy.
//! - `is_valid_utf8(bytes) -> BOOLEAN` — whether the bytes are already UTF-8.
//!
//! NULL/empty input yields NULL for the label/confidence (there is nothing to
//! detect); `is_valid_utf8(NULL)` is NULL, and an empty BLOB is valid UTF-8.

use std::sync::Arc;

use arrow_array::builder::{BooleanBuilder, Float64Builder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::DataType;
use vgi::{ArgSpec, BindParams, BindResponse, FunctionMetadata, ProcessParams, ScalarFunction};
use vgi_rpc::{Result, RpcError};

use crate::arrow_io::blob_bytes;
use crate::charset;

pub struct DetectEncoding;

impl ScalarFunction for DetectEncoding {
    fn name(&self) -> &str {
        "detect_encoding"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "Detect the character encoding of text bytes (BOM check, then the \
                          chardetng heuristic); returns the encoding label, e.g. 'UTF-8', \
                          'windows-1252', 'Shift_JIS'. NULL for empty/NULL input."
                .into(),
            return_type: Some(DataType::Utf8),
            tags: crate::meta::object_tags(
                "Detect Character Encoding",
                "Detect the character encoding of a `BLOB` of raw text bytes and return its \
                 canonical label, e.g. 'UTF-8', 'windows-1252', or 'Shift_JIS'. It first checks \
                 for a byte-order mark, then falls back to the chardetng (Firefox) heuristic. \
                 Returns NULL for empty or NULL input.",
                "## detect_encoding\n\n\
                 Guesses the character encoding of a `BLOB` of raw text bytes and returns its \
                 canonical label.\n\n\
                 **How it works:** a byte-order-mark check runs first; if there is no BOM, the \
                 `chardetng` (Firefox) heuristic inspects the whole buffer.\n\n\
                 **Returns:** a label such as `UTF-8`, `windows-1252`, or `Shift_JIS`; `NULL` \
                 for empty or `NULL` input.\n\n\
                 **Tip:** detection is statistical, so short or ambiguous input can be \
                 mis-guessed — combine with `detect_confidence` before trusting the result.\n\n\
                 ```sql\n\
                 SELECT charset.main.detect_encoding('\\x63\\x61\\x66\\xE9'::BLOB); -- windows-1252\n\
                 ```",
                "detect encoding, charset detection, guess encoding, chardetng, BOM, \
                 sniff encoding, identify encoding, windows-1252, shift_jis, utf-8",
                "Detection",
                &[(
                    "Detect the encoding of the bytes for \"café\" stored as windows-1252 \
                     (returns 'windows-1252').",
                    "SELECT charset.main.detect_encoding('\\x63\\x61\\x66\\xE9'::BLOB);",
                )],
            ),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::column(
            "bytes",
            0,
            "blob",
            "The raw text bytes whose character encoding should be detected; inspected \
             as-is, with no decoding. NULL or empty input yields NULL.",
        )]
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(DataType::Utf8))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let col = batch.column(0);
        let rows = batch.num_rows();
        let mut out = StringBuilder::new();
        for i in 0..rows {
            match blob_bytes(col, i)? {
                Some(bytes) => match charset::detect(bytes) {
                    Some(label) => out.append_value(label),
                    None => out.append_null(), // empty input
                },
                None => out.append_null(),
            }
        }
        let arr: ArrayRef = Arc::new(out.finish());
        RecordBatch::try_new(params.output_schema.clone(), vec![arr])
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}

pub struct DetectConfidence;

impl ScalarFunction for DetectConfidence {
    fn name(&self) -> &str {
        "detect_confidence"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "A confidence proxy in [0,1] for the detected encoding, derived from \
                          whether the bytes decode losslessly (1.0) or required U+FFFD \
                          replacements (scaled down). NULL for empty/NULL input."
                .into(),
            return_type: Some(DataType::Float64),
            tags: crate::meta::object_tags(
                "Detection Confidence Score",
                "Return a confidence proxy in the range [0, 1] for the encoding detected on a \
                 `BLOB` of text bytes. It is 1.0 when the bytes decode losslessly (or carry a \
                 BOM), and is scaled down by the fraction of U+FFFD replacement characters the \
                 decode produced. Returns NULL for empty or NULL input.",
                "## detect_confidence\n\n\
                 Scores how trustworthy the detected encoding of a `BLOB` is, on a `[0, 1]` \
                 scale.\n\n\
                 **How it works:** `chardetng` itself does not expose a probability, so this \
                 proxies confidence from the decode — `1.0` when the bytes decode losslessly (or \
                 carry a BOM), scaled down by the fraction of `U+FFFD` replacement characters the \
                 decode produced.\n\n\
                 **Returns:** a `DOUBLE` in `[0, 1]`; `NULL` for empty or `NULL` input.\n\n\
                 **Usage:** gate automated cleanup on a threshold so low-confidence guesses are \
                 flagged for review rather than silently transformed.\n\n\
                 ```sql\n\
                 SELECT charset.main.detect_confidence('\\x63\\x61\\x66\\xE9'::BLOB); -- 1.0\n\
                 ```",
                "detection confidence, encoding confidence, score, reliability, certainty, \
                 chardetng, replacement characters, lossless decode",
                "Detection",
                &[(
                    "Score how confidently the bytes for \"café\" decode under the detected \
                     encoding (1.0 when lossless).",
                    "SELECT charset.main.detect_confidence('\\x63\\x61\\x66\\xE9'::BLOB);",
                )],
            ),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::column(
            "bytes",
            0,
            "blob",
            "The raw text bytes to score: confidence is derived from how cleanly these \
             bytes decode under the detected encoding. NULL or empty input yields NULL.",
        )]
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(DataType::Float64))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let col = batch.column(0);
        let rows = batch.num_rows();
        let mut out = Float64Builder::new();
        for i in 0..rows {
            match blob_bytes(col, i)? {
                Some(bytes) if !bytes.is_empty() => out.append_value(charset::confidence(bytes)),
                // Empty or NULL → NULL (nothing to be confident about).
                _ => out.append_null(),
            }
        }
        let arr: ArrayRef = Arc::new(out.finish());
        RecordBatch::try_new(params.output_schema.clone(), vec![arr])
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}

pub struct IsValidUtf8;

impl ScalarFunction for IsValidUtf8 {
    fn name(&self) -> &str {
        "is_valid_utf8"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "Whether the bytes are already valid UTF-8. NULL for NULL input.".into(),
            return_type: Some(DataType::Boolean),
            tags: crate::meta::object_tags(
                "Is Valid UTF-8 Check",
                "Test whether a `BLOB` of bytes is already well-formed UTF-8, returning true or \
                 false. Use it to decide whether bytes need decoding/transcoding at all. An \
                 empty `BLOB` is valid UTF-8; NULL input returns NULL.",
                "## is_valid_utf8\n\n\
                 Tests whether a `BLOB` of bytes is already well-formed UTF-8.\n\n\
                 **Returns:** `BOOLEAN` — `true` when every byte forms a valid UTF-8 sequence, \
                 `false` otherwise. An empty `BLOB` is valid UTF-8; `NULL` input returns `NULL`.\n\n\
                 **When to use:** as a cheap pre-check to skip decoding/transcoding for data \
                 that is already clean, or to find rows that still need repair.\n\n\
                 ```sql\n\
                 SELECT charset.main.is_valid_utf8('\\x63\\x61\\x66\\xC3\\xA9'::BLOB); -- true\n\
                 ```",
                "valid utf-8, is utf8, well-formed, utf-8 check, validate encoding, \
                 byte validity, malformed bytes",
                "Detection",
                &[(
                    "Check whether a BLOB already holds valid UTF-8 (returns true).",
                    "SELECT charset.main.is_valid_utf8('\\x63\\x61\\x66\\xC3\\xA9'::BLOB);",
                )],
            ),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::column(
            "bytes",
            0,
            "blob",
            "The raw bytes to test for well-formed UTF-8. Checked as-is, with no \
             decoding; empty input is valid UTF-8 and NULL input yields NULL.",
        )]
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(DataType::Boolean))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let col = batch.column(0);
        let rows = batch.num_rows();
        let mut out = BooleanBuilder::new();
        for i in 0..rows {
            match blob_bytes(col, i)? {
                Some(bytes) => out.append_value(charset::is_valid_utf8(bytes)),
                None => out.append_null(),
            }
        }
        let arr: ArrayRef = Arc::new(out.finish());
        RecordBatch::try_new(params.output_schema.clone(), vec![arr])
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrow_io::test_support::{bound_type, run_scalar_blob};
    use arrow_array::cast::AsArray;
    use arrow_array::types::Float64Type;
    use arrow_array::Array;
    use vgi::arguments::Arguments;

    const CAFE_1252: &[u8] = &[b'c', b'a', b'f', 0xE9];

    #[test]
    fn binds() {
        assert_eq!(bound_type(&DetectEncoding), DataType::Utf8);
        assert_eq!(bound_type(&DetectConfidence), DataType::Float64);
        assert_eq!(bound_type(&IsValidUtf8), DataType::Boolean);
    }

    #[test]
    fn detect_encoding_known_inputs() {
        let out = run_scalar_blob(
            &DetectEncoding,
            &[
                Some("Héllo wörld — façade".as_bytes()),
                Some(CAFE_1252),
                Some(&[]),
                None,
            ],
            Arguments::default(),
        )
        .unwrap();
        let s = out.as_string::<i32>();
        assert_eq!(s.value(0), "UTF-8");
        assert_eq!(s.value(1), "windows-1252");
        assert!(out.is_null(2), "empty -> NULL");
        assert!(out.is_null(3), "NULL -> NULL");
    }

    #[test]
    fn confidence_lossless_and_empty() {
        let out = run_scalar_blob(
            &DetectConfidence,
            &[Some("hello".as_bytes()), Some(&[]), None],
            Arguments::default(),
        )
        .unwrap();
        let d = out.as_primitive::<Float64Type>();
        assert_eq!(d.value(0), 1.0);
        assert!(out.is_null(1));
        assert!(out.is_null(2));
    }

    #[test]
    fn is_valid_utf8_rows() {
        let out = run_scalar_blob(
            &IsValidUtf8,
            &[Some("café".as_bytes()), Some(CAFE_1252), None],
            Arguments::default(),
        )
        .unwrap();
        let b = out.as_boolean();
        assert!(b.value(0));
        assert!(!b.value(1));
        assert!(out.is_null(2));
    }
}
