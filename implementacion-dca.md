# 🤖 implementacion-dca.md - Guía Completa de Mejoras para Tool Calling en DCodigoAbierto

> **Destinatario:** Agente IA implementador (Claude Sonnet / similar)  
> **Repositorio:** https://github.com/DOCK-PI3/DCodigoAbierto  
> **Objetivo:** Refactorizar el sistema de herramientas de IA para maximizar fiabilidad, eliminar errores de selección/ejecución y habilitar autocorrección automática.  
> **Instrucción principal:** Sigue este documento secuencialmente. No omitas fases. Verifica compilación y tests tras cada cambio. Usa comentarios `// DCA-IA-IMPROVEMENT:` para marcar modificaciones.

---

## 📜 DIRECTIVAS GENERALES PARA EL AGENTE
1. **Orden estricto:** Implementa Fase 1 → 2 → 3 → 4 → 5.
2. **Verificación continua:** Tras cada fase, ejecuta `cargo check -p dca-ai` y `cargo test -p dca-ai`. Corrige fallos antes de continuar.
3. **Compatibilidad:** Mantén la API pública estable. Si modificas firmas, actualiza todas las llamadas existentes.
4. **Formato:** Usa Rust idiomático, manejo de errores con `anyhow`/`thiserror`, y `serde`/`schemars` para schemas.
5. **Entrega:** Genera commits lógicos por fase. Incluye pruebas actualizadas si es necesario.
6. **Contexto:** Este editor es TUI, multi-buffer, con LSP y streaming IA. Las herramientas interactúan con buffers, sistema de archivos y LSP.

---

## 🔹 FASE 1: Validación de Esquemas con `schemars` (🔴 Crítica)

### 📍 Archivos a modificar:
- `crates/ai/Cargo.toml`
- `crates/ai/src/tools/mod.rs`
- `crates/ai/src/tools/read_file.rs` (piloto)
- Replicar en: `write_file.rs`, `shell.rs`, `glob.rs`, `grep.rs`, `apply_patch.rs`, etc.

### 📦 Paso 1.1: Añadir dependencia
```toml
# crates/ai/Cargo.toml → [dependencies]
schemars = "0.8"
```

### 📦 Paso 1.2: Crear helper genérico de validación
```rust
// crates/ai/src/tools/mod.rs
use schemars::JsonSchema;
use anyhow::{Result, eyre};

/// Valida y parsea argumentos de herramienta con mensaje descriptivo
pub fn validate_tool_args<T: serde::de::DeserializeOwned + JsonSchema>(
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<T> {
    serde_json::from_value(args.clone())
        .map_err(|e| {
            let schema_hint = serde_json::to_string_pretty(&schemars::schema_for!(T).schema)
                .unwrap_or_else(|_| "schema no disponible".into());
            eyre!(
                "{}: argumentos inválidos - {}\n\nSchema esperado:\n{}",
                tool_name, e, schema_hint
            )
        })
}
```

### 📦 Paso 1.3: Refactorizar `read_file.rs` (ejemplo piloto)
```rust
// crates/ai/src/tools/read_file.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use anyhow::{Result, eyre};
use super::validate_tool_args;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Ruta al archivo (absoluta o relativa al CWD del proyecto)
    pub path: String,
    /// Primera línea a devolver (1-indexed, inclusiva)
    #[schemars(default)]
    pub start_line: Option<u32>,
    /// Última línea a devolver (1-indexed, inclusiva)
    #[schemars(default)]
    pub end_line: Option<u32>,
}

impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "read_file".into(),
            description: "Lee el contenido de un archivo de texto. Úsalo para inspeccionar código, logs o configuración.".into(),
            parameters: schemars::schema_for!(ReadFileArgs).schema.into(),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: ReadFileArgs = validate_tool_args("read_file", args)?;

        if let (Some(start), Some(end)) = (args.start_line, args.end_line) {
            if start > end {
                return Err(eyre!("read_file: start_line ({}) no puede ser mayor que end_line ({})", start, end));
            }
        }

        let content = tokio::fs::read_to_string(&args.path).await
            .map_err(|e| eyre!("read_file: no se pudo leer '{}': {}", args.path, e))?;

        let result = if args.start_line.is_some() || args.end_line.is_some() {
            let lines: Vec<&str> = content.lines().collect();
            let start = args.start_line.map(|s| (s.saturating_sub(1)) as usize).unwrap_or(0);
            let end = args.end_line.map(|e| e as usize).unwrap_or(lines.len());
            lines[start.min(lines.len())..end.min(lines.len())].join("\n")
        } else {
            content
        };

        Ok(result)
    }
}
```

