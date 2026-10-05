//! 端末内画像ビューア（`#20` Phase 1 —— `czv photo.png` / `curl … | czv`）。
//!
//! `examples/czv_image.rs` の PoC（2026-09-27・Ghostty/Kitty で実測済み）を本番化した物。
//! プロトコルの選択は `ratatui-image` の `Picker`（端末への問い合わせ）に任せる ——
//! Ghostty では Kitty・argotty では Sixel が**自動で**選ばれる（argotty `#110` で実測）。
//!
//! # Sixel だけは 2 段構え（🧑 決定「エンコーダは 4096 色ディザで」）
//!
//! `Picker` が Sixel を選んだら、まず **XTSMGRAPHICS SET**（`CSI ? 1;3;4096 S`）で
//! 受け手に 4096 レジスタを頼む。通れば自前の [`sixel4096`]（16³ 均等キューブ＋
//! Floyd–Steinberg）、通らなければ `ratatui-image` の既定（icy_sixel の 256 色適応）へ。
//! ⭐ **交渉は in-band** なので ssh / berthd 越しでも env の伝搬なしに効く —— DA1 で
//! Sixel を見つけるのと同じ線。xterm.js の addon-image は SET を実装している
//! （`_xtermGraphicsAttributes`・上限 4096）し、本物の xterm も同じ作法を受ける。

pub(crate) mod inline;
mod sixel4096;

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use fast_image_resize::images::{Image as FirImage, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, GenericImageView, ImageReader, RgbaImage};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    buffer::{Buffer, CellDiffOption},
    layout::{Constraint, Direction, Layout, Position, Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use ratatui_image::{
    Image as ImageWidget, Resize,
    picker::{Picker, ProtocolType},
    protocol::Protocol,
};

/// 開く絵の出どころ。
pub(crate) enum ImageSource {
    Path(PathBuf),
    /// パイプ（`czv -`）—— 既に読み切ったバイト列。
    Memory {
        bytes: Vec<u8>,
        label: String,
    },
}

/// `czv` 起動時の分岐の答え（`host.rs` の `run_cli_mode` が訊く）。
pub(crate) enum Startup {
    /// 絵だった —— ビューアで開く。
    Image(ImageSource),
    /// stdin を読んだが絵ではなかった —— このバイト列を**文章の道へ運ぶこと**
    /// （stdin はもう空なので、捨てると「空のパイプ」に化ける）。
    StdinText(Vec<u8>),
    /// 絵ではない（またはまだ何も読んでいない）—— 従来どおりの道へ。
    NotImage,
}

/// マジックバイトで画像形式を言い当てる。**拡張子は見ない** —— 嘘をつくのは名前の方
/// （`curl` の落とし物や `czv -` には名前がそもそも無い）。
pub(crate) fn sniff_image(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("PNG");
    }
    if head.starts_with(b"\xFF\xD8\xFF") {
        return Some("JPEG");
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some("GIF");
    }
    if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        return Some("WebP");
    }
    if head.starts_with(b"BM") && head.len() >= 14 {
        return Some("BMP");
    }
    // ICO: 予約 0・種別 1・枚数 ≥1、最初の項の予約バイト（9）も 0。⚠️ 先頭 4 バイトだけだと
    // 素のバイナリを拾いやすいので、枚数と項の予約まで見る。
    if head.len() >= 10
        && head.starts_with(b"\x00\x00\x01\x00")
        && u16::from_le_bytes([head[4], head[5]]) >= 1
        && head[9] == 0
    {
        return Some("ICO");
    }
    None
}

