# DCodigoAbierto (`dca`)

Editor de texto TUI escrito en Rust, con integración LSP, multi-buffer, fuzzy finder, hot-reload de configuración e **integración IA nativa** con soporte de streaming para Ollama, OpenAI-compatible, Groq, OpenRouter y Anthropic.

## Compilar y ejecutar

```bash
cargo build --release
./target/release/dca
```

> Binario resultante: **~6.5 MB** (incluye reqwest + rustls para llamadas a APIs de IA).

## Uso

```bash
# Abrir directamente un archivo
dca src/main.rs

# Usar un servidor LSP distinto al configurado
dca --lsp clangd main.c

# Guardar logs en fichero (útil para depurar sin contaminar la TUI)
dca --log-file /tmp/dca.log --log-level debug

# Instalar completions de shell
dca --completions bash  >> ~/.bashrc
dca --completions zsh   >> ~/.zshrc
dca --completions fish  > ~/.config/fish/completions/dca.fish
```

## Controles

| Atajo | Acción |
|---|---|
| `Ctrl+Q` | Salir |
| `Ctrl+S` | Guardar archivo |
| `Ctrl+P` | Fuzzy finder de archivos |
| `Ctrl+G` | Ir a definición (LSP) |
| `Ctrl+R` | Encontrar referencias (LSP) |
| `Ctrl+W` | Cerrar buffer activo |
| `Ctrl+Tab` | Ciclar entre buffers abiertos |
| `Ctrl+Space` | Solicitar autocompletado (LSP) |
| `Ctrl+B` | Mostrar/ocultar sidebar |
| `Ctrl+A` | Mostrar/ocultar panel de chat IA |
| `Ctrl+O` | Abrir selector de modelos |
| `Ctrl+T` | Abrir selector de temas |
| `Ctrl+I` | Inyectar buffer actual como contexto IA |
| `Ctrl+X` | Abortar stream de respuesta IA |
| `Tab` / `Shift+Tab` | Cambiar foco: editor → sidebar → chat → editor |
| `↑↓` en listas | Navegar (sidebar, completions, referencias, fuzzy, modelos) |
| `Enter` en listas | Abrir / confirmar selección |
| `Esc` | Cerrar popup activo / volver al editor |

### En el panel de chat

| Atajo | Acción |
|---|---|
| `Enter` | Enviar mensaje |
| `↑` / `↓` | Desplazar historial |
| `Backspace` / `←` / `→` | Editar input |
| `Esc` | Volver el foco al editor |

### Diálogo de aprobación de herramientas

| Atajo | Acción |
|---|---|
| `Y` / `Enter` | Permitir ejecución de la herramienta |
| `N` / `Esc` | Denegar ejecución |

## Configuración

Archivo: `~/.config/dca/config.toml` (se crea automáticamente con valores por defecto).

```toml
tick_rate_ms = 200
lsp_server   = "rust-analyzer"   # vacío para desactivar LSP

[theme]
name         = "Solarized Dark"
bg           = "#1e1e2e"
bg_secondary = "#313244"
fg           = "#cdd6f4"
accent       = "#89b4fa"
info         = "#89dceb"
warning      = "#f9e2af"
error        = "#f38ba8"
success      = "#a6e3a1"
```

Los cambios en este archivo se aplican en vivo sin reiniciar el editor.

### Configuración IA

```toml
[ai]
provider      = "ollama"                   # ollama | openai | anthropic | groq | openrouter | custom
base_url      = "http://localhost:11434"    # URL base del proveedor
api_key       = ""                         # clave API (o usa la variable DCA_AI_API_KEY)
model         = "llama3.2"                 # modelo a usar
system_prompt = "Eres un asistente experto en programación."
max_tokens    = 4096
tools_enabled = true    # habilitar herramientas (leer/escribir archivos, shell, grep…)
web_enabled   = false   # habilitar herramienta web_fetch
temperature   = 0.7     # creatividad del modelo (0.0 – 2.0)
top_p         = 0.95    # nucleus sampling (0.0 – 1.0)
```

#### Ejemplos por proveedor

```toml
# Ollama local
[ai]
provider = "ollama"
base_url = "http://localhost:11434"
model    = "llama3.2"

# OpenAI
[ai]
provider = "openai"
base_url = "https://api.openai.com"
api_key  = "sk-..."
model    = "gpt-4o"

# Anthropic Claude
[ai]
provider = "anthropic"
base_url = "https://api.anthropic.com"
api_key  = "sk-ant-..."
model    = "claude-opus-4-5"

# Groq
[ai]
provider = "groq"
base_url = "https://api.groq.com/openai"
api_key  = "gsk_..."
model    = "llama-3.3-70b-versatile"

# OpenRouter
[ai]
provider = "openrouter"
base_url = "https://openrouter.ai/api"
api_key  = "sk-or-..."
model    = "meta-llama/llama-3.3-70b-instruct"
```

---

## Arquitectura

El proyecto sigue el **patrón Elm / Actor**: un único canal de mensajes (`AppMessage`), función de actualización pura (`update`) y renderizado inmediato con Ratatui.

