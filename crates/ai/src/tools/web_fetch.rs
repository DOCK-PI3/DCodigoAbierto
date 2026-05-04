use async_trait::async_trait;
use color_eyre::Result;
use crate::provider::ToolDef;
use super::Tool;

// ── WebFetchTool ──────────────────────────────────────────────────────────────

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "web_fetch".into(),
            description: "Descarga una URL y devuelve el texto limpio (sin HTML). \
                          Úsalo para leer documentación, artículos o páginas web. \
                          Para BUSCAR en internet usa web_search en su lugar.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL completa a descargar (http:// o https://)"
                    }
                }
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let url = args["url"].as_str()
            .ok_or_else(|| color_eyre::eyre::eyre!("Falta 'url'"))?;

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(color_eyre::eyre::eyre!("Solo se permiten URLs http/https"));
        }

        let text = fetch_and_clean(url).await?;
        Ok(text)
    }
}

// ── WebSearchTool ─────────────────────────────────────────────────────────────

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "web_search".into(),
            description: "Busca en internet usando DuckDuckGo. Devuelve los primeros resultados \
                          (título + URL + fragmento). Úsalo cuando necesites información actualizada, \
                          ejemplos de código, o documentación de librerías.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Consulta de búsqueda en lenguaje natural o términos técnicos"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Número máximo de resultados (default: 8, máx: 15)"
                    }
                }
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let query = args["query"].as_str()
            .ok_or_else(|| color_eyre::eyre::eyre!("Falta 'query'"))?;
        let max = args["max_results"].as_u64().unwrap_or(8).min(15) as usize;

        let results = ddg_search(query, max).await?;
        if results.is_empty() {
            return Ok("No se encontraron resultados para esa búsqueda.".into());
        }
        Ok(results)
    }
}

// ── Implementación ────────────────────────────────────────────────────────────

