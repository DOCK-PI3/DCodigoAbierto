use color_eyre::Result;
use std::sync::Arc;
use tokio::sync::{mpsc::unbounded_channel, Mutex};
use tokio::time::{interval, Duration};
use tokio_util::sync::CancellationToken;
use tracing::debug;
use walkdir::WalkDir;

use dca_ai::{
    agent::ApprovalDecision,
    providers::build_provider,
    session::ChatSession,
    tools::{all_tools, plan_tools},
    AiAgent, AiEvent,
};
use dca_config::{spawn_config_watcher, AppConfig};
use dca_events::{spawn_crossterm_task, EventBus};
use dca_lsp::LspClient;
use dca_types::view_state::{ChatMessageView, FileEntry, ToolCallView, ViewState};

use crate::command::Command;
use crate::message::AppMessage;
use crate::state::{AppState, ChatMode, LspStatus};
use crate::update::update;

/// Tipo del sender de aprobación del agente activo.
type ApprovalSender = Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<ApprovalDecision>>>>;

/// Punto de entrada de la aplicación.
pub struct App {
    config: AppConfig,
    initial_file: Option<std::path::PathBuf>,
    /// Sender al agente IA activo (para aprobar/denegar tools)
    agent_approval_tx: ApprovalSender,
    /// CancellationToken del stream activo
    ai_cancel: Option<CancellationToken>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            initial_file: None,
            agent_approval_tx: Arc::new(Mutex::new(None)),
            ai_cancel: None,
        }
    }

    pub fn set_initial_file(&mut self, path: std::path::PathBuf) {
        self.initial_file = Some(path);
    }

    fn load_file_tree() -> Vec<FileEntry> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        WalkDir::new(&cwd)
            .max_depth(3)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy();
                name != ".git" && name != "target"
            })
            .map(|e| FileEntry {
                depth: e.depth(),
                is_dir: e.file_type().is_dir(),
                name: e.file_name().to_string_lossy().to_string(),
                path: e.path().to_string_lossy().to_string(),
            })
            .collect()
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut terminal = ratatui::init();
        let mut state = AppState::default();
        state.chat.selected_model = self.config.ai.model.clone();

        let bus = EventBus::new();
        let tx = bus.sender.clone();
        let mut rx = bus.receiver;

        let mut chat_session = ChatSession::new();

        // Árbol de archivos (lazy)
        {
            let tx_tree = tx.clone();
            tokio::spawn(async move {
                let tree = tokio::task::spawn_blocking(Self::load_file_tree)
                    .await
                    .unwrap_or_default();
                let _ = tx_tree.send(AppMessage::FileTreeLoaded(tree));
            });
        }

        spawn_crossterm_task(tx.clone());

        // Config hot-reload
        {
            let cfg_path = AppConfig::config_path();
            let (cfg_tx, mut cfg_rx) = unbounded_channel();
            spawn_config_watcher(cfg_path, cfg_tx);
            let tx_cfg = tx.clone();
            tokio::spawn(async move {
                while let Some(new_cfg) = cfg_rx.recv().await {
                    if tx_cfg.send(AppMessage::ConfigReload(new_cfg)).is_err() { break; }
                }
            });
        }

        // Tick
        let tick_rate = Duration::from_millis(self.config.tick_rate_ms);
        let tx_tick = tx.clone();
        tokio::spawn(async move {
            let mut ticker = interval(tick_rate);
            loop {
                ticker.tick().await;
                if tx_tick.send(AppMessage::Tick).is_err() { break; }
            }
        });

        // LSP
        let lsp_server = self.config.lsp_server.clone();
        let lsp_client: Option<LspClient> = if !lsp_server.is_empty() {
            state.lsp_status = LspStatus::Connecting;
            Some(LspClient::spawn(&lsp_server, tx.clone()))
        } else {
            None
        };

        if let Some(path) = self.initial_file.take() {
            let path_str = path.to_string_lossy().to_string();
            if let Ok(content) = std::fs::read_to_string(&path) {
                state.open_buffer(&path_str, &content);
                if let Some(client) = &lsp_client { client.open(&path_str, &content); }
            }
        }

        let mut tab_buf: Vec<dca_types::view_state::BufferTab> = Vec::with_capacity(8);
        let mut chat_msg_view_buf: Vec<ChatMessageView> = Vec::with_capacity(64);
        let mut theme_names: Vec<String> = state.available_themes.iter().map(|t| t.name.clone()).collect();
        let mut theme_accents: Vec<(String, String)> = state.available_themes.iter()
            .map(|t| (t.accent.clone(), t.bg.clone()))
            .collect();

        loop {
            let term_height = terminal.size().map(|s| s.height as usize).unwrap_or(24);
            state.buffer_mut().update_scroll(term_height.saturating_sub(3));

            tab_buf.clear();
            tab_buf.extend(state.buffers.iter().map(|b| dca_types::view_state::BufferTab {
                name: b.file_name.as_deref().unwrap_or("*nuevo*").to_string(),
                dirty: b.dirty,
            }));

            theme_names.clear();
            theme_names.extend(state.available_themes.iter().map(|t| t.name.clone()));
            theme_accents.clear();
            theme_accents.extend(state.available_themes.iter().map(|t| (t.accent.clone(), t.bg.clone())));

            chat_msg_view_buf.clear();
            chat_msg_view_buf.extend(state.chat.messages.iter().map(|m| ChatMessageView {
                role: m.role.clone(),
                content: m.content.clone(),
                is_streaming: m.is_streaming,
            }));

            let tool_pending_view = state.chat.tool_pending.as_ref().map(|p| ToolCallView {
                name: p.name.clone(),
                args_display: serde_json::to_string_pretty(&p.args).unwrap_or_default(),
            });

            terminal.draw(|frame| {
                let current_diags = state.buffer().file_name.as_deref()
                    .and_then(|p| state.diagnostics.get(p))
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);

                let view = ViewState {
                    buffer: state.buffer(),
                    focus: state.focus,
                    status_message: &state.status_message,
                    file_tree: &state.file_tree,
                    sidebar_selected: state.sidebar_selected,
                    sidebar_visible: state.sidebar_visible,
                    diagnostics: current_diags,
                    completions: &state.completions,
                    show_completions: state.show_completions,
                    completion_selected: state.completion_selected,
                    references: &state.references,
                    references_selected: state.references_selected,
                    show_references: state.show_references,
                    buffers: tab_buf.clone(),
                    active_buffer: state.active_buffer,
                    fuzzy_active: state.fuzzy_active,
                    fuzzy_query: &state.fuzzy_query,
                    fuzzy_results: &state.fuzzy_results,
                    fuzzy_selected: state.fuzzy_selected,
                    chat_visible: state.chat_visible,
                    chat_messages: &chat_msg_view_buf,
                    chat_input: &state.chat.input,
                    chat_input_cursor: state.chat.input_cursor,
                    chat_scroll: state.chat.scroll,
                    chat_streaming: state.chat.streaming,
                    chat_tool_pending: tool_pending_view,
                    model_selector_active: state.model_selector_active,
                    model_selector_models: &state.model_selector_models,
                    model_selector_selected: state.model_selector_selected,
                    active_model: &state.chat.selected_model,
                    provider_name: &self.config.ai.provider,
                    theme_selector_active: state.theme_selector_active,
                    theme_selector_selected: state.theme_selector_selected,
                    available_theme_names: &theme_names,
                    available_theme_accents: &theme_accents,
                    chat_mode_label: state.chat.mode.label(),
                    chat_mode_is_build: state.chat.mode == ChatMode::Build,
                    session_name: &state.chat.session_name,
                    tokens_generated: state.chat.tokens_generated,
                    streaming_elapsed_secs: state.chat.streaming_elapsed_secs,
                    palette_active: state.palette_active,
                    palette_query: &state.palette_query,
                    palette_query_cursor: state.palette_query_cursor,
                    palette_selected: state.palette_selected,
                };
                dca_ui::render::render(frame, &view, &self.config.theme);
            })?;

            // Drenar mensajes sin bloquear
            while let Ok(msg) = rx.try_recv() {
                debug!("msg (drain): {:?}", msg);
                if let AppMessage::ConfigReload(new_cfg) = msg {
                    state.chat.selected_model = new_cfg.ai.model.clone();
                    self.config = new_cfg;
                } else if let AppMessage::AiSessionUpdate(msgs) = msg {
                    chat_session.messages = msgs;
                } else if let Some(cmd) = update(&mut state, msg) {
                    self.execute_command(cmd, &lsp_client, &state, &tx, &mut chat_session).await;
                }
                // ChangeTheme es ejecutado directamente en execute_command arriba
            }

            // Esperar siguiente mensaje
            match rx.recv().await {
                Some(msg) => {
                    debug!("msg (await): {:?}", msg);
                    if let AppMessage::ConfigReload(new_cfg) = msg {
                        state.chat.selected_model = new_cfg.ai.model.clone();
                        self.config = new_cfg;
                    } else if let AppMessage::AiSessionUpdate(msgs) = msg {
                        chat_session.messages = msgs;
                    } else if let Some(cmd) = update(&mut state, msg) {
                        self.execute_command(cmd, &lsp_client, &state, &tx, &mut chat_session).await;
                    }
                }
                None => break,
            }

            if state.quit { break; }
        }

        ratatui::restore();
        Ok(())
    }

    async fn execute_command(
        &mut self,
        cmd: Command,
        lsp: &Option<LspClient>,
        state: &AppState,
        tx: &dca_events::EventSender,
        session: &mut ChatSession,
    ) {
        match cmd {
            // ── LSP ──────────────────────────────────────────────────────────
            Command::LspOpen { path, text } => {
                if let Some(c) = lsp { c.open(&path, &text); }
            }
            Command::LspChange { path, text } => {
                if let Some(c) = lsp { c.change(&path, &text); }
            }
            Command::LspRequestCompletion => {
                if let Some(c) = lsp {
                    if let Some(p) = &state.buffer().file_name {
                        c.request_completion(p, state.buffer().cursor.row as u32, state.buffer().cursor.col as u32);
                    }
                }
            }
            Command::LspGotoDefinition => {
                if let Some(c) = lsp {
                    if let Some(p) = &state.buffer().file_name {
                        c.goto_definition(p, state.buffer().cursor.row as u32, state.buffer().cursor.col as u32);
                    }
                }
            }
            Command::LspFindReferences => {
                if let Some(c) = lsp {
                    if let Some(p) = &state.buffer().file_name {
                        c.find_references(p, state.buffer().cursor.row as u32, state.buffer().cursor.col as u32);
                    }
                }
            }
            Command::OpenFile(path) => {
                if let Some(c) = lsp {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        c.open(&path, &text);
                    }
                }
            }

            // ── IA ────────────────────────────────────────────────────────────
            Command::AiSendMessage => {
                // Añadir el último mensaje del usuario a la sesión del agente
                if let Some(user_msg) = state.chat.messages.iter().rev().find(|m| m.role == "user") {
                    session.push_user(&user_msg.content);
                }

                let provider = build_provider(&self.config.ai);
                let tools = match state.chat.mode {
                    ChatMode::Plan => plan_tools(),
                    ChatMode::Build => {
                        if self.config.ai.tools_enabled {
                            all_tools(self.config.ai.web_enabled)
                        } else {
                            vec![]
                        }
                    }
                };
                let agent = AiAgent::new(
                    provider,
                    tools,
                    &self.config.ai.system_prompt,
                    self.config.ai.max_tokens,
                    self.config.ai.temperature,
                    self.config.ai.top_p,
                );

                let token = CancellationToken::new();
                self.ai_cancel = Some(token.clone());

                let (event_tx, mut event_rx) = unbounded_channel::<AiEvent>();
                let (pending_tx, mut pending_rx) = unbounded_channel::<dca_ai::provider::ToolCall>();
                // Canal de aprobación exclusivo para este agente
                let (agent_appr_tx, mut agent_appr_rx) = unbounded_channel::<ApprovalDecision>();

                // Registrar el sender para que AiExecuteTool pueda alcanzarlo
                {
                    let mut guard = self.agent_approval_tx.lock().await;
                    *guard = Some(agent_appr_tx);
                }

                // Clonar sesión para el spawn
                let mut session_clone = ChatSession::new();
                session_clone.messages = session.messages.clone();

                // Forwarder: tool requests → AppMessage
                let tx_fwd = tx.clone();
                tokio::spawn(async move {
                    while let Some(tc) = pending_rx.recv().await {
                        let _ = tx_fwd.send(AppMessage::AiToolRequest {
                            id: tc.id,
                            name: tc.name,
                            args: tc.arguments,
                        });
                    }
                });

                // Forwarder: AI events → AppMessage
                let tx_events = tx.clone();
                tokio::spawn(async move {
                    while let Some(ev) = event_rx.recv().await {
                        match ev {
                            AiEvent::Chunk(c) => { let _ = tx_events.send(AppMessage::AiStreamChunk(c)); }
                            AiEvent::Done => { let _ = tx_events.send(AppMessage::AiStreamDone); break; }
                            AiEvent::Error(e) => { let _ = tx_events.send(AppMessage::AiStreamError(e)); break; }
                            AiEvent::ToolResult { name, result } => {
                                let _ = tx_events.send(AppMessage::AiToolResult { name, result });
                            }
                            AiEvent::ToolCallRequest(_) => {}
                        }
                    }
                });

                // Agente IA — al terminar, envía la sesión de vuelta por canal
                let (sess_tx, sess_rx) = tokio::sync::oneshot::channel::<Vec<dca_ai::provider::AiMessage>>();
                tokio::spawn(async move {
                    let _ = agent.chat_stream(
                        &mut session_clone,
                        event_tx,
                        pending_tx,
                        &mut agent_appr_rx,
                        token,
                    ).await;
                    let _ = sess_tx.send(session_clone.messages);
                });

                // Tarea que espera la sesión y la reinyecta como mensaje al loop
                let tx_sess = tx.clone();
                tokio::spawn(async move {
                    if let Ok(msgs) = sess_rx.await {
                        let _ = tx_sess.send(AppMessage::AiSessionUpdate(msgs));
                    }
                });
            }

            Command::AiExecuteTool { id } => {
                let guard = self.agent_approval_tx.lock().await;
                if let Some(tx_a) = guard.as_ref() {
                    let _ = tx_a.send(ApprovalDecision::Approved(id));
                }
            }

            Command::AiDenyTool { id } => {
                let guard = self.agent_approval_tx.lock().await;
                if let Some(tx_a) = guard.as_ref() {
                    let _ = tx_a.send(ApprovalDecision::Denied(id));
                }
            }

            Command::AiAbortStream => {
                if let Some(token) = self.ai_cancel.take() {
                    token.cancel();
                }
                let mut guard = self.agent_approval_tx.lock().await;
                // Cerrar el canal de aprobación para que el agente salga
                *guard = None;
            }

            Command::AiLoadModels => {
                let provider = build_provider(&self.config.ai);
                let tx_models = tx.clone();
                tokio::spawn(async move {
                    let models = provider.list_models().await.unwrap_or_default();
                    let _ = tx_models.send(AppMessage::AiModelsLoaded(models));
                });
            }

            Command::AiInjectBuffer => {
                let buf_content = state.buffer().lines.join("\n");
                let fname = state.buffer().file_name.as_deref().unwrap_or("buffer").to_string();
                session.push_user(format!("[Contexto: {fname}]\n```\n{buf_content}\n```"));
            }

            Command::ChangeTheme(name) => {
                if let Some(t) = state.available_themes.iter().find(|t| t.name == name).cloned() {
                    self.config.theme = t;
                    let _ = self.config.save_theme();
                }
            }
        }
    }
}