```
src/main.rs                  ← binario `dca`, punto de entrada
crates/
  types/    dca-types        ← tipos compartidos (ViewState, LspEvent, FileLocation…)
  config/   dca-config       ← AppConfig, Theme, AiConfig, config watcher (notify)
  events/   dca-events       ← AppMessage enum, EventBus, tarea crossterm
  lsp/      dca-lsp          ← cliente LSP manual (wire protocol JSON-RPC)
  app/      dca-app          ← estado, update(), comandos, fuzzy (nucleo)
  ui/       dca-ui           ← widgets Ratatui, layout, render()
  ai/       dca-ai           ← proveedores IA, agente, herramientas, sesión
```

### Crate `dca-ai`

```
crates/ai/src/
  provider.rs          ← tipos: AiRole, AiMessage, ToolCall, ToolDef, AiEvent, trait AiProvider
  providers/
    ollama.rs          ← POST /api/chat  stream=true (NDJSON)
    openai.rs          ← POST /v1/chat/completions  stream=true (SSE)
    anthropic.rs       ← POST /v1/messages  stream=true (SSE, x-api-key)
    mod.rs             ← build_provider(cfg) → Box<dyn AiProvider>
  tools/
    read_file.rs       ← lectura de archivos con rango de líneas
    write_file.rs      ← escritura con creación de directorios (requiere aprobación)
    shell.rs           ← /bin/sh -c cmd, timeout, cwd (requiere aprobación)
    web_fetch.rs       ← GET URL, truncado a 32 KB
    glob_tool.rs       ← búsqueda recursiva de archivos por patrón glob
    grep_tool.rs       ← búsqueda de texto/regex en archivos
    diagnostics.rs     ← expone diagnósticos LSP al modelo
    get_buffer.rs      ← expone el buffer activo del editor al modelo
    mod.rs             ← all_tools(web_enabled) → Vec<Box<dyn Tool>>
  session.rs           ← ChatSession: historial de mensajes por turno
  agent.rs             ← AiAgent: loop agentico con gate de aprobación
  lib.rs               ← re-exports públicos
```

### Flujo de mensajes

```
Teclado ──────────────────────────────────────────┐
LSP server ──→ LspClient ──→ LspEvent ────────────┤
notify watcher ───────────────────────────────────┤→ AppMessage
Tick periódico ───────────────────────────────────┤
AiAgent stream  ──→ AiEvent ──────────────────────┘
                  │
                  ▼
            update(state, msg)  →  Option<Command>
                  │                      │
                  ▼                      ▼
            render(frame, view)    execute_command
                                   (LSP, I/O, spawn AiAgent)
```

#### Flujo de aprobación de herramientas IA

```
AiAgent solicita tool
   → pending_tx → AppMessage::AiToolRequest
   → update() establece tool_pending en estado
   → render() muestra PermissionDialog
   → usuario pulsa Y/N
   → update() devuelve Command::AiExecuteTool / deny
   → app.rs envía ApprovalDecision al agente
   → AiAgent ejecuta (o salta) la herramienta y continúa el loop
```

### Fase 7 — Optimización agentica & Temas ✅
- **Timeouts HTTP**: `connect_timeout` 10 s + `timeout` 180 s en todos los proveedores — el editor ya no se congela con modelos lentos o inaccesibles
- **Límite de iteraciones**: `MAX_TOOL_ITERATIONS = 20` en el loop agentico — evita bucles infinitos de herramientas
- **Timeout de aprobación**: 300 s para responder al diálogo de permiso — el agente se desbloquea solo si el usuario no responde
- **`temperature` y `top_p`**: nuevos campos en `[ai]` del config (con defaults 0.7 / 0.95), propagados a los tres proveedores
- **9 temas integrados** (`themes.txt` embebido en el binario vía `include_str!`):
  `tokyo-night`, `forest-moss`, `deep-ocean`, `rose-pine`, `nord-frost`,
  `espresso-night`, `cyber-dusk`, `serene-slate`, `midnight-plum`
- **Selector de temas** (`Ctrl+T` o `Ctrl+P → Seleccionar tema`): popup con vista previa de color accent, persiste en `config.toml`
- **Compatibilidad de config**: campos nuevos con `#[serde(default)]` — configs antiguos sin `temperature`/`top_p` siguen funcionando

---

## Estado del desarrollo

### Fase 1 — Scaffolding ✅
- Workspace Cargo multi-crate (6 crates)
- Loop principal Tokio + Ratatui + Crossterm
- Editor básico con cursor, inserción y borrado de texto
- Sidebar de árbol de archivos
- Statusbar

### Fase 2 — Editor completo ✅
- Gestión de líneas con `TextBuffer`
- Movimiento de cursor (↑↓←→, Home/End, PageUp/PageDown)
- Selección, cortar, copiar, pegar
- Scroll sincronizado con el cursor
- Apertura y guardado de archivos
- Diagnósticos inline (subrayado de errores)
- Gutter con números de línea

