use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::palette::Palette;

const BG_STATUS:  Color = Color::Rgb(30, 30, 46);
const FG_STATUS:  Color = Color::Rgb(205, 214, 244);
const ACCENT:     Color = Color::Rgb(137, 180, 250);
const SUBTLE:     Color = Color::Rgb(108, 112, 134);
const STREAMING:  Color = Color::Rgb(249, 226, 175);
const BUILD_C:    Color = Color::Rgb(249, 226, 175);
const PLAN_C:     Color = Color::Rgb(137, 180, 250);

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
        // Fondo sólido
        for x in area.left()..area.right() {
            buf[(x, area.y)].set_bg(BG_STATUS);
        }

        if self.streaming {
            // ── Modo streaming ────────────────────────────────────────────
            let left = Line::from(vec![
                Span::styled("●●● ", Style::default().fg(STREAMING).add_modifier(Modifier::BOLD)),
                Span::styled("generando… ", Style::default().fg(FG_STATUS)),
                Span::styled("ctrl+x interrumpir", Style::default().fg(SUBTLE)),
            ]);
            let tokens_k = format_tokens(self.tokens_generated);
            let right = Line::from(vec![
                Span::styled(format!("{tokens_k}  "), Style::default().fg(STREAMING)),
                Span::styled("ctrl+p", Style::default().fg(SUBTLE)),
            ]);

            Paragraph::new(left)
                .alignment(Alignment::Left)
                .style(Style::default().bg(BG_STATUS))
                .render(area, buf);
            Paragraph::new(right)
                .alignment(Alignment::Right)
                .style(Style::default().bg(BG_STATUS))
                .render(area, buf);
        } else if self.home_mode {
            // ── Home mode ─────────────────────────────────────────────────
            let mode_color = if self.chat_mode_is_build { BUILD_C } else { PLAN_C };

            let left = Line::from(vec![
                Span::styled(self.chat_mode_label, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
                Span::styled(" · ", Style::default().fg(SUBTLE)),
                Span::styled(self.active_model, Style::default().fg(FG_STATUS)),
            ]);
            let right = Line::from(vec![
                Span::styled("ctrl+p ", Style::default().fg(SUBTLE)),
                Span::styled("comandos", Style::default().fg(ACCENT)),
            ]);

            Paragraph::new(left)
                .alignment(Alignment::Left)
                .style(Style::default().bg(BG_STATUS))
                .render(area, buf);
            Paragraph::new(right)
                .alignment(Alignment::Right)
                .style(Style::default().bg(BG_STATUS))
                .render(area, buf);
        } else {
            // ── Modo normal (archivo abierto) ─────────────────────────────
            let line = Line::from(vec![Span::styled(self.message, Style::default().fg(FG_STATUS))]);
            Paragraph::new(line)
                .alignment(Alignment::Left)
                .style(Style::default().bg(BG_STATUS))
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
