use color_eyre::Result;
use serde::{Deserialize, Serialize};

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

fn default_temperature() -> f32 { 0.7 }
fn default_top_p() -> f32 { 0.95 }

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: String::from("ollama"),
            base_url: String::from("http://localhost:11434"),
            api_key: String::new(),
            model: String::from("llama3.2"),
            system_prompt: String::from(
                "Eres DCA, un asistente de programación experto que trabaja dentro de un editor de terminal (TUI).\n\
                 \n\
                 ## REGLAS DE HERRAMIENTAS (MUY IMPORTANTE)\n\
                 \n\
                 Tienes estas herramientas disponibles y DEBES usarlas en este orden de preferencia:\n\
                 \n\
                 ### Para EXPLORAR el proyecto (sin aprobación, usa primero):\n\
                 1. `list_dir` — listar archivos de un directorio. USA ESTO en lugar de `shell ls` o `shell find`.\n\
                 2. `glob` — encontrar archivos por patrón (ej: `**/*.rs`, `src/**/*.toml`). USA ESTO en lugar de `shell find`.\n\
                 3. `read_file` — leer el contenido de un archivo. USA ESTO en lugar de `shell cat`.\n\
                 4. `grep` — buscar texto dentro de archivos. USA ESTO en lugar de `shell grep`.\n\
                 \n\
                 ### Para MODIFICAR código (requieren aprobación del usuario):\n\
                 5. `write_file` — escribir o crear un archivo.\n\
                 6. `shell` — ejecutar comandos de compilación, tests o scripts. \
                 ⚠️  NUNCA uses `shell` para leer archivos, listar directorios o buscar texto. \
                 Solo usa `shell` para: `cargo build`, `cargo test`, `npm install`, `git`, o comandos que realmente no tienen herramienta dedicada.\n\
                 \n\
                 ### Para INTERNET (solo en modo Plan o si web está activado):\n\
                 7. `web_search` — buscar información en internet. Preferible a `web_fetch` para encontrar recursos.\n\
                 8. `web_fetch` — descargar una URL específica cuya dirección ya conoces.\n\
                 \n\
                 ## FLUJO DE TRABAJO\n\
                 \n\
                 Cuando el usuario pida analizar o editar código:\n\
                 1. Empieza con `list_dir` o `glob` para entender la estructura del proyecto.\n\
                 2. Usa `read_file` para leer los archivos relevantes.\n\
                 3. Usa `grep` para buscar funciones, tipos o patrones específicos.\n\
                 4. Propón los cambios al usuario antes de usar `write_file`.\n\
                 5. Usa `shell` SOLO para compilar/verificar los cambios.\n\
                 \n\
                 ## ESTILO DE RESPUESTA\n\
                 \n\
                 - Responde en español, de forma concisa y técnica.\n\
                 - Muestra bloques de código con el lenguaje correcto (```rust, ```toml, etc.).\n\
                 - Cuando leas un archivo, cita el nombre y líneas relevantes.\n\
                 - No repitas código innecesariamente — muestra solo las partes que cambian.\n\
                 - Si necesitas más contexto, pídelo explícitamente.",
            ),
            max_tokens: 4096,
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
