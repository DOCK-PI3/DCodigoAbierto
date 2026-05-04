use crossterm::event::{KeyCode, KeyModifiers};
use dca_types::view_state::Focus;
use dca_types::LspEvent;

use crate::command::Command;
use crate::fuzzy::fuzzy_filter;
use crate::message::AppMessage;
use crate::state::{AppState, ChatMessage, ChatMode, LspStatus, PendingTool};

/// Función pura de actualización del estado.
/// Retorna un `Command` opcional para side-effects externos (LSP, I/O, IA).
pub fn update(state: &mut AppState, msg: AppMessage) -> Option<Command> {
    match msg {
        AppMessage::Key(key) => handle_key(state, key.code, key.modifiers),
        AppMessage::Lsp(event) => {
            handle_lsp(state, event);
            None
        }
        AppMessage::Resize(_, _) => None,
        AppMessage::Tick => {
            if state.chat.streaming {
                state.chat.streaming_elapsed_secs =
                    state.chat.streaming_elapsed_secs.saturating_add(1);
            }
            None
        }
        AppMessage::Quit => {
            state.quit = true;
            None
        }
        AppMessage::ConfigReload(_) => None, // manejado en app.rs
        AppMessage::FileTreeLoaded(tree) => {
            state.fuzzy_all_files = tree.iter()
                .filter(|e| !e.is_dir)
                .map(|e| e.path.clone())
                .collect();
            state.file_tree = tree;
            None
        }

        // ── IA / Chat ─────────────────────────────────────────────────────
        AppMessage::AiStreamChunk(chunk) => {
            state.chat.streaming = true;
            state.chat.tokens_generated += chunk.chars().count();
            state.chat.streaming_buffer.push_str(&chunk);
            // Actualizar o crear el último mensaje del asistente en streaming
            if let Some(last) = state.chat.messages.last_mut() {
                if last.role == "assistant" && last.is_streaming {
                    last.content.push_str(&chunk);
                    return None;
                }
            }
            state.chat.messages.push(ChatMessage {
                role: "assistant".into(),
                content: chunk,
                is_streaming: true,
            });
            // Auto-scroll al fondo (0 = mostrar fondo en la nueva semántica)
            state.chat.scroll = 0;
            None
        }
        AppMessage::AiStreamDone => {
            state.chat.streaming = false;
            state.chat.streaming_elapsed_secs = 0;
            state.chat.streaming_buffer.clear();
            if let Some(last) = state.chat.messages.last_mut() {
                last.is_streaming = false;
            }
            update_status(state);
            None
        }
        AppMessage::AiStreamError(err) => {
            state.chat.streaming = false;
            state.chat.streaming_buffer.clear();
            state.chat.messages.push(ChatMessage {
                role: "assistant".into(),
                content: format!("[Error: {err}]"),
                is_streaming: false,
            });
            update_status(state);
            None
        }
        AppMessage::AiToolRequest { id, name, args } => {
            let args_display = serde_json::to_string_pretty(&args).unwrap_or_default();
            state.chat.tool_pending = Some(PendingTool { id, name: name.clone(), args });
            state.chat.messages.push(ChatMessage {
                role: "tool".into(),
                content: format!("⚙ Solicitando: {name} — esperando aprobación…"),
                is_streaming: false,
            });
            None
        }
        AppMessage::AiToolApproved(id) => {
            if let Some(p) = &state.chat.tool_pending {
                if p.id == id {
                    let cmd_id = id.clone();
                    state.chat.tool_pending = None;
                    return Some(Command::AiExecuteTool { id: cmd_id });
                }
            }
            None
        }
        AppMessage::AiToolDenied(id) => {
            state.chat.tool_pending = None;
            state.chat.messages.push(ChatMessage {
                role: "tool".into(),
                content: format!("✗ Herramienta {id} denegada por el usuario."),
                is_streaming: false,
            });
            None
        }
        AppMessage::AiToolResult { name, result } => {
            state.chat.messages.push(ChatMessage {
                role: "tool".into(),
                content: format!("✓ {name}\n{result}"),
                is_streaming: false,
            });
            state.chat.scroll = 0; // auto-scroll al fondo
            None
        }
        AppMessage::AiModelsLoaded(models) => {
            state.model_selector_models = models;
            None
        }
        // Sesión sincronizada tras cada respuesta del agente (manejada en app.rs)
        AppMessage::AiSessionUpdate(_) => None,
    }
}

