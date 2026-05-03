use async_trait::async_trait;
use color_eyre::Result;
use crate::provider::ToolDef;
use super::Tool;

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "glob".into(),
            description: "Busca archivos cuyas rutas coinciden con un patrón glob. Devuelve las rutas encontradas (máx 200).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "pattern": { "type": "string", "description": "Patrón glob (ej: src/**/*.rs)" },
                    "cwd": { "type": "string", "description": "Directorio base (default: CWD)" }
                }
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let pattern = args["pattern"].as_str()
            .ok_or_else(|| color_eyre::eyre::eyre!("Falta 'pattern'"))?;
        let cwd = args["cwd"].as_str().unwrap_or(".");

        let full_pattern = format!("{cwd}/{pattern}");
        let mut matches: Vec<String> = vec![];

        // Usamos walkdir + fnmatch-like via glob crate (no disponible, hacemos con find manual)
        // Implementación básica con tokio::fs::read_dir recursivo + glob matching simple
        collect_glob(&full_pattern, &mut matches).await;

        if matches.is_empty() {
            return Ok("No se encontraron archivos con ese patrón.".into());
        }

        matches.truncate(200);
        Ok(matches.join("\n"))
    }
}

/// Implementación glob simple usando `std::fs` sincrono en spawn_blocking
async fn collect_glob(pattern: &str, out: &mut Vec<String>) {
    let pattern = pattern.to_string();
    let result = tokio::task::spawn_blocking(move || {
        glob_walk(&pattern)
    }).await;

    if let Ok(paths) = result {
        out.extend(paths);
    }
}

fn glob_walk(pattern: &str) -> Vec<String> {
    // Separamos el directorio base del patrón
    let path = std::path::Path::new(pattern);
    let (base, pat) = split_glob(path);

    let mut out = vec![];
    walk_dir(std::path::Path::new(&base), &pat, &base, &mut out, 0);
    out
}

fn split_glob(path: &std::path::Path) -> (String, String) {
    let s = path.to_string_lossy().to_string();
    // Todo antes del primer '*' o '?' es el directorio base
    if let Some(pos) = s.find(|c| c == '*' || c == '?') {
        let dir_end = s[..pos].rfind('/').map(|p| p + 1).unwrap_or(0);
        (s[..dir_end].to_string(), s[dir_end..].to_string())
    } else {
        (s, String::new())
    }
}

fn matches_glob(path_str: &str, pattern: &str) -> bool {
    glob_match(pattern, path_str)
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_inner(&p, &t)
}

fn glob_match_inner(p: &[char], t: &[char]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        (Some('*'), _) if p.len() >= 2 && p[1] == '*' => {
            // ** matches any sequence including /
            let rest = &p[2..];
            if rest.is_empty() { return true; }
            for i in 0..=t.len() {
                if glob_match_inner(rest, &t[i..]) { return true; }
            }
            false
        }
        (Some('*'), _) => {
            // * matches anything except /
            let rest = &p[1..];
            for i in 0..=t.len() {
                if t[..i].contains(&'/') { break; }
                if glob_match_inner(rest, &t[i..]) { return true; }
            }
            false
        }
        (Some('?'), Some(tc)) if *tc != '/' => glob_match_inner(&p[1..], &t[1..]),
        (Some(pc), Some(tc)) if pc == tc => glob_match_inner(&p[1..], &t[1..]),
        _ => false,
    }
}

fn walk_dir(
    dir: &std::path::Path,
    pattern: &str,
    base: &str,
    out: &mut Vec<String>,
    depth: usize,
) {
    if depth > 20 { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().to_string();
        if path.is_dir() {
            walk_dir(&path, pattern, base, out, depth + 1);
        } else if matches_glob(&rel, pattern) {
            out.push(path.to_string_lossy().to_string());
            if out.len() >= 200 { return; }
        }
    }
}
