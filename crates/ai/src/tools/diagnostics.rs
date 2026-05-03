use async_trait::async_trait;
use color_eyre::Result;
use tokio::sync::RwLock;
use std::sync::Arc;
use crate::provider::ToolDef;
use super::Tool;

/// Expone los diagnósticos LSP del buffer activo al modelo.
/// Los diagnósticos se inyectan desde fuera a través de `set_diagnostics`.
pub struct DiagnosticsTool {
    diagnostics: Arc<RwLock<Vec<String>>>,
}

impl DiagnosticsTool {
    pub fn new() -> Self {
        Self { diagnostics: Arc::new(RwLock::new(vec![])) }
    }

    pub fn shared(&self) -> Arc<RwLock<Vec<String>>> {
        self.diagnostics.clone()
    }
}

impl Default for DiagnosticsTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for DiagnosticsTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "get_diagnostics".into(),
            description: "Devuelve los errores y advertencias LSP del archivo activo en el editor.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn execute(&self, _args: &serde_json::Value) -> Result<String> {
        let diags = self.diagnostics.read().await;
        if diags.is_empty() {
            Ok("No hay diagnósticos en el buffer activo.".into())
        } else {
            Ok(diags.join("\n"))
        }
    }
}