/// `czv` の起動引数から「絵か・文章か」を決める。
///
/// - ファイル: 先頭 16 バイトだけ覗く（文章の道は従来どおり自分で読み直す）。
///   開けない・短い・絵でない —— どれも `NotImage`（断り方は文章の道の持ち物）。
/// - `-`: ここで**読み切るしかない**（覗き戻しの効かない口）。上限は文章側と同じ
///   [`crate::file_io::MAX_PIPE_BYTES`]。
pub(crate) fn startup_source(filename: Option<&str>) -> Startup {
    match filename {
        Some("-") => {
            let mut bytes = Vec::new();
            // ⚠️ 上限は文章側（`load_stdin_document`）と**同じ +1 で**読む —— 文章へ回した
            // バイト列から truncated の判定がそのまま出るように（ここで丸めると
            // 「切られた」印が黙って消える）。
            let mut take = io::stdin().take((crate::file_io::MAX_PIPE_BYTES + 1) as u64);
            if take.read_to_end(&mut bytes).is_err() {
                return Startup::StdinText(bytes);
            }
            if sniff_image(&bytes).is_some() {
                Startup::Image(ImageSource::Memory {
                    bytes,
                    label: "(stdin)".to_string(),
                })
            } else {
                Startup::StdinText(bytes)
            }
        }
        Some(path) => {
            let expanded = crate::file_io::expand_tilde(path);
            let mut head = [0u8; 16];
            let n = std::fs::File::open(&expanded)
                .and_then(|mut f| f.read(&mut head))
                .unwrap_or(0);
            if sniff_image(&head[..n]).is_some() {
                Startup::Image(ImageSource::Path(expanded))
            } else {
                Startup::NotImage
            }
        }
        None => Startup::NotImage,
    }
}

/// 表示倍率（`z` で切替）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Zoom {
    /// アスペクト比維持で枠に収める（既定・等倍以下へのみ縮小）。
    Fit,
    /// 等倍。枠より大きければ中央を切り出す。
    Actual,
}

/// sixel の受け手が 4096 レジスタを受けたか（交渉の結果）。
enum SixelLane {
    /// 自前の 4096 色エンコーダ（[`sixel4096`]）。
    Hi,
    /// `ratatui-image` の既定（icy_sixel 256 色適応＋ディザ）。
    Plain,
}

enum Rendered {
    /// ratatui-image のプロトコル実装（Kitty / iTerm2 / Sixel(256) / Halfblocks）。
    Proto(Protocol),
    /// 自前 sixel（エスケープ列と、占めるセル数）。
    Escape { data: String, cells: Size },
}

/// 同フォルダの画像を ←/→ でめくるための並び（`#20`・案A）。
struct Gallery {
    entries: Vec<PathBuf>,
    index: usize,
}

struct Viewer {
    label: String,
    image: DynamicImage,
    picker: Picker,
    sixel_lane: SixelLane,
    zoom: Zoom,
    rendered: Option<Rendered>,
    encode_ms: f64,
    last_area: Rect,
    /// パスで開いたときだけ在る（パイプ入力には無い）。
    gallery: Option<Gallery>,
}

/// 絵をビューアで開く（`q`/`Esc` で閉じる・`z` で fit↔等倍・`←`/`→` で同フォルダの前後へ）。
pub(crate) fn run(source: ImageSource) -> io::Result<()> {
    let (image, label, gallery) = load_source(source)?;

    let _session = crate::host::TerminalSession::enter(crate::host::TerminalSessionConfig {
        enable_raw_mode: true,
        enable_alternate_screen: true,
    })?;

    // 端末のプロトコルとセル寸法を問い合わせる（raw モードが要る）。
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    // Sixel のときだけ: 4096 レジスタを in-band で頼む。
    let sixel_lane = if picker.protocol_type() == ProtocolType::Sixel
        && negotiate_sixel_registers().is_some_and(|n| n >= 4096)
    {
        SixelLane::Hi
    } else {
        SixelLane::Plain
    };

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut viewer = Viewer::new(label, image, picker, sixel_lane, gallery);
    view_loop(
        &mut terminal,
        &mut viewer,
        &mut crate::input::CrosstermEventSource,
    )
}

