use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};

use dca_events::bus::{AppMessage, EventSender};
use dca_types::{CompletionEntry, DiagnosticInfo, DiagnosticSeverity, FileLocation, LspEvent};
use serde_json::{json, Value};
use tokio::io::{BufReader, BufWriter};
use tokio::process::Command;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::{debug, warn};

use crate::codec::{read_message, write_message};

static REQ_ID: AtomicI64 = AtomicI64::new(1);

fn next_id() -> i64 {
    REQ_ID.fetch_add(1, Ordering::Relaxed)
}

/// Comandos que la app puede enviar al cliente LSP.
#[derive(Debug)]
pub enum LspCommand {
    /// Notificar apertura de un archivo
    Open { path: String, text: String },
    /// Notificar cambio en el contenido de un archivo
    Change { path: String, text: String },
    /// Pedir autocompletado en posición dada
    Completion { path: String, line: u32, col: u32 },
    /// Ir a la definición del símbolo en posición dada
    GotoDefinition { path: String, line: u32, col: u32 },
    /// Buscar todas las referencias al símbolo en posición dada
    FindReferences { path: String, line: u32, col: u32 },
    /// Apagar el servidor LSP limpiamente
    Shutdown,
}

/// Tipos de petición pendiente (para despachar la respuesta)
enum PendingKind {
    Completion,
    GotoDefinition,
    FindReferences,
}

/// Handle público para que la app envíe comandos al cliente LSP.
#[derive(Clone)]
pub struct LspClient {
    cmd_tx: UnboundedSender<LspCommand>,
}

impl LspClient {
    /// Lanza el servidor LSP en background y devuelve el cliente.
    /// Los resultados (diagnósticos, completions) se envían al `event_tx`.
    /// Si el servidor no se encuentra, envía un `LspEvent::Error`.
    pub fn spawn(server_cmd: &str, event_tx: EventSender) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let cmd = server_cmd.to_owned();
        tokio::spawn(async move {
            if let Err(e) = lsp_task(cmd, cmd_rx, event_tx.clone()).await {
                let _ = event_tx.send(AppMessage::Lsp(LspEvent::Error(e.to_string())));
            }
        });
        Self { cmd_tx }
    }

    pub fn open(&self, path: &str, text: &str) {
        let _ = self.cmd_tx.send(LspCommand::Open {
            path: path.to_owned(),
            text: text.to_owned(),
        });
    }

    pub fn change(&self, path: &str, text: &str) {
        let _ = self.cmd_tx.send(LspCommand::Change {
            path: path.to_owned(),
            text: text.to_owned(),
        });
    }

    pub fn request_completion(&self, path: &str, line: u32, col: u32) {
        let _ = self.cmd_tx.send(LspCommand::Completion {
            path: path.to_owned(),
            line,
            col,
        });
    }

    pub fn goto_definition(&self, path: &str, line: u32, col: u32) {
        let _ = self.cmd_tx.send(LspCommand::GotoDefinition {
            path: path.to_owned(),
            line,
            col,
        });
    }

    pub fn find_references(&self, path: &str, line: u32, col: u32) {
        let _ = self.cmd_tx.send(LspCommand::FindReferences {
            path: path.to_owned(),
            line,
            col,
        });
    }
}

// ── Tarea interna del cliente LSP ─────────────────────────────────────────────

