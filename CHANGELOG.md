# Changelog

## 2026-05-21 — Correcciones de bugs, features nuevas, y sistema de Skills

### 🔴 Correcciones críticas

#### Doble pulsación en Windows (`crates/events/src/crossterm_bridge.rs`)
- **Causa**: crossterm en Windows reporta `KeyEventKind::Press`, `Release` y `Repeat` por separado. El código reenviaba todos al event bus, procesando cada tecla 2+ veces.
- **Fix**: filtrado de `KeyEventKind::Release` para solo procesar `Press` y `Repeat`. Se mantiene soporte de repetición al mantener tecla pulsada.

#### Dependencia `notify` rota en Windows y Linux (`Cargo.toml`)
- **Causa**: `default-features = false` + solo `macos_fsevent` → sin backend de watcher en Windows ni Linux.
- **Fix**: restaurados los default features para detección automática de backend por plataforma.

#### Shell tool tenía `/bin/sh` como valor fijo — no funcionaba en Windows (`crates/ai/src/tools/shell.rs`)
- **Fix**: detección de plataforma con `cfg!(windows)`: usa `cmd /c` en Windows, `/bin/sh -c` en Unix.

#### `unwrap()` peligroso en cliente LSP (`crates/lsp/src/client.rs`)
- **Fix**: reemplazado `result.unwrap()` por `result.filter(|r| r.as_array().is_some())` en `parse_completions`.

#### Cursor con Unicode roto en chat y palette (`crates/app/src/update.rs`)
- **Causa**: `Left`/`Right`/`Backspace` movían el cursor 1 byte, rompiendo con caracteres multi-byte (ñ, á, 😀).
- **Fix**: movimiento de cursor usando `is_char_boundary()` y `ch.len_utf8()`.

#### Data loss: dirty flag limpiado antes de escribir a disco (`crates/app/src/update.rs`, `crates/app/src/app.rs`)
- **Causa**: `state.buffer_mut().dirty = false` se ejecutaba antes de `std::fs::write`. Si la escritura fallaba, el buffer se marcaba limpio pero el archivo no se guardaba.
- **Fix**: dirty flag ahora se limpia solo tras escritura exitosa, vía mensaje `AppMessage::FileSaved { path, success }` enviado al event bus desde `tokio::fs::write`.

### 🟠 Robustez de herramientas IA

#### Shell tool: truncación UTF-8 segura
- Head+tail truncation con helper `find_char_boundary()` para no cortar caracteres multi-byte a la mitad.
- Prevención de solapamiento head/tail con `.max(head_bound + 1024)` + doble guardia de char boundary.
- Prevención de panic en slicing multi-byte.

#### Web fetch: reintentos y guardias (`crates/ai/src/tools/web_fetch.rs`)
- Reintento único con 500ms de delay para errores transitorios.
- Comprobación de content-type: bloquea video/audio/octet-stream.
- Mensajes de error con hints HTTP específicos.

### 🟡 Features nuevas

#### Ctrl+S — Guardar archivo
- `Command::SaveBuffer { path, content }` — escribe el buffer a disco.
- Dirty flag se limpia solo tras confirmación de escritura exitosa (ver fix de data loss arriba).

#### Ctrl+C / Ctrl+V — Portapapeles
- **Copiar** (`Ctrl+C`): copia la línea actual del editor al portapapeles usando `arboard`.
- **Pegar** (`Ctrl+V`): pega contenido del portapapeles en el editor, con soporte multilínea (`\n` → `insert_newline()`).
- Dependencia añadida: `arboard = "3"`.

#### Fuzzy finder optimizado (`crates/app/src/fuzzy.rs`)
- **Antes**: se creaba un `Nucleo` nuevo + se inyectaban todos los archivos + 20 ticks en cada pulsación de tecla.
- **Ahora**: `FuzzyEngine` con `Nucleo` reutilizable, construido una vez al cargar el file tree y almacenado en `AppState.fuzzy_engine`.
- Consistencia: tanto query vacía como no-vacía limitan a 50 resultados.

#### Ctrl+Q solo para salir
- Se eliminó `Ctrl+C` del atajo de quit (ahora `Ctrl+C` es copiar). Solo `Ctrl+Q` cierra la app.

### 🟢 Sistema de Skills

#### Nuevo módulo: `crates/config/src/skills.rs`
- **`SkillsManager`**: gestor completo de skills con:
  - `install_from_github(repo, skill_name?)` — clona repo de GitHub, busca `SKILL.md`, copia al directorio local.
  - `list_installed()` — lista skills en `~/.config/dca/skills/`.
  - `remove_skill(name)` — elimina una skill instalada.
  - `build_skills_prompt()` — genera fragmento de system prompt con todas las skills instaladas.
- **Parser de SKILL.md**: extrae frontmatter YAML (`name`, `description`) y cuerpo (instrucciones).
- **Truncación UTF-8 segura**: instrucciones truncadas a 8 KB con `is_char_boundary()`.
- **Comandos**: `InstallSkills { repo, skill }`, `ListSkills` — accesibles desde la command palette y el event bus.
- **Integración IA**: `AiConfig::build_system_prompt()` añade las skills instaladas al system prompt del agente.

