use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::state::EditorState;

// ── Welcome ───────────────────────────────────────────────────────────────────

/// `notice` は「引数で渡されたファイルを開けなかった」ことの申し送り。
///
/// 🚨 **Welcome には status bar が無い**（`editor_layout` が高さ 0 にしている ——
/// 起動画面を汚さないための意図的な設計）。∴ ここに描かないと、拒んだ理由は
/// **どこにも出ない**。cozy はファイルを守れても「なぜ開かなかったか」を言えず、
/// 利用者はタイプミスを疑うことになる（それが `Ctrl+O` 側の元の欠陥だった）。
pub fn render_welcome(f: &mut Frame, area: Rect, notice: Option<&str>) {
    // Compact/low-spec hosts (e.g. an Android tablet, even in wide landscape) get
    // the lightweight box instead of the heavy block-art logo — the wide art is
    // expensive to rasterize every frame on a full-repaint GPU.
    if area.width < 50 || crate::runtime_env::compact() {
        render_welcome_narrow(f, area, notice);
    } else {
        render_welcome_wide(f, area, notice);
    }
}

/// 申し送りを、箱の幅で折り返した赤い行にする。
/// ⚠️ 折り返さないと**パスの尻が切れる** —— 切れた先にファイル名が在る。
fn notice_lines(notice: &str, width: usize) -> Vec<Line<'static>> {
    let red = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    wrap_notice(notice, width)
        .into_iter()
        .map(|l| Line::from(Span::styled(l, red)))
        .collect()
}

/// 幅で折り返す。**語で折れないときは文字で割る。**
///
/// 🚨 `wrap_help_lines` は空白でしか折らない —— Help の行はどれも語の連なりなので
/// それで足りていた。ところが申し送りに載るのは**パス**で、これは空白を含まない 1 語。
/// ∴ 語単位のままだと折り返されずに描画枠で切られ、**いちばん見たい末尾（ファイル名）が
/// 消える**（狭い箱で実測 —— 網がそこで赤くなった）。
fn wrap_notice(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    for word in text.split_inclusive(' ') {
        let ww = UnicodeWidthStr::width(word);
        if cur_w + ww > width && !cur.is_empty() {
            out.push(std::mem::take(&mut cur).trim_end().to_string());
            cur_w = 0;
        }
        if ww > width {
            // 1 語が箱より長い（＝長いパス）。文字幅で刻む。
            for ch in word.chars() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if cur_w + cw > width && !cur.is_empty() {
                    out.push(std::mem::take(&mut cur).trim_end().to_string());
                    cur_w = 0;
                }
                cur.push(ch);
                cur_w += cw;
            }
            continue;
        }
        cur.push_str(word);
        cur_w += ww;
    }
    if !cur.is_empty() {
        out.push(cur.trim_end().to_string());
    }
    out
}

