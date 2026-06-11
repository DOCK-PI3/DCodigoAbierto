use super::{tool_parameters_schema, validate_tool_args, Tool};
use crate::provider::ToolDef;
use async_trait::async_trait;
use color_eyre::{eyre::eyre, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::io::BufRead;

pub struct GrepTool;

// DCA-IA-IMPROVEMENT: Argumentos tipados para grep.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrepArgs {
    /// Texto o patron a buscar.
    pub pattern: String,
    /// Archivo o directorio donde buscar.
    #[schemars(default = "default_search_path")]
    pub path: Option<String>,
    /// Si diferencia mayusculas/minusculas.
    #[schemars(default)]
    pub case_sensitive: Option<bool>,
    /// Filtro opcional de extension o patron de archivo.
    #[schemars(default)]
    pub include: Option<String>,
}

fn default_search_path() -> Option<String> {
    Some(".".to_string())
}

#[async_trait]
impl Tool for GrepTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "grep".into(),
            description: "Busca texto literal en archivos del proyecto. Devuelve las líneas que coinciden con su número y ruta (máx 100 resultados).".into(),
            parameters: tool_parameters_schema::<GrepArgs>(),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: GrepArgs = validate_tool_args("grep", args)?;
        if args.pattern.trim().is_empty() {
            return Err(eyre!("grep: 'pattern' no puede estar vacio"));
        }

        let pattern = args.pattern;
        let search_path = args.path.unwrap_or_else(|| ".".to_string());
        let case_sensitive = args.case_sensitive.unwrap_or(false);
        let include = args.include;

        let results = tokio::task::spawn_blocking(move || {
            grep_files(&pattern, &search_path, case_sensitive, include.as_deref())
        })
        .await??;

        if results.is_empty() {
            return Ok("No se encontraron coincidencias.".into());
        }

        Ok(results.join("\n"))
    }
}

fn grep_files(
    pattern: &str,
    path: &str,
    case_sensitive: bool,
    include: Option<&str>,
) -> Result<Vec<String>> {
    let re = if case_sensitive {
        regex_simple::compile(pattern)
    } else {
        regex_simple::compile_icase(pattern)
    };

    let mut results = vec![];
    let root = std::path::Path::new(path);
    if !root.exists() {
        return Err(eyre!("grep: la ruta '{}' no existe", path));
    }

    if root.is_file() {
        grep_file(root, &re, include, &mut results);
    } else {
        walk_grep(root, &re, include, &mut results, 0);
    }

    Ok(results)
}

fn grep_file(
    path: &std::path::Path,
    pattern: &regex_simple::Pattern,
    include: Option<&str>,
    out: &mut Vec<String>,
) {
    if out.len() >= 100 {
        return;
    }

    // Filtro de extensión
    if let Some(inc) = include {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !glob_ext_match(inc, &name) {
            return;
        }
    }

    // Solo archivos de texto (heurística: ignora binarios >1 MB)
    if let Ok(meta) = path.metadata() {
        if meta.len() > 1_048_576 {
            return;
        }
    }

    let Ok(f) = std::fs::File::open(path) else {
        return;
    };
    let reader = std::io::BufReader::new(f);

    for (i, line) in reader.lines().enumerate() {
        if out.len() >= 100 {
            break;
        }
        let Ok(line) = line else { break };
        if pattern.matches(&line) {
            out.push(format!("{}:{}: {}", path.display(), i + 1, line));
        }
    }
}

fn walk_grep(
    dir: &std::path::Path,
    pattern: &regex_simple::Pattern,
    include: Option<&str>,
    out: &mut Vec<String>,
    depth: usize,
) {
    if depth > 15 || out.len() >= 100 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Ignorar directorios ocultos y target/
        if let Some(name) = path.file_name() {
            let n = name.to_string_lossy();
            if n.starts_with('.') || n == "target" {
                continue;
            }
        }
        if path.is_dir() {
            walk_grep(&path, pattern, include, out, depth + 1);
        } else {
            grep_file(&path, pattern, include, out);
        }
    }
}

fn glob_ext_match(pattern: &str, name: &str) -> bool {
    if let Some(ext) = pattern.strip_prefix("*.") {
        name.ends_with(&format!(".{ext}"))
    } else {
        name == pattern
    }
}

// ── Regex mínimo (sin crate regex para no añadir dependencia) ────────────────
mod regex_simple {
    pub struct Pattern {
        text: String,
        icase: bool,
    }

    pub fn compile(p: &str) -> Pattern {
        Pattern {
            text: p.to_string(),
            icase: false,
        }
    }

    pub fn compile_icase(p: &str) -> Pattern {
        Pattern {
            text: p.to_lowercase(),
            icase: true,
        }
    }

    impl Pattern {
        pub fn matches(&self, line: &str) -> bool {
            if self.icase {
                line.to_lowercase().contains(&self.text)
            } else {
                line.contains(&self.text)
            }
        }
    }
}