async fn lsp_task(
    server_cmd: String,
    mut cmd_rx: UnboundedReceiver<LspCommand>,
    event_tx: EventSender,
) -> color_eyre::Result<()> {
    // Lanzar el proceso servidor
    let mut child = Command::new(&server_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            color_eyre::eyre::eyre!("No se pudo iniciar '{server_cmd}': {e}. ¿Está instalado?")
        })?;

    let stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");
    let mut writer = BufWriter::new(stdin);
    let mut reader = BufReader::new(stdout);

    // ── Handshake: initialize ────────────────────────────────────────────
    let init_id = next_id();
    let init_msg = json!({
        "jsonrpc": "2.0",
        "id": init_id,
        "method": "initialize",
        "params": {
            "processId": std::process::id(),
            "clientInfo": { "name": "dcodigoabierto", "version": "0.1.0" },
            "rootUri": null,
            "capabilities": {
                "textDocument": {
                    "synchronization": {
                        "dynamicRegistration": false,
                        "didSave": false,
                        "willSave": false
                    },
                    "completion": {
                        "completionItem": {
                            "snippetSupport": false,
                            "documentationFormat": ["plaintext"]
                        }
                    },
                    "publishDiagnostics": {}
                }
            }
        }
    });
    write_message(&mut writer, &init_msg.to_string()).await?;

    // Esperar respuesta initialize
    loop {
        let raw = read_message(&mut reader).await?;
        let val: Value = serde_json::from_str(&raw)?;
        if val.get("id") == Some(&json!(init_id)) {
            debug!("LSP: initialize OK");
            break;
        }
    }

    // Enviar initialized notification
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    write_message(&mut writer, &initialized.to_string()).await?;

    let _ = event_tx.send(AppMessage::Lsp(LspEvent::Ready {
        server_name: server_cmd.clone(),
    }));

    // ── Loop principal ────────────────────────────────────────────────────
    let mut pending: HashMap<i64, PendingKind> = HashMap::new();
    let mut doc_versions: HashMap<String, i32> = HashMap::new();

    loop {
        tokio::select! {
            // Mensaje entrante del servidor
            result = read_message(&mut reader) => {
                match result {
                    Ok(raw) => dispatch_server_message(&raw, &mut pending, &event_tx),
                    Err(e) => {
                        warn!("LSP: error leyendo: {e}");
                        break;
                    }
                }
            }

            // Comando de la app
            cmd = cmd_rx.recv() => {
                match cmd {
                    None => break, // canal cerrado → la app terminó
                    Some(LspCommand::Shutdown) => {
                        let msg = json!({
                            "jsonrpc": "2.0",
                            "id": next_id(),
                            "method": "shutdown",
                            "params": null
                        });
                        let _ = write_message(&mut writer, &msg.to_string()).await;
                        break;
                    }
                    Some(LspCommand::Open { path, text }) => {
                        let version = 1;
                        doc_versions.insert(path.clone(), version);
                        let uri = path_to_uri(&path);
                        let msg = json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didOpen",
                            "params": {
                                "textDocument": {
                                    "uri": uri,
                                    "languageId": lang_id(&path),
                                    "version": version,
                                    "text": text
                                }
                            }
                        });
                        let _ = write_message(&mut writer, &msg.to_string()).await;
                    }
                    Some(LspCommand::Change { path, text }) => {
                        let version = doc_versions.entry(path.clone()).or_insert(0);
                        *version += 1;
                        let v = *version;
                        let uri = path_to_uri(&path);
                        let msg = json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didChange",
                            "params": {
                                "textDocument": { "uri": uri, "version": v },
                                "contentChanges": [{ "text": text }]
                            }
                        });
                        let _ = write_message(&mut writer, &msg.to_string()).await;
                    }
                    Some(LspCommand::Completion { path, line, col }) => {
                        let id = next_id();
                        pending.insert(id, PendingKind::Completion);
                        let uri = path_to_uri(&path);
                        let msg = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/completion",
                            "params": {
                                "textDocument": { "uri": uri },
                                "position": { "line": line, "character": col }
                            }
                        });
                        let _ = write_message(&mut writer, &msg.to_string()).await;
                    }
                    Some(LspCommand::GotoDefinition { path, line, col }) => {
                        let id = next_id();
                        pending.insert(id, PendingKind::GotoDefinition);
                        let uri = path_to_uri(&path);
                        let msg = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/definition",
                            "params": {
                                "textDocument": { "uri": uri },
                                "position": { "line": line, "character": col }
                            }
                        });
                        let _ = write_message(&mut writer, &msg.to_string()).await;
                    }
                    Some(LspCommand::FindReferences { path, line, col }) => {
                        let id = next_id();
                        pending.insert(id, PendingKind::FindReferences);
                        let uri = path_to_uri(&path);
                        let msg = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/references",
                            "params": {
                                "textDocument": { "uri": uri },
                                "position": { "line": line, "character": col },
                                "context": { "includeDeclaration": true }
                            }
                        });
                        let _ = write_message(&mut writer, &msg.to_string()).await;
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Parseo de mensajes del servidor ──────────────────────────────────────────

fn dispatch_server_message(
    raw: &str,
    pending: &mut HashMap<i64, PendingKind>,
    event_tx: &EventSender,
) {
    let Ok(val) = serde_json::from_str::<Value>(raw) else {
        warn!("LSP: JSON inválido: {raw}");
        return;
    };

    if let Some(id) = val.get("id").and_then(|v| v.as_i64()) {
        // Es una respuesta a una petición
        if let Some(kind) = pending.remove(&id) {
            match kind {
                PendingKind::Completion => {
                    let items = parse_completions(&val);
                    let _ = event_tx.send(AppMessage::Lsp(LspEvent::Completions(items)));
                }
                PendingKind::GotoDefinition => {
                    let loc = parse_single_location(&val);
                    let _ = event_tx.send(AppMessage::Lsp(LspEvent::Definition(loc)));
                }
                PendingKind::FindReferences => {
                    let locs = parse_locations(&val);
                    let _ = event_tx.send(AppMessage::Lsp(LspEvent::References(locs)));
                }
            }
        }
        return;
    }

    // Es una notificación del servidor
    if let Some(method) = val.get("method").and_then(|v| v.as_str()) {
        match method {
            "textDocument/publishDiagnostics" => {
                if let Some(params) = val.get("params") {
                    let path = params
                        .get("uri")
                        .and_then(|v| v.as_str())
                        .map(uri_to_path)
                        .unwrap_or_default();
                    let items = parse_diagnostics(params);
                    let _ = event_tx.send(AppMessage::Lsp(LspEvent::Diagnostics { path, items }));
                }
            }
            _ => {
                debug!("LSP: notificación ignorada: {method}");
            }
        }
    }
}