### 🔧 Cambios en archivos

| Archivo | Cambio |
|---|---|
| `crates/events/src/crossterm_bridge.rs` | Filtro `KeyEventKind::Release` |
| `crates/events/src/bus.rs` | Variante `FileSaved { path, success }` |
| `Cargo.toml` | `notify` default features, `arboard` workspace dep |
| `crates/ai/src/tools/shell.rs` | Shell platform-aware, truncación UTF-8, `find_char_boundary` |
| `crates/ai/src/tools/web_fetch.rs` | Reintentos, guardia content-type |
| `crates/lsp/src/client.rs` | `unwrap()` → `filter()` seguro |
| `crates/app/src/update.rs` | Ctrl+S/C/V, fuzzy engine, Unicode cursor, FileSaved handler |
| `crates/app/src/app.rs` | SaveBuffer async, skills handlers, `build_system_prompt()` |
| `crates/app/src/command.rs` | `SaveBuffer`, `InstallSkills`, `ListSkills` |
| `crates/app/src/fuzzy.rs` | `FuzzyEngine` con Nucleo reutilizable |
| `crates/app/src/state.rs` | Campo `fuzzy_engine: Option<Arc<FuzzyEngine>>` |
| `crates/app/Cargo.toml` | Dependencia `arboard` |
| `crates/config/src/config.rs` | `build_system_prompt()` con skills |
| `crates/config/src/lib.rs` | Export de `skills` module, `Skill`, `SkillsManager` |
| `crates/config/src/skills.rs` | **NUEVO** — `SkillsManager`, parser SKILL.md |

### Verificación
- `cargo check` — sin errores ni warnings
- `cargo build --release --bin dca` — exitoso, binario ~7.76 MB

---

## 2026-05-08

### Tool calling IA: fiabilidad y autocorreccion

- Fase 1:
  - Se añadió `schemars = "0.8"` en [crates/ai/Cargo.toml](crates/ai/Cargo.toml).
  - [crates/ai/src/tools/mod.rs](crates/ai/src/tools/mod.rs) ahora expone `validate_tool_args<T>()` y `tool_parameters_schema<T>()`.
  - Todas las herramientas activas de [crates/ai/src/tools](crates/ai/src/tools) usan argumentos tipados con `Serialize`, `Deserialize` y `JsonSchema`.
  - Se eliminaron los schemas JSON escritos a mano de `read_file`, `write_file`, `list_dir`, `glob`, `grep`, `shell`, `web_fetch`, `web_search`, `get_buffer` y `get_diagnostics`.

- Fase 2:
  - Se reemplazó el prompt por defecto en [crates/config/src/config.rs](crates/config/src/config.rs) por una version estructurada con reglas de no-uso, formato JSON de tool calls, few-shot y estrategia de autocorreccion.
  - Se verificó el punto de inyeccion del `system prompt` en [crates/ai/src/agent.rs](crates/ai/src/agent.rs) y su propagacion en proveedores OpenAI-compatible, Ollama y Anthropic.

- Fase 3:
  - [crates/ai/src/tools/mod.rs](crates/ai/src/tools/mod.rs) define `ToolError` y `to_json()`.
  - [crates/ai/src/tools/read_file.rs](crates/ai/src/tools/read_file.rs) devuelve errores estructurados para archivo inexistente, permisos y rangos invalidos.
  - [crates/ai/src/tools/write_file.rs](crates/ai/src/tools/write_file.rs) devuelve errores estructurados para preparacion de directorios y escritura.
  - [crates/ai/src/agent.rs](crates/ai/src/agent.rs) serializa esos errores como JSON y los reinyecta como resultado de herramienta.

- Fase 4:
  - [crates/ai/src/agent.rs](crates/ai/src/agent.rs) incorpora `PruneConfig` y `prune_context()` antes de cada llamada al proveedor.
  - [crates/ai/src/provider.rs](crates/ai/src/provider.rs) amplia `ToolCall` con `buffer_version` y `target_buffer_id` opcionales.
  - [crates/types/src/buffer.rs](crates/types/src/buffer.rs) incorpora `id` y `version` monotona para cada buffer.
  - Nota: la validacion completa contra buffers modificados queda preparada en la API, pero requiere una futura herramienta de edicion sobre buffers en memoria para usarse de extremo a extremo.

- Fase 5:
  - Se creo [crates/ai/tests/tool_eval.rs](crates/ai/tests/tool_eval.rs) con un proveedor de evaluacion controlado y 6 casos representativos.
  - El harness valida seleccion de `read_file`, `glob`, `grep`, `list_dir` y casos de respuesta directa sin herramientas.
  - `cargo test -p dca-ai tool_eval` pasa con precision >= 90%.

### Verificacion ejecutada

- `cargo check -p dca-ai`
- `cargo test -p dca-ai`
- `cargo test -p dca-ai tool_eval`

### Entorno

- Se instalo Rust mediante `winget` con `Rustlang.Rustup` para habilitar `cargo` en el entorno Windows de trabajo.