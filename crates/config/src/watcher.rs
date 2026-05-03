use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, warn};

use crate::AppConfig;

/// Lanza un watcher que observa el archivo de configuración y envía
/// una señal `true` al canal cuando se detecta un cambio.
///
/// El canal es `UnboundedSender<AppConfig>` — envía la config recargada directamente.
pub fn spawn_config_watcher(
    config_path: PathBuf,
    tx: UnboundedSender<AppConfig>,
) {
    std::thread::spawn(move || {
        let (std_tx, std_rx) = std_mpsc::channel::<notify::Result<Event>>();

        let mut watcher = match RecommendedWatcher::new(
            std_tx,
            Config::default().with_poll_interval(Duration::from_secs(2)),
        ) {
            Ok(w) => w,
            Err(e) => {
                warn!("config watcher: no se pudo crear: {e}");
                return;
            }
        };

        if let Err(e) = watcher.watch(&config_path, RecursiveMode::NonRecursive) {
            warn!("config watcher: no se pudo observar {:?}: {e}", config_path);
            return;
        }

        debug!("config watcher: observando {:?}", config_path);

        for event in std_rx {
            match event {
                Ok(ev) if matches!(ev.kind, EventKind::Modify(_) | EventKind::Create(_)) => {
                    debug!("config watcher: cambio detectado, recargando…");
                    // Pequeña pausa para asegurar que la escritura terminó
                    std::thread::sleep(Duration::from_millis(100));
                    match AppConfig::load() {
                        Ok(new_cfg) => {
                            if tx.send(new_cfg).is_err() {
                                break; // canal cerrado → app terminó
                            }
                        }
                        Err(e) => warn!("config watcher: error al recargar: {e}"),
                    }
                }
                Err(e) => {
                    warn!("config watcher: error: {e}");
                    break;
                }
                _ => {}
            }
        }
    });
}
