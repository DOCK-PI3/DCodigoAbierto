use crate::FileLocation;

/// Severidad de un diagnóstico LSP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// Diagnóstico de un archivo (línea, mensaje, severidad).
/// La línea es 0-indexed (igual que el protocolo LSP).
#[derive(Debug, Clone)]
pub struct DiagnosticInfo {
    pub line: u32,
    pub col: u32,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

/// Item de autocompletado LSP simplificado.
#[derive(Debug, Clone)]
pub struct CompletionEntry {
    pub label: String,
    pub detail: Option<String>,
    pub kind: Option<String>,
    /// Texto a insertar (si es None, se usa `label`)
    pub insert_text: Option<String>,
}

/// Eventos que el cliente LSP puede enviar al EventBus.
#[derive(Debug, Clone)]
pub enum LspEvent {
    /// El servidor LSP se inicializó correctamente
    Ready { server_name: String },
    /// Diagnósticos actualizados para un archivo
    Diagnostics {
        path: String,
        items: Vec<DiagnosticInfo>,
    },
    /// Respuesta a una petición de autocompletado
    Completions(Vec<CompletionEntry>),
    /// Respuesta a go-to-definition (puede ser None si no encontrado)
    Definition(Option<FileLocation>),
    /// Respuesta a find-references
    References(Vec<FileLocation>),
    /// Error en el cliente LSP (servidor no encontrado, etc.)
    Error(String),
}
