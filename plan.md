# Plan: Next-Gen Rust TUI — "DCodigoAbierto"

> Fecha: 3 de mayo de 2026

## TL;DR
Construir una TUI CLI en Rust que supere OpenCode CLI, usando Ratatui 0.30+, tokio, async-lsp y arquitectura Elm/Actor-Model reactiva. Workspace multi-crate, single binary, arranque < 50ms, LSP nativo sin bloquear la UI.

---

## A. Arquitectura de Carpetas

```
dcodigoabierto/
├── Cargo.toml                  ← workspace root
├── Cargo.lock
├── crates/
│   ├── app/                    ← crate: core app logic
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs          ← App struct, main loop
│   │       ├── state.rs        ← AppState (source of truth)
│   │       ├── message.rs      ← AppMessage enum (Elm messages)
│   │       └── update.rs       ← update() función pura
│   ├── ui/                     ← crate: todo el renderizado ratatui
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── layout.rs       ← motor de layouts dinámico
│   │       ├── theme.rs        ← sistema de temas
│   │       ├── widgets/
│   │       │   ├── editor.rs
│   │       │   ├── sidebar.rs
│   │       │   ├── statusbar.rs
│   │       │   ├── completion.rs   ← popup de autocompletado LSP
│   │       │   └── diagnostics.rs
│   │       └── render.rs       ← función render() principal
│   ├── lsp/                    ← crate: cliente LSP
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs       ← async-lsp MainLoop + router
│   │       ├── handler.rs      ← notificaciones server→client
│   │       └── types.rs        ← tipos puente con lsp-types
│   ├── events/                 ← crate: manejo de eventos
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── crossterm_bridge.rs  ← crossterm → AppMessage
│   │       └── bus.rs          ← EventBus (mpsc channels)
│   └── config/                 ← crate: config y temas
│       └── src/
│           ├── lib.rs
│           ├── config.rs       ← cargador TOML
│           └── theme.rs        ← definiciones de temas
├── src/
│   └── main.rs                 ← entry point, tokio runtime
└── assets/
    └── themes/                 ← archivos .toml de temas
```

---

## B. Stack de Crates Recomendados

| Crate | Versión | Por qué es vital |
|---|---|---|
| `ratatui` | 0.30 | Motor TUI con diff-rendering automático por celda; modular desde 0.30 |
| `crossterm` | 0.29 | Backend terminal: raw mode, eventos teclado/mouse, resize |
| `tokio` | 1.51 `full` | Runtime async: `mpsc`, `spawn`, timers. Backbone de toda la concurrencia |
| `async-lsp` | 0.2.4 | **La única opción correcta para clientes LSP**: soporta `&mut self` en notificaciones; tower-lsp es solo para servidores |
| `lsp-types` | 0.97 | Tipos del protocolo: `CompletionItem`, `Diagnostic`, `TextEdit`, etc. |
| `tower` | 0.4 | Middleware LSP: retry, timeout, tracing — composición type-safe |
| `color-eyre` | 0.6 | Error handling ergonómico con contexto y backtrace coloreado |
| `tracing` + `tracing-subscriber` | 0.1/0.3 | Logging non-blocking (file sink async, sin impacto en UI) |
| `serde` + `serde_json` | 1.0 | Serialización config y mensajes LSP |
| `toml` | 0.8 | Parseo de `config.toml` y archivos de temas |
| `dashmap` | 5.5 | HashMap concurrente para cache de diagnósticos sin Mutex |
| `unicode-width` | 0.2 | Ancho real de caracteres Unicode y Nerd Fonts |
| `syntect` | 5.0 | Syntax highlighting integrable con ratatui (TextMate grammars) |

---

## C. Flujo de Datos — State Management Asíncrono

**Principio**: AppState es owned por el main loop, nunca compartido con Mutex en el hot path. Toda comunicación es por mensajes.

```
[crossterm EventStream] ─────┐
[async-lsp notifications] ───┼──► EventBus (tokio::mpsc) ──► update(state, msg)
[tokio::spawn task results] ─┘         │                            │
                                        │                      Option<Command>
                                        │                            │
                                        └─── SpawnLspRequest ────────┘
                                             OpenFile, etc.
                                                  │
                                          tokio::spawn(...)
                                          resultado → EventBus
```

- **`AppState`**: struct plana, mutada sincrónicamente en el main loop
- **`AppMessage`**: enum exhaustivo con todas las variantes de eventos
- **`update()`**: función pura que muta estado y retorna `Option<Command>`
- **`Command`**: representa side-effects (llamadas LSP, I/O) ejecutados fuera del loop
- Sin `Arc<Mutex<AppState>>` — la contención se elimina por diseño

---

## D. Roadmap de Implementación — 5 Fases

### Fase 1 — Foundation: El Loop Reactivo (semanas 1-2)
1. `cargo new --workspace dcodigoabierto` + estructura multi-crate
2. `main.rs`: `tokio::main` + `ratatui::init()` + raw mode
3. `EventBus`: `crossterm::event::EventStream` como `tokio::Stream` → `AppMessage`
4. Elm loop completo: `update()` + `render()` con `AppState { quit: bool }`
5. Layout base: panel editor + statusbar + manejo de `Ctrl+C` / `q`

