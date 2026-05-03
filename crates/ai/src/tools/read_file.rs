use async_trait::async_trait;
use color_eyre::Result;
use crate::provider::ToolDef;
use super::Tool;

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "read_file".into(),
            description: "Lee el contenido de un archivo. Devuelve el texto completo o un rango de líneas si se especifican start_line y end_line.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Ruta al archivo (absoluta o relativa al CWD)" },
                    "start_line": { "type": "integer", "description": "Primera línea a devolver (1-indexed)" },
                    "end_line": { "type": "integer", "description": "Última línea a devolver (inclusiva, 1-indexed)" }
                }
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let path = args["path"].as_str().ok_or_else(|| color_eyre::eyre::eyre!("Falta 'path'"))?;
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| color_eyre::eyre::eyre!("No se pudo leer {path}: {e}"))?;

        let start = args["start_line"].as_u64().map(|n| n.saturating_sub(1) as usize);
        let end = args["end_line"].as_u64().map(|n| n as usize);

        if start.is_none() && end.is_none() {
            return Ok(content);
        }

        let lines: Vec<&str> = content.lines().collect();
        let s = start.unwrap_or(0);
        let e = end.unwrap_or(lines.len()).min(lines.len());
        Ok(lines[s..e].join("\n"))
    }
}
