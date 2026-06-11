use super::{tool_parameters_schema, validate_tool_args, Tool};
use crate::provider::ToolDef;
use async_trait::async_trait;
use color_eyre::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Expone el contenido del buffer activo del editor al modelo.
/// El contenido se inyecta desde fuera a través de `set_content`.
pub struct GetBufferTool {
    content: Arc<RwLock<String>>,
}

// DCA-IA-IMPROVEMENT: Esquema vacio explicito para herramientas sin argumentos.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GetBufferArgs {}

impl GetBufferTool {
    pub fn new() -> Self {
        Self {
            content: Arc::new(RwLock::new(String::new())),
        }
    }

    pub fn shared(&self) -> Arc<RwLock<String>> {
        self.content.clone()
    }
}

impl Default for GetBufferTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetBufferTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "get_buffer".into(),
            description:
                "Devuelve el contenido completo del archivo abierto actualmente en el editor."
                    .into(),
            parameters: tool_parameters_schema::<GetBufferArgs>(),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let _: GetBufferArgs = validate_tool_args("get_buffer", args)?;
        let content = self.content.read().await;
        if content.is_empty() {
            Ok("El editor no tiene ningún archivo abierto.".into())
        } else {
            Ok(content.clone())
        }
    }
}
