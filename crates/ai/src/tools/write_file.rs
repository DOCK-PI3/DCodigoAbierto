use async_trait::async_trait;
use color_eyre::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::provider::ToolDef;
use super::{Tool, ToolError, tool_parameters_schema, validate_tool_args};

pub struct WriteFileTool;

// DCA-IA-IMPROVEMENT: Argumentos tipados para eliminar parseo JSON manual.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// Ruta del archivo a escribir.
    pub path: String,
    /// Contenido completo del archivo.
    pub content: String,
}

#[async_trait]
impl Tool for WriteFileTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "write_file".into(),
            description: "Escribe o sobreescribe un archivo con el contenido dado. PELIGROSO: requiere confirmación del usuario.".into(),
            parameters: tool_parameters_schema::<WriteFileArgs>(),
        }
    }

    fn requires_approval(&self) -> bool { true }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: WriteFileArgs = validate_tool_args("write_file", args)?;

        // Crea directorios padre si no existen
        if let Some(parent) = std::path::Path::new(&args.path).parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| match error.kind() {
                std::io::ErrorKind::PermissionDenied => ToolError::PermissionDenied {
                    message: format!("write_file: permiso denegado creando '{}'", parent.display()),
                    path: parent.display().to_string(),
                    hint: Some("Elige una ruta con permisos de escritura".to_string()),
                },
                _ => ToolError::Generic {
                    message: format!("write_file: no se pudo preparar '{}': {}", parent.display(), error),
                    code: "prepare_parent_failed".to_string(),
                    context: Some(serde_json::json!({ "path": parent.display().to_string() })),
                    hint: Some("Revisa que la ruta padre sea valida".to_string()),
                },
            })?;
        }

        tokio::fs::write(&args.path, args.content).await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::PermissionDenied => ToolError::PermissionDenied {
                    message: format!("write_file: permiso denegado para '{}'", args.path),
                    path: args.path.clone(),
                    hint: Some("Elige una ruta con permisos de escritura".to_string()),
                },
                _ => ToolError::Generic {
                    message: format!("write_file: no se pudo escribir {}: {}", args.path, error),
                    code: "write_failed".to_string(),
                    context: Some(serde_json::json!({ "path": args.path })),
                    hint: Some("Verifica que la ruta sea valida y que el disco este disponible".to_string()),
                },
            })?;

        Ok(format!("Archivo escrito: {}", args.path))
    }
}
