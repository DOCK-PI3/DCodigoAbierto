use serde::{Deserialize, Serialize};

/// Paleta de colores del tema.
/// Los valores son colores ANSI/hex que crossterm/ratatui puede interpretar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    /// Color de fondo principal
    pub bg: String,
    /// Color de fondo secundario (bordes, paneles)
    pub bg_secondary: String,
    /// Color de texto principal
    pub fg: String,
    /// Color de texto secundario / comentarios
    pub fg_dim: String,
    /// Color de acento (cursor, selección, títulos)
    pub accent: String,
    /// Color de error / diagnósticos
    pub error: String,
    /// Color de advertencia
    pub warn: String,
    /// Color de éxito / info
    pub info: String,
}

impl Default for Theme {
    fn default() -> Self {
        // Tema oscuro por defecto inspirado en Tokyo Night
        Self {
            name: "tokyo-night".into(),
            bg: "#1a1b26".into(),
            bg_secondary: "#24283b".into(),
            fg: "#c0caf5".into(),
            fg_dim: "#565f89".into(),
            accent: "#7aa2f7".into(),
            error: "#f7768e".into(),
            warn: "#e0af68".into(),
            info: "#73daca".into(),
        }
    }
}
