use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::palette::Palette;

/// Barra de estado inferior de 1 fila.
pub struct StatusbarWidget<'a> {
    pub palette: &'a Palette,
    pub message: &'a str,
    // Nuevos campos para el diseño mejorado
    pub streaming: bool,
    pub tokens_generated: usize,
    pub streaming_elapsed_secs: u64,
    pub chat_mode_label: &'a str,
    pub chat_mode_is_build: bool,
    pub active_model: &'a str,
    pub home_mode: bool,   // true si no hay archivo abierto
}

impl<'a> Widget for StatusbarWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg   = self.palette.bg_secondary;
        let fg   = self.palette.fg;
        let acc  = self.palette.accent;
        let dim  = self.palette.fg_dim;
        let warn = self.palette.warn;

        // Fondo sólido
        for x in area.left()..area.right() {
            buf[(x, area.y)].set_bg(bg);
        }

        if self.streaming {
            // ── Modo streaming ────────────────────────────────────────────
            let left = Line::from(vec![
                Span::styled("●●● ", Style::default().fg(warn).add_modifier(Modifier::BOLD)),
                Span::styled("generando… ", Style::default().fg(fg)),
                Span::styled("ctrl+x interrumpir", Style::default().fg(dim)),
            ]);
            let tokens_k = format_tokens(self.tokens_generated);
            let right = Line::from(vec![
                Span::styled(format!("{tokens_k}  "), Style::default().fg(warn)),
                Span::styled("ctrl+p", Style::default().fg(dim)),
            ]);

            Paragraph::new(left)
                .alignment(Alignment::Left)
                .style(Style::default().bg(bg))
                .render(area, buf);
            Paragraph::new(right)
                .alignment(Alignment::Right)
                .style(Style::default().bg(bg))
                .render(area, buf);
        } else if self.home_mode {
            // ── Home mode ─────────────────────────────────────────────────
            let mode_color = if self.chat_mode_is_build { warn } else { acc };

            let left = Line::from(vec![
                Span::styled(self.chat_mode_label, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
                Span::styled(" · ", Style::default().fg(dim)),
                Span::styled(self.active_model, Style::default().fg(fg)),
            ]);
            let right = Line::from(vec![
                Span::styled("ctrl+p ", Style::default().fg(dim)),
                Span::styled("comandos", Style::default().fg(acc)),
            ]);

            Paragraph::new(left)
                .alignment(Alignment::Left)
                .style(Style::default().bg(bg))
                .render(area, buf);
            Paragraph::new(right)
                .alignment(Alignment::Right)
                .style(Style::default().bg(bg))
                .render(area, buf);
        } else {
            // ── Modo normal (archivo abierto) ─────────────────────────────
            let line = Line::from(vec![Span::styled(self.message, Style::default().fg(fg))]);
            Paragraph::new(line)
                .alignment(Alignment::Left)
                .style(Style::default().bg(bg))
                .render(area, buf);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn format_tokens(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M tokens", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K tokens", n as f64 / 1_000.0)
    } else {
        format!("{n} tokens")
    }
}
