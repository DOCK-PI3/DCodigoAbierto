use async_trait::async_trait;
use color_eyre::Result;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use crate::provider::{AiEvent, AiMessage, AiProvider, AiRole, ToolCall, ToolDef};

pub struct OpenAiProvider {
    base_url: String,
    api_key: String,
    default_model: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(base_url: &str, api_key: &str, default_model: &str) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            default_model: default_model.to_string(),
            client,
        }
    }
}

// ── SSE chunk ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    function: Option<DeltaFunction>,
}

#[derive(Debug, Deserialize)]
struct DeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct ModelData {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelData>,
}

fn build_messages(messages: &[AiMessage]) -> Vec<serde_json::Value> {
    messages.iter().map(|m| {
        let role = match m.role {
            AiRole::System    => "system",
            AiRole::User      => "user",
            AiRole::Assistant => "assistant",
            AiRole::Tool      => "tool",
        };
        if let Some(tr) = &m.tool_result {
            serde_json::json!({
                "role": "tool",
                "tool_call_id": tr.call_id,
                "content": tr.content,
            })
        } else if !m.tool_calls.is_empty() {
            let calls: Vec<_> = m.tool_calls.iter().map(|tc| serde_json::json!({
                "id": tc.id,
                "type": "function",
                "function": { "name": tc.name, "arguments": tc.arguments.to_string() }
            })).collect();
            serde_json::json!({
                "role": role,
                "content": serde_json::Value::Null,
                "tool_calls": calls,
            })
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
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str { "openai" }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/v1/models", self.base_url);
        let resp: ModelsResponse = self.client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send().await?.json().await?;
        let mut models: Vec<_> = resp.data.into_iter().map(|m| m.id).collect();
        models.sort();
        Ok(models)
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
        let url = format!("{}/v1/chat/completions", self.base_url);

        let mut body = serde_json::json!({
            "model": self.default_model,
            "messages": build_messages(messages),
            "stream": true,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "top_p": top_p,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(build_tools(tools));
            body["tool_choice"] = serde_json::json!("auto");
        }

        debug!("openai chat_stream → {url}");

        let resp = self.client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let _ = tx.send(AiEvent::Error(format!("OpenAI error {status}: {text}")));
            return Ok(());
        }

        let mut stream = resp.bytes_stream();

        // Buffers para reconstruir tool calls (pueden llegar fragmentados)
        let mut tc_ids: Vec<Option<String>> = vec![];
        let mut tc_names: Vec<String> = vec![];
        let mut tc_args: Vec<String> = vec![];

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
                if line.is_empty() || line == "data: [DONE]" {
                    // Flush tool calls pendientes
                    for (i, name) in tc_names.iter().enumerate() {
                        if name.is_empty() { continue; }
                        let id = tc_ids.get(i).and_then(|x| x.clone())
                            .unwrap_or_else(|| format!("call_{i}"));
                        let args: serde_json::Value = serde_json::from_str(
                            tc_args.get(i).map(|s| s.as_str()).unwrap_or("{}"),
                        ).unwrap_or(serde_json::json!({}));
                        let _ = tx.send(AiEvent::ToolCallRequest(ToolCall {
                            id, name: name.clone(), arguments: args,
                        }));
                    }
                    if line == "data: [DONE]" {
                        let _ = tx.send(AiEvent::Done);
                        return Ok(());
                    }
                    continue;
                }

                let data = line.strip_prefix("data: ").unwrap_or(line);
                match serde_json::from_str::<StreamChunk>(data) {
                    Ok(sc) => {
                        for choice in &sc.choices {
                            if let Some(text) = &choice.delta.content {
                                if !text.is_empty() {
                                    let _ = tx.send(AiEvent::Chunk(text.clone()));
                                }
                            }
                            if let Some(tcs) = &choice.delta.tool_calls {
                                for dtc in tcs {
                                    let idx = dtc.index;
                                    while tc_names.len() <= idx {
                                        tc_names.push(String::new());
                                        tc_args.push(String::new());
                                        tc_ids.push(None);
                                    }
                                    if let Some(id) = &dtc.id {
                                        tc_ids[idx] = Some(id.clone());
                                    }
                                    if let Some(f) = &dtc.function {
                                        if let Some(n) = &f.name {
                                            tc_names[idx].push_str(n);
                                        }
                                        if let Some(a) = &f.arguments {
                                            tc_args[idx].push_str(a);
                                        }
                                    }
                                }
                            }
                            if choice.finish_reason.as_deref() == Some("stop") ||
                               choice.finish_reason.as_deref() == Some("tool_calls") {
                                // done handled by [DONE] line
                            }
                        }
                    }
                    Err(e) => {
                        debug!("openai parse error: {e} | data: {data}");
                    }
                }
            }
        }

        let _ = tx.send(AiEvent::Done);
        Ok(())
    }
}
