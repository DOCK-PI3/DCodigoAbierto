/// Syntax highlight básico para bloques de código en los mensajes de chat.
///
/// Detecta bloques ``` ... ``` y colorea palabras clave por lenguaje.
/// Retorna un `Vec<Line>` listo para renderizar con Ratatui.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// ── Colores ───────────────────────────────────────────────────────────────────
const C_KW:     Color = Color::Rgb(203, 166, 247); // morado — keywords
const C_TYPE:   Color = Color::Rgb(137, 180, 250); // azul   — types / builtins
const C_STRING: Color = Color::Rgb(166, 227, 161); // verde  — strings
const C_NUMBER: Color = Color::Rgb(250, 179, 135); // naranja — números
const C_COMMENT:Color = Color::Rgb(108, 112, 134); // gris   — comentarios
const C_FN:     Color = Color::Rgb(137, 220, 235); // cian   — funciones
const C_PLAIN:  Color = Color::Rgb(205, 214, 244); // blanco — texto normal

// ── Palabras clave por lenguaje ────────────────────────────────────────────────

fn keywords(lang: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match lang {
        "rust" | "rs" => (
            &["fn", "let", "mut", "pub", "use", "mod", "struct", "enum",
              "impl", "trait", "where", "for", "in", "if", "else", "match",
              "return", "self", "Self", "super", "crate", "async", "await",
              "loop", "while", "break", "continue", "type", "const", "static",
              "unsafe", "extern", "dyn", "ref", "move"],
            &["i8","i16","i32","i64","i128","isize","u8","u16","u32","u64",
              "u128","usize","f32","f64","bool","char","str","String","Vec",
              "Option","Result","Ok","Err","Some","None","Box","Arc","Rc",
              "HashMap","HashSet","BTreeMap","BTreeSet","true","false"],
        ),
        "javascript" | "js" | "typescript" | "ts" => (
            &["const","let","var","function","return","if","else","for","while",
              "do","switch","case","break","continue","new","delete","typeof",
              "instanceof","class","extends","import","export","default","from",
              "async","await","try","catch","finally","throw","in","of","this",
              "super","null","undefined","void","yield","static","get","set"],
            &["true","false","NaN","Infinity","console","Promise","Array",
              "Object","String","Number","Boolean","Symbol","Map","Set",
              "parseInt","parseFloat","Math","JSON","Date","Error"],
        ),
        "python" | "py" => (
            &["def","class","return","if","elif","else","for","while","break",
              "continue","import","from","as","with","try","except","finally",
              "raise","pass","yield","lambda","and","or","not","in","is",
              "global","nonlocal","del","assert","async","await"],
            &["True","False","None","int","float","str","list","dict","tuple",
              "set","bool","bytes","print","len","range","type","isinstance",
              "hasattr","getattr","setattr","open","super","self"],
        ),
        "html" | "xml" => (
            &["<!DOCTYPE","<html","<head","<body","<div","<span","<p","<a",
              "<img","<ul","<ol","<li","<table","<tr","<td","<th","<form",
              "<input","<button","<script","<style","<link","<meta","<title",
              "</div>","</span>","</p>","</a>","</body>","</html>"],
            &["class","id","href","src","style","type","name","value",
              "placeholder","action","method","rel","charset","lang",
              "onclick","onchange","onsubmit"],
        ),
        "sh" | "bash" | "shell" | "zsh" => (
            &["if","then","fi","else","elif","for","in","do","done","while",
              "until","case","esac","function","return","exit","break",
              "continue","export","local","readonly","source",".","echo",
              "printf","read","shift","set","unset"],
            &["true","false","$","${","$(","||","&&",";;","2>","1>",">>",
              "<<","pipe","|"],
        ),
        _ => (&[], &[]),
    }
}

// ── Función principal ─────────────────────────────────────────────────────────

