use std::collections::HashMap;

use dca_types::{
    view_state::{FileEntry, Focus},
    CompletionEntry, DiagnosticInfo, FileLocation, TextBuffer,
};

// ── Chat mode ─────────────────────────────────────────────────────────────────

/// Modo de operación del agente.
/// - `Build`: todas las herramientas activas (lectura, escritura, shell, web…)
/// - `Plan`:  solo herramienta web_fetch activa
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMode {
    Build,
    Plan,
}

impl ChatMode {
    pub fn label(&self) -> &'static str {
        match self {
            ChatMode::Build => "Build",
            ChatMode::Plan  => "Plan",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            ChatMode::Build => ChatMode::Plan,
            ChatMode::Plan  => ChatMode::Build,
        }
    }
}

// ── Chat ──────────────────────────────────────────────────────────────────────

/// Un mensaje visible en el panel de chat.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// "user" | "assistant" | "tool"
    pub role: String,
    pub content: String,
    pub is_streaming: bool,
}

/// Herramienta pendiente de aprobación.
#[derive(Debug, Clone)]
pub struct PendingTool {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

/// Estado del panel de chat.
#[derive(Debug, Clone)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub input_cursor: usize,
    /// Primera línea visible en el scroll del chat
    pub scroll: usize,
    /// El modelo está generando una respuesta ahora mismo
    pub streaming: bool,
    /// Texto acumulado del stream en curso (se añade al último mensaje)
    pub streaming_buffer: String,
    /// Modelo seleccionado actualmente
    pub selected_model: String,
    /// Herramienta esperando aprobación
    pub tool_pending: Option<PendingTool>,
    /// Modo activo (Build / Plan)
    pub mode: ChatMode,
    /// Tokens generados en la sesión actual
    pub tokens_generated: usize,
    /// Segundos transcurridos desde el inicio del stream activo
    pub streaming_elapsed_secs: u64,
    /// Nombre de la sesión
    pub session_name: String,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            messages: vec![],
            input: String::new(),
            input_cursor: 0,
            scroll: 0,
            streaming: false,
            streaming_buffer: String::new(),
            selected_model: String::new(),
            tool_pending: None,
            mode: ChatMode::Build,
            tokens_generated: 0,
            streaming_elapsed_secs: 0,
            session_name: new_session_name(),
        }
    }
}

fn new_session_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("Sesión {secs}")
}

// ── Estado global de la aplicación ───────────────────────────────────────────

/// Estado global de la aplicación.
#[derive(Debug, Clone)]
pub struct AppState {
    pub quit: bool,
    pub focus: Focus,

    // ── Multi-buffer ─────────────────────────────────────────────────────
    /// Lista de buffers abiertos (pestaña activa = active_buffer)
    pub buffers: Vec<TextBuffer>,
    /// Índice del buffer activo
    pub active_buffer: usize,

    pub file_tree: Vec<FileEntry>,
    pub sidebar_selected: usize,
    pub sidebar_visible: bool,
    pub status_message: String,

    // ── LSP ──────────────────────────────────────────────────────────────
    pub diagnostics: HashMap<String, Vec<DiagnosticInfo>>,
    pub completions: Vec<CompletionEntry>,
    pub show_completions: bool,
    pub completion_selected: usize,
    pub lsp_status: LspStatus,

    // ── Referencias / Definición ─────────────────────────────────────────
    pub references: Vec<FileLocation>,
    pub references_selected: usize,
    pub show_references: bool,

    // ── Fuzzy Finder ─────────────────────────────────────────────────────
    pub fuzzy_active: bool,
    pub fuzzy_query: String,
    pub fuzzy_results: Vec<String>,   // rutas filtradas
    pub fuzzy_selected: usize,
    /// Todos los archivos del proyecto para filtrar
    pub fuzzy_all_files: Vec<String>,

    // ── Chat / IA ─────────────────────────────────────────────────────────
    pub chat_visible: bool,
    pub chat: ChatState,
    pub model_selector_active: bool,
    pub model_selector_models: Vec<String>,
    pub model_selector_selected: usize,

    // ── Theme Selector ────────────────────────────────────────────────────
    pub theme_selector_active: bool,
    pub theme_selector_selected: usize,
    /// Temas disponibles cargados al inicio
    pub available_themes: Vec<dca_config::Theme>,

    // ── Command Palette ───────────────────────────────────────────────────
    pub palette_active: bool,
    pub palette_query: String,
    pub palette_query_cursor: usize,
    pub palette_selected: usize,
}

/// Alias conveniente para el buffer activo
impl AppState {
    #[inline]
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffers[self.active_buffer]
    }

    #[inline]
    pub fn buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffers[self.active_buffer]
    }

    /// Abre un archivo en un nuevo buffer (o activa si ya está abierto).
    pub fn open_buffer(&mut self, path: &str, content: &str) {
        // ¿Ya está abierto?
        if let Some(idx) = self.buffers.iter().position(|b| {
            b.file_name.as_deref() == Some(path)
        }) {
            self.active_buffer = idx;
        } else {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_owned();
            self.buffers.push(TextBuffer::from_str(content, Some(name)));
            self.active_buffer = self.buffers.len() - 1;
        }
    }

    /// Cierra el buffer activo; si era el único, lo vacía.
    pub fn close_buffer(&mut self) {
        if self.buffers.len() == 1 {
            self.buffers[0] = TextBuffer::from_str("", None);
            return;
        }
        self.buffers.remove(self.active_buffer);
        if self.active_buffer >= self.buffers.len() {
            self.active_buffer = self.buffers.len() - 1;
        }
    }

    /// Nombre del modelo activo (de la config o el seleccionado)
    pub fn active_model(&self) -> &str {
        &self.chat.selected_model
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspStatus {
    Disabled,
    Connecting,
    Ready(String),
    Error(String),
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            quit: false,
            focus: Focus::Chat,
            buffers: vec![TextBuffer::from_str("", None)],
            active_buffer: 0,
            file_tree: vec![],
            sidebar_selected: 0,
            sidebar_visible: false,
            status_message: String::from(
                " DCodigoAbierto  |  Ctrl+P: archivos  |  Ctrl+G: goto def  |  Ctrl+Q: salir",
            ),
            diagnostics: HashMap::new(),
            completions: vec![],
            show_completions: false,
            completion_selected: 0,
            lsp_status: LspStatus::Disabled,
            references: vec![],
            references_selected: 0,
            show_references: false,
            fuzzy_active: false,
            fuzzy_query: String::new(),
            fuzzy_results: vec![],
            fuzzy_selected: 0,
            fuzzy_all_files: vec![],
            chat_visible: false,
            chat: ChatState::default(),
            model_selector_active: false,
            model_selector_models: vec![],
            model_selector_selected: 0,
            theme_selector_active: false,
            theme_selector_selected: 0,
            available_themes: dca_config::builtin_themes(),
            palette_active: false,
            palette_query: String::new(),
            palette_query_cursor: 0,
            palette_selected: 0,
        }
    }
}
