use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, StatefulWidget, Widget},
};

pub struct ModelSelectorWidget<'a> {
    pub models: &'a [String],
    pub selected: usize,
}

impl<'a> Widget for ModelSelectorWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = 50u16.min(area.width);
        let height = (self.models.len() as u16 + 2).min(20).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup = Rect { x, y, width, height };

        Clear.render(popup, buf);

        let items: Vec<ListItem> = self.models.iter().enumerate().map(|(i, m)| {
            if i == self.selected {
                ListItem::new(format!(" ▶ {m}"))
                    .style(Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
            } else {
                ListItem::new(format!("   {m}"))
                    .style(Style::default().fg(Color::White))
            }
        }).collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Seleccionar modelo (↑↓ Enter) ");

        let list = List::new(items).block(block);
        let mut state = ListState::default();
        state.select(Some(self.selected));
        StatefulWidget::render(list, popup, buf, &mut state);
    }
}