// ── Teclado ───────────────────────────────────────────────────────────────────

fn handle_key(state: &mut AppState, code: KeyCode, mods: KeyModifiers) -> Option<Command> {
    let ctrl  = mods.contains(KeyModifiers::CONTROL);
    let shift = mods.contains(KeyModifiers::SHIFT);

    // Diálogo de permiso de herramienta (prioridad máxima)
    if state.chat.tool_pending.is_some() {
        return handle_permission_key(state, code);
    }

    // Command Palette activa
    if state.palette_active {
        return handle_palette_key(state, code, mods);
    }

    // Selector de modelo activo
    if state.model_selector_active {
        return handle_model_selector_key(state, code);
    }

    // Fuzzy finder activo → prioridad absoluta
    if state.fuzzy_active {
        return handle_fuzzy_key(state, code, mods);
    }

    // Popup completions activo
    if state.show_completions {
        return handle_completion_key(state, code);
    }

    // Panel de referencias activo
    if state.show_references {
        return handle_references_key(state, code);
    }

    // Atajos globales ctrl
    if ctrl {
        match code {
            KeyCode::Char('q') | KeyCode::Char('c') => {
                state.quit = true;
                return None;
            }
            KeyCode::Char('b') => {
                state.sidebar_visible = !state.sidebar_visible;
                update_status(state);
                return None;
            }
            KeyCode::Char('p') => {
                state.palette_active = true;
                state.palette_query.clear();
                state.palette_query_cursor = 0;
                state.palette_selected = 0;
                return None;
            }
            KeyCode::Char('w') => {
                state.close_buffer();
                update_status(state);
                return None;
            }
            KeyCode::Char('g') => {
                if state.focus == Focus::Editor {
                    return Some(Command::LspGotoDefinition);
                }
                return None;
            }
            KeyCode::Char('r') => {
                if state.focus == Focus::Editor {
                    return Some(Command::LspFindReferences);
                }
                return None;
            }
            KeyCode::Char(' ') => {
                if state.focus == Focus::Editor {
                    return Some(Command::LspRequestCompletion);
                }
                return None;
            }
            KeyCode::Tab => {
                cycle_buffer(state, 1);
                return None;
            }
            // ── IA atajos ─────────────────────────────────────────────────
            KeyCode::Char('a') => {
                state.chat_visible = !state.chat_visible;
                if state.chat_visible {
                    state.focus = Focus::Chat;
                } else {
                    state.focus = Focus::Editor;
                }
                update_status(state);
                return None;
            }
            KeyCode::Char('o') => {
                state.model_selector_active = true;
                if state.model_selector_models.is_empty() {
                    return Some(Command::AiLoadModels);
                }
                return None;
            }
            KeyCode::Char('i') => {
                if state.chat_visible {
                    return Some(Command::AiInjectBuffer);
                }
                return None;
            }
            KeyCode::Char('x') => {
                if state.chat.streaming {
                    return Some(Command::AiAbortStream);
                }
                return None;
            }
            _ => {}
        }
    }

    // Ctrl+Shift+Tab → buffer anterior
    if shift && code == KeyCode::BackTab {
        cycle_buffer(state, -1);
        return None;
    }

    if code == KeyCode::Tab && !ctrl {
        // En home mode (sin archivo), Tab alterna Build ↔ Plan
        let home_mode = state.buffer().file_name.is_none();
        if home_mode {
            state.chat.mode = state.chat.mode.toggle();
            update_status(state);
            return None;
        }
        state.focus = match state.focus {
            Focus::Editor  => {
                if state.sidebar_visible { Focus::Sidebar } else if state.chat_visible { Focus::Chat } else { Focus::Editor }
            }
            Focus::Sidebar => {
                if state.chat_visible { Focus::Chat } else { Focus::Editor }
            }
            Focus::Chat    => Focus::Editor,
        };
        update_status(state);
        return None;
    }

    match state.focus {
        Focus::Editor  => handle_editor_key(state, code),
        Focus::Sidebar => handle_sidebar_key(state, code),
        Focus::Chat    => handle_chat_key(state, code),
    }
}

