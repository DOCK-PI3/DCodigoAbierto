use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Widget},
};

use dca_types::CompletionEntry;

use crate::palette::Palette;

/// Popup flotante de autocompletado LSP.
///
/// Se renderiza sobre el editor; el caller calcula el `area` basándose
/// en la posición del cursor.
pub struct CompletionWidget<'a> {
    pub items: &'a [CompletionEntry],
    pub selected: usize,
    pub palette: &'a Palette,
}

impl<'a> Widget for CompletionWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.items.is_empty() || area.height < 3 {
            return;
        }

        let block = Block::default()
            .title(Span::styled(
                " Completions  Esc: cerrar  Enter: aceptar ",
                Style::default()
                    .fg(self.palette.bg)
                    .bg(self.palette.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.palette.accent))
            .style(Style::default().bg(self.palette.bg_secondary));

        let inner = block.inner(area);
        block.render(area, buf);

        // Split inner: label (izq) | kind+detail (der)
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Min(1)])
            .split(inner);

        let label_area = chunks[0];
        let detail_area = chunks[1];

        let visible = label_area.height as usize;
        // Scroll para que `selected` siempre sea visible
        let scroll = if self.selected >= visible {
            self.selected - visible + 1
        } else {
            0
        };

        let labels: Vec<ListItem> = self
            .items
            .iter()
            .skip(scroll)
            .take(visible)
            .enumerate()
            .map(|(i, item)| {
                let real_idx = i + scroll;
                let is_sel = real_idx == self.selected;
                let style = if is_sel {
                    Style::default()
                        .fg(self.palette.bg)
                        .bg(self.palette.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(self.palette.fg)
                        .bg(self.palette.bg_secondary)
                };
                let prefix = item.kind.as_deref().unwrap_or("·");
                let label = format!(" {prefix:<8} {}", item.label);
                ListItem::new(Line::from(Span::styled(label, style)))
            })
            .collect();

        List::new(labels)
            .style(Style::default().bg(self.palette.bg_secondary))
            .render(label_area, buf);

        // Panel derecho: detail del item seleccionado
        if let Some(item) = self.items.get(self.selected) {
            if let Some(detail) = &item.detail {
                let d = Span::styled(
                    format!(" {detail}"),
                    Style::default().fg(self.palette.info).bg(self.palette.bg_secondary),
                );
                ratatui::widgets::Paragraph::new(Line::from(d))
                    .style(Style::default().bg(self.palette.bg_secondary))
                    .render(detail_area, buf);
            }
        }
    }
}

/// Calcula el `Rect` donde debe aparecer el popup de completions,
/// justo debajo de la posición visual del cursor dentro de `editor_area`.
pub fn completion_popup_rect(
    editor_area: Rect,
    cursor_screen_row: u16, // fila del cursor en coordenadas de pantalla
    cursor_screen_col: u16,
    item_count: usize,
) -> Rect {
    let popup_height = (item_count.min(10) as u16 + 2).min(editor_area.height.saturating_sub(2));
    let popup_width = 60u16.min(editor_area.width.saturating_sub(2));

    // Intentar mostrar debajo del cursor; si no cabe, encima
    let y = if cursor_screen_row + popup_height + 1 <= editor_area.y + editor_area.height {
        cursor_screen_row + 1
    } else {
        cursor_screen_row.saturating_sub(popup_height)
    };

    // Alinear a la izquierda del cursor; clamp al borde derecho
    let x = cursor_screen_col
        .min(editor_area.x + editor_area.width - popup_width - 1)
        .max(editor_area.x);

    Rect { x, y, width: popup_width, height: popup_height }
}
