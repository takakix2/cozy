use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::state::EditorState;

// ── Welcome ───────────────────────────────────────────────────────────────────

pub fn render_welcome(f: &mut Frame, area: Rect) {
    if area.width < 50 {
        render_welcome_narrow(f, area);
    } else {
        render_welcome_wide(f, area);
    }
}

fn render_welcome_narrow(f: &mut Frame, area: Rect) {
    use unicode_width::UnicodeWidthStr;

    let w = area.width as usize;
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

    let lines: Vec<Line> = vec![
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

    // 垂直センタリング
    let h = lines.len() as u16;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect::new(area.x, y, area.width, h.min(area.height)).intersection(area);
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Left), rect);
}

fn render_welcome_wide(f: &mut Frame, area: Rect) {
    let cyan = Style::default().fg(Color::Cyan);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let yellow = Style::default().fg(Color::Yellow);

    let lines = vec![
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
    ];

    let width = 60u16.min(area.width);
    let height = (lines.len() as u16).min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let rect = Rect::new(x, y, width, height).intersection(area);

    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rect);
}

// ── Help ──────────────────────────────────────────────────────────────────────

pub fn render_help(editor: &EditorState, f: &mut Frame, area: Rect) {
    if area.width < 50 {
        render_help_narrow(editor, f, area);
    } else {
        render_help_wide(editor, f, area);
    }
}

fn render_help_narrow(editor: &EditorState, f: &mut Frame, area: Rect) {
    let w = area.width as usize;
    let col = w / 2;
    let hdr = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);

    let lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "cozy Help  (↑↓ scroll)",
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
        Line::from("^B  → Browse folder tree"),
        Line::from("^G  → Glide mode (vim)"),
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

    let para = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .scroll((editor.help_scroll_offset, 0));

    f.render_widget(para, area);
}

fn render_help_wide(editor: &EditorState, f: &mut Frame, area: Rect) {
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
        Line::from("  Ctrl+O          Open file"),
        Line::from("  Ctrl+B          Browse folder tree"),
        Line::from("  Ctrl+S          Save"),
        Line::from("  Ctrl+X          Exit"),
        Line::from("  Ctrl+F          Find"),
        Line::from("  Ctrl+R          Replace"),
        Line::from("  Ctrl+H          Help"),
        Line::from("  Ctrl+Z / Y      Undo / Redo"),
        Line::from("  Ctrl+K          Cut line"),
        Line::from("  Ctrl+J          Jump to line"),
        Line::from("  Ctrl+G          Enter Glide mode"),
        Line::from("  F2 / Ctrl+D     Toggle Markdown preview"),
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

    let para = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .scroll((editor.help_scroll_offset, 0));

    f.render_widget(para, area);
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
