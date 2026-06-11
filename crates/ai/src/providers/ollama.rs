use async_trait::async_trait;
use color_eyre::Result;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, warn};

use crate::provider::{AiEvent, AiMessage, AiProvider, AiRole, ToolCall, ToolDef};

pub struct OllamaProvider {
    base_url: String,
    default_model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: &str, default_model: &str) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
            client,
        }
    }
}

// ── Tipos de respuesta Ollama ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Debug, Deserialize)]
struct OllamaToolCall {
    function: OllamaToolCallFunction,
}

#[derive(Debug, Deserialize)]
struct OllamaToolCallFunction {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    #[serde(default)]
    done: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModel>,
}

// ── Construcción del cuerpo de la petición ───────────────────────────────────

fn build_messages(messages: &[AiMessage]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| {
            let role = match m.role {
                AiRole::System => "system",
                AiRole::User => "user",
                AiRole::Assistant => "assistant",
                AiRole::Tool => "tool",
            };
            if let Some(tr) = &m.tool_result {
                serde_json::json!({ "role": role, "content": tr.content })
            } else if !m.tool_calls.is_empty() {
                let calls: Vec<_> = m
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "function": { "name": tc.name, "arguments": tc.arguments }
                        })
                    })
                    .collect();
                serde_json::json!({ "role": role, "content": m.content, "tool_calls": calls })
            } else {
                serde_json::json!({ "role": role, "content": m.content })
            }
        })
        .collect()
}

fn build_tools(tools: &[ToolDef]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        })
        .collect()
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);
        let resp: OllamaModelsResponse = self.client.get(&url).send().await?.json().await?;
        Ok(resp.models.into_iter().map(|m| m.name).collect())
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDef],
        max_tokens: u32,
        temperature: f32,
        top_p: f32,
        tx: UnboundedSender<AiEvent>,
    ) -> Result<()> {
        let url = format!("{}/api/chat", self.base_url);

        let mut body = serde_json::json!({
            "model": self.default_model,
            "messages": build_messages(messages),
            "stream": true,
            "options": {
                "num_predict": max_tokens,
                "temperature": temperature,
                "top_p": top_p,
            },
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(build_tools(tools));
        }

        debug!("ollama chat_stream → {url}");

        let resp = self.client.post(&url).json(&body).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let _ = tx.send(AiEvent::Error(format!("Ollama error {status}: {text}")));
            return Ok(());
        }

        let mut stream = resp.bytes_stream();
        let mut stream_buffer = String::new();
        let mut accumulated_text = String::new();
        let mut has_native_tool_calls = false;

        while let Some(chunk) = stream.next().await {
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(AiEvent::Error(e.to_string()));
                    break;
                }
            };

            for line in drain_complete_lines(&mut stream_buffer, &bytes) {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }

                match serde_json::from_str::<OllamaChatResponse>(&line) {
                    Ok(resp) => {
                        // Tool calls nativos (function calling del modelo)
                        for tc in &resp.message.tool_calls {
                            has_native_tool_calls = true;
                            let id = format!("call_{}", uuid_simple());
                            let _ = tx.send(AiEvent::ToolCallRequest(ToolCall {
                                id,
                                name: tc.function.name.clone(),
                                arguments: tc.function.arguments.clone(),
                                buffer_version: None,
                                target_buffer_id: None,
                            }));
                        }
                        // Texto
                        if !resp.message.content.is_empty() {
                            accumulated_text.push_str(&resp.message.content);
                            let _ = tx.send(AiEvent::Chunk(resp.message.content));
                        }
                        if resp.done {
                            // Fallback: si no hubo tool calls nativos, buscar JSON en el texto
                            if !has_native_tool_calls && !tools.is_empty() {
                                emit_text_tool_calls(&accumulated_text, &tx);
                            }
                            let _ = tx.send(AiEvent::Done);
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        debug!("ollama parse error: {e} | line: {line}");
                    }
                }
            }
        }

        for line in drain_remaining_line(&mut stream_buffer) {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<OllamaChatResponse>(&line) {
                Ok(resp) => {
                    for tc in &resp.message.tool_calls {
                        has_native_tool_calls = true;
                        let id = format!("call_{}", uuid_simple());
                        let _ = tx.send(AiEvent::ToolCallRequest(ToolCall {
                            id,
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                            buffer_version: None,
                            target_buffer_id: None,
                        }));
                    }
                    if !resp.message.content.is_empty() {
                        accumulated_text.push_str(&resp.message.content);
                        let _ = tx.send(AiEvent::Chunk(resp.message.content));
                    }
                    if resp.done {
                        if !has_native_tool_calls && !tools.is_empty() {
                            emit_text_tool_calls(&accumulated_text, &tx);
                        }
                        let _ = tx.send(AiEvent::Done);
                        return Ok(());
                    }
                }
                Err(e) => {
                    debug!("ollama parse error: {e} | line: {line}");
                }
            }
        }

        // Fallback si el stream terminó sin done flag
        if !has_native_tool_calls && !tools.is_empty() {
            emit_text_tool_calls(&accumulated_text, &tx);
        }

        let _ = tx.send(AiEvent::Done);
        Ok(())
    }
}

/// Busca tool calls en formato texto JSON dentro del contenido acumulado.
/// Esto es un fallback para modelos que no usan function calling nativo
/// y en su lugar escriben el JSON como texto (siguiendo el formato del system prompt).
///
/// Formato esperado: `{"tool":"nombre","arguments":{...}}`
fn emit_text_tool_calls(accumulated_text: &str, tx: &UnboundedSender<AiEvent>) {
    // Buscar patrones como {"tool":"read_file","arguments":{"path":"..."}}
    let mut idx = 0usize;
    while let Some(start) = accumulated_text[idx..].find("{\"tool\":\"") {
        let abs_start = idx + start;
        // Encontrar el cierre del JSON (balanceando llaves)
        let slice = &accumulated_text[abs_start..];
        let mut depth = 0u32;
        let mut end = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (i, ch) in slice.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        end = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        if end == 0 {
            // No se encontró cierre; avanzar
            idx = abs_start + 1;
            continue;
        }

        let json_str = &slice[..end];
        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(obj) => {
                let tool_name = obj.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = obj
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                if !tool_name.is_empty() {
                    let id = format!("call_{}", uuid_simple());
                    warn!(
                        "Ollama: modelo usó tool call en texto en vez de nativo. Tool={tool_name}. \
                         Considera usar un modelo con mejor soporte de function calling."
                    );
                    let _ = tx.send(AiEvent::ToolCallRequest(ToolCall {
                        id,
                        name: tool_name.to_string(),
                        arguments,
                        buffer_version: None,
                        target_buffer_id: None,
                    }));
                }
            }
            Err(e) => {
                debug!("ollama text tool parse error: {e}");
            }
        }

        idx = abs_start + end;
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{t:08x}")
}

fn drain_complete_lines(buffer: &mut String, bytes: &[u8]) -> Vec<String> {
    buffer.push_str(&String::from_utf8_lossy(bytes));
    let mut lines = Vec::new();
    while let Some(pos) = buffer.find('\n') {
        let line: String = buffer.drain(..=pos).collect();
        lines.push(line.trim_end_matches(['\r', '\n']).to_string());
    }
    lines
}

fn drain_remaining_line(buffer: &mut String) -> Vec<String> {
    if buffer.trim().is_empty() {
        buffer.clear();
        Vec::new()
    } else {
        vec![std::mem::take(buffer)]
    }
}