### Fase 2 — Editor & UI (semanas 3-5)
1. Widget `Editor` con scroll, cursor posicionado y selección de texto
2. Sistema de temas: cargar `theme.toml` + aplicar en render
3. Sidebar con árbol de archivos (crate `walkdir`)
4. Integración `unicode-width` para Nerd Fonts y emojis
5. Layouts dinámicos: splits horizontales/verticales toggleables

### Fase 3 — LSP Integration (semanas 6-9)
1. `async-lsp` client: spawn `rust-analyzer` como proceso hijo con `tokio::process`
2. `textDocument/didOpen` + `textDocument/didChange` al editar
3. Recibir `publishDiagnostics` → `AppMessage::Diagnostics` → gutter + statusbar
4. `textDocument/completion` → popup flotante con `CompletionItem`
5. `textDocument/hover` → tooltip en cursor

### Fase 4 — Advanced Features (semanas 10-13)
1. Go-to-definition, find-references (multi-buffer)
2. Code actions (`workspace/executeCommand`)
3. Multi-buffer: múltiples archivos abiertos con tabs
4. Fuzzy finder integrado (crate `nucleo` — motor de `telescope.nvim`)
5. Config hot-reload con crate `notify` (watch `config.toml`)

### Fase 5 — Optimización & Release (semanas 14-16) ✅ COMPLETADO
1. Profiling con `cargo-flamegraph` → identificar hotspots en render loop
2. Reducir allocations: pre-allocar buffers, reusar `Vec`s entre frames
3. Startup profiling: mover I/O síncrono a `tokio::spawn` lazy
4. `Cargo.toml` release profile: `lto = true`, `codegen-units = 1`, `strip = "symbols"`
5. Packaging: single binary + shell completions via `clap`

### Fase 6 — Integración IA: Chat + Proveedores + Skills (semanas 17-22)

#### 6A — Crate `dca-ai`: proveedores y herramientas

1. **Estructura del crate** `crates/ai/`
   - `Cargo.toml`: reqwest 0.12, async-trait, tokio-stream, serde_json, serde
   - `src/provider.rs` — trait `AiProvider`, tipos `AiMessage`, `AiRole`, `AiEvent`
   - `src/providers/{ollama,openai,anthropic}.rs`
   - `src/tools/{read_file,write_file,shell,web_fetch,glob,grep,diagnostics,get_buffer}.rs`
   - `src/agent.rs` — bucle de chat con tool calling + CancellationToken
   - `src/session.rs` — historial de mensajes en memoria

2. **Tres proveedores con streaming**
   - **Ollama**: `POST {base_url}/api/chat` stream=true → NDJSON
   - **OpenAI-compatible**: `POST {base_url}/v1/chat/completions` stream=true → SSE
     (OpenAI, Groq, LM Studio, Together, OpenRouter, cualquier endpoint local)
   - **Anthropic**: `POST https://api.anthropic.com/v1/messages` stream=true → SSE

3. **Sistema de herramientas (Skills)**

   | Herramienta       | Accion                              | Confirmacion |
   |-------------------|-------------------------------------|:---:|
   | `read_file`       | Lee cualquier archivo del proyecto  | No  |
   | `write_file`      | Escribe/modifica archivo            | Si  |
   | `shell`           | Ejecuta comando shell               | Si  |
   | `web_fetch`       | GET de URL, extrae texto plano      | No  |
   | `glob`            | Busca archivos por patron           | No  |
   | `grep`            | Busca texto en archivos             | No  |
   | `get_diagnostics` | Diagnosticos LSP del buffer activo  | No  |
   | `get_buffer`      | Contenido del editor en curso       | No  |

4. **Seccion `[ai]` en config.toml**
   ```toml
   [ai]
   provider      = "ollama"
   base_url      = "http://localhost:11434"
   api_key       = ""         # o env DCA_AI_API_KEY
   model         = "llama3.2"
   system_prompt = "Eres un asistente de programacion experto..."
   max_tokens    = 4096
   tools_enabled = true
   web_enabled   = true
   ```

#### 6B — Panel de Chat TUI

5. **Layout** con panel de chat (38% ancho derecho cuando visible):
   ```
   +-------------------------------------------------+
   | sidebar |  tabbar        | [provider/model]     |
   |  (28%)  |  editor (34%)  | [mensajes chat]      |
   |         |                | [input > ___  (38%)] |
   +-------------------------------------------------+
   |                   statusbar                     |
   +-------------------------------------------------+
   ```

6. **Atajos**:
   - `Ctrl+A` — toggle panel chat / `Ctrl+O` — selector de modelo
   - `Ctrl+I` — inyectar buffer activo como contexto
   - `Ctrl+X` — cancelar respuesta en curso
   - `Enter` (Focus::Chat) — enviar mensaje / `Esc` — volver al editor

