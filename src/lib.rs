mod action;
mod browse;
mod clipboard_io;
mod commands;
mod config_io;
mod event_loop;
mod file_io;
mod glide;
mod host;
mod input;
mod reducer;
mod runtime_env;
mod shortcuts;
mod state;
mod swap;
mod ui;
mod utils;

/// **設定ファイルの住所を答えるのは cozy 自身**。
///
/// ⚠️ 埋め込むホスト（argo）が同じ判定を書き写すと、**同じ問いに 2 つの答え**ができる。
/// ⭐ 既に `CozyConfig::config_dir` で「上書きできる」ことは公開しているので、
/// **「上書きしなければどこか」も公開する**のが対（片方だけ公開している状態だった）。
pub use config_io::user_config_path;
pub use host::{
    run_cli, run_cli_from_env, run_cli_view, run_cli_view_from_env, run_cli_with_config,
};
pub use input::{CrosstermEventSource, EventSource};
use ratatui::{Terminal, backend::CrosstermBackend};
use state::EditorState;
use state::editor::EditorStateInit;
use std::io::{self, Write};
use std::path::PathBuf;

/// Configuration for embedding cozy in a host application.
pub struct CozyConfig {
    /// File to open on launch.
    pub filename: Option<String>,
    /// Override config file search directory (e.g. iOS Documents/.hsh/).
    /// `None` uses the default XDG / home-dir search.
    pub config_dir: Option<PathBuf>,
    /// CLI host setup only: set to false when the host already owns raw mode.
    /// `run()` does not toggle raw mode; this is consumed by `run_cli_with_config()`.
    pub enable_raw_mode: bool,
    /// CLI host setup only: set to false when the host manages the screen buffer.
    /// `run()` does not enter/leave the alternate screen; this is consumed by
    /// `run_cli_with_config()`.
    pub enable_alternate_screen: bool,
    /// Terminal size (cols, rows). Required when the host is not a real TTY.
    /// `None` lets ratatui detect the size via ioctl (CLI use).
    pub terminal_size: Option<(u16, u16)>,
    /// 閲覧だけで開く（`czv`）。⭐ **利用者の `default_mode` より強い** ——
    /// `default_mode = "glide"` の人が `czv` で編集面に降りては困る。
    ///
    /// 📌 立てると `EditorMode::View` で開き、`read_only` も立つ
    /// （**ファイルの mode ビットに関係なく** —— 利用者が「見るだけ」と言ったから）。
    pub view: bool,
}

impl Default for CozyConfig {
    fn default() -> Self {
        Self {
            filename: None,
            config_dir: None,
            enable_raw_mode: true,
            enable_alternate_screen: true,
            terminal_size: None,
            view: false,
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
    let mut terminal = create_terminal(writer, config.terminal_size)?;
    let mut editor = create_editor(config);

    event_loop::run(&mut terminal, &mut editor, event_src)?;

    Ok(())
}

fn create_terminal<W: Write>(
    writer: W,
    terminal_size: Option<(u16, u16)>,
) -> io::Result<Terminal<CrosstermBackend<W>>> {
    let backend = CrosstermBackend::new(writer);
    if let Some((cols, rows)) = terminal_size {
        use ratatui::layout::Rect;
        Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(Rect::new(0, 0, cols, rows)),
            },
        )
    } else {
        Terminal::new(backend)
    }
}

fn create_editor(config: CozyConfig) -> EditorState {
    let view = config.view;
    let mut editor = EditorState::from_init(EditorStateInit::from_runtime(
        config.filename,
        config.config_dir,
    ));
    // ⭐ **`view` は `default_mode` より後に、上書きで効かせる。**
    // 🚨 順序が逆だと `default_mode = "glide"` の人が `czv` で編集面に降りる。
    //
    // ⚠️ **中身が無いときは掛けない** —— ファイルを開けなかった（または引数が無い）
    // ときは起動画面が出る。そこを `View` にすると**空を読む面**になる。
    //
    // 📌 `read_only` は**ファイルの mode ビットに関係なく**立てる。
    // ⭐ 立てる理由が違う —— あちらは「書けないファイルだから」、こちらは
    // **「利用者が見るだけだと言ったから」**。帯に出る印は同じでよい（`#14`）。
    if view && editor.filename.is_some() {
        // ⭐ **席の性質**を立てるのが先。`home_mode()` がこれを見るので、
        // 検索やヘルプから戻ったときも**閲覧へ帰る**。
        // 🚨 `mode` だけ変えると `czv` → `/` → `Esc` で編集面に降りる（実機で踏んだ）。
        editor.view_session = true;
        editor.mode = crate::state::EditorMode::View;
        editor.read_only = true;
    }
    editor
}
