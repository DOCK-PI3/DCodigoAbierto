use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;

/// Escribe un mensaje LSP al writer con la cabecera Content-Length.
pub async fn write_message(
    writer: &mut BufWriter<ChildStdin>,
    content: &str,
) -> color_eyre::Result<()> {
    let header = format!("Content-Length: {}\r\n\r\n", content.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(content.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

/// Lee un mensaje LSP del reader (Content-Length framing).
/// Devuelve el body JSON como String.
pub async fn read_message(
    reader: &mut BufReader<ChildStdout>,
) -> color_eyre::Result<String> {
    let mut content_length: Option<usize> = None;

    // Leer cabeceras hasta la línea vacía
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(color_eyre::eyre::eyre!("LSP: conexión cerrada"));
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length: ") {
            content_length = rest.trim().parse().ok();
        }
    }

    let len = content_length
        .ok_or_else(|| color_eyre::eyre::eyre!("LSP: cabecera Content-Length ausente"))?;

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(String::from_utf8(buf)?)
}