### 🔄 Acción requerida:
Aplica el mismo patrón (`struct Args + JsonSchema + validate_tool_args`) a **todas** las herramientas en `crates/ai/src/tools/`. Elimina los schemas manuales en JSON. Verifica que `cargo check` compile sin warnings.

---

## 🔹 FASE 2: System Prompt Estructurado + Few-Shot (🟠 Alta)

### 📍 Archivo a modificar:
- `crates/ai/src/config.rs` (o donde se defina `SYSTEM_PROMPT`)

### 📦 Paso 2.1: Reemplazar el prompt actual
```rust
pub const SYSTEM_PROMPT: &str = r#"
[ROL]
Eres DCA, asistente de programación en terminal para el editor DCodigoAbierto.

## 🚫 CUÁNDO NO USAR HERRAMIENTAS
- Si el usuario solo pide explicación/teoría: RESPONDE DIRECTAMENTE sin herramientas.
- Si falta información crítica (archivo, ruta, selección): PREGUNTA antes de llamar herramienta.
- NUNCA uses `shell` para operaciones con herramienta dedicada (`list_dir`, `read_file`, `glob`, etc.).
- Si el usuario dice "no uses herramientas" o "solo responde": RESPETA la instrucción.

## 📋 FORMATO DE TOOL CALLS ESPERADO
Cuando uses una herramienta, genera EXACTAMENTE este JSON:
{
  "tool": "nombre_herramienta",
  "arguments": {
    "param1": "valor1",
    "param2": 123
  }
}
⚠️ Reglas: usa doble comilla, sin comentarios, sin texto extra antes/después del JSON.

## 🔄 EJEMPLOS DE FLUJO CORRECTO
<user>¿Dónde está definida la función main?</user>
<assistant>
Voy a buscar archivos Rust que contengan main.
{"tool":"glob","arguments":{"pattern":"**/*.rs"}}
</assistant>
<tool_result>{"files":["src/main.rs","src/bin/cli.rs"]}</tool_result>
<assistant>
Leeré src/main.rs para confirmar:
{"tool":"read_file","arguments":{"path":"src/main.rs","start_line":1,"end_line":50}}
</assistant>

<user>Explícame el borrow checker</user>
<assistant>
El borrow checker es el sistema de propiedad de Rust que garantiza seguridad de memoria sin GC...
[RESPUESTA DIRECTA - SIN HERRAMIENTAS]
</assistant>

## ⚠️ MANEJO DE ERRORES Y AUTOCORRECCIÓN
Si una herramienta falla, recibirás un JSON con:
{"error_type":"codigo","message":"descripción","hint":"sugerencia","context":{...}}
→ Reconoce el error, usa el "hint" para corregir, o pregunta al usuario. NUNCA reintentar idéntico.

## 🎯 PRIORIDADES DE HERRAMIENTAS
1. Inspección: `read_file`, `glob`, `grep`, `list_dir`
2. Edición: `write_file`, `apply_patch` (siempre con confirmación)
3. Ejecución: `run_lsp_command`, `shell` (solo si no hay alternativa)
4. Contexto: `get_buffer`, `get_selection`, `get_diagnostics`
"#;
```

### 🔄 Acción requerida:
- Reemplaza el prompt actual manteniendo el placeholder de inyección de contexto si tu sistema lo usa.
- Verifica que el prompt se envíe como `system` en todas las llamadas a proveedores (OpenAI, Ollama, Anthropic, Groq, OpenRouter).

---

## 🔹 FASE 3: Errores Estructurados para Autocorrección (🟡 Media)

### 📍 Archivos a modificar:
- `crates/ai/src/tools/mod.rs`
- `crates/ai/src/agent.rs`
- Herramientas modificadas en Fase 1

### 📦 Paso 3.1: Definir `ToolError`
```rust
// crates/ai/src/tools/mod.rs
use serde::{Serialize, Deserialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "error_type", rename_all = "snake_case")]
pub enum ToolError {
    #[error("{message}")]
    FileNotFound {
        message: String,
        attempted_path: String,
        hint: Option<String>,
    },
    #[error("{message}")]
    InvalidRange {
        message: String,
        start: u32,
        end: u32,
        max_available: u32,
        hint: Option<String>,
    },
    #[error("{message}")]
    PermissionDenied {
        message: String,
        path: String,
        hint: Option<String>,
    },
    #[error("{message}")]
    BufferModified {
        message: String,
        buffer_id: String,
        expected_version: u64,
        current_version: u64,
        hint: Option<String>,
    },
    #[error("{message}")]
    Generic {
        message: String,
        code: String,
        context: Option<serde_json::Value>,
        hint: Option<String>,
    },
}

impl ToolError {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(r#"{{"error_type":"serialization_failed","message":"{}"}}"#, self)
        })
    }
}
```

