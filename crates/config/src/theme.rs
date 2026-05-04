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

/// Lista de todos los themes incluidos en el binario.
/// Los temas provienen del archivo `themes.txt` en la raíz del repo,
/// embebido en tiempo de compilación con `include_str!`.
pub fn builtin_themes() -> Vec<Theme> {
    const THEMES_SRC: &str = include_str!("../../../themes.txt");
    parse_themes(THEMES_SRC)
}

/// Parsea el formato de `themes.txt`:
/// Bloques de `key = "value"` separados por líneas vacías.
/// La clave `name` inicia un nuevo bloque de tema.
pub fn parse_themes(src: &str) -> Vec<Theme> {
    let mut themes = Vec::new();
    let mut current: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for line in src.lines() {
        let line = line.trim();
        // Ignorar comentarios y cabeceras [theme]
        if line.is_empty() || line.starts_with('[') || line.starts_with('#') {
            // Línea vacía = separador de bloque
            if line.is_empty() {
                if let Some(t) = try_build_theme(&current) {
                    themes.push(t);
                    current.clear();
                }
            }
            continue;
        }
        if let Some((k, v)) = parse_kv(line) {
            current.insert(k, v);
        }
    }
    // Último bloque sin línea vacía al final
    if let Some(t) = try_build_theme(&current) {
        themes.push(t);
    }
    themes
}

fn parse_kv(line: &str) -> Option<(String, String)> {
    let (key, rest) = line.split_once('=')?;
    let key = key.trim().to_string();
    let val = rest.trim().trim_matches('"').to_string();
    if key.is_empty() || val.is_empty() { return None; }
    Some((key, val))
}

fn try_build_theme(map: &std::collections::HashMap<String, String>) -> Option<Theme> {
    Some(Theme {
        name: map.get("name")?.clone(),
        bg: map.get("bg")?.clone(),
        bg_secondary: map.get("bg_secondary")?.clone(),
        fg: map.get("fg")?.clone(),
        fg_dim: map.get("fg_dim")?.clone(),
        accent: map.get("accent")?.clone(),
        error: map.get("error")?.clone(),
        warn: map.get("warn")?.clone(),
        info: map.get("info")?.clone(),
    })
}
