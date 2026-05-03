use unicode_width::UnicodeWidthStr;

use crate::cursor::Cursor;

/// Buffer de texto editable.
///
/// Almacena el contenido como `Vec<String>` (una entrada por línea).
/// El cursor y el offset de scroll se gestionan aquí para mantener
/// la lógica separada del renderizado.
#[derive(Debug, Clone)]
pub struct TextBuffer {
    /// Líneas de texto (siempre al menos una)
    pub lines: Vec<String>,
    /// Posición actual del cursor
    pub cursor: Cursor,
    /// Primera línea visible (scroll vertical)
    pub scroll_row: usize,
    /// Primera columna visible (scroll horizontal)
    pub scroll_col: usize,
    /// Nombre del archivo (o None si es un buffer nuevo)
    pub file_name: Option<String>,
    /// Indica si hay cambios sin guardar
    pub dirty: bool,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: Cursor::default(),
            scroll_row: 0,
            scroll_col: 0,
            file_name: None,
            dirty: false,
        }
    }
}

impl TextBuffer {
    /// Crea un buffer a partir de un string multilinea.
    pub fn from_str(content: &str, file_name: Option<String>) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };
        Self {
            lines,
            file_name,
            ..Default::default()
        }
    }

    // ── Consultas ─────────────────────────────────────────────────────────

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn current_line(&self) -> &str {
        &self.lines[self.cursor.row]
    }

    /// Ancho visual de la línea actual hasta la columna del cursor.
    pub fn cursor_visual_col(&self) -> usize {
        let line = self.current_line();
        let to_cursor = &line[..self.cursor.col.min(line.len())];
        UnicodeWidthStr::width(to_cursor)
    }

    // ── Movimiento de cursor ───────────────────────────────────────────────

    pub fn move_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.clamp_col();
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.clamp_col();
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor.col > 0 {
            // retroceder un carácter UTF-8
            let line = &self.lines[self.cursor.row];
            self.cursor.col = prev_char_boundary(line, self.cursor.col);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = self.lines[self.cursor.row].len();
        }
    }

    pub fn move_right(&mut self) {
        let line_len = self.lines[self.cursor.row].len();
        if self.cursor.col < line_len {
            let line = &self.lines[self.cursor.row];
            self.cursor.col = next_char_boundary(line, self.cursor.col);
        } else if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
    }

    pub fn move_line_start(&mut self) {
        self.cursor.col = 0;
    }

    pub fn move_line_end(&mut self) {
        self.cursor.col = self.lines[self.cursor.row].len();
    }

    // ── Edición ───────────────────────────────────────────────────────────

    pub fn insert_char(&mut self, ch: char) {
        let col = self.cursor.col;
        let line = &mut self.lines[self.cursor.row];
        line.insert(col, ch);
        self.cursor.col += ch.len_utf8();
        self.dirty = true;
    }

    pub fn insert_newline(&mut self) {
        let col = self.cursor.col;
        let rest = self.lines[self.cursor.row].split_off(col);
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.lines.insert(self.cursor.row, rest);
        self.dirty = true;
    }

    pub fn delete_char_before(&mut self) {
        if self.cursor.col > 0 {
            let prev = prev_char_boundary(&self.lines[self.cursor.row], self.cursor.col);
            self.lines[self.cursor.row].remove(prev);
            self.cursor.col = prev;
            self.dirty = true;
        } else if self.cursor.row > 0 {
            // unir con la línea anterior
            let current = self.lines.remove(self.cursor.row);
            self.cursor.row -= 1;
            self.cursor.col = self.lines[self.cursor.row].len();
            self.lines[self.cursor.row].push_str(&current);
            self.dirty = true;
        }
    }

    // ── Scroll ────────────────────────────────────────────────────────────

    /// Ajusta `scroll_row` para que el cursor sea siempre visible.
    pub fn update_scroll(&mut self, visible_rows: usize) {
        if self.cursor.row < self.scroll_row {
            self.scroll_row = self.cursor.row;
        } else if self.cursor.row >= self.scroll_row + visible_rows {
            self.scroll_row = self.cursor.row - visible_rows + 1;
        }
    }

    // ── Helpers privados ──────────────────────────────────────────────────

    /// Fija la columna al final de la línea actual si sobresale.
    fn clamp_col(&mut self) {
        let max = self.lines[self.cursor.row].len();
        if self.cursor.col > max {
            self.cursor.col = max;
        }
    }
}

/// Índice del primer byte del carácter UTF-8 anterior a `pos`.
fn prev_char_boundary(s: &str, pos: usize) -> usize {
    let mut i = pos.saturating_sub(1);
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Índice del primer byte del siguiente carácter UTF-8 después de `pos`.
fn next_char_boundary(s: &str, pos: usize) -> usize {
    let mut i = pos + 1;
    while i <= s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i.min(s.len())
}
