use async_trait::async_trait;
use color_eyre::Result;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use crate::provider::{AiEvent, AiMessage, AiProvider, AiRole, ToolCall, ToolDef};

pub struct AnthropicProvider {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

// ── SSE types (Anthropic streaming) ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicEvent {
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDelta,
    },
    MessageStop,
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct MessageDelta {
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
}
#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

// ── Construcción del cuerpo ──────────────────────────────────────────────────

fn build_messages(messages: &[AiMessage]) -> (Option<String>, Vec<serde_json::Value>) {
    let mut system = None;
    let mut out = vec![];

    for m in messages {
        match m.role {
            AiRole::System => {
                system = Some(m.content.clone());
            }
            AiRole::User => {
                out.push(serde_json::json!({ "role": "user", "content": m.content }));
            }
            AiRole::Assistant => {
                if !m.tool_calls.is_empty() {
                    let blocks: Vec<_> = m.tool_calls.iter().map(|tc| serde_json::json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": tc.arguments,
                    })).collect();
                    out.push(serde_json::json!({ "role": "assistant", "content": blocks }));
                } else {
                    out.push(serde_json::json!({ "role": "assistant", "content": m.content }));
                }
            }
            AiRole::Tool => {
                if let Some(tr) = &m.tool_result {
                    out.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tr.call_id,
                            "content": tr.content,
                        }]
                    }));
                }
            }
        }
    }
    (system, out)
}

fn build_tools(tools: &[ToolDef]) -> Vec<serde_json::Value> {
    tools.iter().map(|t| serde_json::json!({
        "name": t.name,
        "description": t.description,
        "input_schema": t.parameters,
    })).collect()
}

// ── Implementación del trait ─────────────────────────────────────────────────

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/v1/models", self.base_url);
        let resp: AnthropicModelsResponse = self.client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .send().await?.json().await?;
        Ok(resp.data.into_iter().map(|m| m.id).collect())
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDef],
        max_tokens: u32,
        tx: UnboundedSender<AiEvent>,
    ) -> Result<()> {
        let url = format!("{}/v1/messages", self.base_url);
        let (system, msgs) = build_messages(messages);

        let mut body = serde_json::json!({
            "model": "claude-opus-4-5",
            "messages": msgs,
            "max_tokens": max_tokens,
            "stream": true,
        });
        if let Some(s) = system {
            body["system"] = serde_json::Value::String(s);
        }
        if !tools.is_empty() {
            body["tools"] = serde_json::json!(build_tools(tools));
        }

        debug!("anthropic chat_stream → {url}");

        let resp = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let _ = tx.send(AiEvent::Error(format!("Anthropic error {status}: {text}")));
            return Ok(());
        }

        let mut stream = resp.bytes_stream();

        // Buffers indexados para tool_use blocks
        let mut block_ids: Vec<String> = vec![];
        let mut block_names: Vec<String> = vec![];
        let mut block_args: Vec<String> = vec![];

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
                if !line.starts_with("data: ") { continue; }
                let data = &line["data: ".len()..];

                match serde_json::from_str::<AnthropicEvent>(data) {
                    Ok(event) => match event {
                        AnthropicEvent::ContentBlockStart { index, content_block } => {
                            while block_ids.len() <= index {
                                block_ids.push(String::new());
                                block_names.push(String::new());
                                block_args.push(String::new());
                            }
                            if let ContentBlock::ToolUse { id, name } = content_block {
                                block_ids[index] = id;
                                block_names[index] = name;
                            }
                        }
                        AnthropicEvent::ContentBlockDelta { index, delta } => {
                            match delta {
                                ContentDelta::TextDelta { text } => {
                                    let _ = tx.send(AiEvent::Chunk(text));
                                }
                                ContentDelta::InputJsonDelta { partial_json } => {
                                    if let Some(buf) = block_args.get_mut(index) {
                                        buf.push_str(&partial_json);
                                    }
                                }
                            }
                        }
                        AnthropicEvent::ContentBlockStop { index } => {
                            if let Some(name) = block_names.get(index) {
                                if !name.is_empty() {
                                    let id = block_ids.get(index)
                                        .cloned().unwrap_or_else(|| format!("call_{index}"));
                                    let args: serde_json::Value = serde_json::from_str(
                                        block_args.get(index).map(|s| s.as_str()).unwrap_or("{}")
                                    ).unwrap_or(serde_json::json!({}));
                                    let _ = tx.send(AiEvent::ToolCallRequest(ToolCall {
                                        id,
                                        name: name.clone(),
                                        arguments: args,
                                    }));
                                }
                            }
                        }
                        AnthropicEvent::MessageStop => {
                            let _ = tx.send(AiEvent::Done);
                            return Ok(());
                        }
                        _ => {}
                    },
                    Err(e) => {
                        debug!("anthropic parse: {e} | {data}");
                    }
                }
            }
        }

        let _ = tx.send(AiEvent::Done);
        Ok(())
    }
}
