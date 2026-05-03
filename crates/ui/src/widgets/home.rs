use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Widget},
};
use dca_types::view_state::ChatMessageView;

use crate::highlight;

// ── Logo ──────────────────────────────────────────────────────────────────────
const LOGO: &[&str] = &[
    " ██████╗  ██████╗ █████╗ ",
    " ██╔══██╗██╔════╝██╔══██╗",
    " ██║  ██║██║     ███████║",
    " ██║  ██║██║     ██╔══██║",
    " ██████╔╝╚██████╗██║  ██║",
    " ╚═════╝  ╚═════╝╚═╝  ╚═╝",
];

const TIPS: &[&str] = &[
    "Ctrl+I para inyectar el buffer actual como contexto",
    "Ctrl+P para buscar archivos y cambiar modelo",
    "Ctrl+G para ir a definición (LSP)",
    "Tab para alternar modo Build ↔ Plan",
];

// ── Colores ───────────────────────────────────────────────────────────────────
const BG:          Color = Color::Rgb(14, 14, 22);
const BG_INPUT:    Color = Color::Rgb(24, 24, 37);
const BG_INPUT_HL: Color = Color::Rgb(32, 32, 52);
const BG_MSG:      Color = Color::Rgb(18, 18, 28);
const FG:          Color = Color::Rgb(205, 214, 244);
const ACCENT:      Color = Color::Rgb(137, 180, 250);
const SUBTLE:      Color = Color::Rgb(108, 112, 134);
const BUILD_COLOR: Color = Color::Rgb(249, 226, 175);
const PLAN_COLOR:  Color = Color::Rgb(137, 180, 250);
const TIP_COLOR:   Color = Color::Rgb(249, 226, 175);
const USER_COLOR:  Color = Color::Rgb(166, 227, 161);
const AI_COLOR:    Color = Color::Rgb(137, 180, 250);
const TOOL_COLOR:  Color = Color::Rgb(249, 226, 175);

// ── Widget ────────────────────────────────────────────────────────────────────

pub struct HomeWidget<'a> {
    pub chat_input: &'a str,
    pub chat_input_cursor: usize,
    pub focused: bool,
    pub streaming: bool,
    pub active_model: &'a str,
    pub provider_name: &'a str,
    pub chat_mode_label: &'a str,
    pub chat_mode_is_build: bool,
    pub version: &'a str,
    /// Historial de mensajes de la sesión actual
    pub messages: &'a [ChatMessageView],
}

impl<'a> Widget for HomeWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_bg(BG);
            }
        }

        let has_content = !self.messages.is_empty() || self.streaming;
        if has_content {
            self.render_chat_mode(area, buf);
        } else {
            self.render_splash_mode(area, buf);
        }
    }
}

