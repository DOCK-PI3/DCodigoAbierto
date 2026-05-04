use color_eyre::Result;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

const MAX_TOOL_ITERATIONS: u32 = 20;
const STREAM_TIMEOUT_SECS: u64 = 180;
const APPROVAL_TIMEOUT_SECS: u64 = 300;

use crate::provider::{AiEvent, AiMessage, AiProvider, ToolCall, ToolDef};
use crate::session::ChatSession;
use crate::tools::Tool;

/// Orquesta una sesión de chat con un proveedor de IA y un conjunto de herramientas.
pub struct AiAgent {
    provider: Box<dyn AiProvider>,
    tools: Vec<Box<dyn Tool>>,
    system_prompt: String,
    max_tokens: u32,
    temperature: f32,
    top_p: f32,
}

impl AiAgent {
    pub fn new(
        provider: Box<dyn AiProvider>,
        tools: Vec<Box<dyn Tool>>,
        system_prompt: impl Into<String>,
        max_tokens: u32,
        temperature: f32,
        top_p: f32,
    ) -> Self {
        Self {
            provider,
            tools,
            system_prompt: system_prompt.into(),
            max_tokens,
            temperature,
            top_p,
        }
    }

    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        self.provider.list_models().await
    }

    /// Herramientas que requieren aprobación del usuario (write_file, shell).
    pub fn tools_requiring_approval(&self) -> Vec<String> {
        self.tools.iter()
            .filter(|t| t.requires_approval())
            .map(|t| t.definition().name)
            .collect()
    }

    /// Definiciones de herramientas para el modelo.
    fn tool_defs(&self) -> Vec<ToolDef> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    /// Lanza un stream de chat. Los eventos se envían al canal `event_tx`.
    ///
    /// - `session`: historial previo (se muta añadiendo los nuevos mensajes)
    /// - `pending_approvals_tx`: cuando hay una herramienta peligrosa, el agente
    ///   envía el `ToolCall` por aquí y espera que el caller llame a
    ///   `approve_tool()` / `deny_tool()` para continuar.
    /// - `token`: CancellationToken para abortar mid-stream.
    pub async fn chat_stream(
        &self,
        session: &mut ChatSession,
        event_tx: UnboundedSender<AiEvent>,
        pending_approval_tx: UnboundedSender<ToolCall>,
        approval_rx: &mut tokio::sync::mpsc::UnboundedReceiver<ApprovalDecision>,
        token: CancellationToken,
    ) -> Result<()> {
        // Construimos el listado de mensajes con el system prompt
        let mut context = build_context(&self.system_prompt, &session.messages);
        let defs = self.tool_defs();
        let mut tool_iteration: u32 = 0;

        loop {
            if token.is_cancelled() {
                let _ = event_tx.send(AiEvent::Done);
                return Ok(());
            }

            // Guardia: evitar bucles infinitos de herramientas
            if tool_iteration >= MAX_TOOL_ITERATIONS {
                let _ = event_tx.send(AiEvent::Error(format!(
                    "Límite de iteraciones de herramientas alcanzado ({MAX_TOOL_ITERATIONS}). Abortando."
                )));
                return Ok(());
            }

            // Lanzar stream con timeout
            let (inner_tx, mut inner_rx) = unbounded_channel::<AiEvent>();
            let stream_fut = self.provider.chat_stream(&context, &defs, self.max_tokens, self.temperature, self.top_p, inner_tx);

            let stream_result = tokio::select! {
                _ = token.cancelled() => {
                    let _ = event_tx.send(AiEvent::Done);
                    return Ok(());
                }
                res = timeout(std::time::Duration::from_secs(STREAM_TIMEOUT_SECS), stream_fut) => res,
            };

            match stream_result {
                Err(_elapsed) => {
                    let _ = event_tx.send(AiEvent::Error(format!(
                        "Timeout: el modelo no respondió en {STREAM_TIMEOUT_SECS}s."
                    )));
                    return Ok(());
                }
                Ok(Err(e)) => {
                    let _ = event_tx.send(AiEvent::Error(e.to_string()));
                    return Ok(());
                }
                Ok(Ok(())) => {}
            }

            // Recopilar respuesta completa
            let mut assistant_text = String::new();
            let mut tool_calls_this_turn: Vec<ToolCall> = vec![];

            while let Ok(event) = inner_rx.try_recv() {
                match event {
                    AiEvent::Chunk(t) => {
                        let _ = event_tx.send(AiEvent::Chunk(t.clone()));
                        assistant_text.push_str(&t);
                    }
                    AiEvent::ToolCallRequest(tc) => {
                        let _ = event_tx.send(AiEvent::ToolCallRequest(tc.clone()));
                        tool_calls_this_turn.push(tc);
                    }
                    AiEvent::Done => {}
                    AiEvent::Error(e) => {
                        let _ = event_tx.send(AiEvent::Error(e));
                        return Ok(());
                    }
                    AiEvent::ToolResult { .. } => {} // emitido por el agente, no por los providers
                }
            }

            // Guardar mensaje del asistente en sesión
            if tool_calls_this_turn.is_empty() {
                session.push_assistant(&assistant_text);
                let _ = event_tx.send(AiEvent::Done);
                return Ok(());
            } else {
                session.push_assistant_with_tools(&assistant_text, tool_calls_this_turn.clone());
                context.push(AiMessage {
                    role: crate::provider::AiRole::Assistant,
                    content: assistant_text,
                    tool_calls: tool_calls_this_turn.clone(),
                    tool_result: None,
                });
            }

            // Ejecutar herramientas
            tool_iteration += 1;
            for tc in &tool_calls_this_turn {
                if token.is_cancelled() { break; }

                // Buscar la herramienta
                let tool = self.tools.iter().find(|t| t.definition().name == tc.name);
                let Some(tool) = tool else {
                    let result = format!("Herramienta '{}' no encontrada.", tc.name);
                    push_tool_result_to_context(&mut context, session, &tc.id, &result);
                    continue;
                };

                // Pedir aprobación si es necesario
                if tool.requires_approval() {
                    let _ = pending_approval_tx.send(tc.clone());
                    // Esperar decisión con timeout para evitar deadlock
                    let decision = timeout(
                        std::time::Duration::from_secs(APPROVAL_TIMEOUT_SECS),
                        approval_rx.recv(),
                    ).await;
                    match decision {
                        Err(_) => {
                            let result = "Timeout esperando aprobación del usuario.";
                            push_tool_result_to_context(&mut context, session, &tc.id, result);
                            continue;
                        }
                        Ok(Some(ApprovalDecision::Approved(id))) if id == tc.id => {}
                        Ok(Some(ApprovalDecision::Denied(id))) if id == tc.id => {
                            let result = "El usuario denegó la ejecución de esta herramienta.";
                            push_tool_result_to_context(&mut context, session, &tc.id, result);
                            continue;
                        }
                        _ => {
                            let result = "Herramienta cancelada.";
                            push_tool_result_to_context(&mut context, session, &tc.id, result);
                            continue;
                        }
                    }
                }

                // Ejecutar
                debug!("Ejecutando herramienta: {} args={}", tc.name, tc.arguments);
                let result = match tool.execute(&tc.arguments).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("Tool {} error: {e}", tc.name);
                        format!("Error: {e}")
                    }
                };

                // Notificar al UI el resultado de la herramienta
                let preview = if result.len() > 300 {
                    format!("{}…", &result[..300])
                } else {
                    result.clone()
                };
                let _ = event_tx.send(crate::provider::AiEvent::ToolResult {
                    name: tc.name.clone(),
                    result: preview,
                });

                push_tool_result_to_context(&mut context, session, &tc.id, &result);
            }

            if token.is_cancelled() {
                let _ = event_tx.send(AiEvent::Done);
                return Ok(());
            }

            // Volver a llamar al modelo con los resultados de las herramientas
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn build_context(system_prompt: &str, messages: &[AiMessage]) -> Vec<AiMessage> {
    let mut ctx = vec![AiMessage::system(system_prompt)];
    ctx.extend_from_slice(messages);
    ctx
}

fn push_tool_result_to_context(
    context: &mut Vec<AiMessage>,
    session: &mut ChatSession,
    call_id: &str,
    result: &str,
) {
    let msg = AiMessage::tool_result(call_id, result);
    context.push(msg.clone());
    session.push_tool_result(call_id, result);
}

/// Decisión del usuario sobre una herramienta peligrosa.
#[derive(Debug)]
pub enum ApprovalDecision {
    Approved(String),
    Denied(String),
}