/// Procesa el contenido de un mensaje de chat y devuelve las líneas
/// renderizables con syntax highlight para bloques de código.
pub fn render_message(content: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = vec![];
    let mut in_code_block = false;
    let mut current_lang = String::new();

    for raw_line in content.lines() {
        if raw_line.starts_with("```") {
            if !in_code_block {
                // Abrir bloque
                in_code_block = true;
                current_lang = raw_line.trim_start_matches('`').trim().to_lowercase();
                // Mostrar la línea del fence en gris
                lines.push(Line::from(Span::styled(
                    raw_line.to_string(),
                    Style::default().fg(C_COMMENT),
                )));
            } else {
                // Cerrar bloque
                in_code_block = false;
                current_lang.clear();
                lines.push(Line::from(Span::styled(
                    raw_line.to_string(),
                    Style::default().fg(C_COMMENT),
                )));
            }
        } else if in_code_block {
            lines.push(highlight_code_line(raw_line, &current_lang));
        } else {
            lines.push(plain_line(raw_line));
        }
    }

    lines
}

/// Línea de texto plano (fuera de bloques de código).
fn plain_line(line: &str) -> Line<'static> {
    Line::from(Span::styled(line.to_string(), Style::default().fg(C_PLAIN)))
}

/// Tokeniza y colorea una línea de código.
fn highlight_code_line(line: &str, lang: &str) -> Line<'static> {
    // Comentario de línea
    let comment_prefix = comment_prefix_for(lang);
    if !comment_prefix.is_empty() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(comment_prefix) {
            return Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(C_COMMENT).add_modifier(Modifier::ITALIC),
            ));
        }
    }

    let (kws, types) = keywords(lang);
    let mut spans: Vec<Span<'static>> = vec![];

    // Tokenizar por palabra/símbolo
    let mut chars = line.chars().peekable();
    let mut token = String::new();
    let mut in_string = false;
    let mut str_char = '"';

    while let Some(ch) = chars.next() {
        if in_string {
            token.push(ch);
            if ch == str_char {
                spans.push(Span::styled(token.clone(), Style::default().fg(C_STRING)));
                token.clear();
                in_string = false;
            }
        } else if ch == '"' || ch == '\'' {
            flush_token(&mut token, &mut spans, kws, types);
            token.push(ch);
            in_string = true;
            str_char = ch;
        } else if ch.is_alphanumeric() || ch == '_' {
            token.push(ch);
        } else {
            flush_token(&mut token, &mut spans, kws, types);
            // Números solos
            if ch.is_ascii_digit() {
                spans.push(Span::styled(ch.to_string(), Style::default().fg(C_NUMBER)));
            } else {
                spans.push(Span::styled(ch.to_string(), Style::default().fg(C_PLAIN)));
            }
        }
    }
    flush_token(&mut token, &mut spans, kws, types);

    // Si la cadena quedó abierta sin cerrar
    if in_string && !token.is_empty() {
        spans.push(Span::styled(token, Style::default().fg(C_STRING)));
    }

    Line::from(spans)
}

fn flush_token(
    token: &mut String,
    spans: &mut Vec<Span<'static>>,
    kws: &[&str],
    types: &[&str],
) {
    if token.is_empty() { return; }

    let style = if kws.contains(&token.as_str()) {
        Style::default().fg(C_KW).add_modifier(Modifier::BOLD)
    } else if types.contains(&token.as_str()) {
        Style::default().fg(C_TYPE)
    } else if token.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        Style::default().fg(C_NUMBER)
    } else if token.chars().all(|c| c.is_uppercase() || c == '_') && token.len() > 1 {
        // CONSTANTES_EN_MAYUS
        Style::default().fg(C_NUMBER)
    } else {
        Style::default().fg(C_PLAIN)
    };

    spans.push(Span::styled(token.clone(), style));
    token.clear();
}

fn comment_prefix_for(lang: &str) -> &'static str {
    match lang {
        "rust" | "rs" | "js" | "javascript" | "ts" | "typescript" => "//",
        "python" | "py" | "sh" | "bash" | "shell" => "#",
        "html" | "xml" => "<!--",
        _ => "",
    }
}