/// 埋め込みホストから絵を開く（`#22`）—— 端末に**訊かず**、ホストが渡した能力で描く。
///
/// ⭐ CLI の [`run`] との違いは 3 つだけ: 能力は `caps` から（tty が無いので問い合わせが往復しない）、
/// 書く先はホストの `writer`、読む先はホストの `events`。raw モードと代替画面はホストの持ち物
/// （`run()` と同じ・argotty は自分で切り替える）。描画とキー処理は [`view_loop`] を共有する。
pub(crate) fn run_embedded<W: io::Write>(
    path: PathBuf,
    writer: W,
    events: &mut dyn crate::input::EventSource,
    caps: &crate::ImageCaps,
    terminal_size: Option<(u16, u16)>,
) -> io::Result<()> {
    let (image, label, gallery) = load_source(ImageSource::Path(path))?;
    let (picker, sixel_hi) = picker_from_caps(caps);
    let sixel_lane = if sixel_hi {
        SixelLane::Hi
    } else {
        SixelLane::Plain
    };
    let backend = CrosstermBackend::new(writer);
    let mut terminal = match terminal_size {
        Some((cols, rows)) => Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(Rect::new(0, 0, cols, rows)),
            },
        )?,
        None => Terminal::new(backend)?,
    };
    let mut viewer = Viewer::new(label, image, picker, sixel_lane, gallery);
    view_loop(&mut terminal, &mut viewer, events)
}

/// ホストの申告から Picker を組む（`#22`）。2 つ目は「自前の 4096 色 sixel を使うか」。
///
/// ⚠️ `from_fontsize` は ratatui-image 9 で非推奨（「`from_query_stdio` を使え」）だが、
/// あちらは tty に訊く —— 埋め込みには tty が無いので、**訊かずに組む道はこれしか無い**。
/// プロトコルは直後に上書きする（`from_fontsize` は環境変数から当て推量するので、それは捨てる）。
pub(crate) fn picker_from_caps(caps: &crate::ImageCaps) -> (Picker, bool) {
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize(ratatui_image::FontSize::new(
        caps.cell_px.0.max(1),
        caps.cell_px.1.max(1),
    ));
    picker.set_protocol_type(match caps.protocol {
        crate::ImageProtocol::Sixel => ProtocolType::Sixel,
        crate::ImageProtocol::Kitty => ProtocolType::Kitty,
        crate::ImageProtocol::Iterm2 => ProtocolType::Iterm2,
    });
    let sixel_hi = caps.protocol == crate::ImageProtocol::Sixel && caps.sixel_registers >= 4096;
    (picker, sixel_hi)
}

/// 埋め込みの `czv <file>` が絵かどうか（`#22`）。絵ならパス（`~` は展開済み）。
///
/// ⚠️ `-`（stdin）は扱わない —— 埋め込みにはパイプの stdin が無い。
pub(crate) fn embedded_image_path(filename: &str) -> Option<PathBuf> {
    if filename == "-" {
        return None;
    }
    match startup_source(Some(filename)) {
        Startup::Image(ImageSource::Path(path)) => Some(path),
        _ => None,
    }
}

/// 出どころから 1 枚目を読む（パスなら同フォルダの並びも作る）。
fn load_source(source: ImageSource) -> io::Result<(DynamicImage, String, Option<Gallery>)> {
    // パスで開いたときは同フォルダを並べる。パイプ入力はその場の 1 枚だけ。
    match source {
        ImageSource::Path(path) => {
            let gallery = build_gallery(&path);
            let (image, label) = load_entry(&gallery.entries[gallery.index])?;
            Ok((image, label, Some(gallery)))
        }
        ImageSource::Memory { bytes, label } => {
            let image = ImageReader::new(std::io::Cursor::new(bytes))
                .with_guessed_format()?
                .decode()
                .map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("cannot decode image: {e}"),
                    )
                })?;
            Ok((image, label, None))
        }
    }
}

impl Viewer {
    fn new(
        label: String,
        image: DynamicImage,
        picker: Picker,
        sixel_lane: SixelLane,
        gallery: Option<Gallery>,
    ) -> Self {
        Self {
            label,
            image,
            picker,
            sixel_lane,
            zoom: Zoom::Fit,
            rendered: None,
            encode_ms: 0.0,
            last_area: Rect::default(),
            gallery,
        }
    }
}