fn render_welcome_narrow(f: &mut Frame, area: Rect, notice: Option<&str>) {
    // Cap the box width so it doesn't stretch edge-to-edge on wide screens — a
    // compact/low-spec tablet routes here even in landscape, where a full-width
    // box looks stretched. On a real narrow phone (width <= cap) this is a no-op.
    const MAX_BOX_WIDTH: usize = 40;
    let w = (area.width as usize).min(MAX_BOX_WIDTH);
    let iw = w.saturating_sub(2); // inner width (border uses 1 char each side)

    let border_h = Style::default().fg(Color::DarkGray);
    let cyan_b = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let gray = Style::default().fg(Color::DarkGray);
    let yellow = Style::default().fg(Color::Yellow);

    let top = format!("┌{}┐", "─".repeat(iw));
    let bot = format!("└{}┘", "─".repeat(iw));

    // タイトル行: "cozy" シアン + " · editor" 白 をセンタリング
    // display width で計算（`·` は UTF-8 2バイトだが表示幅は1）
    let cozy_part = "cozy";
    let rest_part = " · editor";
    let title_dw = UnicodeWidthStr::width(cozy_part) + UnicodeWidthStr::width(rest_part);
    let pad = iw.saturating_sub(title_dw);
    let lpad = pad / 2;
    let rpad = pad - lpad;
    let editor_span = format!("{}{}", rest_part, " ".repeat(rpad));
    let title_line = Line::from(vec![
        Span::styled("│", border_h),
        Span::raw(" ".repeat(lpad)),
        Span::styled(cozy_part, cyan_b),
        Span::styled(editor_span, Style::default()),
        Span::styled("│", border_h),
    ]);

    let sub = "Comfort First TUI";
    let sub_dw = UnicodeWidthStr::width(sub);
    let sub_pad = iw.saturating_sub(sub_dw);
    let sub_l = sub_pad / 2;
    let sub_r = sub_pad - sub_l;
    let sub_line = Line::from(vec![
        Span::styled("│", border_h),
        Span::raw(" ".repeat(sub_l)),
        Span::styled(sub, gray),
        Span::raw(" ".repeat(sub_r)),
        Span::styled("│", border_h),
    ]);

    let col = w / 2;

    // ショートカットブロックの中央寄せ用 left pad
    // 左列 = col 文字固定、右列 = key_w(4)+1+desc(4) = 9 文字
    let sc_lpad = w.saturating_sub(col + 9) / 2;

    // "Enter: start editing" を端末幅で中央寄せ
    let enter_text = "Enter: start editing";
    let enter_lpad = w.saturating_sub(UnicodeWidthStr::width(enter_text)) / 2;
    let enter_line = Line::from(Span::styled(
        format!("{}{}", " ".repeat(enter_lpad), enter_text),
        yellow,
    ));

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(top, border_h)),
        title_line,
        sub_line,
        Line::from(Span::styled(bot, border_h)),
        Line::from(""),
        shortcut_pair("^O", "Open", "^S", "Save", col, sc_lpad),
        shortcut_pair("^X", "Exit", "^F", "Find", col, sc_lpad),
        shortcut_pair("^R", "Repl", "^H", "Help", col, sc_lpad),
        shortcut_pair("^Z", "Undo", "^Y", "Redo", col, sc_lpad),
        shortcut_pair("^B", "Browse", "^G", "Glide", col, sc_lpad),
        Line::from(""),
        enter_line,
    ];

    if let Some(notice) = notice {
        lines.push(Line::from(""));
        lines.extend(notice_lines(notice, w));
    }

    // 垂直センタリング
    let h = lines.len() as u16;
    let y = area.y + area.height.saturating_sub(h) / 2;
    // Center the (capped-width) box horizontally.
    let bw = w as u16;
    let x = area.x + area.width.saturating_sub(bw) / 2;
    let rect = Rect::new(x, y, bw, h.min(area.height)).intersection(area);
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Left), rect);
}

fn render_welcome_wide(f: &mut Frame, area: Rect, notice: Option<&str>) {
    let cyan = Style::default().fg(Color::Cyan);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let yellow = Style::default().fg(Color::Yellow);

    let width = 60u16.min(area.width);

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(" ██████╗ ██████╗ ███████╗██╗   ██╗", cyan)),
        Line::from(Span::styled("██╔════╝██╔═══██╗╚══███╔╝╚██╗ ██╔╝", cyan)),
        Line::from(Span::styled("██║     ██║   ██║  ███╔╝  ╚████╔╝ ", cyan)),
        Line::from(Span::styled("██║     ██║   ██║ ███╔╝    ╚██╔╝  ", cyan)),
        Line::from(Span::styled("╚██████╗╚██████╔╝███████╗   ██║   ", cyan)),
        Line::from(Span::styled(" ╚═════╝ ╚═════╝ ╚══════╝   ╚═╝   ", cyan)),
        Line::from(""),
        Line::from(Span::styled("cozy editor — Comfort First TUI", bold)),
        Line::from(""),
    ];

    // ⭐ ここに置くのは、**利用者がこの画面に居る理由**だから。ショートカット表より先に読む。
    if let Some(notice) = notice {
        lines.extend(notice_lines(notice, width as usize));
        lines.push(Line::from(""));
    }

    lines.extend([
        Line::from(format!(
            "{:<16}{:<17}{}",
            "Ctrl+O Open", "Ctrl+S Save", "Ctrl+X Exit"
        )),
        Line::from(format!(
            "{:<16}{:<17}{}",
            "Ctrl+F Find", "Ctrl+R Replace", "Ctrl+H Help"
        )),
        Line::from(format!(
            "{:<16}{:<17}{}",
            "Ctrl+Z Undo", "Ctrl+Y Redo", "Ctrl+J Jump"
        )),
        Line::from(format!(
            "{:<16}{:<17}{}",
            "Ctrl+B Browse", "Ctrl+G Glide", ""
        )),
        Line::from(""),
        Line::from(Span::styled("Press Enter to start editing...", yellow)),
        Line::from(""),
    ]);

    let height = (lines.len() as u16).min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let rect = Rect::new(x, y, width, height).intersection(area);

    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rect);
}

