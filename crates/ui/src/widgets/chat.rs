use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget, Wrap},
};
use dca_types::view_state::ChatMessageView;

use crate::highlight;

pub struct ChatWidget<'a> {
    pub messages: &'a [ChatMessageView],
    pub input: &'a str,
    pub input_cursor: usize,
    pub scroll: usize,
    pub streaming: bool,
    pub focused: bool,
}

impl<'a> Widget for ChatWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 4 { return; }

        // Dividir en área de mensajes + barra de input
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(area);

        let msg_area = chunks[0];
        let input_area = chunks[1];

        // ── Panel de mensajes ─────────────────────────────────────────────

        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Chat IA ");

        let inner = block.inner(msg_area);
        block.render(msg_area, buf);

        let mut items: Vec<ListItem> = vec![];

        for msg in self.messages {
            let (prefix, style) = match msg.role.as_str() {
                "user"      => ("▶ Tú     ", Style::default().fg(Color::Green)),
                "assistant" => ("◀ AI     ", Style::default().fg(Color::Cyan)),
                "tool"      => ("⚙ Tool   ", Style::default().fg(Color::Yellow)),
                _           => ("  ?      ", Style::default()),
            };

            // Encabezado de rol
            let header = Line::from(vec![
                Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
            ]);
            items.push(ListItem::new(header));

            // Líneas del contenido con syntax highlight
            let content = if msg.is_streaming {
                format!("{}▋", msg.content)
            } else {
                msg.content.clone()
            };

            if msg.role == "assistant" {
                // Aplicar syntax highlight en mensajes del asistente
                for hl_line in highlight::render_message(&content) {
                    // Indentar cada línea
                    let mut spans = vec![Span::raw("         ")];
                    spans.extend(hl_line.spans);
                    items.push(ListItem::new(Line::from(spans)));
                }
            } else {
                let text_style = Style::default().fg(Color::White);
                for line in content.lines() {
                    let indented = format!("         {line}");
                    items.push(ListItem::new(
                        Line::from(Span::styled(indented, text_style))
                    ));
                }
            }

            // Separador
            items.push(ListItem::new(Line::from("")));
        }

        // scroll = líneas subidas desde el fondo (0 = mostrar fondo)
        let total  = items.len();
        let height = inner.height as usize;
        let max_from_top = total.saturating_sub(height);
        let clamped = self.scroll.min(max_from_top);
        let skip = max_from_top.saturating_sub(clamped);

        let visible: Vec<ListItem> = items
            .into_iter()
            .skip(skip)
            .take(height)
            .collect();

        let list = List::new(visible);
        list.render(inner, buf);

        // ── Input ─────────────────────────────────────────────────────────

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Mensaje ");

        let input_inner = input_block.inner(input_area);
        input_block.render(input_area, buf);

        let display = if self.focused {
            // Muestra cursor
            let (before, after) = self.input.split_at(
                self.input_cursor.min(self.input.len())
            );
            let after_char = after.chars().next().map(|c| c.to_string()).unwrap_or_default();
            let rest = after.chars().skip(1).collect::<String>();
            Text::from(Line::from(vec![
                Span::raw(before),
                Span::styled(
                    if after_char.is_empty() { " ".to_string() } else { after_char },
                    Style::default().bg(Color::Cyan).fg(Color::Black),
                ),
                Span::raw(rest),
            ]))
        } else {
            Text::from(self.input)
        };

        Paragraph::new(display)
            .wrap(Wrap { trim: false })
            .render(input_inner, buf);
    }
}
