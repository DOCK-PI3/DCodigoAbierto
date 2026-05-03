use ratatui::style::Color;

use dca_config::Theme;

/// Convierte la paleta de colores del `Theme` (strings hex/nombre)
/// a colores `ratatui::style::Color` listos para usar en widgets.
pub struct Palette {
    pub bg: Color,
    pub bg_secondary: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub accent: Color,
    pub error: Color,
    pub warn: Color,
    pub info: Color,
}

impl Palette {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            bg: parse_color(&theme.bg),
            bg_secondary: parse_color(&theme.bg_secondary),
            fg: parse_color(&theme.fg),
            fg_dim: parse_color(&theme.fg_dim),
            accent: parse_color(&theme.accent),
            error: parse_color(&theme.error),
            warn: parse_color(&theme.warn),
            info: parse_color(&theme.info),
        }
    }
}

/// Parsea una cadena de color en formato `#RRGGBB` o nombre ANSI.
/// Si el formato no se reconoce devuelve `Color::Reset`.
fn parse_color(s: &str) -> Color {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Color::Rgb(r, g, b);
            }
        }
    }
    // Fallback a nombre ANSI básico
    match s.to_lowercase().as_str() {
        "black"   => Color::Black,
        "red"     => Color::Red,
        "green"   => Color::Green,
        "yellow"  => Color::Yellow,
        "blue"    => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan"    => Color::Cyan,
        "white"   => Color::White,
        _         => Color::Reset,
    }
}