7. Nuevos widgets: `ChatWidget`, `PermissionDialog` (tools peligrosos), `ModelSelector`

#### Decisiones Fase 6
- Sin SQLite en v1: historial en memoria
- web_fetch = GET HTTP simple + texto plano (sin parser HTML)
- Tool calling auto-disable si el proveedor no lo soporta
- Panel 38% fijo, no redimensionable en v1
- Excluido: MCP, sesiones persistentes, auto-compactacion de contexto

---

## E. Boilerplate — Loop Reactivo con Ratatui

```rust
// src/main.rs
use color_eyre::Result;
use crossterm::event::{EventStream, Event, KeyCode, KeyEvent};
use futures::StreamExt;
use ratatui::{Frame, DefaultTerminal};
use tokio::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};

// ── Mensajes (Elm Messages) ──────────────────────────────────────────────────
#[derive(Debug)]
pub enum AppMessage {
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
    Quit,
}

// ── Estado de la aplicación ──────────────────────────────────────────────────
#[derive(Default)]
pub struct AppState {
    pub quit: bool,
    pub counter: u32,
}

// ── update() — función pura ──────────────────────────────────────────────────
fn update(state: &mut AppState, msg: AppMessage) {
    match msg {
        AppMessage::Key(key) if key.code == KeyCode::Char('q') => {
            state.quit = true;
        }
        AppMessage::Key(key) if key.code == KeyCode::Char('+') => {
            state.counter += 1;
        }
        AppMessage::Tick => { /* lógica de tick periódico */ }
        _ => {}
    }
}

// ── render() — sin estado propio, puro ───────────────────────────────────────
fn render(frame: &mut Frame, state: &AppState) {
    use ratatui::widgets::{Block, Borders, Paragraph};
    use ratatui::text::Text;

    let text = Text::raw(format!("Counter: {} | Press 'q' to quit", state.counter));
    let widget = Paragraph::new(text)
        .block(Block::default().title("DCodigoAbierto").borders(Borders::ALL));
    frame.render_widget(widget, frame.area());
}

// ── Bridge: crossterm → EventBus ─────────────────────────────────────────────
async fn crossterm_task(tx: UnboundedSender<AppMessage>) {
    let mut stream = EventStream::new();
    loop {
        match stream.next().await {
            Some(Ok(Event::Key(key)))         => { let _ = tx.send(AppMessage::Key(key)); }
            Some(Ok(Event::Resize(w, h)))     => { let _ = tx.send(AppMessage::Resize(w, h)); }
            None | Some(Err(_))               => break,
            _                                 => {}
        }
    }
}

// ── Main loop ─────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let mut terminal = ratatui::init();
    let mut state = AppState::default();

    let (tx, mut rx): (UnboundedSender<AppMessage>, UnboundedReceiver<AppMessage>) =
        mpsc::unbounded_channel();

    // Spawn: bridge crossterm → canal
    tokio::spawn(crossterm_task(tx.clone()));

    // Spawn: tick cada 200ms
    let tx_tick = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            if tx_tick.send(AppMessage::Tick).is_err() { break; }
        }
    });

    loop {
        // Renderizar el frame actual
        terminal.draw(|frame| render(frame, &state))?;

        // Drenar mensajes pendientes sin bloquear
        while let Ok(msg) = rx.try_recv() {
            update(&mut state, msg);
        }

        // Esperar el siguiente mensaje (cede el thread a tokio)
        if let Some(msg) = rx.recv().await {
            update(&mut state, msg);
        }

        if state.quit { break; }
    }

    ratatui::restore();
    Ok(())
}
```

**Cargo.toml mínimo:**
```toml
[package]
name = "dcodigoabierto"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui    = { version = "0.30", features = ["all-widgets"] }
crossterm  = { version = "0.29", features = ["event-stream"] }
tokio      = { version = "1",    features = ["full"] }
futures    = "0.3"
color-eyre = "0.6"

[profile.release]
lto           = true
codegen-units = 1
strip         = "symbols"
```

---

## Decisiones Técnicas Clave

| Decisión | Elección | Alternativa descartada | Razón |
|---|---|---|---|
| LSP client | `async-lsp` | `tower-lsp` | tower-lsp es exclusivo para servidores |
| State sharing | `mpsc` channels | `Arc<Mutex<AppState>>` | Elimina contención en hot path del render loop |
| Project layout | Workspace multi-crate | Single crate | Compilación incremental, separación de concerns |
| Syntax highlight | `syntect` | `tree-sitter` | Menor complejidad inicial; tree-sitter se puede añadir en Fase 4 |
| GPU rendering | No en v1 | `ratatui-wgpu` | Experimental; diff-rendering de ratatui ya es óptimo |

---

## Criterios de Éxito

1. `cargo build --release` sin warnings ni errores
2. Startup time < 50ms medido con `hyperfine`
3. CPU idle < 1% en reposo (event poll a 200ms)
4. LSP: autocompletado funciona en `.rs` con `rust-analyzer`
5. `cargo test` pasa en todos los crates del workspace