/// 描いて、キーを読んで、を `q` / `Esc` / `Ctrl+C` まで繰り返す（CLI と埋め込みで共有）。
fn view_loop<W: io::Write>(
    terminal: &mut Terminal<CrosstermBackend<W>>,
    viewer: &mut Viewer,
    events: &mut dyn crate::input::EventSource,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, viewer))?;
        if !events.poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        // ⭐ 連打はまとめて飲む（通り過ぎた中間の絵は用意せず捨てる）。
        // キューに溜まった入力を全部読み、←/→ の**正味の移動量**を 1 回だけ適用する ——
        // → を押しっぱなしでも途中のページを 1 枚ずつ用意せず、最後の 1 枚へ直行する。
        let mut delta: isize = 0;
        let mut quit = false;
        let mut toggle_zoom = false;
        let mut resized: Option<(u16, u16)> = None;
        let mut first = Some(events.read()?);
        loop {
            let ev = match first.take() {
                Some(e) => e,
                None => {
                    if events.poll(std::time::Duration::ZERO)? {
                        events.read()?
                    } else {
                        break;
                    }
                }
            };
            match ev {
                Event::Key(key) => {
                    let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c');
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => quit = true,
                        _ if ctrl_c => quit = true,
                        KeyCode::Char('z') => toggle_zoom = !toggle_zoom,
                        KeyCode::Left => delta -= 1,
                        KeyCode::Right => delta += 1,
                        _ => {}
                    }
                }
                Event::Resize(cols, rows) => {
                    viewer.rendered = None;
                    resized = Some((cols, rows));
                }
                _ => {}
            }
        }
        if quit {
            break;
        }
        // 埋め込みの固定ビューポートは自分で広げ直す（`event_loop.rs` と同じ）。CLI では無害。
        if let Some((cols, rows)) = resized {
            terminal.resize(Rect::new(0, 0, cols, rows))?;
        }
        if toggle_zoom {
            viewer.zoom = match viewer.zoom {
                Zoom::Fit => Zoom::Actual,
                Zoom::Actual => Zoom::Fit,
            };
            viewer.rendered = None;
        }
        if delta != 0 {
            navigate(viewer, delta);
        }
    }
    Ok(())
}

/// 正味 `delta` 枚だけ移動して、その 1 枚を用意する（アトミック: 用意が済むまで
/// 今の表示は消さない。次の `draw` で丸ごと差し替わる）。壊れて読めない隣は飛ばさず止まる。
fn navigate(viewer: &mut Viewer, delta: isize) {
    let Some(g) = viewer.gallery.as_mut() else {
        return;
    };
    if g.entries.len() <= 1 {
        return;
    }
    let last = g.entries.len() as isize - 1;
    let target = (g.index as isize + delta).clamp(0, last) as usize;
    if target == g.index {
        return;
    }
    // 読めない隣は黙って無視（今の 1 枚のまま）。
    if let Ok((image, label)) = load_entry(&g.entries[target]) {
        g.index = target;
        viewer.image = image;
        viewer.label = label;
        viewer.rendered = None; // 次の draw で作り直し ＝ アトミック差し替え
    }
}

/// 1 ファイルを開いて（画像, 短縮名）を返す。
fn load_entry(path: &Path) -> io::Result<(DynamicImage, String)> {
    let label = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let image = ImageReader::open(path)?
        .with_guessed_format()?
        .decode()
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cannot decode image: {e}"),
            )
        })?;
    Ok((image, label))
}

/// 開いたファイルと**同じフォルダ**の画像を名前の自然順で並べ、開いた物の位置を見つける。
fn build_gallery(current: &Path) -> Gallery {
    let dir = current.parent().filter(|p| !p.as_os_str().is_empty());
    let mut entries: Vec<PathBuf> = dir
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && has_image_extension(p))
        .collect();
    entries.sort_by(|a, b| natural_cmp(&file_key(a), &file_key(b)));

    let index = match entries.iter().position(|p| p == current) {
        Some(i) => i,
        // 開いた物が一覧に無い（拡張子なしでマジックだけ等）→ 先頭に入れて単独扱い。
        None => {
            entries.insert(0, current.to_path_buf());
            0
        }
    };
    Gallery { entries, index }
}

