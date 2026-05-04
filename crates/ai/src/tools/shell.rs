use async_trait::async_trait;
use color_eyre::Result;
use crate::provider::ToolDef;
use super::Tool;

pub struct ShellTool;

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
                          y no requieren confirmación.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": { "type": "string", "description": "Comando a ejecutar en /bin/sh" },
                    "cwd": { "type": "string", "description": "Directorio de trabajo (opcional)" },
                    "timeout_secs": { "type": "integer", "description": "Tiempo máximo en segundos (default 120, máx 600)" }
                }
            }),
        }
    }

    fn requires_approval(&self) -> bool { true }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let command = args["command"].as_str()
            .ok_or_else(|| color_eyre::eyre::eyre!("Falta 'command'"))?;
        let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(120).min(600);

        let mut cmd = tokio::process::Command::new("/bin/sh");
        cmd.arg("-c").arg(command)
           .stdout(std::process::Stdio::piped())
           .stderr(std::process::Stdio::piped());

        if let Some(cwd) = args["cwd"].as_str() {
            cmd.current_dir(cwd);
        }

        let child = cmd.spawn()
            .map_err(|e| color_eyre::eyre::eyre!("Error al lanzar shell: {e}"))?;

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            child.wait_with_output(),
        ).await
        .map_err(|_| color_eyre::eyre::eyre!("Timeout después de {timeout_secs}s"))?
        .map_err(|e| color_eyre::eyre::eyre!("Error esperando proceso: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit = output.status.code().unwrap_or(-1);

        let mut result = String::new();
        if !stdout.is_empty() { result.push_str(&stdout); }
        if !stderr.is_empty() {
            if !result.is_empty() { result.push('\n'); }
            result.push_str("[stderr]\n");
            result.push_str(&stderr);
        }
        result.push_str(&format!("\n[exit: {exit}]"));

        // Truncar salida muy larga para no saturar el contexto del modelo
        const MAX_OUTPUT: usize = 20 * 1024;
        if result.len() > MAX_OUTPUT {
            let truncated = &result[..MAX_OUTPUT];
            Ok(format!("{truncated}\n[... salida truncada a 20 KB ...]\n[exit: {exit}]"))
        } else {
            Ok(result)
        }
    }
}