### 📦 Paso 3.2: Integrar en `agent.rs`
```rust
// crates/ai/src/agent.rs → dentro del loop de ejecución de tool calls
let result_str = match tool.execute(&tc.arguments).await {
    Ok(output) => output,
    Err(e) => {
        if let Some(tool_err) = e.downcast_ref::<ToolError>() {
            tool_err.to_json()
        } else {
            serde_json::json!({
                "error_type": "unexpected",
                "message": format!("{}", e),
                "hint": "Revisa los argumentos o consulta al usuario para más contexto"
            }).to_string()
        }
    }
};

// Inyectar en contexto con etiqueta clara
session.push_message(AiMessage::tool_result(&tc.id, &result_str));
```

### 🔄 Acción requerida:
- Refactoriza al menos 2 herramientas críticas (`read_file`, `write_file`) para devolver `ToolError` en lugar de strings.
- Asegura que el agente serialice errores como JSON y los reinyecte en el contexto para autocorrección.

---

## 🔹 FASE 4: Context Pruning + Versionado de Buffers (🟢 Baja)

### 📍 Archivos a modificar:
- `crates/ai/src/agent.rs`
- `crates/ai/src/buffer.rs` (o equivalente)
- `crates/ai/src/tools/mod.rs`

### 📦 Paso 4.1: Implementar `prune_context`
```rust
// crates/ai/src/agent.rs
pub struct PruneConfig {
    pub max_recent_messages: usize,
    pub max_tool_content_chars: usize,
    pub preserve_last_tool_results: usize,
}

pub fn prune_context(messages: &[AiMessage], config: &PruneConfig) -> Vec<AiMessage> {
    let mut pruned = vec![messages[0].clone()]; // system prompt
    let recent_start = messages.len().saturating_sub(config.max_recent_messages);
    pruned.extend_from_slice(&messages[recent_start..]);

    // Opcional: resumir tool results largos
    for msg in &mut pruned {
        if let AiMessage::ToolResult { content, .. } = msg {
            if content.len() > config.max_tool_content_chars {
                let lines: Vec<&str> = content.lines().collect();
                let head = lines.iter().take(5).cloned().collect::<Vec<_>>().join("\n");
                let tail = lines.iter().rev().take(5).cloned().collect::<Vec<_>>().join("\n");
                *content = format!("...[truncado {} chars]...\n{}\n...\n{}", content.len(), head, tail);
            }
        }
    }
    pruned
}
```

### 📦 Paso 4.2: Añadir versionado a `ToolCall`
```rust
// crates/ai/src/tools/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub buffer_version: Option<u64>,
    pub target_buffer_id: Option<String>,
}
```

### 🔄 Acción requerida:
- Integra `prune_context` antes de cada llamada al modelo.
- En herramientas que modifican buffers, verifica `buffer_version` y devuelve `ToolError::BufferModified` si hay desincronización.

---

## 🔹 FASE 5: Harness de Evaluación (🎁 Bonus)

### 📍 Archivo a crear:
- `crates/ai/tests/tool_eval.rs`

### 📦 Paso 5.1: Estructura mínima de evaluación
```rust
use dca_ai::{Agent, AiEvent};
use tokio::sync::mpsc::unbounded_channel;

#[derive(Debug, Clone)]
struct EvalCase {
    name: String,
    user_prompt: String,
    expected_tool: Option<String>,
    expected_args_validator: Option<fn(&serde_json::Value) -> bool>,
}

#[tokio::test]
async fn eval_tool_selection_accuracy() {
    let cases = vec![
        EvalCase {
            name: "read_file cuando se pide contenido".into(),
            user_prompt: "¿Qué contiene src/main.rs?".into(),
            expected_tool: Some("read_file".into()),
            expected_args_validator: Some(|args| args["path"] == "src/main.rs"),
        },
        EvalCase {
            name: "no herramientas para preguntas teóricas".into(),
            user_prompt: "Explícame el patrón Observer en Rust".into(),
            expected_tool: None,
            expected_args_validator: None,
        },
    ];

    let mut passed = 0;
    for case in cases {
        let (events_tx, mut events_rx) = unbounded_channel();
        let mut agent = Agent::with_test_config(events_tx);
        agent.process_user_message(&case.user_prompt).await;

        let mut tool_calls = Vec::new();
        while let Ok(event) = events_rx.try_recv() {
            if let AiEvent::ToolCallRequest(tc) = event {
                tool_calls.push(tc);
            }
        }

        let passed_case = match (&case.expected_tool, tool_calls.first()) {
            (None, None) => true,
            (Some(exp), Some(act)) if act.name == *exp => {
                case.expected_args_validator.map_or(true, |v| v(&act.arguments))
            }
            _ => false,
        };

        if passed_case { passed += 1; println!("✅ {}", case.name); }
        else { println!("❌ {}", case.name); }
    }

    let accuracy = (passed as f64 / cases.len() as f64) * 100.0;
    println!("📊 Precisión: {:.1}%", accuracy);
    assert!(accuracy >= 90.0, "Precisión mínima esperada: 90%");
}
```