fn file_key(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn has_image_extension(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| {
            matches!(
                e.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico"
            )
        })
}

/// 自然順比較（`img2 < img10`）。数字の塊は数値として、他は文字として比べる。
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let na: String = take_digits(&mut ai);
                    let nb: String = take_digits(&mut bi);
                    // 先頭ゼロを無視して数値比較。桁数が同じなら文字列で（安定）。
                    let va = na.trim_start_matches('0');
                    let vb = nb.trim_start_matches('0');
                    let ord = va.len().cmp(&vb.len()).then_with(|| va.cmp(vb));
                    if ord != Ordering::Equal {
                        return ord;
                    }
                } else {
                    let ord = ca.cmp(&cb);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    ai.next();
                    bi.next();
                }
            }
        }
    }
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut s = String::new();
    while let Some(&c) = it.peek() {
        if c.is_ascii_digit() {
            s.push(c);
            it.next();
        } else {
            break;
        }
    }
    s
}

/// XTSMGRAPHICS で色レジスタ数の引き上げを頼み、答えを返す（unix のみ・raw モード前提）。
///
/// 送るのは SET（`CSI ? 1;3;4096 S`）。答えは `CSI ? 1;0;<n> S`（0 = 成功）。
/// ⚠️ **答えない端末が居る**ので 250ms で諦めて `None`（＝ 256 色の既定へ落ちる）。
/// 📌 これは「狭い答えしか返さない計器」ではなく**交渉**: 返事が無い ＝ 頼みは
/// 聞かれなかったとみなすのが安全側（4096 を一方的に流すと 256 の端末で色が折り返す）。
#[cfg(unix)]
pub(super) fn negotiate_sixel_registers() -> Option<u32> {
    use std::io::Write;
    let mut stdout = io::stdout();
    stdout.write_all(b"\x1b[?1;3;4096S").ok()?;
    stdout.flush().ok()?;

    let deadline = Instant::now() + std::time::Duration::from_millis(250);
    let mut buf: Vec<u8> = Vec::with_capacity(32);
    loop {
        let remain = deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            return None;
        }
        let mut pfd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: fd 0 への poll/read。buf の領域は自前の Vec。
        let rc = unsafe { libc::poll(&mut pfd, 1, remain.as_millis() as i32) };
        if rc <= 0 {
            return None;
        }
        let mut byte = [0u8; 64];
        let n = unsafe { libc::read(0, byte.as_mut_ptr().cast(), byte.len()) };
        if n <= 0 {
            return None;
        }
        buf.extend_from_slice(&byte[..n as usize]);
        if let Some(regs) = parse_xtsmgraphics_reply(&buf) {
            return Some(regs);
        }
        // 'S' まで来たのに読めなかった ＝ エラー応答（`?1;3S` 等）。
        if buf.contains(&b'S') {
            return None;
        }
    }
}

#[cfg(not(unix))]
pub(super) fn negotiate_sixel_registers() -> Option<u32> {
    None
}

/// `… ESC [ ? 1 ; 0 ; <n> S …` から n を抜く。先頭に無関係のバイトが混ざっていても
/// （打鍵・他の問い合わせの残り）ESC から読み直す。
fn parse_xtsmgraphics_reply(buf: &[u8]) -> Option<u32> {
    let s = String::from_utf8_lossy(buf);
    for (i, _) in s.match_indices("\x1b[?") {
        let rest = &s[i + 3..];
        let Some(end) = rest.find('S') else { continue };
        let fields: Vec<&str> = rest[..end].split(';').collect();
        if fields.len() >= 3
            && fields[0] == "1"
            && fields[1] == "0"
            && let Ok(n) = fields[2].parse::<u32>()
        {
            return Some(n);
        }
    }
    None
}