// ── Help ──────────────────────────────────────────────────────────────────────

pub fn render_help(editor: &mut EditorState, f: &mut Frame, area: Rect) {
    if area.width < 50 {
        render_help_narrow(editor, f, area);
    } else {
        render_help_wide(editor, f, area);
    }
}

fn render_help_narrow(editor: &mut EditorState, f: &mut Frame, area: Rect) {
    let w = area.width as usize;
    let col = w / 2;
    let hdr = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);

    let lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "cozy Help",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("── Edit Mode ──────────────", hdr)),
        shortcut_pair("^O", "Open", "^S", "Save", col, 0),
        shortcut_pair("^X", "Exit", "^F", "Find", col, 0),
        shortcut_pair("^R", "Replace", "^H", "Help", col, 0),
        shortcut_pair("^Z", "Undo", "^Y", "Redo", col, 0),
        shortcut_pair("^K", "Cutline", "^J", "Jump", col, 0),
        shortcut_pair("^A", "Ln start", "^E", "Ln end", col, 0),
        shortcut_pair("M-\\", "File top", "M-/", "File end", col, 0),
        Line::from("^B  → Browse folder tree"),
        Line::from("^G  → Glide mode (vim)"),
        Line::from(Span::styled("F1 Help  F2 Md  F3 Brws  F4 Diff", dim)),
        Line::from(""),
        Line::from(Span::styled("── View ──────────────────", hdr)),
        shortcut_pair("^L", "LineNo", "^W", "Wrap", col, 0),
        shortcut_pair("^U", "Footer", "^P", "Cmd Pal", col, 0),
        Line::from("^D  → Markdown preview"),
        Line::from(""),
        Line::from(Span::styled("── Glide: Move ────────────", hdr)),
        shortcut_pair("h/←", "Left", "l/→", "Right", col, 0),
        shortcut_pair("j/↓", "Down", "k/↑", "Up", col, 0),
        shortcut_pair("w", "Fwd wrd", "b", "Bck wrd", col, 0),
        shortcut_pair("e", "Wrd end", "0", "Ln start", col, 0),
        shortcut_pair("$", "Ln end", "^", "1st char", col, 0),
        shortcut_pair("gg", "Top", "G", "Bottom", col, 0),
        shortcut_pair("H", "Hi", "L", "Low", col, 0),
        shortcut_pair("M", "Mid", "+/-", "Nxt/Prv", col, 0),
        Line::from(Span::styled("> / < / t / T  char jump", dim)),
        Line::from(""),
        Line::from(Span::styled("── Glide: Edit ────────────", hdr)),
        shortcut_pair("i", "Insert", "a", "After", col, 0),
        shortcut_pair("I", "LnBeg", "A", "LnEnd", col, 0),
        shortcut_pair("o", "New↓", "O", "New↑", col, 0),
        shortcut_pair("x", "Del ch", "X", "Del prev", col, 0),
        shortcut_pair("~", "Case", "J", "Join", col, 0),
        Line::from(""),
        Line::from(Span::styled("── Glide: Ops ─────────────", hdr)),
        Line::from(Span::styled("d/c/y + motion:", dim)),
        shortcut_pair("d", "Delete", "c", "Change", col, 0),
        shortcut_pair("y", "Yank", "dd", "Delline", col, 0),
        shortcut_pair("cc", "Chgline", "yy", "Yank ln", col, 0),
        shortcut_pair("D", "Del→end", "C", "Chg→end", col, 0),
        shortcut_pair("p", "Paste↓", "P", "Paste↑", col, 0),
        Line::from(""),
        Line::from(Span::styled("Esc → back to Edit", dim)),
    ];

    render_help_body(editor, f, area, lines);
}