impl<'a> HomeWidget<'a> {
    // ── Modo chat (hay mensajes o streaming activo) ───────────────────────────
    fn render_chat_mode(self, area: Rect, buf: &mut Buffer) {
        let input_h  = 4u16; // 2 bordes + 1 texto + 1 modo
        let bottom_h = 1u16;

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(input_h),
                Constraint::Length(bottom_h),
            ])
            .split(area);

        let msg_area   = rows[0];
        let input_area = rows[1];
        let hint_area  = rows[2];

        // Fondo del área de mensajes
        for y in msg_area.top()..msg_area.bottom() {
            for x in msg_area.left()..msg_area.right() {
                buf[(x, y)].set_bg(BG_MSG);
            }
        }

        // Construir y renderizar mensajes
        let items = self.build_message_items(msg_area.width);
        let total  = items.len();
        let height = msg_area.height as usize;
        let skip   = total.saturating_sub(height);
        let visible: Vec<ListItem> = items.into_iter().skip(skip).collect();
        List::new(visible)
            .style(Style::default().bg(BG_MSG))
            .render(msg_area, buf);

        // Input centrado
        let iw = self.input_width(area.width);
        let ix = area.x + (area.width.saturating_sub(iw)) / 2;
        let centered_input = Rect { x: ix, y: input_area.y, width: iw, height: input_area.height };
        // Rellena el resto de la fila con BG_INPUT para que quede bien
        self.render_input_box(centered_input, buf);

        // Hint
        self.render_hint_line(hint_area, buf);
    }

    // ── Modo splash (sin mensajes) ────────────────────────────────────────────
    fn render_splash_mode(self, area: Rect, buf: &mut Buffer) {
        let logo_h   = LOGO.len() as u16;
        let gap1:u16 = 1;
        let input_h  = 4u16;
        let gap2:u16 = 1;
        let hints_h  = 1u16;
        let tip_h    = 1u16;
        let cwd_h    = 1u16;

        let input_w = self.input_width(area.width);
        let input_x = area.x + (area.width.saturating_sub(input_w)) / 2;

        let content_h = logo_h + gap1 + input_h + gap2 + hints_h;
        let bottom_h  = tip_h + cwd_h;
        let top_space = area.height.saturating_sub(content_h).saturating_sub(bottom_h) / 2;

        let logo_y  = area.y + top_space;
        let input_y = logo_y + logo_h + gap1;
        let hints_y = input_y + input_h + gap2;

        // Logo
        let logo_area = Rect { x: area.x, y: logo_y, width: area.width, height: logo_h };
        let logo_lines: Vec<Line> = LOGO.iter().map(|l|
            Line::from(Span::styled(*l, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)))
        ).collect();
        Paragraph::new(logo_lines).alignment(Alignment::Center).render(logo_area, buf);

        // Input
        let input_area = Rect { x: input_x, y: input_y, width: input_w, height: input_h };
        self.render_input_box(input_area, buf);

        // Hints
        let hints_area = Rect { x: input_x, y: hints_y, width: input_w, height: hints_h };
        Paragraph::new(Line::from(vec![
            Span::styled("tab agentes", Style::default().fg(SUBTLE)),
            Span::styled("   ", Style::default()),
            Span::styled("ctrl+p comandos", Style::default().fg(SUBTLE)),
        ]))
        .alignment(Alignment::Right)
        .render(hints_area, buf);

        // Tip + CWD
        if area.height >= 4 {
            let tip_y = area.y + area.height.saturating_sub(bottom_h);
            let tip_area = Rect { x: area.x, y: tip_y, width: area.width, height: tip_h };
            let tip_idx = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() / 30)
                .unwrap_or(0) as usize) % TIPS.len();
            Paragraph::new(Line::from(vec![
                Span::styled("● ", Style::default().fg(TIP_COLOR)),
                Span::styled("Tip  ", Style::default().fg(TIP_COLOR).add_modifier(Modifier::BOLD)),
                Span::styled(TIPS[tip_idx], Style::default().fg(SUBTLE)),
            ]))
            .alignment(Alignment::Center)
            .render(tip_area, buf);

            let cwd_area = Rect { x: area.x, y: tip_y + tip_h, width: area.width, height: cwd_h };
            let cwd = cwd_str();
            Paragraph::new(Span::styled(&cwd, Style::default().fg(SUBTLE)))
                .alignment(Alignment::Left)
                .render(cwd_area, buf);
            Paragraph::new(Span::styled(self.version, Style::default().fg(SUBTLE)))
                .alignment(Alignment::Right)
                .render(cwd_area, buf);
        }
    }

    // ── Input box ─────────────────────────────────────────────────────────────
    fn render_input_box(&self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(ACCENT)
        } else {
            Style::default().fg(SUBTLE)
        };
        let input_bg = if self.focused { BG_INPUT_HL } else { BG_INPUT };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().bg(input_bg));

        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height == 0 { return; }

        let text_rows_h = inner.height.saturating_sub(1).max(1);
        let inner_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(text_rows_h), Constraint::Length(1)])
            .split(inner);

        let text_area = inner_split[0];
        let mode_area = inner_split[1];

        // Texto del input
        if self.streaming {
            Paragraph::new(Line::from(Span::styled(
                "▋ Generando respuesta…",
                Style::default().fg(ACCENT),
            )))
            .render(text_area, buf);
        } else {
            self.render_input_text(text_area, buf);
        }

        // Línea de modo
        let mode_color = if self.chat_mode_is_build { BUILD_COLOR } else { PLAN_COLOR };
        let provider_cap = capitalize(if self.provider_name.is_empty() { "Local" } else { self.provider_name });
        Paragraph::new(Line::from(vec![
            Span::styled(self.chat_mode_label, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
            Span::styled(" · ", Style::default().fg(SUBTLE)),
            Span::styled(self.active_model, Style::default().fg(FG)),
            Span::styled("  ", Style::default()),
            Span::styled(provider_cap, Style::default().fg(SUBTLE)),
        ]))
        .render(mode_area, buf);
    }

    /// Renderiza el texto del input con scroll horizontal:
    /// el cursor siempre es visible aunque el texto sea más ancho que el área.
    fn render_input_text(&self, area: Rect, buf: &mut Buffer) {
        if self.chat_input.is_empty() {
            Paragraph::new(Span::styled(
                "Escribe algo… p.ej: \"Explica este código\"",
                Style::default().fg(SUBTLE),
            ))
            .render(area, buf);
            return;
        }

        let chars: Vec<char> = self.chat_input.chars().collect();
        let cursor    = self.chat_input_cursor.min(chars.len());
        let visible_w = area.width as usize;

        // Desplazar ventana para que el cursor siempre sea visible
        let window_start = if cursor >= visible_w {
            cursor - visible_w + 1
        } else {
            0
        };

        let before_chars: String = chars[window_start..cursor].iter().collect();
        let cur_ch: String = if cursor < chars.len() {
            chars[cursor].to_string()
        } else {
            " ".to_string()
        };
        let after_start = cursor + 1;
        let after_end   = (window_start + visible_w).min(chars.len());
        let after_chars: String = if after_start < after_end {
            chars[after_start..after_end].iter().collect()
        } else {
            String::new()
        };

        // "‹" si hay texto oculto a la izquierda
        let scroll_indicator = if window_start > 0 {
            Span::styled("‹", Style::default().fg(SUBTLE))
        } else {
            Span::raw("")
        };

        let line = Line::from(vec![
            scroll_indicator,
            Span::styled(before_chars, Style::default().fg(FG)),
            Span::styled(cur_ch, Style::default().bg(ACCENT).fg(BG).add_modifier(Modifier::BOLD)),
            Span::styled(after_chars, Style::default().fg(FG)),
        ]);
        Paragraph::new(line).render(area, buf);
    }

    fn render_hint_line(&self, area: Rect, buf: &mut Buffer) {
        for x in area.left()..area.right() {
            buf[(x, area.y)].set_bg(BG);
        }
        let mode_color = if self.chat_mode_is_build { BUILD_COLOR } else { PLAN_COLOR };
        Paragraph::new(Line::from(vec![
            Span::styled(self.chat_mode_label, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
            Span::styled(" · ", Style::default().fg(SUBTLE)),
            Span::styled(self.active_model, Style::default().fg(SUBTLE)),
        ]))
        .alignment(Alignment::Left)
        .render(area, buf);
        Paragraph::new(Line::from(vec![
            Span::styled("ctrl+p ", Style::default().fg(SUBTLE)),
            Span::styled("comandos", Style::default().fg(ACCENT)),
        ]))
        .alignment(Alignment::Right)
        .render(area, buf);
    }

    fn build_message_items(&self, width: u16) -> Vec<ListItem<'static>> {
        let wrap_w = (width as usize).saturating_sub(8);
        let mut items: Vec<ListItem<'static>> = vec![];

        for msg in self.messages {
            let (prefix, color) = match msg.role.as_str() {
                "user"      => ("▶ Tú",   USER_COLOR),
                "assistant" => ("◀ IA",   AI_COLOR),
                "tool"      => ("⚙ Tool", TOOL_COLOR),
                _           => ("  ?",    SUBTLE),
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ])));

            let content = if msg.is_streaming {
                format!("{}▋", msg.content)
            } else {
                msg.content.clone()
            };

            if msg.role == "assistant" {
                for hl_line in highlight::render_message(&content) {
                    let mut spans = vec![Span::raw("    ")];
                    spans.extend(hl_line.spans);
                    items.push(ListItem::new(Line::from(spans)));
                }
            } else {
                for line in word_wrap(&content, wrap_w) {
                    items.push(ListItem::new(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(line, Style::default().fg(FG)),
                    ])));
                }
            }

            items.push(ListItem::new(Line::from("")));
        }

        items
    }

    fn input_width(&self, area_width: u16) -> u16 {
        (area_width * 60 / 100).max(44).min(80).min(area_width.saturating_sub(4))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn word_wrap(text: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 { return vec![text.to_string()]; }
    let mut lines = vec![];
    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let word_len = word.chars().count();
            let cur_len  = current.chars().count();
            if cur_len == 0 {
                current.push_str(word);
            } else if cur_len + 1 + word_len <= max_w {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current.clone());
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None    => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn cwd_str() -> String {
    std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".to_string())
}
