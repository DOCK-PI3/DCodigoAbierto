# Plan de rediseño UI — estilo OpenCode

> Fecha: 3 de mayo de 2026  
> Objetivo: remodelar DCodigoAbierto para que su interfaz y flujo de uso sea lo más parecido posible a OpenCode CLI, manteniendo las opciones actuales del proyecto.

---

## TL;DR

Pantalla home mejorada · sistema de modos Build/Plan · paleta de comandos unificada (Ctrl+P) · vista de chat full-screen con panel de metadatos · barra de estado rediseñada con tokens/progreso · syntax highlight en mensajes · corrección de texto invisible al escribir.

---

## Fase 1 — Corrección crítica: texto invisible al escribir

**Problema**: en `HomeWidget` el input renderiza texto blanco sobre fondo casi blanco, o el cursor invierte colores incorrectamente, haciendo el texto ilegible.

**Archivos**: `crates/ui/src/widgets/home.rs`

**Fix**: garantizar que `fg` del texto sea siempre visible contra `bg_input = Rgb(24,24,37)`. El placeholder usa color subtle, el texto normal usa fg blanco suave, el cursor usa bg acento.

---

## Fase 2 — Sistema de modos (Build / Plan)

**Qué**:
- `ChatMode::Build` = todas las herramientas activas (file read/write, shell, glob, grep, diagnostics, get_buffer, web_fetch)
- `ChatMode::Plan` = solo herramienta `web_fetch` activa

**Archivos a modificar**:
| Archivo | Cambio |
|---|---|
| `crates/app/src/state.rs` | añadir `ChatMode` enum + `pub chat_mode: ChatMode` en `ChatState` |
| `crates/types/src/view_state.rs` | añadir `pub chat_mode: ChatMode` a `ViewState` |
| `crates/app/src/update.rs` | `Tab` en home alterna modo; lógica de keybind |
| `crates/app/src/app.rs` | `execute_command(AiSendMessage)`: filtrar tools según modo |
| `crates/ai/src/tools/mod.rs` | añadir `plan_tools()` que retorna solo `[web_fetch]` |

**Visual en home**: debajo del input, segunda línea muestra el modo con color:
- `Build` → naranja `#f9e2af`
- `Plan` → azul `#89b4fa`

---

## Fase 3 — HomeWidget rediseñado

**Diseño objetivo** (inspirado en capturas de OpenCode):

```
[espacio top – centrado verticalmente]

       d c a                          ← logo ascii simple, centrado

   ╭─────────────────────────────────╮
   │  Escribe algo...                │   ← línea 1: texto / placeholder
   │  Build · llama3.2  Ollama       │   ← línea 2: modo + modelo + proveedor
   ╰─────────────────────────────────╯
                  tab agentes  ctrl+p comandos  ← alineado a derecha del input

[espacio flex]

● Tip  texto del tip rotatorio               ← centrado
~/directorio/actual              v0.1.0      ← bottom
```

**Cambios concretos**:
- Input box: `BorderType::Rounded`, sin título
- Input height = 4 (2 líneas internas + 2 bordes)
- Hints alineados a la DERECHA del ancho del input (no centrados globalmente)
- Logo simplificado usando texto monospace sin block-chars pesados

---

## Fase 4 — Paleta de comandos unificada (Ctrl+P)

**Qué**: popup único `CommandPaletteWidget` que reemplaza fuzzy finder directo y model selector directo.

**Secciones** (como en capturas de OpenCode):
```
Commands                              esc
─────────────────────────────────────────
Search...

Suggested
  Switch model                 ctrl+o
  Open file                    ctrl+f

Session
  Open editor                  ctrl+e
  New session                  ctrl+n

System
  Cambiar modo (Build ↔ Plan)
  Inyectar buffer              ctrl+i
  Abort stream                 ctrl+x
  Salir                        ctrl+q
```

**Estado nuevo en `AppState`**:
```rust
pub struct CommandPaletteState {
    pub active: bool,
    pub query: String,
    pub cursor: usize,
    pub selected_flat: usize,    // índice en lista aplanada
}
```

**Widget**: `crates/ui/src/widgets/command_palette.rs`