/// Help の 1 行を、**いま効いている鍵**から作る。
///
/// 🚨 Help はキー名をべた書きしていたので、`[keys]` の上書きに追随しなかった（`#9`）。
/// ⭐ 帯は 3〜5 枠しか出せないので、**キーの全体像を見る場所は Help しかない** ——
/// 上書きした人ほど Help を見るのに、そこが最も古かった。
///
/// ⚠️ **鍵は全部出す**（`Ctrl+B / F3`）。帯が 1 本だけ出すのとは役目が違う。
/// ⚠️ 鍵を 1 本も持たないアクションは**行ごと落とす** —— 押せない案内を残さない。
fn help_key_line(
    editor: &EditorState,
    actions: &[crate::shortcuts::EditorAction],
    text: &str,
) -> Option<String> {
    let mut keys = Vec::new();
    for a in actions {
        let mut k = crate::shortcuts::keys_for(
            &editor.shortcut_map,
            *a,
            crate::shortcuts::KeyStyle::Spelled,
        );
        if k.is_empty() {
            return None;
        }
        keys.append(&mut k);
    }
    // 既定の見た目（2 桁下げ + 16 桁の欄）に合わせる。
    Some(format!("  {:<16}{}", keys.join(" / "), text))
}

fn render_help_wide(editor: &mut EditorState, f: &mut Frame, area: Rect) {
    let cyan = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let gray = Style::default().fg(Color::Gray);
    let yel = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(Span::styled("cozy — Keyboard Shortcuts", cyan)),
        Line::from(""),
        Line::from(Span::styled(
            "Edit mode: just type.  Ctrl+G → Glide (vim-style).",
            gray,
        )),
        Line::from(Span::styled("Numbers repeat:  3j  5w  3dd", gray)),
        Line::from(""),
        Line::from(Span::styled("=== Edit Mode ===", yel)),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::EnterOpen],
            "Open file",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::EnterBrowse],
            "Browse folder tree (F3 for tmux)",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        help_key_line(editor, &[crate::shortcuts::EditorAction::EnterSave], "Save")
            .map(Line::from)
            .unwrap_or_else(|| Line::from("")),
        help_key_line(editor, &[crate::shortcuts::EditorAction::EnterExit], "Exit")
            .map(Line::from)
            .unwrap_or_else(|| Line::from("")),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::EnterSearch],
            "Find",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::EnterReplace],
            "Replace",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        help_key_line(
            editor,
            &[
                crate::shortcuts::EditorAction::Undo,
                crate::shortcuts::EditorAction::Redo,
            ],
            "Undo / Redo",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::DeleteLine],
            "Cut line",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::EnterGoto],
            "Jump to line",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        Line::from("  Ctrl+A / E      Line start / end"),
        Line::from("  Alt+\\ / Alt+/   File top / bottom"),
        Line::from("  Ctrl+Home/End   The same, without Alt"),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::EnterGlide],
            "Enter Glide mode",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        Line::from(""),
        Line::from(Span::styled("=== View ===", yel)),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::ToggleLineNumbers],
            "Toggle line numbers",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        Line::from("  Ctrl+W          Toggle line wrap"),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::ToggleFooter],
            "Toggle footer",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::ToggleMarkdownPreview],
            "Toggle Markdown preview",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        Line::from("  F4              Toggle diff review"),
        Line::from(""),
        Line::from(Span::styled("=== Global ===", yel)),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::EnterCommand],
            "Command palette",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        help_key_line(
            editor,
            &[crate::shortcuts::EditorAction::EnterHelp],
            "Help (F1 if Ctrl+H is taken)",
        )
        .map(Line::from)
        .unwrap_or_else(|| Line::from("")),
        Line::from(""),
        Line::from(Span::styled("=== Glide Mode — Movement ===", yel)),
        Line::from("  hjkl / arrows   Move cursor"),
        Line::from("  w / b / e       Fwd / back / end of word"),
        Line::from("  W / B / E       Same, WORD (whitespace)"),
        Line::from("  0 / ^ / $       Ln start / first non-blank / ln end"),
        Line::from("  gg / G          File top / bottom"),
        Line::from("  H / M / L       Scr hi / mid / low"),
        Line::from("  + / -           Next / prev line (first non-ws)"),
        Line::from("  > / < <char>    Jump to next / prev char"),
        Line::from("  t / T <char>    Jump just before / after char"),
        Line::from("  . / ,           Repeat last char jump fwd / back"),
        Line::from(""),
        Line::from(Span::styled("=== Glide Mode — Edit Entry ===", yel)),
        Line::from("  i / I           Insert at cursor / line start"),
        Line::from("  a / A           Append after cursor / line end"),
        Line::from("  o / O           Open line below / above"),
        Line::from(""),
        Line::from(Span::styled("=== Glide Mode — Operators ===", yel)),
        Line::from("  d / c / y       Delete / Change / Yank + motion"),
        Line::from("  dd / cc / yy    Operate on whole line  (3dd = 3 lines)"),
        Line::from("  D / C / Y       To end of line"),
        Line::from(""),
        Line::from(Span::styled("=== Glide Mode — Other ===", yel)),
        Line::from("  x / X           Delete char at / before cursor"),
        Line::from("  ~               Toggle case  (3~ = 3 chars)"),
        Line::from("  J               Join line with next"),
        Line::from("  p / P           Paste below / above"),
        Line::from("  f               Enter Find mode"),
        Line::from("  r               Enter Replace mode"),
        Line::from("  Esc             Return to Edit mode"),
        Line::from(""),
    ];

    render_help_body(editor, f, area, lines);
}

