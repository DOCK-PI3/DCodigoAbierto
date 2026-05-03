use async_trait::async_trait;
use color_eyre::Result;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use crate::provider::{AiEvent, AiMessage, AiProvider, AiRole, ToolCall, ToolDef};

pub struct OllamaProvider {
    base_url: String,
    default_model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: &str, default_model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

// ── Tipos de respuesta Ollama ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OllamaMessage {
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
    messages.iter().map(|m| {
        let role = match m.role {
            AiRole::System    => "system",
            AiRole::User      => "user",
            AiRole::Assistant => "assistant",
            AiRole::Tool      => "tool",
        };
        if let Some(tr) = &m.tool_result {
            serde_json::json!({ "role": role, "content": tr.content })
        } else if !m.tool_calls.is_empty() {
            let calls: Vec<_> = m.tool_calls.iter().map(|tc| {
                serde_json::json!({
                    "function": { "name": tc.name, "arguments": tc.arguments }
                })
            }).collect();
            serde_json::json!({ "role": role, "content": m.content, "tool_calls": calls })
        } else {
            serde_json::json!({ "role": role, "content": m.content })
        }
    }).collect()
}

fn build_tools(tools: &[ToolDef]) -> Vec<serde_json::Value> {
    tools.iter().map(|t| serde_json::json!({
        "type": "function",
        "function": {
            "name": t.name,
            "description": t.description,
            "parameters": t.parameters,
        }
    })).collect()
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn name(&self) -> &str { "ollama" }

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
        tx: UnboundedSender<AiEvent>,
    ) -> Result<()> {
        let url = format!("{}/api/chat", self.base_url);

        let mut body = serde_json::json!({
            "model": self.default_model,
            "messages": build_messages(messages),
            "stream": true,
            "options": { "num_predict": max_tokens },
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
        let mut tool_call_buf: Vec<(String, String, serde_json::Value)> = vec![];

        while let Some(chunk) = stream.next().await {
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(AiEvent::Error(e.to_string()));
                    break;
                }
            };

            for line in std::str::from_utf8(&bytes).unwrap_or("").lines() {
                let line = line.trim();
                if line.is_empty() { continue; }

                match serde_json::from_str::<OllamaChatResponse>(line) {
                    Ok(resp) => {
                        // Tool calls
                        for tc in &resp.message.tool_calls {
                            let id = format!("call_{}", uuid_simple());
                            tool_call_buf.push((
                                id.clone(),
                                tc.function.name.clone(),
                                tc.function.arguments.clone(),
                            ));
                            let _ = tx.send(AiEvent::ToolCallRequest(ToolCall {
                                id,
                                name: tc.function.name.clone(),
                                arguments: tc.function.arguments.clone(),
                            }));
                        }
                        // Texto
                        if !resp.message.content.is_empty() {
                            let _ = tx.send(AiEvent::Chunk(resp.message.content));
                        }
                        if resp.done {
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

        let _ = tx.send(AiEvent::Done);
        Ok(())
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
    format!("{t:08x}")
}
