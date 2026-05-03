use async_trait::async_trait;
use color_eyre::Result;

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