/// Render a Help page the same way the Markdown preview renders: pre-wrap to the
/// width, keep the cursor line on screen, and paint row-by-row with the current
/// line highlighted. Sharing this model means Help scrolls a page/line at a time
/// (one repaint per motion) instead of repainting every row on each key.
fn render_help_body(
    editor: &mut EditorState,
    f: &mut Frame,
    area: Rect,
    lines: Vec<Line<'static>>,
) {
    let lines = wrap_help_lines(lines, area.width as usize);
    editor.help_view_height = area.height as usize;
    editor.help_rendered_line_count = lines.len().max(1);

    let height = (area.height as usize).max(1);
    let max_line = lines.len().saturating_sub(1);
    let max_scroll = lines.len().saturating_sub(area.height as usize);
    editor.help_cursor_line = editor.help_cursor_line.min(max_line);

    let cursor = editor.help_cursor_line;
    let top = editor.help_scroll_offset;
    if cursor < top {
        editor.help_scroll_offset = cursor;
    } else if cursor >= top.saturating_add(height) {
        editor.help_scroll_offset = cursor.saturating_sub(height - 1);
    }
    editor.help_scroll_offset = editor.help_scroll_offset.min(max_scroll);

    for row in 0..area.height {
        let idx = editor.help_scroll_offset + row as usize;
        let mut line = lines.get(idx).cloned().unwrap_or_else(|| Line::from(""));
        let row_area = Rect {
            x: area.x,
            y: area.y + row,
            width: area.width,
            height: 1,
        };
        if idx == editor.help_cursor_line && idx <= max_line {
            f.render_widget(
                Block::default().style(Style::default().bg(Color::DarkGray)),
                row_area,
            );
            line = line.style(Style::default().bg(Color::DarkGray));
        }
        f.render_widget(Paragraph::new(line).alignment(Alignment::Left), row_area);
    }
}

