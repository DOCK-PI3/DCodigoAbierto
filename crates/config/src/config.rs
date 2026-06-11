use color_eyre::Result;
use serde::{Deserialize, Serialize};

use crate::skills::SkillsManager;
use crate::theme::Theme;

// ── Configuración del proveedor de IA ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Proveedor: "ollama" | "openai" | "anthropic" | "groq" | "openrouter" | "custom"
    pub provider: String,
    /// URL base del endpoint (sin trailing slash)
    pub base_url: String,
    /// API key (también leída de la variable de entorno DCA_AI_API_KEY)
    pub api_key: String,
    /// Modelo a usar (ej: "llama3.2", "gpt-4o", "claude-opus-4-5")
    pub model: String,
    /// Prompt del sistema
    pub system_prompt: String,
    /// Tokens máximos de respuesta
    pub max_tokens: u32,
    /// Temperatura del modelo (creatividad). Rango: 0.0 - 2.0
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Top-p (nucleus sampling). Rango: 0.0 - 1.0
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    /// Habilitar herramientas (function calling)
    pub tools_enabled: bool,
    /// Habilitar la herramienta web_fetch
    pub web_enabled: bool,
}

fn default_temperature() -> f32 {
    0.7
}
fn default_top_p() -> f32 {
    0.95
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: String::from("ollama"),
            base_url: String::from("http://localhost:11434"),
            api_key: String::new(),
            model: String::from("llama3.2"),
            // DCA-IA-IMPROVEMENT: Prompt estructurado con reglas de no-uso y few-shot.
            system_prompt: String::from(
                r#"[ROL]
Eres DCA, asistente de programacion en terminal para el editor DCodigoAbierto.

## CUANDO NO USAR HERRAMIENTAS
- Si el usuario solo pide explicacion o teoria: responde directamente sin herramientas.
- Si falta informacion critica como archivo, ruta o seleccion: pregunta antes de llamar una herramienta.
- Nunca uses `shell` para operaciones con herramienta dedicada como `list_dir`, `read_file`, `glob` o `grep`.
- Si el usuario dice "no uses herramientas" o "solo responde": respeta la instruccion.

## USO DE HERRAMIENTAS
Usa el sistema de function calling nativo de tu modelo para invocar herramientas.
Si tu modelo no soporta function calling nativo, usa este formato JSON exacto:
{
    "tool": "nombre_herramienta",
    "arguments": {
        "param1": "valor1",
        "param2": 123
    }
}
Reglas: usa doble comilla, sin comentarios, sin texto extra antes o despues del JSON.

## EJEMPLOS DE FLUJO CORRECTO
<user>Donde esta definida la funcion main?</user>
<assistant>
Voy a buscar archivos Rust que contengan main.
{"tool":"glob","arguments":{"pattern":"**/*.rs"}}
</assistant>
<tool_result>{"files":["src/main.rs","src/bin/cli.rs"]}</tool_result>
<assistant>
Leere src/main.rs para confirmar:
{"tool":"read_file","arguments":{"path":"src/main.rs","start_line":1,"end_line":50}}
</assistant>

<user>Explicame el borrow checker</user>
<assistant>
El borrow checker es el sistema de propiedad de Rust que garantiza seguridad de memoria sin GC.
</assistant>

## MANEJO DE ERRORES Y AUTOCORRECCION
Si una herramienta falla, recibiras un JSON con este formato:
{"error_type":"codigo","message":"descripcion","hint":"sugerencia","context":{}}
Reconoce el error, usa el `hint` para corregirlo o pregunta al usuario. Nunca reintentes exactamente igual.

## PRIORIDADES DE HERRAMIENTAS
1. Inspeccion: `read_file`, `glob`, `grep`, `list_dir`
2. Edicion: `write_file`, `apply_patch` siempre con confirmacion
3. Ejecucion: `run_lsp_command`, `shell` solo si no hay alternativa
4. Contexto: `get_buffer`, `get_selection`, `get_diagnostics`

## ESTILO DE RESPUESTA
- Responde en espanol, de forma concisa y tecnica.
- Si no necesitas herramientas, responde directamente.
- Si necesitas contexto adicional, pidelo antes de actuar.
"#,
            ),
            max_tokens: 8192,
            temperature: 0.7,
            top_p: 0.95,
            tools_enabled: true,
            web_enabled: true,
        }
    }
}

impl AiConfig {
    /// Devuelve la api_key efectiva: primero la variable de entorno, luego la config.
    pub fn effective_api_key(&self) -> String {
        std::env::var("DCA_AI_API_KEY").unwrap_or_else(|_| self.api_key.clone())
    }

    /// Construye el system prompt completo incluyendo skills instaladas.
    pub fn build_system_prompt(&self) -> String {
        let mut prompt = self.system_prompt.clone();

        // Añadir skills instaladas
        let skills_mgr = SkillsManager::new(SkillsManager::default_dir());
        if let Ok(skills_prompt) = skills_mgr.build_skills_prompt() {
            if !skills_prompt.is_empty() {
                prompt.push_str(&skills_prompt);
            }
        }

        prompt
    }
}

// ── Configuración principal de la aplicación ─────────────────────────────────

/// Configuración principal de la aplicación.
/// Se carga desde `~/.config/dca/config.toml` si existe,
/// o se usa la configuración por defecto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub tick_rate_ms: u64,
    pub theme: Theme,
    /// Comando del servidor LSP a usar (vacío = desactivado).
    /// Ejemplo: "rust-analyzer"
    pub lsp_server: String,
    /// Configuración del proveedor de IA.
    #[serde(default)]
    pub ai: AiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tick_rate_ms: 200,
            theme: Theme::default(),
            lsp_server: String::from("rust-analyzer"),
            ai: AiConfig::default(),
        }
    }
}

impl AppConfig {
    /// Intenta cargar la configuración desde disco.
    /// Si el archivo no existe o hay un error de parseo, devuelve la config por defecto.
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            match toml::from_str::<AppConfig>(&content) {
                Ok(cfg) => {
                    tracing::info!("Configuración cargada desde {:?}", config_path);
                    return Ok(cfg);
                }
                Err(e) => {
                    tracing::warn!(
                        "Error al parsear config.toml ({e}), usando configuración por defecto"
                    );
                }
            }
        } else {
            let cfg = AppConfig::default();
            cfg.save_default()?;
            return Ok(cfg);
        }

        Ok(AppConfig::default())
    }

    /// Escribe la configuración por defecto en disco, creando el directorio si es necesario.
    fn save_default(&self) -> Result<()> {
        let config_path = Self::config_path();
        if let Some(dir) = config_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        tracing::info!("Configuración por defecto escrita en {:?}", config_path);
        Ok(())
    }

    /// Devuelve la ruta canónica del archivo de configuración.
    pub fn config_path() -> std::path::PathBuf {
        dirs_next::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("dca")
            .join("config.toml")
    }

    /// Persiste únicamente el tema activo en config.toml.
    /// Si no puede leer/escribir, falla silenciosamente.
    pub fn save_theme(&self) -> Result<()> {
        let path = Self::config_path();
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
