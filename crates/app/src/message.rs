// Re-exportamos AppMessage desde el crate events para que
// los crates superiores solo dependan de dca-app.
pub use dca_events::bus::AppMessage;
