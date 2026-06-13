mod action;
mod browse;
mod commands;
mod event_loop;
mod glide;
mod host;
mod reducer;
mod shortcuts;
mod state;
mod ui;
mod utils;

pub use event_loop::{CrosstermEventSource, EventSource};
pub use host::run_cli;
use ratatui::{Terminal, backend::CrosstermBackend};
use state::EditorState;
use std::io::{self, Write};
use std::path::PathBuf;

/// Configuration for embedding cozy in a host application.
pub struct CozyConfig {
    /// File to open on launch.
    pub filename: Option<String>,
    /// Override config file search directory (e.g. iOS Documents/.hsh/).
    /// `None` uses the default XDG / home-dir search.
    pub config_dir: Option<PathBuf>,
    /// Set to false when the host (e.g. xterm.js PTY) already owns raw mode.
    pub enable_raw_mode: bool,
    /// Set to false when the host manages the screen buffer.
    pub enable_alternate_screen: bool,
    /// Terminal size (cols, rows). Required when the host is not a real TTY.
    /// `None` lets ratatui detect the size via ioctl (CLI use).
    pub terminal_size: Option<(u16, u16)>,
}

impl Default for CozyConfig {
    fn default() -> Self {
        Self {
            filename: None,
            config_dir: None,
            enable_raw_mode: true,
            enable_alternate_screen: true,
            terminal_size: None,
        }
    }
}

/// Run the editor, writing output to `writer` and reading events from `event_src`.
///
/// For CLI use, pass `io::stdout()` and `CrosstermEventSource`.
/// For hsh-ios, pass `TauriWriter` and the IPC event queue.
pub fn run<W: Write>(
    writer: W,
    config: CozyConfig,
    event_src: &mut dyn EventSource,
) -> io::Result<()> {
    let backend = CrosstermBackend::new(writer);
    let mut terminal = if let Some((cols, rows)) = config.terminal_size {
        use ratatui::layout::Rect;
        Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(Rect::new(0, 0, cols, rows)),
            },
        )?
    } else {
        Terminal::new(backend)?
    };

    let mut editor = match config.config_dir.as_ref() {
        Some(dir) => EditorState::new_with_config_dir(config.filename, Some(dir)),
        None => EditorState::new(config.filename),
    };

    event_loop::run(&mut terminal, &mut editor, event_src)?;

    Ok(())
}