fn cycle_buffer(state: &mut AppState, dir: i32) {
    let n = state.buffers.len();
    if n <= 1 { return; }
    state.active_buffer = ((state.active_buffer as i32 + dir).rem_euclid(n as i32)) as usize;
    update_status(state);
}

// ── Editor ────────────────────────────────────────────────────────────────────

fn handle_editor_key(state: &mut AppState, code: KeyCode) -> Option<Command> {
    let buf = state.buffer_mut();
    let was_dirty = buf.dirty;

    match code {
        KeyCode::Up        => buf.move_up(),
        KeyCode::Down      => buf.move_down(),
        KeyCode::Left      => buf.move_left(),
        KeyCode::Right     => buf.move_right(),
        KeyCode::Home      => buf.move_line_start(),
        KeyCode::End       => buf.move_line_end(),
        KeyCode::Esc       => {}
        KeyCode::Enter     => buf.insert_newline(),
        KeyCode::Backspace => buf.delete_char_before(),
        KeyCode::Char(ch)  => buf.insert_char(ch),
        _ => {}
    }

    let row   = state.buffer().cursor.row + 1;
    let col   = state.buffer().cursor_visual_col() + 1;
    let dirty = if state.buffer().dirty { " [+]" } else { "" };
    let name  = state.buffer().file_name.as_deref().unwrap_or("*nuevo*");
    let lsp   = lsp_indicator(&state.lsp_status);
    state.status_message = format!(
        " {name}{dirty}  Ln {row}, Col {col}{lsp}  |  Ctrl+A: chat  |  Ctrl+P: fuzzy  |  Ctrl+Q: salir"
    );

    if state.buffer().dirty && (!was_dirty || matches!(code, KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace)) {
        if let Some(path) = state.buffer().file_name.clone() {
            let text = state.buffer().lines.join("\n");
            return Some(Command::LspChange { path, text });
        }
    }
    None
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

fn handle_sidebar_key(state: &mut AppState, code: KeyCode) -> Option<Command> {
    match code {
        KeyCode::Up => {
            if state.sidebar_selected > 0 {
                state.sidebar_selected -= 1;
            }
            None
        }
        KeyCode::Down => {
            if !state.file_tree.is_empty()
                && state.sidebar_selected + 1 < state.file_tree.len()
            {
                state.sidebar_selected += 1;
            }
            None
        }
        KeyCode::Enter => open_file_from_sidebar(state),
        _ => None,
    }
}

fn open_file_from_sidebar(state: &mut AppState) -> Option<Command> {
    let entry = state.file_tree.get(state.sidebar_selected)?;
    if entry.is_dir { return None; }
    let path = entry.path.clone();
    load_file_into_buffer(state, &path)
}

// ── Chat ──────────────────────────────────────────────────────────────────────

fn handle_chat_key(state: &mut AppState, code: KeyCode) -> Option<Command> {
    match code {
        KeyCode::Esc => {
            state.focus = Focus::Editor;
            update_status(state);
            None
        }
        KeyCode::Enter => {
            if state.chat.input.trim().is_empty() || state.chat.streaming {
                return None;
            }
            let text = state.chat.input.clone();
            state.chat.messages.push(ChatMessage {
                role: "user".into(),
                content: text,
                is_streaming: false,
            });
            // Placeholder del asistente visible inmediatamente (cursor parpadeante)
            state.chat.messages.push(ChatMessage {
                role: "assistant".into(),
                content: String::new(),
                is_streaming: true,
            });
            state.chat.streaming = true;
            state.chat.streaming_elapsed_secs = 0;
            state.chat.input.clear();
            state.chat.input_cursor = 0;
            state.chat.scroll = 0; // auto-scroll al fondo
            Some(Command::AiSendMessage)
        }
        KeyCode::Backspace => {
            if state.chat.input_cursor > 0 {
                let byte_idx = state.chat.input_cursor.min(state.chat.input.len());
                if byte_idx > 0 {
                    let char_start = state.chat.input[..byte_idx]
                        .char_indices().last().map(|(i, _)| i).unwrap_or(0);
                    state.chat.input.remove(char_start);
                    state.chat.input_cursor = char_start;
                }
            }
            None
        }
        KeyCode::Left => {
            if state.chat.input_cursor > 0 {
                state.chat.input_cursor -= 1;
            }
            None
        }
        KeyCode::Right => {
            if state.chat.input_cursor < state.chat.input.len() {
                state.chat.input_cursor += 1;
            }
            None
        }
        KeyCode::Up => {
            // Scroll hacia arriba: aumenta las líneas subidas desde el fondo
            state.chat.scroll = state.chat.scroll.saturating_add(3);
            None
        }
        KeyCode::Down => {
            // Scroll hacia abajo: reduce las líneas subidas desde el fondo
            state.chat.scroll = state.chat.scroll.saturating_sub(3);
            None
        }
        KeyCode::Char(ch) => {
            let idx = state.chat.input_cursor.min(state.chat.input.len());
            state.chat.input.insert(idx, ch);
            state.chat.input_cursor = idx + ch.len_utf8();
            None
        }
        _ => None,
    }
}

// ── Diálogo de permiso ────────────────────────────────────────────────────────

fn handle_permission_key(state: &mut AppState, code: KeyCode) -> Option<Command> {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(p) = state.chat.tool_pending.clone() {
                state.chat.tool_pending = None;
                return Some(Command::AiExecuteTool { id: p.id });
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            if let Some(p) = state.chat.tool_pending.take() {
                state.chat.messages.push(ChatMessage {
                    role: "tool".into(),
                    content: format!("✗ {} denegado.", p.name),
                    is_streaming: false,
                });
                return Some(Command::AiDenyTool { id: p.id });
            }
        }
        _ => {}
    }
    None
}