fn draw(f: &mut Frame, v: &mut Viewer) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());
    let image_area = chunks[0];
    let status_area = chunks[1];

    if v.rendered.is_none() || v.last_area != image_area {
        update(v, image_area);
    }

    match &v.rendered {
        Some(Rendered::Proto(proto)) => {
            let cells = proto.size();
            f.render_widget(ImageWidget::new(proto), centered(image_area, cells));
        }
        Some(Rendered::Escape { data, cells }) => {
            render_escape(data, centered(image_area, *cells), f.buffer_mut());
        }
        None => {}
    }

    render_footer(f, v, status_area);
}

/// `cells` ぶんの矩形を `area` の中央に置く（はみ出すときは切る）。
fn centered(area: Rect, cells: Size) -> Rect {
    let w = cells.width.min(area.width);
    let h = cells.height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// 自前エスケープ列の描画 —— `ratatui-image` の `Sixel::render` と同じ作法:
/// 先頭セルに列ごと持たせ（幅 1 に固定）、残りのセルは diff から外す。
pub(super) fn render_escape(data: &str, area: Rect, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if let Some(cell) = buf.cell_mut(Position::from(area)) {
        cell.set_symbol(data)
            .set_diff_option(CellDiffOption::ForcedWidth(
                std::num::NonZeroU16::new(1).expect("1 is non-zero"),
            ));
    }
    let mut first = true;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if first {
                first = false;
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_diff_option(CellDiffOption::Skip);
            }
        }
    }
}

fn update(v: &mut Viewer, image_area: Rect) {
    v.last_area = image_area;
    if image_area.width == 0 || image_area.height == 0 {
        return;
    }
    let font = v.picker.font_size();
    let (font_w, font_h) = ((font.width as u32).max(1), (font.height as u32).max(1));
    let max_px_w = image_area.width as u32 * font_w;
    let max_px_h = image_area.height as u32 * font_h;
    let (orig_w, orig_h) = v.image.dimensions();

    // 倍率と素材を決める。fit は縮小のみ・等倍は中央切り出し。
    // ⚠️ Fit 側で `v.image.clone()` しないこと —— 12MP の絵なら再描画のたび 48MB 複製になる。
    let t0 = Instant::now();
    let (rgba, target_w, target_h) = match v.zoom {
        Zoom::Fit => {
            let scale = (max_px_w as f64 / orig_w as f64)
                .min(max_px_h as f64 / orig_h as f64)
                .min(1.0);
            let tw = ((orig_w as f64 * scale).round() as u32).max(1);
            let th = ((orig_h as f64 * scale).round() as u32).max(1);
            (resize_rgba(&v.image, tw, th), tw, th)
        }
        Zoom::Actual => {
            let cw = orig_w.min(max_px_w);
            let ch = orig_h.min(max_px_h);
            let x0 = (orig_w - cw) / 2;
            let y0 = (orig_h - ch) / 2;
            let crop = v.image.crop_imm(x0, y0, cw, ch);
            (resize_rgba(&crop, cw, ch), cw, ch)
        }
    };

    let cells = Size::new(
        (target_w.div_ceil(font_w) as u16).min(image_area.width),
        (target_h.div_ceil(font_h) as u16).min(image_area.height),
    );

    v.rendered = match v.sixel_lane {
        SixelLane::Hi if v.picker.protocol_type() == ProtocolType::Sixel => {
            let data = sixel4096::encode(&rgba, target_w, target_h);
            Some(Rendered::Escape { data, cells })
        }
        _ => {
            let dynimg = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(target_w, target_h, rgba).expect("resized rgba"),
            );
            v.picker
                .new_protocol(dynimg, cells, Resize::Fit(None))
                .ok()
                .map(Rendered::Proto)
        }
    };
    v.encode_ms = t0.elapsed().as_secs_f64() * 1000.0;
}

