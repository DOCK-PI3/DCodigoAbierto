use crossterm::event::KeyEvent;
use dca_config::AppConfig;
use dca_types::{view_state::FileEntry, LspEvent};
use tokio::sync::mpsc;

/// Todos los mensajes que puede recibir el loop principal de la aplicación.
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// Evento de teclado del usuario
    Key(KeyEvent),
    /// Terminal redimensionado
    Resize(u16, u16),
    /// Tick periódico para refrescar la UI o lanzar tareas
    Tick,
    /// Señal interna para salir limpiamente
    Quit,
    /// Evento proveniente del cliente LSP
    Lsp(LspEvent),
    /// Config recargada por hot-reload
    ConfigReload(AppConfig),
    /// Árbol de archivos cargado de forma lazy al inicio
    FileTreeLoaded(Vec<FileEntry>),

    // ── IA / Chat ─────────────────────────────────────────────────────────────
    /// Fragmento de texto del modelo (streaming)
    AiStreamChunk(String),
    /// El stream terminó con éxito
    AiStreamDone,
    /// Error irrecuperable en el stream
    AiStreamError(String),
    /// El modelo quiere ejecutar una herramienta (requiere aprobación)
    AiToolRequest {
        id: String,
        name: String,
        args: serde_json::Value,
    },
    /// El usuario aprobó la ejecución de la herramienta `id`
    AiToolApproved(String),
    /// El usuario denegó la ejecución de la herramienta `id`
    AiToolDenied(String),
    /// Lista de modelos disponibles obtenida del proveedor
    AiModelsLoaded(Vec<String>),
}

/// Alias del lado emisor del canal de mensajes.
pub type EventSender = mpsc::UnboundedSender<AppMessage>;

/// Alias del lado receptor del canal de mensajes.
pub type EventReceiver = mpsc::UnboundedReceiver<AppMessage>;

/// Bus de eventos central de la aplicación.
///
/// Todos los productores de mensajes (crossterm, LSP, tareas async)
/// reciben un clon del `sender` para enviar mensajes al loop principal.
pub struct EventBus {
    pub sender: EventSender,
    pub receiver: EventReceiver,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self { sender, receiver }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
