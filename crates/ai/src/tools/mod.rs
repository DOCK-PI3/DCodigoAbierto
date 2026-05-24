use async_trait::async_trait;
use color_eyre::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod diagnostics;
pub mod get_buffer;
pub mod glob_tool;
pub mod grep_tool;
pub mod list_dir;
pub mod read_file;
pub mod shell;
pub mod web_fetch;
pub mod write_file;

pub use diagnostics::DiagnosticsTool;
pub use get_buffer::GetBufferTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use list_dir::ListDirTool;
pub use read_file::ReadFileTool;
pub use shell::ShellTool;
pub use web_fetch::{WebFetchTool, WebSearchTool};
pub use write_file::WriteFileTool;

use crate::provider::ToolDef;

#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "error_type", rename_all = "snake_case")]
pub enum ToolError {
    #[error("{message}")]
    FileNotFound {
        message: String,
        attempted_path: String,
        hint: Option<String>,
    },
    #[error("{message}")]
    InvalidRange {
        message: String,
        start: u32,
        end: u32,
        max_available: u32,
        hint: Option<String>,
    },
    #[error("{message}")]
    PermissionDenied {
        message: String,
        path: String,
        hint: Option<String>,
    },
    #[error("{message}")]
    BufferModified {
        message: String,
        buffer_id: String,
        expected_version: u64,
        current_version: u64,
        hint: Option<String>,
    },
    #[error("{message}")]
    Generic {
        message: String,
        code: String,
        context: Option<serde_json::Value>,
        hint: Option<String>,
    },
}

impl ToolError {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(
                "{{\"error_type\":\"serialization_failed\",\"message\":\"{}\"}}",
                self
            )
        })
    }
}

// DCA-IA-IMPROVEMENT: Validacion tipada centralizada para argumentos de herramientas.
pub fn validate_tool_args<T>(tool_name: &str, args: &serde_json::Value) -> Result<T>
where
    T: serde::de::DeserializeOwned + JsonSchema,
{
    serde_json::from_value(args.clone()).map_err(|error| {
        let schema_value = serde_json::to_value(schemars::schema_for!(T).schema).ok();
        ToolError::Generic {
            message: format!("{}: argumentos invalidos - {}", tool_name, error),
            code: "invalid_arguments".to_string(),
            context: Some(serde_json::json!({
                "provided_args": args,
                "expected_schema": schema_value,
            })),
            hint: Some("Corrige los argumentos para que coincidan con el schema esperado".to_string()),
        }
        .into()
    })
}

pub fn tool_parameters_schema<T>() -> serde_json::Value
where
    T: JsonSchema,
{
    serde_json::to_value(schemars::schema_for!(T).schema)
        .unwrap_or_else(|_| serde_json::json!({ "type": "object" }))
}

/// Una herramienta/skill que el modelo puede invocar.
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDef;
    /// `true` → requiere confirmación del usuario antes de ejecutar
    fn requires_approval(&self) -> bool { false }
    async fn execute(&self, args: &serde_json::Value) -> Result<String>;
}

/// Devuelve todas las herramientas habilitadas.
pub fn all_tools(web_enabled: bool) -> Vec<Box<dyn Tool>> {
    let mut v: Vec<Box<dyn Tool>> = vec![
        // Lectura (sin aprobación) — el modelo debe preferir estas
        Box::new(ListDirTool),
        Box::new(ReadFileTool),
        Box::new(GlobTool),
        Box::new(GrepTool),
        Box::new(DiagnosticsTool::new()),
        Box::new(GetBufferTool::new()),
        // Escritura/peligrosas (con aprobación)
        Box::new(WriteFileTool),
        Box::new(ShellTool),
    ];
    if web_enabled {
        v.push(Box::new(WebSearchTool));
        v.push(Box::new(WebFetchTool));
    }
    v
}

/// Herramientas del modo Plan: búsqueda web únicamente.
pub fn plan_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(WebSearchTool),
        Box::new(WebFetchTool),
    ]
}