**Keybinding**: `Ctrl+P` → abre palette (antes abría fuzzy directamente). `Ctrl+O` desaparece como binding independiente (integrado en palette → Switch model).

---

## Fase 5 — Vista de chat: layout full-screen + panel de metadatos

**Qué**: cuando hay mensajes activos, el layout cambia a:

```
┌────────────────────────────────────────────┬──────────────────────┐
│                                            │ Session name         │
│   Mensajes del chat (scroll)               │                      │
│                                            │ Context              │
│                                            │   X tokens           │
│                                            │   Y% used            │
│                                            │                      │
│                                            │ LSP                  │
│                                            │   rust-analyzer ●    │
│                                            │                      │
├─────────────────────────────────────────── │ ~/cwd/actual         │
│ ▌ input...                                 │                      │
│ Build · model  Provider                    │                      │
└────────────────────────────────────────────┴──────────────────────┘
```

**Archivos nuevos**:
- `crates/ui/src/widgets/meta_panel.rs` — `MetaPanelWidget`
- `crates/ui/src/layout.rs` — nuevo `chat_layout()`

**ViewState nuevos campos**:
- `pub session_name: &'a str`
- `pub tokens_used: usize`
- `pub tokens_total: usize`
- `pub chat_mode: ChatMode`

---

## Fase 6 — Statusbar rediseñada

| Estado | Left | Right |
|---|---|---|
| Home | vacío | `ctrl+p comandos` |
| Streaming | `●●● esc interrumpir` (animado) | `X.XK (Y%)  ctrl+p` |
| Chat activo | `Mode · Model · Provider · Xs` | `ctrl+p comandos` |

**Archivos**: `crates/ui/src/widgets/statusbar.rs` (reescribir)

**ViewState nuevos campos**:
- `pub streaming_elapsed_secs: u64`
- `pub total_chars_generated: usize`

---

## Fase 7 — Syntax highlight en mensajes de chat

**Qué**: los bloques ` ```lang ... ``` ` en las respuestas del asistente se renderizan con colores básicos por lenguaje.

**Estrategia** (sin parser completo, basada en palabras clave):
- Detectar apertura/cierre de bloque con regex simple
- Dentro del bloque: colorear keywords Rust / JS / Python / HTML / Shell
- Implementar en `crates/ui/src/highlight.rs` como función pura: `fn highlight_line(line: &str, lang: &str) -> Vec<Span>`

---

## Archivos que se crean / modifican

| Archivo | Fase | Acción |
|---|---|---|
| `crates/ui/src/widgets/home.rs` | 1, 3 | Reescribir |
| `crates/ui/src/widgets/command_palette.rs` | 4 | Crear |
| `crates/ui/src/widgets/meta_panel.rs` | 5 | Crear |
| `crates/ui/src/widgets/statusbar.rs` | 6 | Reescribir |
| `crates/ui/src/widgets/chat.rs` | 7 | Modificar |
| `crates/ui/src/widgets/mod.rs` | 4,5 | Registrar módulos |
| `crates/ui/src/render.rs` | 5 | Lógica de modos de layout |
| `crates/ui/src/layout.rs` | 5 | Añadir `chat_layout()` |
| `crates/ui/src/highlight.rs` | 7 | Crear |
| `crates/app/src/state.rs` | 2 | `ChatMode`, `CommandPaletteState` |
| `crates/app/src/update.rs` | 2, 4 | Nuevos keybindings |
| `crates/app/src/app.rs` | 2 | Filtrar tools por modo |
| `crates/types/src/view_state.rs` | 2, 5, 6 | Campos nuevos |
| `crates/ai/src/tools/mod.rs` | 2 | `plan_tools()` |

---

## Criterios de aceptación

1. `cargo build --release` sin errores de compilación
2. Home screen: texto del input siempre visible al escribir
3. `Tab` en home alterna Build (naranja) ↔ Plan (azul)
4. `Ctrl+P` abre palette de comandos con secciones navegables
5. Enviar mensaje → vista cambia a chat full-screen con panel de metadatos
6. Panel de metadatos muestra tokens usados correctamente
7. Durante streaming: statusbar inferior muestra animación + tokens
8. Bloques de código en respuestas se muestran con colores básicos
9. `Esc` en chat → vuelve a home screen