async fn fetch_and_clean(url: &str) -> Result<String> {
    let client = build_client()?;
    let resp = client.get(url)
        .send().await
        .map_err(|e| color_eyre::eyre::eyre!("Error al conectar con {url}: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(color_eyre::eyre::eyre!("HTTP {} para {url}", status));
    }

    let bytes = resp.bytes().await
        .map_err(|e| color_eyre::eyre::eyre!("Error leyendo respuesta: {e}"))?;

    // HTML parsing es CPU-intensivo — hacerlo en spawn_blocking para no bloquear el runtime
    let url_owned = url.to_string();
    let text = tokio::task::spawn_blocking(move || {
        let raw = String::from_utf8_lossy(&bytes);
        let is_html = raw.contains("<html") || raw.contains("<!DOCTYPE") || raw.contains("<body");
        let text = if is_html { html_to_text(&raw) } else { raw.into_owned() };

        const MAX: usize = 40 * 1024;
        if text.len() > MAX {
            format!("{}\n\n[Contenido truncado a 40 KB — URL: {url_owned}]", &text[..MAX])
        } else {
            format!("{text}\n\n[Fuente: {url_owned}]")
        }
    }).await
    .map_err(|e| color_eyre::eyre::eyre!("Error en spawn_blocking: {e}"))?;

    Ok(text)
}

async fn ddg_search(query: &str, max: usize) -> Result<String> {
    // DuckDuckGo Lite (sin JS, sin API key)
    let encoded = url_encode(query);
    let search_url = format!("https://lite.duckduckgo.com/lite/?q={encoded}");

    let client = build_client()?;
    let resp = client.get(&search_url)
        .send().await
        .map_err(|e| color_eyre::eyre::eyre!("Error buscando en DuckDuckGo: {e}"))?;

    if !resp.status().is_success() {
        return Err(color_eyre::eyre::eyre!("DuckDuckGo devolvió HTTP {}", resp.status()));
    }

    let bytes = resp.bytes().await?;
    let query_owned = query.to_string();

    // Parsear en spawn_blocking — el HTML de DDG puede ser grande
    let result = tokio::task::spawn_blocking(move || {
        let html = String::from_utf8_lossy(&bytes);
        let results = parse_ddg_lite(&html, max);
        if results.is_empty() {
            let text = html_to_text(&html);
            let excerpt: String = text.lines()
                .filter(|l| !l.trim().is_empty())
                .take(40)
                .collect::<Vec<_>>()
                .join("\n");
            format!("Resultados de búsqueda para «{query_owned}»:\n\n{excerpt}")
        } else {
            format!("Resultados de búsqueda para «{query_owned}»:\n\n{results}")
        }
    }).await
    .map_err(|e| color_eyre::eyre::eyre!("Error en spawn_blocking: {e}"))?;

    Ok(result)
}

/// Extrae resultados de la versión lite de DuckDuckGo.
/// El HTML tiene la estructura: <a class="result-link" href="...">título</a>
/// seguido de un <td class="result-snippet">fragmento</td>
fn parse_ddg_lite(html: &str, max: usize) -> String {
    let mut results = vec![];

    // Buscar enlaces de resultado
    let mut pos = 0;
    while results.len() < max {
        // Buscar href dentro de result-link
        let Some(link_start) = html[pos..].find("class=\"result-link\"") else { break };
        let abs = pos + link_start;

        // Retroceder para encontrar el href en el mismo <a>
        let tag_start = html[..abs].rfind('<').unwrap_or(abs);
        let tag_end = html[abs..].find('>').map(|p| abs + p + 1).unwrap_or(abs + 1);
        let tag = &html[tag_start..tag_end];

        let url = extract_attr(tag, "href").unwrap_or_default();
        // DDG lite wraps URLs as /lite/?uddg=<encoded>
        let real_url = if url.contains("uddg=") {
            url.split("uddg=").nth(1)
                .and_then(|s| s.split('&').next())
                .map(|s| url_decode(s))
                .unwrap_or(url.clone())
        } else {
            url.clone()
        };

        // Título: texto dentro del <a>
        let title = if tag_end < html.len() {
            let close = html[tag_end..].find("</a>").map(|p| tag_end + p).unwrap_or(tag_end);
            strip_tags(&html[tag_end..close])
        } else {
            String::new()
        };

        // Snippet: buscar la siguiente celda result-snippet
        let snippet = if tag_end < html.len() {
            if let Some(snip_start) = html[tag_end..].find("result-snippet") {
                let abs_snip = tag_end + snip_start;
                let cell_content_start = html[abs_snip..].find('>').map(|p| abs_snip + p + 1).unwrap_or(abs_snip);
                let cell_content_end  = html[cell_content_start..].find("</td>").map(|p| cell_content_start + p).unwrap_or(cell_content_start + 200);
                strip_tags(&html[cell_content_start..cell_content_end.min(html.len())])
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if !real_url.is_empty() && !title.is_empty() {
            let title_clean = title.trim().to_string();
            let snippet_clean = snippet.trim().to_string();
            if !title_clean.is_empty() {
                results.push(format!("**{title_clean}**\n{real_url}\n{snippet_clean}"));
            }
        }

        pos = abs + 20;
    }

    results.join("\n\n")
}

// ── HTML → texto limpio ───────────────────────────────────────────────────────

fn html_to_text(html: &str) -> String {
    // 1. Eliminar bloques <script> y <style> completos
    let no_script = remove_tag_blocks(html, "script");
    let no_style  = remove_tag_blocks(&no_script, "style");
    let no_noscript = remove_tag_blocks(&no_style, "noscript");
    let no_nav    = remove_tag_blocks(&no_noscript, "nav");
    let no_footer = remove_tag_blocks(&no_nav, "footer");
    let no_head   = remove_tag_blocks(&no_footer, "head");

    // 2. Convertir algunos tags en saltos de línea
    let with_newlines = no_head
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</div>", "\n")
        .replace("</li>", "\n")
        .replace("</h1>", "\n\n")
        .replace("</h2>", "\n\n")
        .replace("</h3>", "\n\n")
        .replace("</tr>", "\n")
        .replace("</td>", "  ")
        .replace("</th>", "  ");

    // 3. Eliminar todos los tags restantes
    let text = strip_tags(&with_newlines);

    // 4. Decodificar entidades HTML comunes
    let decoded = decode_entities(&text);

    // 5. Limpiar líneas vacías consecutivas
    let lines: Vec<&str> = decoded.lines().collect();
    let mut out = String::with_capacity(decoded.len());
    let mut blank_count = 0u32;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                out.push('\n');
            }
        } else {
            blank_count = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    out.trim().to_string()
}

/// Elimina todos los bloques `<tag ...>...</tag>` (case-insensitive, no anidados).
fn remove_tag_blocks(html: &str, tag: &str) -> String {
    let open_lower  = format!("<{tag}");
    let close_lower = format!("</{tag}>");
    let open_upper  = format!("<{}", tag.to_uppercase());

    let mut result = String::with_capacity(html.len());
    let mut pos = 0;

    while pos < html.len() {
        // Buscar apertura
        let lower_pos = find_icase(&html[pos..], &open_lower, &open_upper);
        let Some(rel) = lower_pos else {
            result.push_str(&html[pos..]);
            break;
        };
        let start = pos + rel;
        result.push_str(&html[pos..start]);

        // Buscar cierre
        let after_open = start + open_lower.len();
        let close_rel_lower = html[after_open..].to_lowercase().find(&close_lower);
        let close_rel = close_rel_lower.map(|p| after_open + p + close_lower.len());
        if let Some(end) = close_rel {
            pos = end;
        } else {
            // No encontró cierre, saltar el tag de apertura
            pos = after_open;
        }
    }

    result
}

fn find_icase(haystack: &str, lower: &str, _upper: &str) -> Option<usize> {
    let h_lower = haystack.to_lowercase();
    h_lower.find(lower)
}

/// Elimina todas las etiquetas HTML (<...>).
fn strip_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => result.push(c),
            _ => {}
        }
    }
    result
}

fn decode_entities(text: &str) -> String {
    text.replace("&amp;",  "&")
        .replace("&lt;",   "<")
        .replace("&gt;",   ">")
        .replace("&quot;", "\"")
        .replace("&#39;",  "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;","…")
        .replace("&laquo;", "«")
        .replace("&raquo;", "»")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
        .replace("&#xA0;", " ")
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    let start = tag.find(&pattern)? + pattern.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(ch),
            ' ' => out.push('+'),
            c => {
                for byte in c.to_string().as_bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    out
}

fn url_decode(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            if let Ok(byte) = u8::from_str_radix(&format!("{h1}{h2}"), 16) {
                out.push(byte as char);
            }
        } else if ch == '+' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn build_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; dca/1.0)")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?)
}

