use super::{tool_parameters_schema, validate_tool_args, Tool};
use crate::provider::ToolDef;
use async_trait::async_trait;
use color_eyre::{eyre::eyre, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Encuentra el límite de carácter UTF-8 más cercano hacia atrás desde `pos`.
fn find_char_boundary(s: &str, pos: usize) -> usize {
    let mut p = pos.min(s.len());
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

pub struct ShellTool;

// DCA-IA-IMPROVEMENT: Argumentos tipados para shell.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShellArgs {
    /// Comando a ejecutar.
    pub command: String,
    /// Directorio de trabajo opcional.
    #[schemars(default)]
    pub cwd: Option<String>,
    /// Timeout maximo en segundos.
    #[schemars(default = "default_shell_timeout")]
    pub timeout_secs: Option<u64>,
}

fn default_shell_timeout() -> Option<u64> {
    Some(120)
}

#[async_trait]
impl Tool for ShellTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "shell".into(),
            description: "Ejecuta un comando de shell arbitrario. \
                          ⚠️  REQUIERE APROBACIÓN DEL USUARIO — úsalo SOLO para: \
                          compilar (cargo build), ejecutar tests (cargo test), instalar paquetes, \
                          o comandos que NO se pueden hacer con otras herramientas. \
                          NUNCA uses shell para: leer archivos (usa read_file), \
                          listar directorios (usa list_dir), buscar texto (usa grep), \
                          encontrar archivos (usa glob). Esas herramientas son más rápidas \
                          y no requieren confirmación."
                .into(),
            parameters: tool_parameters_schema::<ShellArgs>(),
        }
    }

    fn requires_approval(&self) -> bool {
        true
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: ShellArgs = validate_tool_args("shell", args)?;
        if args.command.trim().is_empty() {
            return Err(eyre!("shell: 'command' no puede estar vacio"));
        }

        let timeout_secs = args.timeout_secs.unwrap_or(120).min(600);

        // Platform-aware shell: cmd /c on Windows, /bin/sh -c on Unix
        let (shell, flag) = if cfg!(windows) {
            ("cmd", "/c")
        } else {
            ("/bin/sh", "-c")
        };
        let mut cmd = tokio::process::Command::new(shell);
        cmd.arg(flag)
            .arg(&args.command)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(cwd) = args.cwd.as_deref() {
            let cwd_path = std::path::Path::new(cwd);
            if !cwd_path.is_dir() {
                return Err(eyre!(
                    "shell: cwd '{}' no existe o no es un directorio",
                    cwd
                ));
            }
            cmd.current_dir(cwd);
        }

        let child = cmd
            .spawn()
            .map_err(|error| eyre!("shell: error al lanzar proceso: {}", error))?;

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| eyre!("shell: timeout despues de {}s", timeout_secs))?
        .map_err(|error| eyre!("shell: error esperando proceso: {}", error))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit = output.status.code().unwrap_or(-1);

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("[stderr]\n");
            result.push_str(&stderr);
        }
        result.push_str(&format!("\n[exit: {exit}]"));

        // Truncar salida muy larga para no saturar el contexto del modelo
        const MAX_OUTPUT: usize = 20 * 1024;
        if result.len() > MAX_OUTPUT {
            // Mostrar inicio y final para no perder contexto importante.
            // Usar límites de carácter UTF-8 para no cortar a mitad de un carácter.
            let head_bound = find_char_boundary(&result, MAX_OUTPUT / 2);
            let raw_tail = find_char_boundary(&result, result.len().saturating_sub(MAX_OUTPUT / 2));
            // Evitar solapamiento head/tail y asegurar límite de carácter UTF-8
            let tail_start = find_char_boundary(&result, raw_tail.max(head_bound + 1024));
            let head = &result[..head_bound];
            let tail = &result[tail_start..];
            Ok(format!(
                "{head}\n[... {:.1} KB omitidos ...]\n{tail}\n[exit: {exit}]",
                (result.len() - MAX_OUTPUT) as f64 / 1024.0
            ))
        } else {
            Ok(result)
        }
    }
}
