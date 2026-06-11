use async_trait::async_trait;
use color_eyre::Result;
use dca_ai::agent::ApprovalDecision;
use dca_ai::tools::all_tools;
use dca_ai::{AiAgent, AiEvent, AiMessage, AiProvider, AiRole, ChatSession, ToolCall, ToolDef};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
struct EvalCase {
    name: &'static str,
    user_prompt: &'static str,
    expected_tool: Option<&'static str>,
    expected_args_validator: Option<fn(&serde_json::Value) -> bool>,
}

struct RuleBasedEvalProvider;

#[async_trait]
impl AiProvider for RuleBasedEvalProvider {
    fn name(&self) -> &str {
        "eval-provider"
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        Ok(vec!["eval-provider".to_string()])
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        _tools: &[ToolDef],
        _max_tokens: u32,
        _temperature: f32,
        _top_p: f32,
        tx: UnboundedSender<AiEvent>,
    ) -> Result<()> {
        if messages.iter().any(|message| message.role == AiRole::Tool) {
            let _ = tx.send(AiEvent::Chunk("Resultado procesado".to_string()));
            let _ = tx.send(AiEvent::Done);
            return Ok(());
        }

        let prompt = messages
            .iter()
            .rev()
            .find(|message| message.role == AiRole::User)
            .map(|message| message.content.as_str())
            .unwrap_or_default();

        if let Some(tool_call) = select_tool_for_prompt(prompt) {
            let _ = tx.send(AiEvent::ToolCallRequest(tool_call));
        } else {
            let _ = tx.send(AiEvent::Chunk("Respuesta directa".to_string()));
        }

        let _ = tx.send(AiEvent::Done);
        Ok(())
    }
}

fn select_tool_for_prompt(prompt: &str) -> Option<ToolCall> {
    let lowered = prompt.to_lowercase();

    if lowered.contains("src/main.rs") && lowered.contains("contiene") {
        return Some(tool_call(
            "read_file",
            serde_json::json!({ "path": "src/main.rs" }),
        ));
    }

    if lowered.contains("archivos rust") {
        return Some(tool_call(
            "glob",
            serde_json::json!({ "pattern": "**/*.rs" }),
        ));
    }

    if lowered.contains("aiagent") {
        return Some(tool_call(
            "grep",
            serde_json::json!({ "pattern": "AiAgent", "path": "crates" }),
        ));
    }

    if lowered.contains("crates/ai/src/tools") && lowered.contains("lista") {
        return Some(tool_call(
            "list_dir",
            serde_json::json!({ "path": "crates/ai/src/tools" }),
        ));
    }

    None
}

fn tool_call(name: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: format!("eval-{name}"),
        name: name.to_string(),
        arguments,
        buffer_version: None,
        target_buffer_id: None,
    }
}

// DCA-IA-IMPROVEMENT: Casos base para ampliar la evaluacion de seleccion de herramientas.
#[tokio::test]
async fn tool_eval_selection_accuracy() {
    let cases = vec![
        EvalCase {
            name: "read_file cuando se pide contenido",
            user_prompt: "¿Qué contiene src/main.rs?",
            expected_tool: Some("read_file"),
            expected_args_validator: Some(|args| args["path"] == "src/main.rs"),
        },
        EvalCase {
            name: "glob cuando se buscan archivos rust",
            user_prompt: "Busca todos los archivos Rust del proyecto",
            expected_tool: Some("glob"),
            expected_args_validator: Some(|args| args["pattern"] == "**/*.rs"),
        },
        EvalCase {
            name: "grep cuando se busca un simbolo",
            user_prompt: "¿Dónde aparece AiAgent en el proyecto?",
            expected_tool: Some("grep"),
            expected_args_validator: Some(|args| args["pattern"] == "AiAgent"),
        },
        EvalCase {
            name: "list_dir cuando se pide listar un directorio",
            user_prompt: "Lista crates/ai/src/tools",
            expected_tool: Some("list_dir"),
            expected_args_validator: Some(|args| args["path"] == "crates/ai/src/tools"),
        },
        EvalCase {
            name: "sin herramientas para preguntas teoricas",
            user_prompt: "Explícame el patrón Observer en Rust",
            expected_tool: None,
            expected_args_validator: None,
        },
        EvalCase {
            name: "sin herramientas si el usuario pide solo responder",
            user_prompt: "Solo responde: ¿qué es ownership en Rust?",
            expected_tool: None,
            expected_args_validator: None,
        },
    ];

    let mut passed = 0usize;

    for case in &cases {
        let (event_tx, mut event_rx) = unbounded_channel();
        let (pending_tx, _pending_rx) = unbounded_channel();
        let (_approval_tx, mut approval_rx) = unbounded_channel::<ApprovalDecision>();
        let token = CancellationToken::new();

        let mut session = ChatSession::new();
        session.push_user(case.user_prompt);

        let agent = AiAgent::new(
            Box::new(RuleBasedEvalProvider),
            all_tools(false),
            "eval system prompt",
            512,
            0.0,
            1.0,
        );

        agent
            .chat_stream(&mut session, event_tx, pending_tx, &mut approval_rx, token)
            .await
            .expect("agent chat_stream should succeed");

        let mut tool_calls = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            if let AiEvent::ToolCallRequest(tool_call) = event {
                tool_calls.push(tool_call);
            }
        }

        let passed_case = match (case.expected_tool, tool_calls.first()) {
            (None, None) => true,
            (Some(expected_tool), Some(actual_tool)) if actual_tool.name == expected_tool => case
                .expected_args_validator
                .map(|validator| validator(&actual_tool.arguments))
                .unwrap_or(true),
            _ => false,
        };

        if passed_case {
            passed += 1;
        } else {
            panic!(
                "Caso '{}' fallo. expected_tool={:?}, actual={:?}",
                case.name,
                case.expected_tool,
                tool_calls
                    .first()
                    .map(|tool_call| (&tool_call.name, &tool_call.arguments))
            );
        }
    }

    let accuracy = (passed as f64 / cases.len() as f64) * 100.0;
    assert!(
        accuracy >= 90.0,
        "Precision minima esperada: 90%, actual: {:.1}%",
        accuracy
    );
}
