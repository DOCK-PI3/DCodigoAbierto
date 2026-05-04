use async_trait::async_trait;
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

// ── Rol de un mensaje ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiRole {
    System,
    User,
    Assistant,
    Tool,
}

// ── Llamada a herramienta (solicitada por el modelo) ────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// ID único de la llamada (generado por el modelo)
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Resultado de ejecutar una herramienta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub content: String,
}

// ── Mensaje en el historial ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: AiRole,
    /// Texto del mensaje (puede estar vacío si hay tool_calls)
    pub content: String,
    /// Herramientas que el modelo quiere invocar
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Resultados de herramientas (cuando role = Tool)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<ToolResult>,
}

impl AiMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: AiRole::System, content: content.into(), tool_calls: vec![], tool_result: None }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: AiRole::User, content: content.into(), tool_calls: vec![], tool_result: None }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: AiRole::Assistant, content: content.into(), tool_calls: vec![], tool_result: None }
    }
    pub fn tool_result(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        let call_id = call_id.into();
        let content_str: String = content.into();
        Self {
            role: AiRole::Tool,
            content: content_str.clone(),
            tool_calls: vec![],
            tool_result: Some(ToolResult { call_id, content: content_str }),
        }
    }
}

// ── Definición de herramienta (JSON Schema para function calling) ────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema del objeto `parameters`
    pub parameters: serde_json::Value,
}

// ── Eventos que emite el streaming ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum AiEvent {
    /// Fragmento de texto del modelo
    Chunk(String),
    /// El modelo quiere invocar una herramienta
    ToolCallRequest(ToolCall),
    /// Una herramienta se ejecutó; lleva su nombre y el resultado (truncado)
    ToolResult { name: String, result: String },
    /// Respuesta completada
    Done,
    /// Error irrecuperable
    Error(String),
}

// ── Trait del proveedor ──────────────────────────────────────────────────────

#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Nombre del proveedor ("ollama", "openai", "anthropic", …)
    fn name(&self) -> &str;

    /// Lista los modelos disponibles en este proveedor
    async fn list_models(&self) -> Result<Vec<String>>;

    /// Lanza un stream de chat. Los eventos se envían por `tx`.
    /// `tools` puede estar vacío → el modelo responde solo con texto.
    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDef],
        max_tokens: u32,
        temperature: f32,
        top_p: f32,
        tx: UnboundedSender<AiEvent>,
    ) -> Result<()>;
}