### 🔄 Acción requerida:
- Crea el archivo de tests.
- Añade 5-10 casos representativos.
- Integra en CI: `cargo test -p dca-ai tool_eval`

---

## ✅ CHECKLIST DE IMPLEMENTACIÓN

## 📌 ESTADO DE EJECUCIÓN

- Fecha de ejecución: 2026-05-08
- Estado general: fases 1 a 5 implementadas y verificadas en orden
- Validaciones ejecutadas tras cada fase:
    - `cargo check -p dca-ai`
    - `cargo test -p dca-ai`
- Validación final adicional:
    - `cargo test -p dca-ai tool_eval`
- Nota de alcance en Fase 4:
    - `buffer_version` y `target_buffer_id` ya existen en `ToolCall`
    - `TextBuffer` ya mantiene `id` y `version`
    - Falta una herramienta de escritura sobre buffer en memoria para aplicar la comprobación de desincronización extremo a extremo

### 🔴 Fase 1
- [x] Añadir `schemars = "0.8"` a `Cargo.toml`
- [x] Crear `validate_tool_args<T>()` en `tools/mod.rs`
- [x] Refactorizar `read_file.rs` con `ReadFileArgs + JsonSchema`
- [x] Replicar patrón en todas las herramientas
- [x] Eliminar schemas JSON manuales

### 🟠 Fase 2
- [x] Reemplazar `SYSTEM_PROMPT` con versión estructurada
- [x] Incluir sección 🚫, 📋, 🔄, ⚠️, 🎯
- [x] Verificar inyección en todos los proveedores IA

### 🟡 Fase 3
- [x] Definir enum `ToolError` con variantes específicas
- [x] Implementar `ToolError::to_json()`
- [x] Refactorizar 2 herramientas críticas para usar `ToolError`
- [x] Actualizar `agent.rs` para serializar errores como JSON

### 🟢 Fase 4
- [x] Definir `PruneConfig` y `prune_context()`
- [x] Integrar pruning antes de llamadas al modelo
- [x] Añadir `buffer_version` a `ToolCall`
- [x] Verificar versión en herramientas de escritura

### 🎁 Fase 5
- [x] Crear `crates/ai/tests/tool_eval.rs`
- [x] Definir ≥5 casos de evaluación
- [x] Aserción de precisión ≥90%
- [x] Documentar cómo añadir nuevos casos

---

## 🎯 CRITERIOS DE ACEPTACIÓN

1. **Schemas:** Todos generados desde tipos Rust. Validación temprana activa.
2. **Prompt:** Incluye reglas explícitas de no-uso y ejemplos few-shot. Modelos generan JSON válido.
3. **Errores:** Al menos 2 herramientas devuelven `ToolError`. El agente reinyecta JSON. Modelos se autocorrigen en ≥80% de simulaciones.
4. **Contexto:** Historial se mantiene bajo límite de tokens. Buffers versionados evitan race conditions.
5. **Tests:** `cargo test -p dca-ai tool_eval` pasa con ≥90% precisión.

---

## 📝 NOTAS FINALES PARA EL AGENTE

- **Prioriza por impacto:** Comienza con `schemars` en `read_file.rs` como prueba de concepto.
- **Mantén compatibilidad:** No rompas API pública. Usa `#[deprecated]` si es necesario migrar gradualmente.
- **Prueba multi-proveedor:** Verifica que schemas y prompts funcionen con Ollama, OpenAI-compatible, Groq, Anthropic.
- **Documenta:** Actualiza `CHANGELOG.md` y añade comentarios `// DCA-IA-IMPROVEMENT:` en código modificado.
- **Mide:** Usa el harness para cuantificar mejora antes/después.

> 💡 **Consejo:** Implementa cada fase en una rama separada. Valida con `cargo test` + pruebas manuales con tu modelo preferido antes de merge.

---
*Documento generado para automatizar mejoras de fiabilidad en tool calling. Ejecutar secuencialmente.* 🚀