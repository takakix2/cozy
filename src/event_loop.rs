use crate::input::{self, EventSource, InputEvent};
use crate::reducer::{EventResult, reduce};
use crate::state::EditorState;
use crate::ui::Renderer;
use ratatui::{Terminal, backend::Backend};
use std::io;
use std::time::Duration;

pub fn run<B: Backend>(
    terminal: &mut Terminal<B>,
    editor: &mut EditorState,
    event_src: &mut dyn EventSource,
) -> io::Result<()> {
    let mut needs_redraw = true;

    loop {
        if needs_redraw {
            terminal.draw(|f| {
                let chunks = Renderer::editor_layout(f.area(), editor);
                Renderer::render_body(editor, f, chunks[0]);
                Renderer::render_shortcuts(editor, f, chunks[1]);
                Renderer::render_status_bar(editor, f, chunks[2]);
            })?;
            needs_redraw = false;
        }

        // Poll with longer timeout to avoid clearing IME composition overlays.
        // crossterm receives no events during IME composition (macOS intercepts
        // keystrokes at the Cocoa layer), so longer polling means the IME inline
        // display persists undisturbed until the user confirms the composition.
        if event_src.poll(Duration::from_millis(1000))? {
            match input::map_event(editor, event_src.read()?) {
                InputEvent::Action(action) => {
                    if let EventResult::Exit = reduce(editor, action) {
                        break;
                    }
                    editor.cursor_blink = true;
                    needs_redraw = true;
                }
                InputEvent::Resize(cols, rows) => {
                    let _ = terminal.resize(ratatui::layout::Rect::new(0, 0, cols, rows));
                    needs_redraw = true;
                }
                InputEvent::Ignore => {}
            }
        } else {
            // Timeout: toggle cursor blink only if enabled
            if editor.config.cursor_blink.unwrap_or(false) {
                editor.cursor_blink = !editor.cursor_blink;
                needs_redraw = true;
            }
        }
    }
    Ok(())
}
