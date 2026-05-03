use async_trait::async_trait;
use color_eyre::Result;
use crate::provider::ToolDef;
use super::Tool;

pub struct ListDirTool;

#[async_trait]
impl Tool for ListDirTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "list_dir".into(),
            description: "Lista el contenido de un directorio (archivos y subcarpetas). \
                          Úsalo en lugar de 'shell ls' para explorar el proyecto sin pedir aprobación. \
                          Devuelve nombre, tipo (file/dir) y tamaño de cada entrada.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Ruta del directorio a listar (default: '.' = CWD)"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Niveles de profundidad a mostrar (1 = solo el directorio raíz, default: 1, máx: 3)"
                    }
                }
            }),
        }
    }

    // No requiere aprobación — solo lectura
    fn requires_approval(&self) -> bool { false }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let path  = args["path"].as_str().unwrap_or(".").to_string();
        let depth = args["depth"].as_u64().unwrap_or(1).min(3) as usize;

        let result = tokio::task::spawn_blocking(move || {
            list_recursive(std::path::Path::new(&path), depth, 0)
        }).await??;

        if result.is_empty() {
            return Ok(format!("El directorio '{}' está vacío.", args["path"].as_str().unwrap_or(".")));
        }

        Ok(result)
    }
}

fn list_recursive(
    dir: &std::path::Path,
    max_depth: usize,
    current_depth: usize,
) -> Result<String> {
    let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)
        .map_err(|e| color_eyre::eyre::eyre!("No se pudo leer '{}': {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .collect();

    // Ordenar: directorios primero, luego archivos, ambos alfabéticos
    entries.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    let indent = "  ".repeat(current_depth);
    let mut out = String::new();
    let mut count = 0;

    for entry in &entries {
        count += 1;
        if count > 200 { 
            out.push_str(&format!("{indent}[… y {} más]\n", entries.len() - 199));
            break;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        // Ignorar directorios ocultos y temporales comunes
        if name.starts_with('.') && current_depth == 0 
           && (name == ".git" || name == ".cache" || name == "node_modules") {
            continue;
        }

        let path = entry.path();
        if path.is_dir() {
            out.push_str(&format!("{indent}📁 {name}/\n"));
            if current_depth + 1 < max_depth {
                if let Ok(sub) = list_recursive(&path, max_depth, current_depth + 1) {
                    out.push_str(&sub);
                }
            }
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let size_str = format_size(size);
            out.push_str(&format!("{indent}📄 {name}  ({size_str})\n"));
        }
    }

    Ok(out)
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{bytes} B")
    }
}
