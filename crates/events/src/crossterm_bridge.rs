use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use tracing::{debug, warn};

use crate::bus::{AppMessage, EventSender};

/// Lanza una tarea tokio que escucha eventos de crossterm y los reenvía
/// al EventBus como `AppMessage`.
///
/// La tarea vive mientras el canal esté abierto; cuando el receptor
/// se cierra (la app termina), el loop sale limpiamente.
pub fn spawn_crossterm_task(tx: EventSender) {
    tokio::spawn(async move {
        let mut stream = EventStream::new();

        loop {
            match stream.next().await {
                Some(Ok(Event::Key(key))) => {
                    debug!("KeyEvent: {:?}", key);
                    if tx.send(AppMessage::Key(key)).is_err() {
                        break; // receptor caído → la app terminó
                    }
                }
                Some(Ok(Event::Resize(w, h))) => {
                    debug!("Resize: {}x{}", w, h);
                    if tx.send(AppMessage::Resize(w, h)).is_err() {
                        break;
                    }
                }
                // Eventos de mouse y focus: ignorados en Fase 1
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    warn!("Error en EventStream de crossterm: {e}");
                    break;
                }
                None => break, // stream agotado
            }
        }

        debug!("Tarea crossterm terminada.");
    });
}
