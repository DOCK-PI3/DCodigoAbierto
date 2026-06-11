use super::{tool_parameters_schema, validate_tool_args, Tool, ToolError};
use crate::provider::ToolDef;
use async_trait::async_trait;
use color_eyre::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub struct ReadFileTool;

// DCA-IA-IMPROVEMENT: Schema Rust -> JSON Schema para validacion consistente.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Ruta al archivo a leer.
    pub path: String,
    /// Primera linea a devolver (1-indexed, inclusiva).
    #[schemars(default)]
    pub start_line: Option<u32>,
    /// Ultima linea a devolver (1-indexed, inclusiva).
    #[schemars(default)]
    pub end_line: Option<u32>,
}

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "read_file".into(),
            description: "Lee el contenido de un archivo. Devuelve el texto completo o un rango de líneas si se especifican start_line y end_line.".into(),
            parameters: tool_parameters_schema::<ReadFileArgs>(),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: ReadFileArgs = validate_tool_args("read_file", args)?;

        if let (Some(start_line), Some(end_line)) = (args.start_line, args.end_line) {
            if start_line == 0 || end_line == 0 {
                return Err(ToolError::InvalidRange {
                    message: "read_file: las lineas son 1-indexed; usa 1 o superior".to_string(),
                    start: start_line,
                    end: end_line,
                    max_available: 0,
                    hint: Some("Usa start_line/end_line empezando en 1".to_string()),
                }
                .into());
            }
            if start_line > end_line {
                return Err(ToolError::InvalidRange {
                    message: format!(
                        "read_file: start_line ({}) no puede ser mayor que end_line ({})",
                        start_line, end_line
                    ),
                    start: start_line,
                    end: end_line,
                    max_available: 0,
                    hint: Some("Usa un rango ascendente de lineas".to_string()),
                }
                .into());
            }
        }
        if args.start_line == Some(0) || args.end_line == Some(0) {
            return Err(ToolError::InvalidRange {
                message: "read_file: las lineas son 1-indexed; usa 1 o superior".to_string(),
                start: args.start_line.unwrap_or(1),
                end: args
                    .end_line
                    .unwrap_or_else(|| args.start_line.unwrap_or(1)),
                max_available: 0,
                hint: Some("Usa start_line/end_line empezando en 1".to_string()),
            }
            .into());
        }

        let content = tokio::fs::read_to_string(&args.path)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => ToolError::FileNotFound {
                    message: format!("read_file: no existe el archivo '{}'", args.path),
                    attempted_path: args.path.clone(),
                    hint: Some(
                        "Verifica la ruta o usa glob/list_dir para encontrar el archivo"
                            .to_string(),
                    ),
                },
                std::io::ErrorKind::PermissionDenied => ToolError::PermissionDenied {
                    message: format!("read_file: permiso denegado para '{}'", args.path),
                    path: args.path.clone(),
                    hint: Some(
                        "Intenta con otro archivo o revisa los permisos del sistema".to_string(),
                    ),
                },
                _ => ToolError::Generic {
                    message: format!("read_file: no se pudo leer '{}': {}", args.path, error),
                    code: "read_failed".to_string(),
                    context: Some(serde_json::json!({ "path": args.path })),
                    hint: Some("Revisa que el archivo exista y sea texto legible".to_string()),
                },
            })?;

        let lines: Vec<&str> = content.lines().collect();
        let max_available = lines.len() as u32;

        if let Some(start_line) = args.start_line {
            if start_line > max_available && max_available > 0
                || (max_available == 0 && start_line > 0)
            {
                return Err(ToolError::InvalidRange {
                    message: format!(
                        "read_file: start_line ({}) excede las lineas disponibles ({})",
                        start_line, max_available
                    ),
                    start: start_line,
                    end: args.end_line.unwrap_or(start_line),
                    max_available,
                    hint: Some("Reduce el rango solicitado al tamano real del archivo".to_string()),
                }
                .into());
            }
        }

        if let Some(end_line) = args.end_line {
            if end_line > max_available && max_available > 0 || (max_available == 0 && end_line > 0)
            {
                return Err(ToolError::InvalidRange {
                    message: format!(
                        "read_file: end_line ({}) excede las lineas disponibles ({})",
                        end_line, max_available
                    ),
                    start: args.start_line.unwrap_or(1),
                    end: end_line,
                    max_available,
                    hint: Some("Reduce el rango solicitado al tamano real del archivo".to_string()),
                }
                .into());
            }
        }

        let start = args.start_line.map(|line| line.saturating_sub(1) as usize);
        let end = args.end_line.map(|line| line as usize);

        if start.is_none() && end.is_none() {
            return Ok(content);
        }

        let s = start.unwrap_or(0);
        let e = end.unwrap_or(lines.len()).min(lines.len());
        if s > e {
            return Err(ToolError::InvalidRange {
                message: format!(
                    "read_file: rango invalido, start_line resuelve a {} y end_line a {}",
                    s + 1,
                    e
                ),
                start: (s + 1) as u32,
                end: e as u32,
                max_available,
                hint: Some("Usa un rango de lineas valido".to_string()),
            }
            .into());
        }
        Ok(lines[s..e].join("\n"))
    }
}
