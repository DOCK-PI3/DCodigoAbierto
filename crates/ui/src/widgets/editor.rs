use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

use dca_types::{DiagnosticInfo, DiagnosticSeverity, TextBuffer};

use crate::palette::Palette;

/// Widget del panel de editor con texto real, numeración y cursor.
pub struct EditorWidget<'a> {
    pub buffer: &'a TextBuffer,
    pub palette: &'a Palette,
    pub active: bool,
    pub diagnostics: &'a [DiagnosticInfo],
}

impl<'a> Widget for EditorWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.active {
            Style::default().fg(self.palette.accent)
        } else {
            Style::default().fg(self.palette.fg_dim)
        };

        let title_text = match &self.buffer.file_name {
            Some(name) => format!(" {} {} ", name, if self.buffer.dirty { "[+]" } else { "" }),
            None => " *nuevo* ".to_string(),
        };

        let block = Block::default()
            .title(Span::styled(
                title_text,
                Style::default()
                    .fg(self.palette.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().bg(self.palette.bg));

        let inner = block.inner(area);
        block.render(area, buf);

        // Calcular ancho de la columna de números de línea
        let total_lines = self.buffer.line_count();
        // gutter = marcador diagnóstico (1) + espacio (1) + número + espacio
        let gutter_width = format!("{}", total_lines).len() + 3; // +3 = "● " + espacio sep

        // Split inner area: gutter | contenido
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(gutter_width as u16),
                Constraint::Min(1),
            ])
            .split(inner);

        let gutter_area = chunks[0];
        let content_area = chunks[1];
        let visible_rows = content_area.height as usize;

        // Actualizar scroll (sin mutar el buffer aquí — solo leer)
        let scroll_row = self.buffer.scroll_row;

        // ── Renderizar números de línea + marcadores de diagnóstico (gutter) ──
        let gutter_lines: Vec<Line> = (scroll_row..scroll_row + visible_rows)
            .map(|row| {
                if row < total_lines {
                    // Buscar el diagnóstico de mayor severidad en esta línea
                    let diag = self.diagnostics.iter().find(|d| d.line as usize == row);
                    let (marker, marker_style) = match diag {
                        Some(d) => match d.severity {
                            DiagnosticSeverity::Error => (
                                "●",
                                Style::default().fg(self.palette.error),
                            ),
                            DiagnosticSeverity::Warning => (
                                "●",
                                Style::default().fg(self.palette.warn),
                            ),
                            DiagnosticSeverity::Info | DiagnosticSeverity::Hint => (
                                "●",
                                Style::default().fg(self.palette.info),
                            ),
                        },
                        None => (" ", Style::default()),
                    };

                    let num = format!("{:>width$} ", row + 1, width = gutter_width - 3);
                    let num_style = if row == self.buffer.cursor.row {
                        Style::default().fg(self.palette.accent)
                    } else {
                        Style::default().fg(self.palette.fg_dim)
                    };
                    Line::from(vec![
                        Span::styled(marker, marker_style),
                        Span::raw(" "),
                        Span::styled(num, num_style),
                    ])
                } else {
                    Line::from(Span::styled(
                        "  ~ ",
                        Style::default().fg(self.palette.fg_dim),
                    ))
                }
            })
            .collect();

        Paragraph::new(gutter_lines)
            .style(Style::default().bg(self.palette.bg))
            .render(gutter_area, buf);

        // ── Renderizar contenido del buffer ───────────────────────────────
        let cursor_row = self.buffer.cursor.row;
        let cursor_col = self.buffer.cursor.col;

        let content_lines: Vec<Line> = (scroll_row..scroll_row + visible_rows)
            .map(|row| {
                if row >= total_lines {
                    return Line::from("");
                }
                let line_str = &self.buffer.lines[row];

                if row != cursor_row || !self.active {
                    // Línea sin cursor — renderizar directamente
                    Line::from(Span::styled(
                        line_str.as_str(),
                        Style::default().fg(self.palette.fg).bg(self.palette.bg),
                    ))
                } else {
                    // Línea con cursor — resaltar la celda del cursor
                    build_cursor_line(line_str, cursor_col, self.palette)
                }
            })
            .collect();

        Paragraph::new(content_lines)
            .style(Style::default().fg(self.palette.fg).bg(self.palette.bg))
            .render(content_area, buf);
    }
}

/// Construye una `Line` con la celda del cursor resaltada.
fn build_cursor_line<'a>(line: &'a str, cursor_col: usize, palette: &'a Palette) -> Line<'a> {
    let cursor_col = cursor_col.min(line.len());

    if line.is_empty() {
        // Línea vacía: mostrar un bloque cursor
        return Line::from(vec![Span::styled(
            " ",
            Style::default()
                .bg(palette.accent)
                .fg(palette.bg)
                .add_modifier(Modifier::BOLD),
        )]);
    }

    // Encontrar el siguiente límite de carácter UTF-8 para el cursor
    let next_boundary = {
        let mut i = cursor_col + 1;
        while i <= line.len() && !line.is_char_boundary(i) {
            i += 1;
        }
        i.min(line.len())
    };

    let before = &line[..cursor_col];
    let at_cursor = if cursor_col < line.len() {
        &line[cursor_col..next_boundary]
    } else {
        " " // cursor al final de línea
    };
    let after = if next_boundary < line.len() {
        &line[next_boundary..]
    } else {
        ""
    };

    let _ = UnicodeWidthStr::width(before); // usado en Fase 3 para scroll horizontal

    Line::from(vec![
        Span::styled(before, Style::default().fg(palette.fg).bg(palette.bg)),
        Span::styled(
            at_cursor,
            Style::default()
                .bg(palette.accent)
                .fg(palette.bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(after, Style::default().fg(palette.fg).bg(palette.bg)),
    ])
}

