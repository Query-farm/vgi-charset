//! Decoding scalars (bytes → UTF-8 VARCHAR):
//!
//! - `to_utf8(bytes) -> VARCHAR` — detect the encoding, then decode.
//! - `to_utf8_from(bytes, encoding VARCHAR) -> VARCHAR` — decode with an
//!   **explicit** label (no detection).
//!
//! NULL/empty input → NULL. For `to_utf8_from`, an **unknown encoding label** is
//! a logic error (the caller named a codec that doesn't exist) and raises a
//! DuckDB ERROR; undecodable bytes within a *known* encoding become U+FFFD.

use std::sync::Arc;

use arrow_array::builder::StringBuilder;
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::DataType;
use vgi::{
    ArgSpec, BindParams, BindResponse, FunctionExample, FunctionMetadata, ProcessParams,
    ScalarFunction,
};
use vgi_rpc::{Result, RpcError};

use crate::arrow_io::{blob_bytes, text_str};
use crate::charset;

pub struct ToUtf8;

impl ScalarFunction for ToUtf8 {
    fn name(&self) -> &str {
        "to_utf8"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "Detect the encoding of text bytes and decode to a UTF-8 string \
                          (U+FFFD for undecodable bytes). NULL for empty/NULL input."
                .into(),
            return_type: Some(DataType::Utf8),
            examples: vec![FunctionExample {
                sql: "SELECT charset.main.to_utf8('\\x63\\x61\\x66\\xE9'::BLOB);".into(),
                description: "Auto-detect and decode windows-1252 bytes to the UTF-8 string \
                              \"café\"."
                    .into(),
                expected_output: None,
            }],
            tags: crate::meta::object_tags(
                "Decode Bytes to UTF-8",
                "Auto-detect the encoding of a BLOB of text bytes and decode it to a UTF-8 \
                 string. Undecodable bytes within the detected encoding become the U+FFFD \
                 replacement character rather than an error. Returns NULL for empty or NULL \
                 input. Use to_utf8_from when you already know the source encoding.",
                "## to_utf8\n\n\
                 Auto-detects the encoding of a `BLOB` of text bytes and decodes it to a UTF-8 \
                 string in one step.\n\n\
                 **How it works:** runs the same detection as `detect_encoding` (BOM, then \
                 `chardetng`) and decodes with `encoding_rs`. Bytes that are undecodable in the \
                 detected encoding become the `U+FFFD` replacement character rather than raising \
                 an error.\n\n\
                 **Returns:** a `VARCHAR`; `NULL` for empty or `NULL` input.\n\n\
                 **When to use:** the convenient default for unlabelled data. Use \
                 `to_utf8_from` instead when you already know the source codec and want to skip \
                 detection.\n\n\
                 ```sql\n\
                 SELECT charset.main.to_utf8('\\x63\\x61\\x66\\xE9'::BLOB); -- 'café'\n\
                 ```",
                "to utf8, decode, convert to utf-8, auto decode, bytes to text, \
                 normalize encoding, clean text, detected encoding",
                "scalar/decode.rs",
            ),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::any_column("bytes", 0, "Text bytes (BLOB)")]
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
                Some(bytes) => match charset::to_utf8(bytes) {
                    Some(text) => out.append_value(&text),
                    None => out.append_null(), // empty
                },
                None => out.append_null(),
            }
        }
        let arr: ArrayRef = Arc::new(out.finish());
        RecordBatch::try_new(params.output_schema.clone(), vec![arr])
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}

pub struct ToUtf8From;