// ── Selector de modelo ────────────────────────────────────────────────────────

fn handle_model_selector_key(state: &mut AppState, code: KeyCode) -> Option<Command> {
    match code {
        KeyCode::Esc => {
            state.model_selector_active = false;
        }
        KeyCode::Up => {
            if state.model_selector_selected > 0 {
                state.model_selector_selected -= 1;
            }
        }
        KeyCode::Down => {
            let max = state.model_selector_models.len().saturating_sub(1);
            if state.model_selector_selected < max {
                state.model_selector_selected += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(model) = state.model_selector_models.get(state.model_selector_selected).cloned() {
                state.chat.selected_model = model;
                state.model_selector_active = false;
                update_status(state);
            }
        }
        _ => {}
    }
    None
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_file_into_buffer(state: &mut AppState, path: &str) -> Option<Command> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let text = content.clone();
            state.open_buffer(path, &content);
            state.focus = Focus::Editor;
            update_status(state);
            Some(Command::LspOpen { path: path.to_owned(), text })
        }
        Err(e) => {
            state.status_message = format!(" Error abriendo {path}: {e}");
            None
        }
    }
}

// ── Completion popup ──────────────────────────────────────────────────────────

fn handle_completion_key(state: &mut AppState, code: KeyCode) -> Option<Command> {
    match code {
        KeyCode::Esc => {
            state.show_completions = false;
            state.completions.clear();
        }
        KeyCode::Up => {
            if state.completion_selected > 0 {
                state.completion_selected -= 1;
            }
        }
        KeyCode::Down => {
            if state.completion_selected + 1 < state.completions.len() {
                state.completion_selected += 1;
            }
        }
        KeyCode::Enter | KeyCode::Tab => {
            accept_completion(state);
        }
        other => {
            state.show_completions = false;
            state.completions.clear();
            return handle_editor_key(state, other);
        }
    }
    None
}

fn accept_completion(state: &mut AppState) {
    if let Some(entry) = state.completions.get(state.completion_selected) {
        let text = entry.insert_text.clone().unwrap_or_else(|| entry.label.clone());
        for ch in text.chars() {
            state.buffer_mut().insert_char(ch);
        }
    }
    state.show_completions = false;
    state.completions.clear();
    state.completion_selected = 0;
}

// ── Referencias panel ─────────────────────────────────────────────────────────