fn parse_diagnostics(params: &Value) -> Vec<DiagnosticInfo> {
    let Some(arr) = params.get("diagnostics").and_then(|v| v.as_array()) else {
        return vec![];
    };
    arr.iter()
        .filter_map(|d| {
            let line = d
                .get("range")
                .and_then(|r| r.get("start"))
                .and_then(|s| s.get("line"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let col = d
                .get("range")
                .and_then(|r| r.get("start"))
                .and_then(|s| s.get("character"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let message = d.get("message").and_then(|v| v.as_str())?.to_owned();
            let severity = match d.get("severity").and_then(|v| v.as_u64()) {
                Some(1) => DiagnosticSeverity::Error,
                Some(2) => DiagnosticSeverity::Warning,
                Some(3) => DiagnosticSeverity::Info,
                _ => DiagnosticSeverity::Hint,
            };
            Some(DiagnosticInfo { line, col, message, severity })
        })
        .collect()
}

fn parse_completions(val: &Value) -> Vec<CompletionEntry> {
    let result = val.get("result");
    let items_val = result
        .and_then(|r| r.get("items"))
        .or_else(|| result.and_then(|r| r.as_array()).map(|_| result.unwrap()));

    let Some(items) = items_val.and_then(|v| v.as_array()) else {
        return vec![];
    };

    items
        .iter()
        .take(20) // máximo 20 items en el popup
        .filter_map(|item| {
            let label = item.get("label").and_then(|v| v.as_str())?.to_owned();
            let detail = item
                .get("detail")
                .and_then(|v| v.as_str())
                .map(String::from);
            let kind = completion_kind_str(item.get("kind").and_then(|v| v.as_u64()));
            let insert_text = item
                .get("insertText")
                .and_then(|v| v.as_str())
                .map(String::from);
            Some(CompletionEntry { label, detail, kind, insert_text })
        })
        .collect()
}

// ── Utilidades ────────────────────────────────────────────────────────────────

fn path_to_uri(path: &str) -> String {
    if path.starts_with("file://") {
        path.to_owned()
    } else {
        format!("file://{}", path)
    }
}

fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_owned()
}

fn lang_id(path: &str) -> &'static str {
    if path.ends_with(".rs") {
        "rust"
    } else if path.ends_with(".ts") || path.ends_with(".tsx") {
        "typescript"
    } else if path.ends_with(".js") || path.ends_with(".jsx") {
        "javascript"
    } else if path.ends_with(".py") {
        "python"
    } else if path.ends_with(".go") {
        "go"
    } else {
        "plaintext"
    }
}

fn completion_kind_str(kind: Option<u64>) -> Option<String> {
    let s = match kind? {
        1 => "text",
        2 => "method",
        3 => "fn",
        4 => "constructor",
        5 => "field",
        6 => "variable",
        7 => "class",
        8 => "interface",
        9 => "module",
        10 => "property",
        14 => "keyword",
        15 => "snippet",
        _ => return None,
    };
    Some(s.to_owned())
}

/// Parsea una respuesta de `textDocument/definition`.
/// Puede ser `Location`, `Location[]` o `LocationLink[]`.
fn parse_single_location(val: &Value) -> Option<FileLocation> {
    let result = val.get("result")?;
    // Si es array, tomar el primero
    let loc = if let Some(arr) = result.as_array() {
        arr.first()?
    } else {
        result
    };
    location_from_value(loc)
}

/// Parsea una respuesta de `textDocument/references` (array de Location).
fn parse_locations(val: &Value) -> Vec<FileLocation> {
    let Some(arr) = val.get("result").and_then(|r| r.as_array()) else {
        return vec![];
    };
    arr.iter().filter_map(location_from_value).collect()
}

fn location_from_value(loc: &Value) -> Option<FileLocation> {
    // Soporta `Location` (uri+range) y `LocationLink` (targetUri+targetRange)
    let uri = loc.get("uri")
        .or_else(|| loc.get("targetUri"))
        .and_then(|v| v.as_str())?;
    let range = loc.get("range")
        .or_else(|| loc.get("targetSelectionRange"))
        .or_else(|| loc.get("targetRange"))?;
    let start = range.get("start")?;
    let line = start.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let col  = start.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let path = uri_to_path(uri);
    Some(FileLocation { path, line, col, preview: None })
}
