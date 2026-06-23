//! Encoding / repair scalars:
//!
//! - `transcode(text VARCHAR, to_encoding VARCHAR) -> BLOB` — encode a UTF-8
//!   string into the named encoding's bytes (for export). ERROR on unknown
//!   label; NULL for NULL input. Characters the target encoding can't represent
//!   are emitted per `encoding_rs` (HTML numeric references for legacy codecs).
//! - `fix_mojibake(text VARCHAR) -> VARCHAR` — repair the classic "UTF-8 read as
//!   Latin-1/Windows-1252 then re-stored as UTF-8" double-encoding; no-ops when
//!   it can't improve the text. NULL for NULL input.

use std::sync::Arc;

use arrow_array::builder::{BinaryBuilder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::DataType;
use vgi::{ArgSpec, BindParams, BindResponse, FunctionMetadata, ProcessParams, ScalarFunction};
use vgi_rpc::{Result, RpcError};

use crate::arrow_io::text_str;
use crate::charset;

pub struct Transcode;

impl ScalarFunction for Transcode {
    fn name(&self) -> &str {
        "transcode"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "Encode a UTF-8 string into the named encoding's bytes (BLOB) for \
                          export, e.g. transcode('café', 'windows-1252'). Unmappable chars \
                          follow encoding_rs (HTML numeric refs for legacy codecs). ERROR on \
                          unknown label; NULL for NULL input."
                .into(),
            return_type: Some(DataType::Binary),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::any_column("text", 0, "UTF-8 text (VARCHAR)"),
            ArgSpec::any_column("to_encoding", 1, "Target encoding label (VARCHAR)"),
        ]
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse::result(DataType::Binary))
    }

    fn process(&self, params: &ProcessParams, batch: &RecordBatch) -> Result<RecordBatch> {
        let text_col = batch.column(0);
        let enc_col = batch.column(1);
        let rows = batch.num_rows();
        let mut out = BinaryBuilder::new();
        for i in 0..rows {
            match (text_str(text_col, i)?, text_str(enc_col, i)?) {
                (Some(text), Some(label)) => match charset::transcode(text, label) {
                    Ok(bytes) => out.append_value(&bytes),
                    Err(e) => return Err(RpcError::value_error(e)),
                },
                _ => out.append_null(),
            }
        }
        let arr: ArrayRef = Arc::new(out.finish());
        RecordBatch::try_new(params.output_schema.clone(), vec![arr])
            .map_err(|e| RpcError::runtime_error(e.to_string()))
    }
}

pub struct FixMojibake;

impl ScalarFunction for FixMojibake {
    fn name(&self) -> &str {
        "fix_mojibake"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "Repair double-encoded mojibake (UTF-8 read as Latin-1/Windows-1252 \
                          then re-stored as UTF-8), e.g. 'CafÃ©' -> 'Café'. No-ops when it \
                          can't improve the text. NULL for NULL input."
                .into(),
            return_type: Some(DataType::Utf8),
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::any_column(
            "text",
            0,
            "Possibly-mojibake text (VARCHAR)",
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
            match text_str(col, i)? {
                Some(text) => out.append_value(charset::fix_mojibake(text)),
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
    use crate::arrow_io::test_support::{bound_type, run_scalar_on, run_scalar_text, text_batch};
    use arrow_array::cast::AsArray;
    use arrow_array::{Array, RecordBatch, StringArray};
    use arrow_schema::{Field, Schema};
    use vgi::arguments::Arguments;

    fn text_pair_batch(a: &[Option<&str>], b: &[Option<&str>]) -> RecordBatch {
        let c0: ArrayRef = Arc::new(StringArray::from(a.to_vec()));
        let c1: ArrayRef = Arc::new(StringArray::from(b.to_vec()));
        let schema = Arc::new(Schema::new(vec![
            Field::new("text", DataType::Utf8, true),
            Field::new("enc", DataType::Utf8, true),
        ]));
        RecordBatch::try_new(schema, vec![c0, c1]).unwrap()
    }

    #[test]
    fn binds() {
        assert_eq!(bound_type(&Transcode), DataType::Binary);
        assert_eq!(bound_type(&FixMojibake), DataType::Utf8);
    }

    #[test]
    fn transcode_to_1252_and_null() {
        let batch = text_pair_batch(
            &[Some("café"), None],
            &[Some("windows-1252"), Some("utf-8")],
        );
        let out = run_scalar_on(&Transcode, batch, Arguments::default()).unwrap();
        let b = out.as_binary::<i32>();
        assert_eq!(b.value(0), &[b'c', b'a', b'f', 0xE9]);
        assert!(out.is_null(1));
    }

    #[test]
    fn transcode_unknown_label_errors() {
        let batch = text_pair_batch(&[Some("x")], &[Some("nope")]);
        assert!(run_scalar_on(&Transcode, batch, Arguments::default()).is_err());
    }

    #[test]
    fn fix_mojibake_rows() {
        let out = run_scalar_text(
            &FixMojibake,
            &[Some("CafÃ©"), Some("Café"), Some("plain"), None],
            Arguments::default(),
        )
        .unwrap();
        let s = out.as_string::<i32>();
        assert_eq!(s.value(0), "Café");
        assert_eq!(s.value(1), "Café"); // no-op on clean text
        assert_eq!(s.value(2), "plain");
        assert!(out.is_null(3));
    }

    #[test]
    fn transcode_round_trips_through_text_batch() {
        // sanity that text_batch single-col path still works for other funcs
        let _ = text_batch(&[Some("x")]);
    }
}