/// fast_image_resize（SIMD・Bilinear 固定）。PoC の実測: 1200×1920 → 枠内で 3.2ms。
pub(super) fn resize_rgba(src: &DynamicImage, target_w: u32, target_h: u32) -> Vec<u8> {
    let (sw, sh) = src.dimensions();
    let src_rgba = src.to_rgba8();
    if (sw, sh) == (target_w, target_h) {
        return src_rgba.into_raw();
    }
    let src_ref =
        ImageRef::new(sw, sh, src_rgba.as_raw(), PixelType::U8x4).expect("source image ref");
    let mut dst = FirImage::new(target_w, target_h, PixelType::U8x4);
    Resizer::new()
        .resize(
            &src_ref,
            &mut dst,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear)),
        )
        .expect("resize");
    dst.into_vec()
}

fn render_footer(f: &mut Frame, v: &Viewer, area: Rect) {
    let (ow, oh) = v.image.dimensions();
    let proto = match (v.picker.protocol_type(), &v.sixel_lane) {
        (ProtocolType::Sixel, SixelLane::Hi) => "Sixel 4096",
        (ProtocolType::Sixel, SixelLane::Plain) => "Sixel 256",
        (ProtocolType::Kitty, _) => "Kitty",
        (ProtocolType::Iterm2, _) => "iTerm2",
        (ProtocolType::Halfblocks, _) => "Halfblocks",
    };
    let zoom = match v.zoom {
        Zoom::Fit => "fit",
        Zoom::Actual => "1:1",
    };
    // フォルダ内の位置（複数あるときだけ）。
    let pos = v
        .gallery
        .as_ref()
        .filter(|g| g.entries.len() > 1)
        .map(|g| format!(" {}/{} ", g.index + 1, g.entries.len()));
    let mut left = vec![
        Span::styled(
            " czv ",
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ", v.label),
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
    ];
    if let Some(pos) = pos {
        left.push(Span::styled(
            pos,
            Style::default().bg(Color::Black).fg(Color::Yellow),
        ));
    }
    left.extend([
        Span::styled(
            format!(" {ow}x{oh} "),
            Style::default().bg(Color::Black).fg(Color::Gray),
        ),
        Span::styled(
            format!(" [{proto}] "),
            Style::default().bg(Color::Blue).fg(Color::White),
        ),
        Span::styled(
            format!(" {zoom} {:.1}ms ", v.encode_ms),
            Style::default().bg(Color::Magenta).fg(Color::White),
        ),
    ]);
    let multi = v.gallery.as_ref().is_some_and(|g| g.entries.len() > 1);
    let key = |s: &'static str| {
        Span::styled(
            s,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    };
    let dim = |s: String| Span::styled(s, Style::default().fg(Color::Gray));
    let mut right = vec![
        key(" q"),
        dim(":quit ".into()),
        key(" z"),
        dim(format!(":{zoom} ")),
    ];
    if multi {
        right.push(key(" ←→"));
        right.push(dim(":page ".into()));
    }
    let right_w = if multi { 34 } else { 20 };
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(right_w)])
        .split(area);
    f.render_widget(
        Paragraph::new(Line::from(left)).style(Style::default().bg(Color::Black)),
        split[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(right))
            .alignment(ratatui::layout::Alignment::Right)
            .style(Style::default().bg(Color::Black)),
        split[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 自然順 —— 数字は数値として比べる（`img2 < img10`・先頭ゼロ無視）。
    #[test]
    fn natural_sort_orders_pages_like_a_human() {
        use std::cmp::Ordering;
        assert_eq!(natural_cmp("img2.png", "img10.png"), Ordering::Less);
        assert_eq!(natural_cmp("p09.png", "p10.png"), Ordering::Less);
        assert_eq!(natural_cmp("a.png", "b.png"), Ordering::Less);
        assert_eq!(natural_cmp("img10.png", "img10.png"), Ordering::Equal);
        // 先頭ゼロの桁違いでも数値として同じ順に。
        assert_eq!(natural_cmp("007", "7"), Ordering::Equal);
        let mut v = vec!["10.png", "2.png", "1.png", "20.png"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["1.png", "2.png", "10.png", "20.png"]);
    }

    #[test]
    fn image_extensions_are_recognized_case_insensitively() {
        assert!(has_image_extension(Path::new("a.PNG")));
        assert!(has_image_extension(Path::new("b.jpeg")));
        assert!(has_image_extension(Path::new("c.WebP")));
        assert!(!has_image_extension(Path::new("notes.md")));
        assert!(!has_image_extension(Path::new("noext")));
    }

    /// 同フォルダを自然順で並べ、開いた物の位置を見つける。
    #[test]
    fn gallery_lists_siblings_and_finds_the_index() {
        let dir = std::env::temp_dir().join(format!("czv_gallery_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        for name in ["3.png", "1.png", "2.png", "readme.txt", "10.png"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let opened = dir.join("2.png");
        let g = build_gallery(&opened);
        // txt は除外・自然順。
        let names: Vec<String> = g
            .entries
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["1.png", "2.png", "3.png", "10.png"]);
        assert_eq!(g.index, 1, "開いた 2.png の位置");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// マジックの言い当て —— 各形式の先頭バイトと、絵でない物。
    #[test]
    fn magic_bytes_identify_each_format() {
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\n...."), Some("PNG"));
        assert_eq!(sniff_image(b"\xFF\xD8\xFF\xE0...."), Some("JPEG"));
        assert_eq!(sniff_image(b"GIF89a.........."), Some("GIF"));
        assert_eq!(sniff_image(b"RIFF\x00\x00\x00\x00WEBP...."), Some("WebP"));
        assert_eq!(
            sniff_image(b"BM\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"),
            Some("BMP")
        );
        assert_eq!(
            sniff_image(b"\x00\x00\x01\x00\x01\x00\x20\x20\x00\x00\x01\x00\x20\x00...."),
            Some("ICO")
        );
        // 枚数 0 の ICO もどきは絵と言わない。
        assert_eq!(
            sniff_image(b"\x00\x00\x01\x00\x00\x00\x20\x20\x00\x00\x01\x00\x20\x00...."),
            None
        );
        // ⚠️ AVIF は読めない（`Cargo.toml` の `image` の項）ので絵と言わない —— 言うと
        // ビューアへ回して decode で落ちる。
        assert_eq!(sniff_image(b"\x00\x00\x00\x1cftypavif...."), None);
        assert_eq!(sniff_image(b"#!/bin/sh\n......."), None);
        assert_eq!(sniff_image(b""), None);
        // ⚠️ 短すぎる断片は「絵」と言わない（RIFF だけでは WebP と決められない）。
        assert_eq!(sniff_image(b"RIFF"), None);
    }

    /// 返事のパース —— 成功・失敗・前置きの混入。
    #[test]
    fn xtsmgraphics_reply_parsing() {
        assert_eq!(parse_xtsmgraphics_reply(b"\x1b[?1;0;4096S"), Some(4096));
        // 失敗（status != 0）は None。
        assert_eq!(parse_xtsmgraphics_reply(b"\x1b[?1;3S"), None);
        // 打鍵などの前置きが混ざっても ESC から読み直す。
        assert_eq!(parse_xtsmgraphics_reply(b"zz\x1b[?1;0;1024S"), Some(1024));
        // 別の問い合わせの返事（DA1 など）は拾わない。
        assert_eq!(parse_xtsmgraphics_reply(b"\x1b[?62;4;9;22c"), None);
        assert_eq!(parse_xtsmgraphics_reply(b""), None);
    }

    /// 中央寄せ —— 偶数の余白は左右に割れ、はみ出しは切られる。
    #[test]
    fn centering_splits_margin_and_clamps() {
        let area = Rect::new(0, 0, 100, 40);
        let r = centered(area, Size::new(10, 10));
        assert_eq!((r.x, r.y, r.width, r.height), (45, 15, 10, 10));
        let r = centered(area, Size::new(200, 10));
        assert_eq!((r.x, r.width), (0, 100));
    }
}
