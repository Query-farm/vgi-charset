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

/// Guaranteed-runnable, catalog-qualified examples (VGI509). Each `sql` is
/// self-contained and re-runnable against an attached `charset` worker. We omit
/// `expected_result` deliberately — the linter only needs each query to execute
/// cleanly, and exact byte/string output is brittle to pin here.
const EXECUTABLE_EXAMPLES: &str = r#"[
  {
    "description": "Detect the encoding of windows-1252 bytes for \"café\".",
    "sql": "SELECT charset.main.detect_encoding('\\x63\\x61\\x66\\xE9'::BLOB) AS encoding"
  },
  {
    "description": "Auto-detect and decode windows-1252 bytes to UTF-8.",
    "sql": "SELECT charset.main.to_utf8('\\x63\\x61\\x66\\xE9'::BLOB) AS text"
  },
  {
    "description": "Decode Shift-JIS bytes to UTF-8 with an explicit codec label.",
    "sql": "SELECT charset.main.to_utf8_from('\\x93\\xFA\\x96\\x7B'::BLOB, 'shift_jis') AS text"
  },
  {
    "description": "Encode a UTF-8 string into windows-1252 bytes for export.",
    "sql": "SELECT charset.main.transcode('café', 'windows-1252') AS bytes"
  },
  {
    "description": "Repair double-encoded mojibake back to clean UTF-8.",
    "sql": "SELECT charset.main.fix_mojibake('CafÃ©') AS fixed"
  },
  {
    "description": "Check whether bytes are already valid UTF-8.",
    "sql": "SELECT charset.main.is_valid_utf8('\\x63\\x61\\x66\\xC3\\xA9'::BLOB) AS ok"
  },
  {
    "description": "List the first few supported encoding labels.",
    "sql": "SELECT label FROM charset.main.supported_encodings() ORDER BY label LIMIT 5"
  }
]"#;

pub struct SupportedEncodings;

/// The columns produced by `supported_encodings` — shared by the table
/// function's `on_bind` and by the catalog table that exposes it as
/// `charset.main.supported_encodings`. The `label` column carries a `comment`
/// (surfaced via `duckdb_columns().comment`) so it is documented wherever the
/// schema surfaces.
pub fn output_schema() -> SchemaRef {
    let label = Field::new("label", DataType::Utf8, false).with_metadata(
        std::collections::HashMap::from([(
            "comment".to_string(),
            "A canonical encoding label accepted by the worker, e.g. 'UTF-8', \
             'windows-1252', or 'Shift_JIS'. Valid as the encoding argument to \
             to_utf8_from and transcode."
                .to_string(),
        )]),
    );
    Arc::new(Schema::new(vec![label]))
}

impl TableFunction for SupportedEncodings {
    fn name(&self) -> &str {
        "supported_encodings"
    }

    fn metadata(&self) -> FunctionMetadata {
        let mut tags = crate::meta::object_tags(
            "Supported Encodings Catalog",
            "List every encoding label the worker accepts — the encoding_rs / WHATWG set of \
             canonical encoding names. Use it to discover which labels are valid inputs to \
             to_utf8_from and transcode.",
            "## supported_encodings\n\n\
             A discovery table that lists every encoding label the worker accepts — the \
             `encoding_rs` / WHATWG set of canonical encoding names.\n\n\
             **Returns:** one row per encoding, with a single `label` column.\n\n\
             **When to use:** to discover which labels are valid inputs to `to_utf8_from` and \
             `transcode`, or to populate a picker of supported codecs.\n\n\
             ```sql\n\
             SELECT label FROM charset.main.supported_encodings() ORDER BY label LIMIT 5;\n\
             ```",
            "supported encodings, list encodings, available codecs, encoding catalog, \
             discovery, what encodings, WHATWG, encoding_rs, labels",
        );
        tags.push((
            "vgi.result_columns_md".into(),
            "| column | type | description |\n\
             |---|---|---|\n\
             | `label` | VARCHAR | A canonical encoding label accepted by `to_utf8_from` and \
             `transcode`, e.g. `UTF-8`, `windows-1252`, `Shift_JIS`. |"
                .into(),
        ));
        tags.push(("vgi.executable_examples".into(), EXECUTABLE_EXAMPLES.into()));
        FunctionMetadata {
            description: "List every encoding label the worker accepts (the encoding_rs / WHATWG \
                          set of canonical names)"
                .into(),
            tags,
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
