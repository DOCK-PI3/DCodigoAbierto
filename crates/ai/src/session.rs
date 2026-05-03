use crate::provider::{AiMessage, AiRole, ToolCall, ToolResult};

/// Historial de mensajes de una sesión de chat (en memoria).
#[derive(Debug, Default)]
pub struct ChatSession {
    pub messages: Vec<AiMessage>,
}

impl ChatSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        self.messages.push(AiMessage::user(text));
    }

    pub fn push_assistant(&mut self, text: impl Into<String>) {
        self.messages.push(AiMessage::assistant(text));
    }

    pub fn push_assistant_with_tools(&mut self, text: impl Into<String>, calls: Vec<ToolCall>) {
        self.messages.push(AiMessage {
            role: AiRole::Assistant,
            content: text.into(),
            tool_calls: calls,
            tool_result: None,
        });
    }

    pub fn push_tool_result(&mut self, call_id: impl Into<String>, result: impl Into<String>) {
        self.messages.push(AiMessage::tool_result(call_id, result));
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Mensajes totales (sin contar el system prompt del agente)
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}
