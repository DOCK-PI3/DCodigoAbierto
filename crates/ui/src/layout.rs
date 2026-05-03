use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Resultado del cálculo de layout de la pantalla principal.
pub struct MainLayout {
    /// Área del sidebar (None si está oculto)
    pub sidebar: Option<Rect>,
    /// Barra de pestañas (1 fila encima del editor)
    pub tabbar: Rect,
    /// Área del panel de editor
    pub editor: Rect,
    /// Área del panel de chat (None si no está visible)
    pub chat: Option<Rect>,
    /// Área de la barra de estado inferior (1 fila)
    pub statusbar: Rect,
}

/// Calcula el layout principal:
///   ┌─────────────────────────────────────────────────┐
///   │  sidebar (opt) │ [tabbar(1)] │  [chat panel]    │
///   │    (28%)       │  editor     │    (38%)         │
///   ├─────────────────────────────────────────────────┤
///   │                   statusbar (1)                 │
///   └─────────────────────────────────────────────────┘
pub fn main_layout(area: Rect, sidebar_visible: bool, chat_visible: bool) -> MainLayout {
    // Split vertical: contenido + statusbar
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // contenido
            Constraint::Length(1), // statusbar
        ])
        .split(area);

    let content_area = vertical[0];
    let statusbar = vertical[1];

    /// Split de editor + tabbar
    fn split_editor_with_tabbar(area: Rect) -> (Rect, Rect) {
        let v = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        (v[0], v[1])
    }

    // Columnas: sidebar | center | chat
    let col_constraints = match (sidebar_visible, chat_visible) {
        (true, true) => vec![
            Constraint::Percentage(28),
            Constraint::Percentage(34),
            Constraint::Percentage(38),
        ],
        (true, false) => vec![
            Constraint::Percentage(28),
            Constraint::Min(1),
        ],
        (false, true) => vec![
            Constraint::Percentage(62),
            Constraint::Percentage(38),
        ],
        (false, false) => vec![
            Constraint::Min(1),
        ],
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(col_constraints)
        .split(content_area);

    let (sidebar_area, center_area, chat_area) = match (sidebar_visible, chat_visible) {
        (true, true)  => (Some(cols[0]), cols[1], Some(cols[2])),
        (true, false) => (Some(cols[0]), cols[1], None),
        (false, true) => (None,          cols[0], Some(cols[1])),
        (false, false) => (None,         cols[0], None),
    };

    let (tabbar, editor) = split_editor_with_tabbar(center_area);

    MainLayout {
        sidebar: sidebar_area,
        tabbar,
        editor,
        chat: chat_area,
        statusbar,
    }
}
