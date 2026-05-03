use async_trait::async_trait;
use color_eyre::Result;
use crate::provider::ToolDef;
use super::Tool;

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "write_file".into(),
            description: "Escribe o sobreescribe un archivo con el contenido dado. PELIGROSO: requiere confirmación del usuario.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["path", "content"],
                "properties": {
                    "path": { "type": "string", "description": "Ruta del archivo a escribir" },
                    "content": { "type": "string", "description": "Contenido completo del archivo" }
                }
            }),
        }
    }

    fn requires_approval(&self) -> bool { true }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let path = args["path"].as_str().ok_or_else(|| color_eyre::eyre::eyre!("Falta 'path'"))?;
        let content = args["content"].as_str().ok_or_else(|| color_eyre::eyre::eyre!("Falta 'content'"))?;

        // Crea directorios padre si no existen
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }

        tokio::fs::write(path, content).await
            .map_err(|e| color_eyre::eyre::eyre!("No se pudo escribir {path}: {e}"))?;

        Ok(format!("Archivo escrito: {path}"))
    }
}