/// Wrap any help line that overflows the width. Overflowing lines are the wide
/// layout's single-styled body text, so word-wrap on the plain text and reuse
/// the line's style for each continuation row; short/multi-span lines pass through.
fn wrap_help_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return lines;
    }
    let mut out: Vec<Line<'static>> = Vec::with_capacity(lines.len());
    for line in lines {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if UnicodeWidthStr::width(text.as_str()) <= width {
            out.push(line);
            continue;
        }
        let style = line.spans.first().map(|s| s.style).unwrap_or_default();
        let mut cur = String::new();
        let mut cur_w = 0usize;
        for word in text.split_inclusive(' ') {
            let ww = UnicodeWidthStr::width(word);
            if cur_w + ww > width && !cur.is_empty() {
                out.push(Line::from(Span::styled(std::mem::take(&mut cur), style)));
                cur_w = 0;
            }
            cur.push_str(word);
            cur_w += ww;
        }
        if !cur.is_empty() {
            out.push(Line::from(Span::styled(cur, style)));
        }
    }
    out
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// 2列ショートカット行を生成する。各列幅は `col` 文字。`lpad` は行頭スペース。
fn shortcut_pair(k1: &str, d1: &str, k2: &str, d2: &str, col: usize, lpad: usize) -> Line<'static> {
    let key_w = 4usize;
    let desc_w = col.saturating_sub(key_w + 1);
    let left = format!("{:<kw$} {:<dw$}", k1, d1, kw = key_w, dw = desc_w);
    let right = format!("{:<kw$} {}", k2, d2, kw = key_w);
    Line::from(format!("{}{}{}", " ".repeat(lpad), left, right))
}

/// Welcome が「開けなかった理由」を描くことの網。
///
/// 🚨 **これは飾りではない。** Welcome には status bar が無いので、ここに出ないと
/// 拒んだ理由は**どこにも出ない** —— ファイルは守れても、利用者にはただの起動画面に見える。
/// ⚠️ 広い版と狭い版は別の関数なので、**両方**撃つ（片方だけ直して緑になる形を潰す）。
#[cfg(test)]
mod welcome_notice_tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn welcome_text(width: u16, height: u16, notice: Option<&str>) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_welcome(f, Rect::new(0, 0, width, height), notice))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 折り返しで語が割れるので、**画面全体を 1 本の文字列に潰してから**探す。
    fn flattened(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn the_wide_welcome_says_why_the_file_did_not_open() {
        let text = welcome_text(100, 26, Some("Not UTF-8 text: sjis.txt (not opened)"));
        assert!(
            flattened(&text).contains("Not UTF-8 text: sjis.txt"),
            "{text}"
        );
    }

    #[test]
    fn the_narrow_welcome_says_it_too() {
        let text = welcome_text(44, 26, Some("Not UTF-8 text: sjis.txt (not opened)"));
        assert!(
            flattened(&text).contains("Not UTF-8 text: sjis.txt"),
            "{text}"
        );
    }

    /// 陽性対照。**申し送りが無いときの起動画面は 1 文字も変わらない** ——
    /// 「常に赤い行を足す」でも上の 2 本は緑になるので、これが無いと何も固定できない。
    #[test]
    fn an_ordinary_start_gets_no_extra_line() {
        for width in [100u16, 44] {
            assert_eq!(
                welcome_text(width, 26, None),
                welcome_text(width, 26, None),
                "決定的であること"
            );
            assert!(
                !welcome_text(width, 26, None).contains("Not UTF-8"),
                "理由が無いのに理由を出している (width={width})"
            );
        }
    }

    /// ⚠️ 長いパスでも**ファイル名の側が切れない**。切れた先に、利用者が探しているものが在る。
    #[test]
    fn a_long_path_wraps_instead_of_losing_its_tail() {
        let notice = "Not UTF-8 text: /Users/someone/very/deep/directory/tree/notes-from-2019.txt (not opened — the file is unchanged)";
        for width in [100u16, 44] {
            assert!(
                flattened(&welcome_text(width, 30, Some(notice))).contains("notes-from-2019.txt"),
                "width={width} で尻が消えた"
            );
        }
    }
}