### Fase 3 — Integración LSP ✅
- Cliente LSP manual (sin librería de alto nivel): proceso hijo, stdio, JSON-RPC
- `initialize` / `initialized` handshake
- `textDocument/didOpen`, `textDocument/didChange`
- `textDocument/completion` con popup de autocompletado
- `textDocument/publishDiagnostics` renderizado en el editor
- Indicador de estado LSP en statusbar (`●` conectado, `…` conectando, `✗` error)

### Fase 4 — Multi-buffer, LSP avanzado, Fuzzy finder ✅
- **Multi-buffer**: pestañas con tab bar (Ctrl+Tab, Ctrl+W)
- **Go-to-definition** (`Ctrl+G`): abre el archivo de destino en un nuevo buffer
- **Find references** (`Ctrl+R`): panel flotante inferior con lista de referencias
- **Fuzzy finder** (`Ctrl+P`): popup centrado con `nucleo` para búsqueda difusa de archivos
- **Config hot-reload**: watcher con `notify` que recarga `config.toml` en vivo

### Fase 5 — Optimización & Release ✅
- **CLI con `clap`**: argumentos `[ARCHIVO]`, `--lsp`, `--log-file`, `--log-level`
- **Shell completions**: `dca --completions bash/zsh/fish` (clap_complete)
- **Startup lazy**: carga del árbol de archivos en `tokio::task::spawn_blocking` — la TUI responde antes de que el árbol esté listo (`AppMessage::FileTreeLoaded`)
- **Reducción de allocations**: `Vec<BufferTab>` pre-allocado fuera del render loop, reutilizado con `.clear()` + `.extend()` en cada frame
- **Panic hook**: restaura el terminal con `ratatui::restore()` antes de imprimir el backtrace
- **Perfil `profiling`**: `[profile.profiling]` en Cargo.toml con `debug=true`/`strip=none` para uso con `cargo flamegraph --profile profiling`
- **Binario**: 3.3 MB, LTO + codegen-units=1 + strip de símbolos

### Fase 6 — Integración IA ✅
- **Crate `dca-ai`**: nuevo crate independiente con API trait `AiProvider` + `Tool`
- **Proveedores con streaming**:
  - **Ollama** (local/remoto): `POST /api/chat` stream NDJSON, soporta function calling
  - **OpenAI-compatible**: `POST /v1/chat/completions` SSE; compatible con OpenAI, Groq, OpenRouter y cualquier API con ese formato
  - **Anthropic**: `POST /v1/messages` SSE, cabecera `x-api-key` + `anthropic-version`
- **8 herramientas agenticas** (las de aprobación requerida se marcan con 🔒):
  - `read_file` — leer archivo con rango de líneas opcional
  - `write_file` 🔒 — escribir/crear archivo
  - `shell` 🔒 — ejecutar comando de shell con timeout y directorio de trabajo
  - `web_fetch` — descargar URL (activable por config)
  - `glob` — búsqueda de archivos por patrón glob
  - `grep` — búsqueda de texto o regex en archivos del proyecto
  - `diagnostics` — expone los errores LSP actuales al modelo
  - `get_buffer` — expone el contenido del buffer del editor al modelo
- **Loop agentico**: el modelo puede encadenar llamadas a herramientas hasta completar la tarea
- **Gate de aprobación**: las herramientas destructivas (🔒) muestran un diálogo de confirmación antes de ejecutarse
- **Panel chat TUI** (`Ctrl+A`): historial de mensajes con colores por rol, barra de input con cursor, scroll
- **Selector de modelos** (`Ctrl+O`): consulta los modelos disponibles al proveedor activo
- **Inyección de buffer** (`Ctrl+I`): añade el archivo abierto como contexto al chat
- **Abort stream** (`Ctrl+X`): cancela la respuesta en curso con `CancellationToken`
- **Hot-reload**: cambiar el proveedor/modelo en `config.toml` surte efecto en el próximo envío
- **Binario**: 6.1 MB (incluye reqwest + rustls)

---

## Dependencias principales

| Crate | Versión | Uso |
|---|---|---|
| `ratatui` | 0.29 | Renderizado TUI |
| `crossterm` | 0.28 | Entrada/salida terminal |
| `tokio` | 1 | Runtime async |
| `lsp-types` | 0.97 | Tipos LSP (solo deserialización) |
| `serde_json` | 1 | JSON-RPC wire protocol |
| `nucleo` | 0.5 | Fuzzy matching |
| `notify` | 6 | Watcher de archivos (hot-reload) |
| `clap` | 4 | CLI: argumentos y subcomandos |
| `clap_complete` | 4 | Shell completions (bash/zsh/fish) |
| `walkdir` | 2 | Árbol de archivos |
| `color-eyre` | 0.6 | Manejo de errores |
| `tracing` | 0.1 | Logging estructurado |
| `reqwest` | 0.12 | HTTP client (streaming SSE/NDJSON para IA) |
| `async-trait` | 0.1 | Traits async (`AiProvider`, `Tool`) |
| `tokio-util` | 0.7 | `CancellationToken` para abort de stream |
| `futures` | 0.3 | Utilidades async |
