//! `supported_encodings() -> (label VARCHAR)` — the discovery table listing every
//! encoding label the worker accepts (the WHATWG / `encoding_rs` set of canonical
//! encoding names).

use std::sync::Arc;

use arrow_array::builder::StringBuilder;
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use vgi::table_function::{TableFunction, TableProducer};
use vgi::{ArgSpec, BindParams, BindResponse, FunctionMetadata, ProcessParams};
use vgi_rpc::{OutputCollector, Result, RpcError};

use crate::charset;

pub struct SupportedEncodings;

fn output_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![Field::new(
        "label",
        DataType::Utf8,
        false,
    )]))
}

impl TableFunction for SupportedEncodings {
    fn name(&self) -> &str {
        "supported_encodings"
    }

    fn metadata(&self) -> FunctionMetadata {
        FunctionMetadata {
            description: "List every encoding label the worker accepts (the encoding_rs / WHATWG \
                          set of canonical names)"
                .into(),
            tags: vec![(
                "vgi.columns_md".into(),
                "| column | type | description |\n\
                 |---|---|---|\n\
                 | `label` | VARCHAR | A canonical encoding label accepted by `to_utf8_from` and \
                 `transcode`, e.g. `UTF-8`, `windows-1252`, `Shift_JIS`. |"
                    .into(),
            )],
            ..Default::default()
        }
    }

    fn argument_specs(&self) -> Vec<ArgSpec> {
        Vec::new()
    }

    fn on_bind(&self, _params: &BindParams) -> Result<BindResponse> {
        Ok(BindResponse {
            output_schema: output_schema(),
            opaque_data: Vec::new(),
        })
    }

    fn producer(&self, params: &ProcessParams) -> Result<Box<dyn TableProducer>> {
        Ok(Box::new(SupportedProducer {
            schema: params.output_schema.clone(),
            done: false,
        }))
    }
}

struct SupportedProducer {
    schema: SchemaRef,
    done: bool,
}

impl TableProducer for SupportedProducer {
    fn next_batch(&mut self, _out: &mut OutputCollector) -> Result<Option<RecordBatch>> {
        if self.done {
            return Ok(None);
        }
        self.done = true;

        let mut label = StringBuilder::new();
        for name in charset::supported_encodings() {
            label.append_value(name);
        }
        let cols: Vec<ArrayRef> = vec![Arc::new(label.finish())];
        Ok(Some(
            RecordBatch::try_new(self.schema.clone(), cols)
                .map_err(|e| RpcError::runtime_error(e.to_string()))?,
        ))
    }
}
