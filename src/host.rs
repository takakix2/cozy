use crate::input::CrosstermEventSource;
use crate::{CozyConfig, run, utils};
use crossterm::{
    cursor::SetCursorStyle,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io::{self, IsTerminal};

pub(crate) struct TerminalSession {
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
    bracketed_paste_enabled: bool,
}

pub(crate) struct TerminalSessionConfig {
    pub(crate) enable_raw_mode: bool,
    pub(crate) enable_alternate_screen: bool,
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
    pub(crate) fn enter(config: TerminalSessionConfig) -> io::Result<Self> {
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
        // The event loop turns wheel reporting off on its way out, but not past a
        // panic — a shell left reporting would echo `^[[<64;…M` on every notch.
        {
            use std::io::Write;
            let mut stdout = io::stdout();
            let _ = stdout.write_all(crate::event_loop::WHEEL_OFF);
            let _ = stdout.flush();
        }
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
    // ⭐ `czv`（view: true）でパイプ入力（stdin が tty でない）または明示の "-" の場合、
    // 標準入力からの読み込みとして受け入れる（#15）。
    // ⚠️ 編集面（view: false）では受けない —— 保存先のない編集面は作らない（#15 決定事項）。
    let filename = if view
        && (filename.as_deref() == Some("-") || (filename.is_none() && !io::stdin().is_terminal()))
    {
        Some("-".to_string())
    } else {
        filename
    };
    // ⭐ 絵なら画像ビューアで開く（`#20` Phase 1・czv だけ・feature `imageview`）。マジックバイトで決める ——
    // ファイルは先頭 16 バイトを覗くだけ・`-` はここで読み切る（覗き戻しが効かないので、
    // 絵でなかったバイト列は file_io の受け渡しスロット経由で文章の道へ戻す）。
    #[cfg(feature = "imageview")]
    if view {
        match crate::imageview::startup_source(filename.as_deref()) {
            crate::imageview::Startup::Image(src) => return crate::imageview::run(src),
            crate::imageview::Startup::StdinText(bytes) => {
                crate::file_io::set_prefetched_stdin(bytes);
            }
            crate::imageview::Startup::NotImage => {}
        }
    }
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

/// 第 1 引数を返す。`--version` / `-V` なら版を、`--help` / `-h` なら使い方を出して `None`
/// （＝もう走らない）。⭐ どちらも**端末に触る前**に答える（raw モードにも代替画面にも入らない）。
///
/// ⚠️ **`name` で名乗りを変える** —— `czv --version` が `cozy` と名乗ると、
/// どちらを入れたのか分からなくなる。📌 版番号は同じ（同じクレートから焼かれる）。
fn args_or_version(name: &str) -> Option<Option<String>> {
    match cli_arg(std::env::args().nth(1)) {
        CliArg::Version => {
            println!("{} {}", name, crate::VERSION);
            None
        }
        CliArg::Help => {
            print!("{}", usage(name));
            None
        }
        CliArg::File(file) => Some(file),
    }
}

/// 第 1 引数の読み方。`std::env::args` から切り離してある ＝ 検体で撃てる。
#[derive(Debug, PartialEq, Eq)]
enum CliArg {
    Version,
    Help,
    File(Option<String>),
}

/// 🆕 **`--help` / `-h` は `#23`（2026-10-10）** —— それまでは `--version` 以外を全部ファイル名にしていたので、
/// `cozy --help` は `--help` という名前のファイルを開こうとした（端末が無ければ `Device not configured`）。
/// ⚠️ 見るのは**完全一致の 4 つだけ** —— `-v` や `--versions` は今までどおりファイル名（`./-v` と書かなくても開ける）。
fn cli_arg(first: Option<String>) -> CliArg {
    match first.as_deref() {
        Some("--version" | "-V") => CliArg::Version,
        Some("--help" | "-h") => CliArg::Help,
        _ => CliArg::File(first),
    }
}

/// `cozy --help` / `czv --help` の本文（末尾に改行あり・`\n` 区切り）。
///
/// ⭐ **埋め込みホストも同じ本文を出す**ために公開している（argotty はアプリ内の `cozy` を横取りする・
/// argotty `#117`）—— 文言を 2 か所で持たない。`name` は打たれた方（`cozy` / `czv`）。
/// 📏 **どの行も 36 桁以内**（`usage_fits_a_phone_in_portrait`）—— だから名前と説明を縦に積む。
/// 🚨 **最初は 43 桁で作って、実機で 5 行折れた**（2026-10-10）—— 43 は argotty の `hsh --help` の検体が
/// **iPhone 17 Pro の sim** で測った幅で、📏 **iPhone XS（argotty の既定のフォント）は 37 桁**だった。
/// ⭐ ∴ 物差しは「借りた数字」ではなく**手元で一番狭い実機**から取る（37 に 1 桁の余裕）。
/// 📌 鍵は**既定値**（`shortcuts.toml` で変えられる）なので、そう書く。全部は中の Help（`Ctrl+H`）が持つ。
pub fn usage(name: &str) -> String {
    let pictures = if cfg!(feature = "imageview") {
        " (pictures too)"
    } else {
        ""
    };
    format!(
        "{name} {version}\n\
         Type like nano, move like vim.\n\
         Usage:\n  cozy [FILE]\n    Edit FILE (none: start screen)\n  \
         czv [FILE]\n    Read FILE{pictures}\n    `czv -` or a pipe reads stdin\n\
         Options:\n  -h, --help     Print this, exit\n  -V, --version  Print version, exit\n\
         Keys (defaults; Ctrl+H lists all):\n  Ctrl+S save · Ctrl+X exit\n  Ctrl+Q quit without saving\n\
         A file whose name starts with '-':\n  cozy ./-name\n",
        version = crate::VERSION,
    )
}

/// Run with CLI terminal setup using an explicit configuration.
pub fn run_cli_with_config(config: CozyConfig) -> io::Result<()> {
    // ⭐ 端末に触る**前**に断る（`#19`）—— `run()` にも同じ判定が在るが、そこまで行くと
    // raw モードと代替画面に一度入ってから出ることになり、画面が一瞬ちらつく。
    if let Some(e) = crate::missing_input(&config) {
        return Err(e);
    }
    let _session = TerminalSession::enter(TerminalSessionConfig::from(&config))?;
    // ⭐ 端末の画像能力は**ここで 1 度だけ**測る（raw モードに入った後・描画ループに入る前）。
    // Markdown プレビューのインライン画像（`#20` Phase 2）が使う。埋め込みホストはこの道を
    // 通らないので caps は空のまま ＝ 画像は出ず文字フォールバック（`inline.rs` の doc）。
    #[cfg(feature = "imageview")]
    crate::imageview::inline::capture_caps();
    let mut event_src = CrosstermEventSource;
    run(io::stdout(), config, &mut event_src)
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn the_four_flags_are_read_and_everything_else_is_a_file() {
        assert_eq!(cli_arg(Some("--version".into())), CliArg::Version);
        assert_eq!(cli_arg(Some("-V".into())), CliArg::Version);
        assert_eq!(cli_arg(Some("--help".into())), CliArg::Help);
        assert_eq!(cli_arg(Some("-h".into())), CliArg::Help);
        assert_eq!(cli_arg(None), CliArg::File(None));
        for f in ["notes.md", "-v", "--versions", "--helpme", "-", "./--help"] {
            assert_eq!(
                cli_arg(Some(f.into())),
                CliArg::File(Some(f.into())),
                "{f:?}"
            );
        }
    }

    /// 📏 iPhone XS 縦の端末は **37 桁**（2026-10-10 実機・argotty の既定フォント）→ 1 桁残して 36。
    /// ⚠️ argotty の `help_fits_a_phone_in_portrait` の 43 は 17 Pro sim の幅 ＝ XS では折れる。
    /// ⚠️ 数えるのは**文字数**（`—` も `·` も 1 桁）。名前は長い方の `cozy` で測る。
    #[test]
    fn usage_fits_a_phone_in_portrait() {
        let text = usage("cozy");
        for line in text.lines() {
            assert!(
                line.chars().count() <= 36,
                "{} 桁: {line:?}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn usage_names_the_typed_command_and_both_flags() {
        assert!(usage("czv").starts_with(&format!("czv {}\n", crate::VERSION)));
        let text = usage("cozy");
        assert!(text.contains("--help") && text.contains("--version"));
        assert!(text.ends_with('\n'));
    }
}
