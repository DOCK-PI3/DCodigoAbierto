use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

const BG:        Color = Color::Rgb(18, 18, 30);
const FG:        Color = Color::Rgb(205, 214, 244);
const SUBTLE:    Color = Color::Rgb(108, 112, 134);
const ACCENT:    Color = Color::Rgb(137, 180, 250);
const GREEN:     Color = Color::Rgb(166, 227, 161);
const RED:       Color = Color::Rgb(243, 139, 168);

pub struct MetaPanelWidget<'a> {
    pub session_name: &'a str,
    pub tokens_generated: usize,
    pub lsp_status_label: &'a str,   // "rust-analyzer ●" | "desactivado" | "…"
    pub lsp_ok: bool,
    pub cwd: &'a str,
    pub version: &'a str,
}

impl<'a> Widget for MetaPanelWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fondo del panel
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::Rgb(50, 50, 70)))
            .style(Style::default().bg(BG));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 { return; }

        let mut row = inner.y;

        // ── Nombre de sesión ──────────────────────────────────────────────────
        if row < inner.y + inner.height {
            Paragraph::new(Line::from(Span::styled(
                self.session_name,
                Style::default().fg(FG).add_modifier(Modifier::BOLD),
            )))
            .render(Rect { x: inner.x, y: row, width: inner.width, height: 1 }, buf);
            row += 1;
        }

        // gap
        row = row.saturating_add(1);
        if row >= inner.y + inner.height { return; }

        // ── Contexto (tokens) ─────────────────────────────────────────────────
        Paragraph::new(Line::from(Span::styled(
            "Context",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .render(Rect { x: inner.x, y: row, width: inner.width, height: 1 }, buf);
        row += 1;
        if row >= inner.y + inner.height { return; }

        // Tokens generados
        let tokens_k = format_tokens(self.tokens_generated);
        Paragraph::new(Line::from(Span::styled(
            format!("  {tokens_k} tokens"),
            Style::default().fg(FG),
        )))
        .render(Rect { x: inner.x, y: row, width: inner.width, height: 1 }, buf);
        row += 1;
        if row >= inner.y + inner.height { return; }

        // gap
        row = row.saturating_add(1);
        if row >= inner.y + inner.height { return; }

        // ── Estado LSP ────────────────────────────────────────────────────────
        Paragraph::new(Line::from(Span::styled(
            "LSP",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .render(Rect { x: inner.x, y: row, width: inner.width, height: 1 }, buf);
        row += 1;
        if row >= inner.y + inner.height { return; }

        let lsp_color = if self.lsp_ok { GREEN } else { SUBTLE };
        Paragraph::new(Line::from(Span::styled(
            format!("  {}", self.lsp_status_label),
            Style::default().fg(lsp_color),
        )))
        .render(Rect { x: inner.x, y: row, width: inner.width, height: 1 }, buf);

        // ── CWD (esquina inferior) ────────────────────────────────────────────
        let last_row = inner.y + inner.height.saturating_sub(2);
        if last_row > row {
            Paragraph::new(Line::from(Span::styled(
                self.cwd,
                Style::default().fg(SUBTLE),
            )))
            .render(Rect { x: inner.x, y: last_row, width: inner.width, height: 1 }, buf);
        }

        // Versión
        let ver_row = inner.y + inner.height.saturating_sub(1);
        if ver_row > row {
            Paragraph::new(Span::styled(
                self.version,
                Style::default().fg(SUBTLE),
            ))
            .alignment(Alignment::Right)
            .render(Rect { x: inner.x, y: ver_row, width: inner.width, height: 1 }, buf);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn format_tokens(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
