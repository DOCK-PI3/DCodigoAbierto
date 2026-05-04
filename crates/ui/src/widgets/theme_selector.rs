use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, StatefulWidget, Widget},
};

/// Convierte un string hex "#rrggbb" en Color de ratatui.
fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    Color::Reset
}

pub struct ThemeSelectorWidget<'a> {
    /// Lista de (nombre, color_acento, color_bg)
    pub themes: &'a [(String, String, String)],
    pub selected: usize,
}

impl<'a> Widget for ThemeSelectorWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = 52u16.min(area.width);
        let height = (self.themes.len() as u16 + 2).min(20).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup = Rect { x, y, width, height };

        Clear.render(popup, buf);

        let items: Vec<ListItem> = self.themes.iter().enumerate().map(|(i, (name, accent, bg))| {
            let acc_color = hex_to_color(accent);
            let bg_color = hex_to_color(bg);

            if i == self.selected {
                ListItem::new(Line::from(vec![
                    Span::styled(" ▶ ", Style::default().fg(Color::Black).bg(acc_color)),
                    Span::styled(
                        format!("{name:<28}"),
                        Style::default().fg(Color::Black).bg(acc_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  ██ ", Style::default().fg(acc_color).bg(bg_color)),
                ]))
            } else {
                ListItem::new(Line::from(vec![
                    Span::styled("   ", Style::default()),
                    Span::styled(
                        format!("{name:<28}"),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled("  ██ ", Style::default().fg(acc_color)),
                ]))
            }
        }).collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" Seleccionar tema (↑↓ Enter · Esc) ");

        let list = List::new(items).block(block);
        let mut state = ListState::default();
        state.select(Some(self.selected));
        StatefulWidget::render(list, popup, buf, &mut state);
    }
}
