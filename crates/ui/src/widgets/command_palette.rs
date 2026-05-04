use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Widget},
};

use crate::palette::Palette;

// ── Tipos de items ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PaletteItemKind {
    /// Abrir un archivo (fuzzy)
    File { path: String },
    /// Seleccionar un modelo de IA
    Model { name: String },
    /// Acción del sistema
    Action { id: PaletteActionId, shortcut: &'static str },
    /// Separador de sección (no seleccionable)
    Section { label: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteActionId {
    SwitchModel,
    SelectTheme,
    ToggleMode,
    OpenEditor,
    NewSession,
    InjectBuffer,
    AbortStream,
    Quit,
}

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub kind:  PaletteItemKind,
    pub label: String,
}

impl PaletteItem {
    pub fn is_selectable(&self) -> bool {
        !matches!(self.kind, PaletteItemKind::Section { .. })
    }
}

// ── Widget ────────────────────────────────────────────────────────────────────

pub struct CommandPaletteWidget<'a> {
    pub query: &'a str,
    pub query_cursor: usize,
    pub items: &'a [PaletteItem],
    pub selected: usize,    pub palette: &'a Palette,}

impl<'a> Widget for CommandPaletteWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg     = self.palette.bg_secondary;
        let fg     = self.palette.fg;
        let dim    = self.palette.fg_dim;
        let acc    = self.palette.accent;
        let info   = self.palette.info;

        // Popup centrado: 60% ancho, 70% alto
        let popup = centered_rect(60, 70, area);
        Clear.render(popup, buf);

        let outer = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(dim))
            .style(Style::default().bg(bg));
        let inner = outer.inner(popup);
        outer.render(popup, buf);

        // Cabecera: "Commands" + "esc"
        let header_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
        let list_area   = Rect { x: inner.x, y: inner.y + 2, width: inner.width,
                                  height: inner.height.saturating_sub(3) };
        let search_area = Rect { x: inner.x, y: inner.y + 1, width: inner.width, height: 1 };

        // Header
        Paragraph::new(Line::from(vec![
            Span::styled("Commands", Style::default().fg(fg).add_modifier(Modifier::BOLD)),
        ])).render(header_area, buf);
        Paragraph::new(Line::from(vec![
            Span::styled("esc", Style::default().fg(dim)),
        ])).render(
            Rect { x: inner.x + inner.width.saturating_sub(3), y: inner.y, width: 3, height: 1 },
            buf,
        );

        // Search box
        let cursor    = self.query_cursor.min(self.query.len());
        let before    = &self.query[..cursor];
        let rest      = &self.query[cursor..];
        let cur_ch    = rest.chars().next().map(|c| c.to_string()).unwrap_or_default();
        let after_str = rest.chars().skip(1).collect::<String>();
        let search_line = if self.query.is_empty() {
            Line::from(Span::styled("Search...", Style::default().fg(dim)))
        } else {
            Line::from(vec![
                Span::styled(before,    Style::default().fg(fg)),
                Span::styled(
                    if cur_ch.is_empty() { " ".to_string() } else { cur_ch },
                    Style::default().bg(acc).fg(bg),
                ),
                Span::styled(after_str, Style::default().fg(fg)),
            ])
        };
        Paragraph::new(search_line).render(search_area, buf);

        // Lista de items
        let visible_h = list_area.height as usize;
        let scroll = if self.selected >= visible_h {
            self.selected - visible_h + 1
        } else {
            0
        };

        let mut items: Vec<ListItem> = vec![];
        for (i, item) in self.items.iter().enumerate() {
            let item_line = match &item.kind {
                PaletteItemKind::Section { label } => {
                    Line::from(Span::styled(
                        *label,
                        Style::default().fg(acc).add_modifier(Modifier::BOLD),
                    ))
                }
                PaletteItemKind::File { path } => {
                    let display = short_path(path);
                    let style = if i == self.selected {
                        Style::default().fg(bg).bg(acc)
                    } else {
                        Style::default().fg(fg)
                    };
                    Line::from(Span::styled(format!("  {display}"), style))
                }
                PaletteItemKind::Model { name } => {
                    let style = if i == self.selected {
                        Style::default().fg(bg).bg(acc)
                    } else {
                        Style::default().fg(fg)
                    };
                    Line::from(Span::styled(format!("  {name}"), style))
                }
                PaletteItemKind::Action { shortcut, .. } => {
                    let label_style = if i == self.selected {
                        Style::default().fg(bg).bg(acc)
                    } else {
                        Style::default().fg(fg)
                    };
                    let short_style = if i == self.selected {
                        Style::default().fg(bg).bg(acc)
                    } else {
                        Style::default().fg(info)
                    };
                    // Alinear shortcut a la derecha del ancho del popup
                    let label_padded = format!("  {}", item.label);
                    let pad = (list_area.width as usize)
                        .saturating_sub(label_padded.len() + shortcut.len());
                    Line::from(vec![
                        Span::styled(label_padded, label_style),
                        Span::styled(" ".repeat(pad), if i == self.selected {
                            Style::default().bg(acc)
                        } else {
                            Style::default()
                        }),
                        Span::styled(*shortcut, short_style),
                    ])
                }
            };
            items.push(ListItem::new(item_line));
        }

        let visible: Vec<ListItem> = items.into_iter().skip(scroll).take(visible_h).collect();
        List::new(visible)
            .style(Style::default().bg(bg))
            .render(list_area, buf);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match parts.len() {
        0 => path.to_string(),
        1 => parts[0].to_string(),
        n => format!("{}/{}", parts[n - 2], parts[n - 1]),
    }
}
