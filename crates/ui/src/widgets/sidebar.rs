use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Widget},
};

use dca_types::view_state::FileEntry;

use crate::palette::Palette;

/// Widget del sidebar con árbol de archivos.
pub struct SidebarWidget<'a> {
    pub entries: &'a [FileEntry],
    pub selected: usize,
    pub palette: &'a Palette,
    pub active: bool,
}

impl<'a> Widget for SidebarWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.active {
            Style::default().fg(self.palette.accent)
        } else {
            Style::default().fg(self.palette.fg_dim)
        };

        let block = Block::default()
            .title(Span::styled(
                " Explorador ",
                Style::default()
                    .fg(self.palette.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().bg(self.palette.bg_secondary));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.entries.is_empty() {
            let empty = Line::from(Span::styled(
                " (vacío)",
                Style::default().fg(self.palette.fg_dim),
            ));
            ratatui::widgets::Paragraph::new(empty)
                .style(Style::default().bg(self.palette.bg_secondary))
                .render(inner, buf);
            return;
        }

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let indent = "  ".repeat(entry.depth);
                let icon = if entry.is_dir { " " } else { " " };
                let label = format!("{indent}{icon}{}", entry.name);

                let style = if i == self.selected {
                    Style::default()
                        .fg(self.palette.bg)
                        .bg(self.palette.accent)
                        .add_modifier(Modifier::BOLD)
                } else if entry.is_dir {
                    Style::default()
                        .fg(self.palette.info)
                        .bg(self.palette.bg_secondary)
                } else {
                    Style::default()
                        .fg(self.palette.fg)
                        .bg(self.palette.bg_secondary)
                };

                ListItem::new(Line::from(Span::styled(label, style)))
            })
            .collect();

        List::new(items)
            .style(Style::default().bg(self.palette.bg_secondary))
            .render(inner, buf);
    }
}
