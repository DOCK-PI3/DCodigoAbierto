use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use dca_types::BufferTab;

use crate::palette::Palette;

/// Barra de pestañas que muestra los buffers abiertos.
pub struct TabBarWidget<'a> {
    pub tabs: &'a [BufferTab],
    pub active: usize,
    pub palette: &'a Palette,
}

impl<'a> Widget for TabBarWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 { return; }

        let spans: Vec<Span> = self.tabs.iter().enumerate().flat_map(|(i, tab)| {
            let dirty = if tab.dirty { "+" } else { "" };
            let label = format!(" {}{} ", tab.name, dirty);
            if i == self.active {
                vec![
                    Span::styled(
                        label,
                        Style::default()
                            .fg(self.palette.bg)
                            .bg(self.palette.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("│", Style::default().fg(self.palette.fg_dim).bg(self.palette.bg_secondary)),
                ]
            } else {
                vec![
                    Span::styled(
                        label,
                        Style::default().fg(self.palette.fg_dim).bg(self.palette.bg_secondary),
                    ),
                    Span::styled("│", Style::default().fg(self.palette.fg_dim).bg(self.palette.bg_secondary)),
                ]
            }
        }).collect();

        Paragraph::new(Line::from(spans))
            .style(Style::default().bg(self.palette.bg_secondary))
            .render(area, buf);
    }
}
