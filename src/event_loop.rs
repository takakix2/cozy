use std::io::{self, Write};
use std::time::Duration;
use crossterm::event::Event;
use ratatui::{backend::CrosstermBackend, Terminal};
use crate::state::EditorState;
use crate::state::key::{KeyCode, KeyModifiers};
use crate::ui::{Renderer, Keymap};
use crate::reducer::{reduce, EventResult};
use crate::action::Action;

/// Abstracts the event source so the loop can run with crossterm or an IPC queue.
pub trait EventSource {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;
    fn read(&mut self) -> io::Result<Event>;
}

/// Default implementation that reads directly from crossterm (CLI use).
pub struct CrosstermEventSource;

impl EventSource for CrosstermEventSource {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        crossterm::event::poll(timeout)
    }
    fn read(&mut self) -> io::Result<Event> {
        crossterm::event::read()
    }
}

fn ct_code(code: crossterm::event::KeyCode) -> Option<KeyCode> {
    use crossterm::event::KeyCode as CT;
    Some(match code {
        CT::Char(c)   => KeyCode::Char(c),
        CT::Enter     => KeyCode::Enter,
        CT::Esc       => KeyCode::Esc,
        CT::Backspace => KeyCode::Backspace,
        CT::Delete    => KeyCode::Delete,
        CT::PageUp    => KeyCode::PageUp,
        CT::PageDown  => KeyCode::PageDown,
        CT::Up        => KeyCode::Up,
        CT::Down      => KeyCode::Down,
        CT::Left      => KeyCode::Left,
        CT::Right     => KeyCode::Right,
        CT::Home      => KeyCode::Home,
        CT::End       => KeyCode::End,
        CT::Tab       => KeyCode::Tab,
        CT::F(n)      => KeyCode::F(n),
        _             => return None,
    })
}

fn ct_mods(mods: crossterm::event::KeyModifiers) -> KeyModifiers {
    use crossterm::event::KeyModifiers as CT;
    let mut result = KeyModifiers::NONE;
    if mods.contains(CT::CONTROL) { result |= KeyModifiers::CONTROL; }
    if mods.contains(CT::SHIFT)   { result |= KeyModifiers::SHIFT; }
    if mods.contains(CT::ALT)     { result |= KeyModifiers::ALT; }
    result
}

pub fn run<W: Write>(
    terminal: &mut Terminal<CrosstermBackend<W>>,
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
            match event_src.read()? {
                Event::Key(key_event) => {
                    if let Some(code) = ct_code(key_event.code) {
                        let mods = ct_mods(key_event.modifiers);
                        if let Some(action) = Keymap::map_key_to_action(editor, code, mods) {
                            if let EventResult::Exit = reduce(editor, action) {
                                break;
                            }
                        }
                    }
                    editor.cursor_blink = true;
                    needs_redraw = true;
                }
                Event::Paste(data) => {
                    let action = Action::InsertString(data);
                    if let EventResult::Exit = reduce(editor, action) {
                        break;
                    }
                    editor.cursor_blink = true;
                    needs_redraw = true;
                }
                Event::Resize(cols, rows) => {
                    let _ = terminal.resize(ratatui::layout::Rect::new(0, 0, cols, rows));
                    needs_redraw = true;
                }
                _ => {}
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
