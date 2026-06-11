use super::{tool_parameters_schema, validate_tool_args, Tool};
use crate::provider::ToolDef;
use async_trait::async_trait;
use color_eyre::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Expone los diagnósticos LSP del buffer activo al modelo.
/// Los diagnósticos se inyectan desde fuera a través de `set_diagnostics`.
pub struct DiagnosticsTool {
    diagnostics: Arc<RwLock<Vec<String>>>,
}

// DCA-IA-IMPROVEMENT: Esquema vacio explicito para herramientas sin argumentos.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosticsArgs {}

impl DiagnosticsTool {
    pub fn new() -> Self {
        Self {
            diagnostics: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn shared(&self) -> Arc<RwLock<Vec<String>>> {
        self.diagnostics.clone()
    }
}

impl Default for DiagnosticsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DiagnosticsTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "get_diagnostics".into(),
            description: "Devuelve los errores y advertencias LSP del archivo activo en el editor."
                .into(),
            parameters: tool_parameters_schema::<DiagnosticsArgs>(),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let _: DiagnosticsArgs = validate_tool_args("get_diagnostics", args)?;
        let diags = self.diagnostics.read().await;
        if diags.is_empty() {
            Ok("No hay diagnósticos en el buffer activo.".into())
        } else {
            Ok(diags.join("\n"))
        }
    }
}