/// 🚨 **Help はキーの全体像を見る場所** —— 帯が 3〜5 枠しか出せないので、
/// 上書きした人ほどここを見る。∴ ここが古いと、上書きした人が最も困る（`#9`）。
#[cfg(test)]
mod help_follows_the_keymap {
    use super::*;
    use crate::state::{Config, EditorState};
    use ratatui::{Terminal, backend::TestBackend};

    fn editor_with_keys(pairs: &[(&str, &str)]) -> EditorState {
        let mut editor = EditorState::new(Some("note.txt".to_string()));
        let mut config = Config::default_values();
        if !pairs.is_empty() {
            config.keys = Some(
                pairs
                    .iter()
                    .map(|(a, k)| (a.to_string(), k.to_string()))
                    .collect(),
            );
        }
        editor.shortcut_map = crate::shortcuts::build_shortcut_map(config.keys.as_ref());
        editor.config = config;
        editor
    }

    fn help_text(editor: &mut EditorState) -> String {
        let (w, h) = (100u16, 60u16);
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_help(editor, f, Rect::new(0, 0, w, h)))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 🚨 **本丸** —— 上書きすると Help の該当行が変わる。
    #[test]
    fn help_follows_an_override() {
        let plain = help_text(&mut editor_with_keys(&[]));
        assert!(plain.contains("Ctrl+S"), "既定の Help が違う");

        let mut e = editor_with_keys(&[("enter_save", "f9")]);
        let text = help_text(&mut e);
        assert!(
            text.contains("F9") && text.contains("Save"),
            "上書きしたのに Help が追随していない"
        );
    }

    /// ⭐ **主とフォールバックの両方が出る**（`Ctrl+B / F3` の形）。
    /// 上書きで主が変われば `F6 / F3` になる —— フォールバックは消えない。
    #[test]
    fn help_shows_the_primary_and_the_fallback() {
        let plain = help_text(&mut editor_with_keys(&[]));
        assert!(
            plain.contains("Ctrl+B / F3"),
            "主とフォールバックが並んでいない"
        );

        let mut e = editor_with_keys(&[("enter_browse", "f6")]);
        let text = help_text(&mut e);
        assert!(
            text.contains("F6 / F3"),
            "上書き後に「利用者の鍵 / フォールバック」になっていない"
        );
    }

    /// 🚨 **陽性対照。** 上書きが無ければ Help は既定の鍵を出す。
    /// これが無いと「全部消す」実装が緑で通る。
    ///
    /// ⚠️ **綴りは 2 か所で変わった**（意図的）—— 鍵から作るようになったので、
    /// **主が先・正式な綴り**で並ぶ:
    /// `F2 / Ctrl+D` → `Ctrl+D / F2`（`Ctrl+D` が主で `F2` がフォールバック）、
    /// `Ctrl+Z / Y` → `Ctrl+Z / Ctrl+Y`（手書きの短縮をやめた）。
    /// ⭐ 手で書いていた間は、並びも短縮も**書いた人の気分**だった。
    #[test]
    fn without_overrides_help_shows_the_default_keys() {
        let text = help_text(&mut editor_with_keys(&[]));
        for want in [
            "Ctrl+O",
            "Ctrl+S",
            "Ctrl+X",
            "Ctrl+Z / Ctrl+Y",
            "Ctrl+H / F1",
            "Ctrl+D / F2",
            "Ctrl+B / F3",
        ] {
            assert!(text.contains(want), "{want} が Help から消えた");
        }
    }

    /// ⚠️ **`[keys]` で上書きできない行は触らない。** Glide のモーションは
    /// `action_from_name` の対象外なので、べた書きのままで**嘘をついていない**。
    /// ⭐ ここが動いたら、直す範囲が広がりすぎている合図。
    #[test]
    fn glide_motions_are_left_alone() {
        let plain = help_text(&mut editor_with_keys(&[]));
        let overridden = help_text(&mut editor_with_keys(&[("enter_save", "f9")]));
        for motion in ["hjkl", "w / b / e"] {
            assert!(plain.contains(motion), "{motion} が既定の Help に無い");
            assert!(
                overridden.contains(motion),
                "{motion} が上書きで変わった（対象外の行に手が入っている）"
            );
        }
    }
}
