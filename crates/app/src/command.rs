/// Side-effects que `update()` puede solicitar al loop principal.
///
/// `update()` es una función pura (sin I/O). Cuando necesita que
/// ocurra algo externo (llamada LSP, apertura de archivo, etc.)
/// devuelve un `Command` que `app.rs` ejecuta.
#[derive(Debug)]
pub enum Command {
    /// Abrir un documento en el servidor LSP
    LspOpen { path: String, text: String },
    /// Notificar al servidor LSP que el documento cambió
    LspChange { path: String, text: String },
    /// Pedir autocompletado al servidor LSP
    LspRequestCompletion,
    /// Pedir go-to-definition al servidor LSP
    LspGotoDefinition,
    /// Pedir find-references al servidor LSP
    LspFindReferences,
    /// Abrir un archivo por ruta (carga en nuevo buffer)
    OpenFile(String),

    // ── IA / Chat ─────────────────────────────────────────────────────────────
    /// Enviar el mensaje del input al agente y lanzar stream
    AiSendMessage,
    /// Ejecutar una herramienta ya aprobada por el usuario
    AiExecuteTool { id: String },
    /// Cancelar el stream de respuesta en curso
    AiAbortStream,
    /// Solicitar la lista de modelos al proveedor
    AiLoadModels,
    /// Inyectar el contenido del buffer activo como contexto en el chat
    AiInjectBuffer,
}
