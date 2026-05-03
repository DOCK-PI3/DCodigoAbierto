use std::io::Write;
use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use color_eyre::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

use dca_app::App;
use dca_config::AppConfig;

/// DCodigoAbierto — editor TUI con integración LSP
#[derive(Parser, Debug)]
#[command(name = "dca", version, about, long_about = None)]
struct Cli {
    /// Archivo a abrir al iniciar
    #[arg(value_name = "ARCHIVO")]
    file: Option<PathBuf>,

    /// Servidor LSP a usar (sobreescribe config.toml)
    #[arg(long, value_name = "COMANDO", env = "DCA_LSP_SERVER")]
    lsp: Option<String>,

    /// Redirigir logs a un archivo en lugar de stderr
    #[arg(long, value_name = "RUTA")]
    log_file: Option<PathBuf>,

    /// Nivel de log (error, warn, info, debug, trace)
    #[arg(long, default_value = "warn", env = "RUST_LOG")]
    log_level: String,

    /// Generar shell completions y salir
    #[arg(long, value_name = "SHELL", value_enum)]
    completions: Option<Shell>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Panic hook: restaurar terminal antes de imprimir el backtrace,
    // para que el mensaje sea legible en el modo normal del terminal.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        default_hook(info);
    }));

    let cli = Cli::parse();

    // Generar shell completions y salir
    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        return Ok(());
    }

    // Configurar tracing
    let filter = EnvFilter::try_new(&cli.log_level)
        .unwrap_or_else(|_| EnvFilter::new("warn"));

    if let Some(log_path) = &cli.log_file {
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(move || -> Box<dyn Write + Send> {
                Box::new(log_file.try_clone().expect("log file clone"))
            })
            .with_ansi(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .init();
    }

    info!("DCodigoAbierto arrancando...");

    let mut config = AppConfig::load()?;

    // CLI overrides
    if let Some(lsp) = cli.lsp {
        config.lsp_server = lsp;
    }

    let mut app = App::new(config);

    // Si se pasó un archivo por argumento, abrirlo al inicio
    if let Some(path) = cli.file {
        app.set_initial_file(path);
    }

    app.run().await?;

    info!("DCodigoAbierto terminado.");
    Ok(())
}
