use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Widget},
};

use dca_types::FileLocation;

use crate::palette::Palette;

/// Panel flotante que muestra la lista de referencias LSP.
pub struct ReferencesWidget<'a> {
    pub references: &'a [FileLocation],
    pub selected: usize,
    pub palette: &'a Palette,
}

impl<'a> Widget for ReferencesWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let count = self.references.len();
        let title = format!(" Referencias ({count})  ↑↓: navegar  Enter: ir  Esc: cerrar ");
        let block = Block::default()
            .title(Span::styled(
                title,
                Style::default().fg(self.palette.bg).bg(self.palette.info).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.palette.info))
            .style(Style::default().bg(self.palette.bg_secondary));

        let inner = block.inner(area);
        block.render(area, buf);

        let visible = inner.height as usize;
        let scroll = if self.selected >= visible {
            self.selected - visible + 1
        } else {
            0
        };

        let items: Vec<ListItem> = self.references.iter().skip(scroll).take(visible).enumerate()
            .map(|(i, loc)| {
                let real_idx = i + scroll;
                let path_short = loc.path.split('/').last().unwrap_or(&loc.path);
                let line_num = loc.line + 1;
                let preview = loc.preview.as_deref().unwrap_or("…");
                let text = format!("  {path_short}:{line_num}  {preview}");
                let style = if real_idx == self.selected {
                    Style::default().fg(self.palette.bg).bg(self.palette.info).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.palette.fg).bg(self.palette.bg_secondary)
                };
                ListItem::new(Line::from(Span::styled(text, style)))
            })
            .collect();

        List::new(items)
            .style(Style::default().bg(self.palette.bg_secondary))
            .render(inner, buf);
    }
}

/// Calcula el Rect del panel de referencias (parte inferior del editor).
pub fn references_panel_rect(editor_area: Rect) -> Rect {
    let height = (editor_area.height / 3).max(5).min(15);
    Rect {
        x: editor_area.x,
        y: editor_area.y + editor_area.height - height,
        width: editor_area.width,
        height,
    }
}
