use crate::event_loop::CrosstermEventSource;
use crate::{CozyConfig, run, utils};
use crossterm::{
    cursor::SetCursorStyle,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io;

struct TerminalSession;

impl TerminalSession {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, SetCursorStyle::SteadyBar)?;
        utils::terminal::enable_bracketed_paste()?;
        Ok(Self)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = utils::terminal::disable_bracketed_paste();
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

/// Convenience: run with full terminal setup (CLI binary entry point).
pub fn run_cli(filename: Option<String>) -> io::Result<()> {
    let _session = TerminalSession::enter()?;
    let config = CozyConfig {
        filename,
        ..Default::default()
    };
    let mut event_src = CrosstermEventSource;
    run(io::stdout(), config, &mut event_src)
}
