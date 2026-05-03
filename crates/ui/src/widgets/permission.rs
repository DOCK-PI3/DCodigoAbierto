use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

pub struct PermissionDialog<'a> {
    pub tool_name: &'a str,
    pub args_display: &'a str,
}

impl<'a> Widget for PermissionDialog<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Centrar el diálogo
        let width = 60u16.min(area.width);
        let height = 10u16.min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let dialog = Rect { x, y, width, height };

        // Limpiar la zona antes de dibujar
        Clear.render(dialog, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Yellow))
            .title(Span::styled(
                " ⚠  Confirmar herramienta ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(dialog);
        block.render(dialog, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(2),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        Paragraph::new(Line::from(vec![
            Span::raw("Herramienta: "),
            Span::styled(self.tool_name, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ])).render(chunks[0], buf);

        Paragraph::new(Line::from(vec![
            Span::raw("Argumentos:"),
        ])).render(chunks[1], buf);

        Paragraph::new(self.args_display)
            .style(Style::default().fg(Color::Gray))
            .render(chunks[2], buf);

        Paragraph::new(Line::from(vec![
            Span::styled(
                " [Y] Permitir  ",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " [N] Denegar ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ])).render(chunks[4], buf);
    }
}
