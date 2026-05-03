/// Posición del cursor en el buffer (fila/columna lógicas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Índice de línea (0-based)
    pub row: usize,
    /// Índice de columna en bytes dentro de la línea (0-based)
    pub col: usize,
}
