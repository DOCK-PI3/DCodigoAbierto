use ratatui::Frame;

use dca_config::Theme;
use dca_types::view_state::{Focus, ViewState};

use crate::{
    layout::main_layout,
    palette::Palette,
    widgets::{
        chat::ChatWidget,
        command_palette::{CommandPaletteWidget, PaletteItem, PaletteItemKind, PaletteActionId},
        completion::{completion_popup_rect, CompletionWidget},
        editor::EditorWidget,
        fuzzy::{fuzzy_popup_rect, FuzzyWidget},
        home::HomeWidget,
        meta_panel::MetaPanelWidget,
        model_selector::ModelSelectorWidget,
        permission::PermissionDialog,
        references::{references_panel_rect, ReferencesWidget},
        sidebar::SidebarWidget,
        statusbar::StatusbarWidget,
        tabbar::TabBarWidget,
        theme_selector::ThemeSelectorWidget,
    },
};

/// Función raíz de renderizado. Llamada en cada frame por el loop principal.
pub fn render(frame: &mut Frame, view: &ViewState<'_>, theme: &Theme) {
    let palette = Palette::from_theme(theme);

    // Modo home: ningún archivo abierto (buffer sin nombre)
    let home_mode = view.buffer.file_name.is_none() && !view.fuzzy_active;

    if home_mode {
        // Split para dejar 1 fila de statusbar al fondo
        let home_split = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .split(frame.area());

        frame.render_widget(
            HomeWidget {
                chat_input: view.chat_input,
                chat_input_cursor: view.chat_input_cursor,
                focused: view.focus == Focus::Chat,
                streaming: view.chat_streaming,
                active_model: view.active_model,
                provider_name: view.provider_name,
                chat_mode_label: view.chat_mode_label,
                chat_mode_is_build: view.chat_mode_is_build,
                version: env!("CARGO_PKG_VERSION"),
                palette: &palette,
                messages: view.chat_messages,
                scroll: view.chat_scroll,
            },
            home_split[0],
        );

        frame.render_widget(
            StatusbarWidget {
                palette: &palette,
                message: view.status_message,
                streaming: view.chat_streaming,
                tokens_generated: view.tokens_generated,
                streaming_elapsed_secs: view.streaming_elapsed_secs,
                chat_mode_label: view.chat_mode_label,
                chat_mode_is_build: view.chat_mode_is_build,
                active_model: view.active_model,
                home_mode: true,
            },
            home_split[1],
        );

        // Popups flotantes que funcionan incluso en home mode
        if view.fuzzy_active {
            let popup_area = fuzzy_popup_rect(frame.area());
            frame.render_widget(
                FuzzyWidget {
                    query: view.fuzzy_query,
                    results: view.fuzzy_results,
                    selected: view.fuzzy_selected,
                    palette: &palette,
                },
                popup_area,
            );
        }

        if let Some(tool) = &view.chat_tool_pending {
            frame.render_widget(
                PermissionDialog {
                    tool_name: &tool.name,
                    args_display: &tool.args_display,
                },
                frame.area(),
            );
        }

        if view.model_selector_active && !view.model_selector_models.is_empty() {
            frame.render_widget(
                ModelSelectorWidget {
                    models: view.model_selector_models,
                    selected: view.model_selector_selected,
                },
                frame.area(),
            );
        }

        if view.theme_selector_active && !view.available_theme_names.is_empty() {
            let themes: Vec<(String, String, String)> = view.available_theme_names.iter()
                .zip(view.available_theme_accents.iter())
                .map(|(n, (a, b))| (n.clone(), a.clone(), b.clone()))
                .collect();
            frame.render_widget(
                ThemeSelectorWidget {
                    themes: &themes,
                    selected: view.theme_selector_selected,
                },
                frame.area(),
            );
        }

        // Palette en home mode también
        if view.palette_active {
            let items = build_palette_items(view);
            frame.render_widget(
                CommandPaletteWidget {
                    query: view.palette_query,
                    query_cursor: view.palette_query_cursor,
                    items: &items,
                    selected: view.palette_selected,
                    palette: &palette,
                },
                frame.area(),
            );
        }

        return;
    }

    // ── Layout normal (archivo abierto) ───────────────────────────────────────

    let layout = main_layout(frame.area(), view.sidebar_visible, view.chat_visible);

    // Sidebar
    if let Some(sidebar_area) = layout.sidebar {
        frame.render_widget(
            SidebarWidget {
                entries: view.file_tree,
                selected: view.sidebar_selected,
                palette: &palette,
                active: view.focus == Focus::Sidebar,
            },
            sidebar_area,
        );
    }

    // Tab bar
    frame.render_widget(
        TabBarWidget {
            tabs: &view.buffers,
            active: view.active_buffer,
            palette: &palette,
        },
        layout.tabbar,
    );

    // Editor
    frame.render_widget(
        EditorWidget {
            buffer: view.buffer,
            palette: &palette,
            active: view.focus == Focus::Editor,
            diagnostics: view.diagnostics,
        },
        layout.editor,
    );

    // Panel de Chat IA
    if let Some(chat_area) = layout.chat {
        // Si hay mensajes, dividir chat(70%) + meta(30%)
        let (chat_render_area, meta_area) = if !view.chat_messages.is_empty() {
            let split = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Percentage(68),
                    ratatui::layout::Constraint::Percentage(32),
                ])
                .split(chat_area);
            (split[0], Some(split[1]))
        } else {
            (chat_area, None)
        };

        frame.render_widget(
            ChatWidget {
                messages: view.chat_messages,
                input: view.chat_input,
                input_cursor: view.chat_input_cursor,
                scroll: view.chat_scroll,
                streaming: view.chat_streaming,
                focused: view.focus == Focus::Chat,
            },
            chat_render_area,
        );

        if let Some(meta) = meta_area {
            let lsp_label = match view.status_message {
                s if s.contains("LSP:●") => "rust-analyzer ●",
                s if s.contains("LSP:…") => "conectando…",
                s if s.contains("LSP:✗") => "error ✗",
                _ => "desactivado",
            };
            let lsp_ok = view.status_message.contains("LSP:●");
            let cwd = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "~".to_string());
            frame.render_widget(
                MetaPanelWidget {
                    session_name: view.session_name,
                    tokens_generated: view.tokens_generated,
                    lsp_status_label: lsp_label,
                    lsp_ok,
                    cwd: &cwd,
                    version: env!("CARGO_PKG_VERSION"),
                    palette: &palette,
                },
                meta,
            );
        }
    }

    // Statusbar
    frame.render_widget(
        StatusbarWidget {
            palette: &palette,
            message: view.status_message,
            streaming: view.chat_streaming,
            tokens_generated: view.tokens_generated,
            streaming_elapsed_secs: view.streaming_elapsed_secs,
            chat_mode_label: view.chat_mode_label,
            chat_mode_is_build: view.chat_mode_is_build,
            active_model: view.active_model,
            home_mode: false,
        },
        layout.statusbar,
    );

    // Panel de referencias (floating, sobre el editor inferior)
    if view.show_references && !view.references.is_empty() {
        let refs_area = references_panel_rect(layout.editor);
        frame.render_widget(
            ReferencesWidget {
                references: view.references,
                selected: view.references_selected,
                palette: &palette,
            },
            refs_area,
        );
    }

    // Completion popup
    if view.show_completions && !view.completions.is_empty() {
        let gutter_width = {
            let n = view.buffer.line_count();
            format!("{n}").len() as u16 + 4
        };
        let editor_inner_x = layout.editor.x + 1 + gutter_width;
        let cursor_row_offset = view.buffer.cursor.row.saturating_sub(view.buffer.scroll_row) as u16;
        let cursor_col_offset = view.buffer.cursor_visual_col() as u16;
        let screen_row = layout.editor.y + 1 + cursor_row_offset;
        let screen_col = editor_inner_x + cursor_col_offset;

        let popup_area = completion_popup_rect(
            layout.editor,
            screen_row,
            screen_col,
            view.completions.len(),
        );

        frame.render_widget(
            CompletionWidget {
                items: view.completions,
                selected: view.completion_selected,
                palette: &palette,
            },
            popup_area,
        );
    }

    // Fuzzy finder (capa superior, centrado)
    if view.fuzzy_active {
        let popup_area = fuzzy_popup_rect(frame.area());
        frame.render_widget(
            FuzzyWidget {
                query: view.fuzzy_query,
                results: view.fuzzy_results,
                selected: view.fuzzy_selected,
                palette: &palette,
            },
            popup_area,
        );
    }

    // Diálogo de permiso para herramienta peligrosa
    if let Some(tool) = &view.chat_tool_pending {
        frame.render_widget(
            PermissionDialog {
                tool_name: &tool.name,
                args_display: &tool.args_display,
            },
            frame.area(),
        );
    }

    // Selector de modelo
    if view.model_selector_active && !view.model_selector_models.is_empty() {
        frame.render_widget(
            ModelSelectorWidget {
                models: view.model_selector_models,
                selected: view.model_selector_selected,
            },
            frame.area(),
        );
    }

    // Selector de tema
    if view.theme_selector_active && !view.available_theme_names.is_empty() {
        let themes: Vec<(String, String, String)> = view.available_theme_names.iter()
            .zip(view.available_theme_accents.iter())
            .map(|(n, (a, b))| (n.clone(), a.clone(), b.clone()))
            .collect();
        frame.render_widget(
            ThemeSelectorWidget {
                themes: &themes,
                selected: view.theme_selector_selected,
            },
            frame.area(),
        );
    }

    // Command Palette (capa superior sobre todo)
    if view.palette_active {
        let items = build_palette_items(view);
        frame.render_widget(
            CommandPaletteWidget {
                query: view.palette_query,
                query_cursor: view.palette_query_cursor,
                items: &items,
                selected: view.palette_selected,
                palette: &palette,
            },
            frame.area(),
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_palette_items<'a>(view: &ViewState<'a>) -> Vec<PaletteItem> {
    let mode_toggle = if view.chat_mode_is_build {
        "Cambiar a Plan"
    } else {
        "Cambiar a Build"
    };
    vec![
        PaletteItem { kind: PaletteItemKind::Section { label: "Archivos" }, label: String::new() },
        PaletteItem {
            kind: PaletteItemKind::Action { id: PaletteActionId::OpenEditor, shortcut: "ctrl+p" },
            label: "Abrir archivo…".to_string(),
        },
        PaletteItem { kind: PaletteItemKind::Section { label: "Sesión" }, label: String::new() },
        PaletteItem {
            kind: PaletteItemKind::Action { id: PaletteActionId::NewSession, shortcut: "" },
            label: "Nueva sesión de chat".to_string(),
        },
        PaletteItem {
            kind: PaletteItemKind::Action { id: PaletteActionId::InjectBuffer, shortcut: "ctrl+i" },
            label: "Inyectar buffer como contexto".to_string(),
        },
        PaletteItem { kind: PaletteItemKind::Section { label: "Agente" }, label: String::new() },
        PaletteItem {
            kind: PaletteItemKind::Action { id: PaletteActionId::SwitchModel, shortcut: "ctrl+o" },
            label: "Cambiar modelo de IA…".to_string(),
        },
        PaletteItem {
            kind: PaletteItemKind::Action { id: PaletteActionId::SelectTheme, shortcut: "" },
            label: "Seleccionar tema…".to_string(),
        },
        PaletteItem {
            kind: PaletteItemKind::Action { id: PaletteActionId::ToggleMode, shortcut: "tab" },
            label: mode_toggle.to_string(),
        },
        PaletteItem { kind: PaletteItemKind::Section { label: "Sistema" }, label: String::new() },
        PaletteItem {
            kind: PaletteItemKind::Action { id: PaletteActionId::AbortStream, shortcut: "ctrl+x" },
            label: "Abortar generación".to_string(),
        },
        PaletteItem {
            kind: PaletteItemKind::Action { id: PaletteActionId::Quit, shortcut: "ctrl+q" },
            label: "Salir".to_string(),
        },
    ]
}

