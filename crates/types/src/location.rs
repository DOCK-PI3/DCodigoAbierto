/// Una ubicación concreta en un archivo (respuesta LSP goto/references).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLocation {
    /// Ruta absoluta del archivo
    pub path: String,
    /// Línea 0-indexada
    pub line: u32,
    /// Columna 0-indexada (carácter UTF-16 según spec LSP)
    pub col: u32,
    /// Texto preview de esa línea (opcional, para la UI)
    pub preview: Option<String>,
}
