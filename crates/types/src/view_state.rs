use crate::buffer::TextBuffer;
use crate::location::FileLocation;
use crate::lsp_event::{CompletionEntry, DiagnosticInfo};

/// Metadatos de una pestaña de buffer para la tab bar.
#[derive(Debug, Clone)]
pub struct BufferTab {
    pub name: String,
    pub dirty: bool,
}

/// Mensaje visible en el panel de chat.
#[derive(Debug, Clone)]
pub struct ChatMessageView {
    pub role: String,
    pub content: String,
    pub is_streaming: bool,
}

/// Herramienta pendiente de aprobación (para el diálogo de permiso).
#[derive(Debug, Clone)]
pub struct ToolCallView {
    pub name: String,
    pub args_display: String,
}

/// Proyección de solo lectura del AppState que la capa UI necesita.
pub struct ViewState<'a> {
    pub buffer: &'a TextBuffer,
    pub focus: Focus,
    pub status_message: &'a str,
    pub file_tree: &'a [FileEntry],
    pub sidebar_selected: usize,
    pub sidebar_visible: bool,
    // LSP
    pub diagnostics: &'a [DiagnosticInfo],
    pub completions: &'a [CompletionEntry],
    pub show_completions: bool,
    pub completion_selected: usize,
    // Referencias
    pub references: &'a [FileLocation],
    pub references_selected: usize,
    pub show_references: bool,
    // Multi-buffer
    pub buffers: Vec<BufferTab>,
    pub active_buffer: usize,
    // Fuzzy finder
    pub fuzzy_active: bool,
    pub fuzzy_query: &'a str,
    pub fuzzy_results: &'a [String],
    pub fuzzy_selected: usize,
    // Chat / IA
    pub chat_visible: bool,
    pub chat_messages: &'a [ChatMessageView],
    pub chat_input: &'a str,
    pub chat_input_cursor: usize,
    pub chat_scroll: usize,
    pub chat_streaming: bool,
    pub chat_tool_pending: Option<ToolCallView>,
    pub model_selector_active: bool,
    pub model_selector_models: &'a [String],
    pub model_selector_selected: usize,
    pub active_model: &'a str,
    pub provider_name: &'a str,
    // Modo y metadatos de sesión
    pub chat_mode_label: &'a str,   // "Build" | "Plan"
    pub chat_mode_is_build: bool,   // true=Build(naranja) false=Plan(azul)
    pub session_name: &'a str,
    pub tokens_generated: usize,
    pub streaming_elapsed_secs: u64,
    // Command Palette
    pub palette_active: bool,
    pub palette_query: &'a str,
    pub palette_query_cursor: usize,
    pub palette_selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Editor,
    Sidebar,
    Chat,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub depth: usize,
    pub is_dir: bool,
}