fn handle_references_key(state: &mut AppState, code: KeyCode) -> Option<Command> {
    match code {
        KeyCode::Esc => {
            state.show_references = false;
        }
        KeyCode::Up => {
            if state.references_selected > 0 {
                state.references_selected -= 1;
            }
        }
        KeyCode::Down => {
            if state.references_selected + 1 < state.references.len() {
                state.references_selected += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(loc) = state.references.get(state.references_selected).cloned() {
                let path = loc.path.clone();
                let line = loc.line as usize;
                let col  = loc.col  as usize;
                let cmd  = load_file_into_buffer(state, &path);
                state.buffer_mut().cursor.row = line;
                state.buffer_mut().cursor.col = col;
                state.show_references = false;
                return cmd;
            }
        }
        _ => {}
    }
    None
}

// ── Fuzzy Finder ──────────────────────────────────────────────────────────────

fn open_fuzzy(state: &mut AppState) {
    state.fuzzy_active = true;
    state.fuzzy_query.clear();
    state.fuzzy_selected = 0;
    state.fuzzy_results = state.fuzzy_all_files.clone();
}

fn handle_fuzzy_key(state: &mut AppState, code: KeyCode, _mods: KeyModifiers) -> Option<Command> {
    match code {
        KeyCode::Esc => {
            state.fuzzy_active = false;
            state.fuzzy_query.clear();
        }
        KeyCode::Up => {
            if state.fuzzy_selected > 0 {
                state.fuzzy_selected -= 1;
            }
        }
        KeyCode::Down => {
            if state.fuzzy_selected + 1 < state.fuzzy_results.len() {
                state.fuzzy_selected += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(path) = state.fuzzy_results.get(state.fuzzy_selected).cloned() {
                state.fuzzy_active = false;
                state.fuzzy_query.clear();
                return load_file_into_buffer(state, &path);
            }
        }
        KeyCode::Backspace => {
            state.fuzzy_query.pop();
            refresh_fuzzy(state);
        }
        KeyCode::Char(ch) => {
            state.fuzzy_query.push(ch);
            refresh_fuzzy(state);
        }
        _ => {}
    }
    None
}

fn refresh_fuzzy(state: &mut AppState) {
    state.fuzzy_selected = 0;
    let all = state.fuzzy_all_files.clone();
    state.fuzzy_results = fuzzy_filter(&state.fuzzy_query, &all);
}

// ── Eventos LSP ───────────────────────────────────────────────────────────────

fn handle_lsp(state: &mut AppState, event: LspEvent) {
    match event {
        LspEvent::Ready { server_name } => {
            state.lsp_status = LspStatus::Ready(server_name);
            update_status(state);
        }
        LspEvent::Diagnostics { path, items } => {
            state.diagnostics.insert(path, items);
        }
        LspEvent::Completions(items) => {
            if !items.is_empty() {
                state.completions = items;
                state.show_completions = true;
                state.completion_selected = 0;
            }
        }
        LspEvent::Definition(maybe_loc) => {
            if let Some(loc) = maybe_loc {
                let path = loc.path.clone();
                let line = loc.line as usize;
                let col  = loc.col  as usize;
                if let Ok(content) = std::fs::read_to_string(&path) {
                    state.open_buffer(&path, &content);
                    state.buffer_mut().cursor.row = line;
                    state.buffer_mut().cursor.col = col;
                    state.focus = Focus::Editor;
                    update_status(state);
                }
            } else {
                state.status_message = " LSP: definición no encontrada".to_string();
            }
        }
        LspEvent::References(locs) => {
            if locs.is_empty() {
                state.status_message = " LSP: sin referencias".to_string();
            } else {
                let enriched = locs.into_iter().map(|mut loc| {
                    if loc.preview.is_none() {
                        if let Ok(content) = std::fs::read_to_string(&loc.path) {
                            loc.preview = content.lines().nth(loc.line as usize)
                                .map(|s| s.trim().to_owned());
                        }
                    }
                    loc
                }).collect();
                state.references = enriched;
                state.references_selected = 0;
                state.show_references = true;
            }
        }
        LspEvent::Error(msg) => {
            state.lsp_status = LspStatus::Error(msg);
            update_status(state);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn update_status(state: &mut AppState) {
    let focus_label = match state.focus {
        Focus::Editor  => "Editor",
        Focus::Sidebar => "Sidebar",
        Focus::Chat    => "Chat",
    };
    let tabs: String = state.buffers.iter().enumerate().map(|(i, b)| {
        let name = b.file_name.as_deref().unwrap_or("*nuevo*");
        let dirty = if b.dirty { "+" } else { "" };
        if i == state.active_buffer {
            format!("[{name}{dirty}] ")
        } else {
            format!(" {name}{dirty}  ")
        }
    }).collect();
    let lsp = lsp_indicator(&state.lsp_status);
    let model = if state.chat_visible && !state.chat.selected_model.is_empty() {
        format!("  AI:{}", state.chat.selected_model)
    } else {
        String::new()
    };
    state.status_message = format!(
        " {tabs}Foco:{focus_label}{lsp}{model}  |  Ctrl+A: chat  |  Ctrl+P: comandos  |  Ctrl+Q: salir"
    );
}

fn lsp_indicator(status: &LspStatus) -> &'static str {
    match status {
        LspStatus::Disabled   => "",
        LspStatus::Connecting => "  LSP:…",
        LspStatus::Ready(_)   => "  LSP:●",
        LspStatus::Error(_)   => "  LSP:✗",
    }
}

// ── Command Palette ───────────────────────────────────────────────────────────

fn handle_palette_key(state: &mut AppState, code: KeyCode, mods: KeyModifiers) -> Option<Command> {
    match code {
        KeyCode::Esc => {
            state.palette_active = false;
            state.palette_query.clear();
            state.palette_query_cursor = 0;
            state.palette_selected = 0;
        }
        KeyCode::Up => {
            if state.palette_selected > 0 {
                state.palette_selected -= 1;
            }
        }
        KeyCode::Down => {
            // Max items: 10 acciones fijas
            let max = palette_item_count(state);
            if state.palette_selected + 1 < max {
                state.palette_selected += 1;
            }
        }
        KeyCode::Enter => {
            return palette_execute(state);
        }
        KeyCode::Backspace => {
            if state.palette_query_cursor > 0 {
                let cur = state.palette_query_cursor;
                state.palette_query.remove(cur - 1);
                state.palette_query_cursor -= 1;
            }
        }
        KeyCode::Char(ch) => {
            let cur = state.palette_query_cursor;
            state.palette_query.insert(cur, ch);
            state.palette_query_cursor += 1;
            state.palette_selected = 0;
        }
        _ => {}
    }
    None
}

fn palette_item_count(_state: &AppState) -> usize {
    // Acciones fijas de la palette (excluyendo separadores)
    7
}

fn palette_execute(state: &mut AppState) -> Option<Command> {
    state.palette_active = false;
    let query = state.palette_query.trim().to_lowercase();
    state.palette_query.clear();
    state.palette_query_cursor = 0;

    // Si la query coincide con "abrir" o hay archivos, abrir fuzzy
    if query.starts_with("abrir") || query.starts_with("open") || query.starts_with("arch") {
        open_fuzzy(state);
        return None;
    }

    // Por índice de selección
    match state.palette_selected {
        0 => { open_fuzzy(state); None }          // Abrir archivo
        1 => {                                      // Cambiar modelo
            state.model_selector_active = true;
            if state.model_selector_models.is_empty() {
                Some(Command::AiLoadModels)
            } else { None }
        }
        2 => {                                      // Alternar modo Build/Plan
            state.chat.mode = state.chat.mode.toggle();
            update_status(state);
            None
        }
        3 => {                                      // Toggle chat
            state.chat_visible = !state.chat_visible;
            if state.chat_visible { state.focus = Focus::Chat; } else { state.focus = Focus::Editor; }
            update_status(state);
            None
        }
        4 => {                                      // Nueva sesión (reset chat)
            state.chat.messages.clear();
            state.chat.tokens_generated = 0;
            state.chat.session_name = {
                use std::time::{SystemTime, UNIX_EPOCH};
                let s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                format!("Sesión {s}")
            };
            update_status(state);
            None
        }
        5 => {                                      // Inyectar buffer
            if state.chat_visible { Some(Command::AiInjectBuffer) } else { None }
        }
        6 => {                                      // Salir
            state.quit = true;
            None
        }
        _ => None,
    }
}
