use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Widget},
};

use crate::palette::Palette;

/// Popup de fuzzy finder de archivos.
pub struct FuzzyWidget<'a> {
    pub query: &'a str,
    pub results: &'a [String],
    pub selected: usize,
    pub palette: &'a Palette,
}

impl<'a> Widget for FuzzyWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(Span::styled(
                " Fuzzy Finder  Esc: cerrar  Enter: abrir ",
                Style::default().fg(self.palette.bg).bg(self.palette.accent).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.palette.accent))
            .style(Style::default().bg(self.palette.bg));

        let inner = block.inner(area);
        block.render(area, buf);

        // Split: prompt (1 línea) | resultados (resto)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);

        // Prompt de búsqueda
        let prompt = format!("> {}_", self.query);
        Paragraph::new(Span::styled(
            prompt,
            Style::default().fg(self.palette.fg).bg(self.palette.bg),
        ))
        .style(Style::default().bg(self.palette.bg))
        .render(chunks[0], buf);

        // Lista de resultados
        let visible = chunks[1].height as usize;
        let scroll = if self.selected >= visible {
            self.selected - visible + 1
        } else {
            0
        };

        let items: Vec<ListItem> = self.results.iter().skip(scroll).take(visible).enumerate()
            .map(|(i, path)| {
                let real_idx = i + scroll;
                // Mostrar solo el nombre + directorio padre para claridad
                let display = short_path(path);
                let style = if real_idx == self.selected {
                    Style::default().fg(self.palette.bg).bg(self.palette.accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.palette.fg).bg(self.palette.bg)
                };
                ListItem::new(Line::from(Span::styled(format!("  {display}"), style)))
            })
            .collect();

        List::new(items)
            .style(Style::default().bg(self.palette.bg))
            .render(chunks[1], buf);
    }
}

/// Calcula el Rect centrado del popup.
pub fn fuzzy_popup_rect(terminal: Rect) -> Rect {
    let width  = (terminal.width  * 70 / 100).max(40).min(terminal.width  - 4);
    let height = (terminal.height * 60 / 100).max(10).min(terminal.height - 4);
    let x = (terminal.width  - width)  / 2;
    let y = (terminal.height - height) / 2;
    Rect { x, y, width, height }
}

fn short_path(path: &str) -> String {
    // Muestra los últimos 2 componentes: dir/file.ext
    let parts: Vec<&str> = path.split('/').collect();
    let n = parts.len();
    if n >= 2 {
        format!("{}/{}", parts[n - 2], parts[n - 1])
    } else {
        path.to_owned()
    }
}
