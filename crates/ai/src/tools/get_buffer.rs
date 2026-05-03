use async_trait::async_trait;
use color_eyre::Result;
use tokio::sync::RwLock;
use std::sync::Arc;
use crate::provider::ToolDef;
use super::Tool;

/// Expone el contenido del buffer activo del editor al modelo.
/// El contenido se inyecta desde fuera a través de `set_content`.
pub struct GetBufferTool {
    content: Arc<RwLock<String>>,
}

impl GetBufferTool {
    pub fn new() -> Self {
        Self { content: Arc::new(RwLock::new(String::new())) }
    }

    pub fn shared(&self) -> Arc<RwLock<String>> {
        self.content.clone()
    }
}

impl Default for GetBufferTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for GetBufferTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "get_buffer".into(),
            description: "Devuelve el contenido completo del archivo abierto actualmente en el editor.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn execute(&self, _args: &serde_json::Value) -> Result<String> {
        let content = self.content.read().await;
        if content.is_empty() {
            Ok("El editor no tiene ningún archivo abierto.".into())
        } else {
            Ok(content.clone())
        }
    }
}