impl ScalarFunction for ToUtf8From {
    fn name(&self) -> &str {
        "to_utf8_from"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "Decode text bytes using an EXPLICIT encoding label (no detection), \
                          e.g. to_utf8_from(b, 'shift_jis'). ERROR if the label is unknown; \
                          NULL for NULL bytes."
                .into(),
            return_type: Some(DataType::Utf8),
            examples: vec![FunctionExample {
                sql: "SELECT charset.main.to_utf8_from('\\x93\\xFA\\x96\\x7B'::BLOB, 'shift_jis');"
                    .into(),
                description: "Decode Shift-JIS bytes to UTF-8 using an explicit codec label \
                              (returns \"日本\")."
                    .into(),
                expected_output: None,
            }],
            tags: crate::meta::object_tags(
                "Decode Bytes With Explicit Encoding",
                "Decode a BLOB of text bytes to a UTF-8 string using an explicit encoding label \
                 you supply (e.g. 'shift_jis', 'windows-1252'), with no auto-detection. Raises \
                 an error if the label names an encoding the worker does not recognise; \
                 undecodable bytes within a known encoding become U+FFFD. Returns NULL for NULL \
                 input.",
                "## to_utf8_from\n\n\
                 Decodes a `BLOB` of text bytes to a UTF-8 string using an **explicit** encoding \
                 label you supply — no auto-detection.\n\n\
                 **Arguments:** `bytes` (the BLOB) and `encoding` (a codec label such as \
                 `shift_jis`, `windows-1252`, or `iso-8859-1`). Call `supported_encodings()` to \
                 see the accepted labels.\n\n\
                 **Errors & nulls:** an unknown label raises an error (the caller named a codec \
                 that does not exist); undecodable bytes within a *known* encoding become \
                 `U+FFFD`; `NULL` input returns `NULL`.\n\n\
                 **When to use:** prefer this over `to_utf8` whenever the source encoding is \
                 known, to avoid detection mistakes.\n\n\
                 ```sql\n\
                 SELECT charset.main.to_utf8_from('\\x93\\xFA\\x96\\x7B'::BLOB, 'shift_jis');\n\
                 ```",
                "to utf8 from, decode with encoding, explicit codec, shift_jis, windows-1252, \
                 latin-1, known encoding, force encoding",
                "scalar/decode.rs",
            ),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::any_column("bytes", 0, "Text bytes (BLOB)"),
            ArgSpec::any_column("encoding", 1, "Encoding label, e.g. 'shift_jis' (VARCHAR)"),
        ]
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(DataType::Utf8))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let bytes_col = batch.column(0);
        let enc_col = batch.column(1);
        let rows = batch.num_rows();
        let mut out = StringBuilder::new();
        for i in 0..rows {
            match (blob_bytes(bytes_col, i)?, text_str(enc_col, i)?) {
                (Some(bytes), Some(label)) => match charset::to_utf8_from(bytes, label) {
                    Ok(text) => out.append_value(&text),
                    // Unknown encoding label → loud error (caller bug).
                    Err(e) => return Err(RpcError::value_error(e)),
                },
                // NULL bytes or NULL label → NULL.
                _ => out.append_null(),
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
    use crate::arrow_io::test_support::{
        blob_text_batch, bound_type, run_scalar_blob, run_scalar_on,
    };
    use arrow_array::cast::AsArray;
    use arrow_array::Array;
    use vgi::arguments::Arguments;

    const CAFE_1252: &[u8] = &[b'c', b'a', b'f', 0xE9];

    #[test]
    fn binds_utf8() {
        assert_eq!(bound_type(&ToUtf8), DataType::Utf8);
        assert_eq!(bound_type(&ToUtf8From), DataType::Utf8);
    }

    #[test]
    fn to_utf8_decodes_and_nulls() {
        let out = run_scalar_blob(
            &ToUtf8,
            &[Some(CAFE_1252), Some("ok".as_bytes()), Some(&[]), None],
            Arguments::default(),
        )
        .unwrap();
        let s = out.as_string::<i32>();
        assert_eq!(s.value(0), "café");
        assert_eq!(s.value(1), "ok");
        assert!(out.is_null(2));
        assert!(out.is_null(3));
    }

    #[test]
    fn to_utf8_from_explicit_shift_jis() {
        let sjis: &[u8] = &[0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA]; // 日本語
        let batch = blob_text_batch(&[Some(sjis), None], &[Some("shift_jis"), Some("utf-8")]);
        let out = run_scalar_on(&ToUtf8From, batch, Arguments::default()).unwrap();
        let s = out.as_string::<i32>();
        assert_eq!(s.value(0), "日本語");
        assert!(out.is_null(1), "NULL bytes -> NULL");
    }

    #[test]
    fn to_utf8_from_unknown_label_errors() {
        let batch = blob_text_batch(&[Some(b"abc")], &[Some("not-a-real-encoding")]);
        let err = run_scalar_on(&ToUtf8From, batch, Arguments::default());
        assert!(err.is_err(), "unknown label must error");
    }
}
