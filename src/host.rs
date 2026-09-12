use crate::input::CrosstermEventSource;
use crate::{CozyConfig, run, utils};
use crossterm::{
    cursor::SetCursorStyle,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io;

struct TerminalSession {
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
    bracketed_paste_enabled: bool,
}

struct TerminalSessionConfig {
    enable_raw_mode: bool,
    enable_alternate_screen: bool,
}

impl From<&CozyConfig> for TerminalSessionConfig {
    fn from(config: &CozyConfig) -> Self {
        Self {
            enable_raw_mode: config.enable_raw_mode,
            enable_alternate_screen: config.enable_alternate_screen,
        }
    }
}

impl TerminalSession {
    fn enter(config: TerminalSessionConfig) -> io::Result<Self> {
        let mut session = Self {
            raw_mode_enabled: false,
            alternate_screen_enabled: false,
            bracketed_paste_enabled: false,
        };

        if config.enable_raw_mode {
            enable_raw_mode()?;
            session.raw_mode_enabled = true;
        }

        let mut stdout = io::stdout();

        if config.enable_alternate_screen {
            execute!(stdout, EnterAlternateScreen)?;
            session.alternate_screen_enabled = true;
        }

        execute!(stdout, SetCursorStyle::SteadyBar)?;
        utils::terminal::enable_bracketed_paste()?;
        session.bracketed_paste_enabled = true;

        Ok(session)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.bracketed_paste_enabled {
            let _ = utils::terminal::disable_bracketed_paste();
        }
        if self.raw_mode_enabled {
            let _ = disable_raw_mode();
        }
        if self.alternate_screen_enabled {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
        }
    }
}

/// Convenience: run with full terminal setup (CLI binary entry point).
pub fn run_cli(filename: Option<String>) -> io::Result<()> {
    run_cli_mode(filename, false)
}

/// 閲覧だけで開く CLI 入口（`czv`）。⭐ **`cozy` と同じ lib を、モードだけ変えて呼ぶ。**
///
/// 📌 だから `czv` 側に引数解析は要らない —— `host.rs` はいま `--version` しか
/// 見ておらず、**フラグの概念が無い**。⚠️ ここへ `--view` のような旗を足すと、
/// その概念を 1 つ増やすことになる（`#14` で A4 を落とした理由）。
pub fn run_cli_view(filename: Option<String>) -> io::Result<()> {
    run_cli_mode(filename, true)
}

fn run_cli_mode(filename: Option<String>, view: bool) -> io::Result<()> {
    let config = CozyConfig {
        filename,
        view,
        ..Default::default()
    };
    run_cli_with_config(config)
}

/// Parse process arguments and run the CLI entry point.
pub fn run_cli_from_env() -> io::Result<()> {
    match args_or_version("cozy") {
        Some(filename) => run_cli(filename),
        None => Ok(()),
    }
}

/// `czv` の入口。⭐ `run_cli_from_env` と同じ引数の見方で、**開く面だけが違う**。
pub fn run_cli_view_from_env() -> io::Result<()> {
    match args_or_version("czv") {
        Some(filename) => run_cli_view(filename),
        None => Ok(()),
    }
}

/// 第 1 引数を返す。`--version` / `-V` なら版を名乗って `None`（＝もう走らない）。
///
/// ⚠️ **`name` で名乗りを変える** —— `czv --version` が `cozy` と名乗ると、
/// どちらを入れたのか分からなくなる。📌 版番号は同じ（同じクレートから焼かれる）。
fn args_or_version(name: &str) -> Option<Option<String>> {
    match std::env::args().nth(1) {
        Some(arg) if arg == "--version" || arg == "-V" => {
            println!("{} {}", name, env!("CARGO_PKG_VERSION"));
            None
        }
        other => Some(other),
    }
}

/// Run with CLI terminal setup using an explicit configuration.
pub fn run_cli_with_config(config: CozyConfig) -> io::Result<()> {
    let _session = TerminalSession::enter(TerminalSessionConfig::from(&config))?;
    let mut event_src = CrosstermEventSource;
    run(io::stdout(), config, &mut event_src)
}
