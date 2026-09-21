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
// Copied from ratatui-markdown 0.3.6; see its mod.rs for why and what changed.
// Upstream code is kept close to its original shape, so unused parts and its own
// lint choices are allowed here rather than rewritten.
#[allow(dead_code, unused_imports, clippy::all)]
mod vendor;

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

/// 開く前に断る物があるか（`#19`）。
///
/// 🚨 **`czv` をファイル名なしで打つと、起動画面（編集できる cozy）が出ていた** ——
/// `create_editor` は「中身が無いときは閲覧を掛けない」（空を読む面を避ける）ので、
/// ページャーを頼んだ人に編集画面が出る。⭐ `less` と同じく**言って終わる**。
///
/// ⭐ **判断は lib に置く** —— argotty は `host.rs` を通らず `run()` を直に呼ぶので、
/// CLI の入口だけで弾くと埋め込み先には効かない。
/// 📌 文言に**コマンド名を入れない**（`missing filename`）—— 名乗るのは呼んだ側
/// （CLI の `czv` / argotty の横取り）。入れると `czv: czv: …` と二重になる。
///
/// ⚠️ `cozy`（編集）を引数なしで打ったときは**今のまま起動画面**（🧑 の話は `czv` だけ）。
pub(crate) fn missing_input(config: &CozyConfig) -> Option<io::Error> {
    (config.view && config.filename.is_none())
        .then(|| io::Error::new(io::ErrorKind::InvalidInput, "missing filename"))
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
    // ⭐ 端末を作る**前**に断る —— 何も描かずに戻るので、埋め込み先の画面も汚れない。
    if let Some(e) = missing_input(&config) {
        return Err(e);
    }
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

#[cfg(test)]
mod missing_input_tests {
    use super::*;

    fn config(view: bool, filename: Option<&str>) -> CozyConfig {
        CozyConfig {
            view,
            filename: filename.map(str::to_string),
            ..Default::default()
        }
    }

    /// ⭐ これが `#19` の本体 —— ファイル名なしの `czv` は開かずに断る。
    #[test]
    fn czv_without_a_file_is_refused() {
        let e = missing_input(&config(true, None)).expect("断るはず");
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            e.to_string(),
            "missing filename",
            "呼んだ側が `czv: ` を付ける前提の文言"
        );
    }

    /// ⚠️ **陰性対照** —— ファイル名が在れば断らない。
    /// 🚨 これが無いと「`view` なら常に断る」実装が上を通る。
    #[test]
    fn czv_with_a_file_opens() {
        assert!(missing_input(&config(true, Some("notes.md"))).is_none());
    }

    /// ⭐ `czv -`（明示の標準入力）でも断らない（`#15`）。
    #[test]
    fn czv_with_dash_opens() {
        assert!(missing_input(&config(true, Some("-"))).is_none());
    }

    /// 📌 `cozy`（編集）を引数なしで打ったときは**今のまま起動画面**（🧑 の話は `czv` だけ）。
    #[test]
    fn cozy_without_a_file_still_opens_the_start_screen() {
        assert!(missing_input(&config(false, None)).is_none());
    }

    /// ⭐ **`run()` 自身が断る**（argotty は `host.rs` を通らない）—— 端末を作る前に戻るので、
    /// 書き込み先には 1 バイトも出ない。
    #[test]
    fn run_refuses_before_writing_anything() {
        struct NoEvents;
        impl EventSource for NoEvents {
            fn poll(&mut self, _: std::time::Duration) -> io::Result<bool> {
                panic!("イベントを読みに行ってはいけない")
            }
            fn read(&mut self) -> io::Result<crossterm::event::Event> {
                panic!("イベントを読みに行ってはいけない")
            }
        }
        let mut out = Vec::new();
        let err = run(&mut out, config(true, None), &mut NoEvents).expect_err("断るはず");
        assert_eq!(err.to_string(), "missing filename");
        assert!(out.is_empty(), "断る前に {} バイト書いている", out.len());
    }
}
