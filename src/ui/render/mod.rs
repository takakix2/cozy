pub mod body;
pub mod browse;
pub mod footer;
pub mod markdown;
pub mod screen;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};
use std::rc::Rc;

use crate::state::{EditorMode, EditorState};

// Re-export public functions
pub use body::render_text_buffer;
pub use screen::{render_help, render_welcome};

pub struct Renderer;

impl Renderer {
    pub fn editor_layout(area: Rect, editor: &EditorState) -> Rc<[Rect]> {
        let is_narrow = area.width < 50;
        let shortcut_h: u16 = match (is_narrow, editor.mode) {
            (_, EditorMode::Welcome) => 0,
            (_, EditorMode::Help) => 1,
            (_, EditorMode::Command) => 10,
            (true, _) => 4,
            (false, _) => 2,
        };
        let status_bar_h: u16 = match editor.mode {
            EditorMode::Welcome | EditorMode::Help => 0,
            _ => 1,
        };
        // On narrow screens (iPhone), collapse shortcuts when height is small
        // (iOS soft keyboard visible typically leaves ~12 rows; without keyboard ~23 rows).
        let actual_shortcut_h = if is_narrow && area.height < 18 && shortcut_h > 0 {
            0
        } else {
            let available = area.height.saturating_sub(1 + status_bar_h);
            shortcut_h.min(available)
        };
        Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Min(1),
                    Constraint::Length(actual_shortcut_h),
                    Constraint::Length(status_bar_h),
                ]
                .as_ref(),
            )
            .split(area)
    }

    pub fn render_body(editor: &mut EditorState, f: &mut Frame, area: Rect) {
        editor.page_size = area.height as usize;
        if editor.mode == EditorMode::Welcome {
            render_welcome(f, area);
            return;
        }
        if editor.mode == EditorMode::Help {
            render_help(editor, f, area);
            return;
        }
        if editor.mode == EditorMode::Browse {
            browse::render_browse(editor, f, area);
            return;
        }
        if editor.mode == EditorMode::Markdown {
            markdown::render_markdown(editor, f, area);
            return;
        }
        render_text_buffer(editor, f, area);
    }

    pub fn render_shortcuts(editor: &EditorState, f: &mut Frame, area: Rect) {
        footer::render_shortcuts(editor, f, area);
    }

    pub fn render_status_bar(editor: &EditorState, f: &mut Frame, area: Rect) {
        footer::render_status_bar(editor, f, area);
    }
}
