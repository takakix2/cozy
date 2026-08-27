use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::state::{EditorMode, EditorState};
use crate::utils::unicode::display_width_up_to;
use crate::utils::wrap::{visual_row_count, wrap_chunks};

/// Clamp cursor position to stay within the terminal buffer bounds.
fn clamp_cursor(frame_area: Rect, x: u16, y: u16) -> (u16, u16) {
    let max_x = frame_area.right().saturating_sub(1);
    let max_y = frame_area.bottom().saturating_sub(1);
    (x.min(max_x), y.min(max_y))
}

pub fn render_text_buffer(editor: &mut EditorState, f: &mut Frame, area: Rect) {
    let viewport_height = area.height as usize;

    let show_line_numbers = editor.line_numbers_visible();

    let (line_number_width, line_number_digits) = if show_line_numbers {
        let max_lines = editor.buffer.lines.len();
        let max_display = std::cmp::max(max_lines, editor.scroll_offset + viewport_height);
        let digits = max_display.to_string().len();
        let width = (digits + 2) as u16;
        (width, digits)
    } else {
        (0u16, 0usize)
    };

    let text_x_offset = if show_line_numbers {
        line_number_width + 1
    } else {
        0
    };
    let text_width = area.width.saturating_sub(text_x_offset) as usize;

    // Store text_display_width before adjust_scroll (which needs it for soft wrap)
    editor.text_display_width = text_width;
    editor.adjust_scroll(viewport_height);

    // Refresh syntax highlight spans for the visible window (no-op unless the
    // buffer is dirty or the window moved). Over-including with soft wrap is
    // fine — the extra lines just won't be drawn.
    let line_count = editor.buffer.lines.len();
    let vis_end = (editor.scroll_offset + viewport_height).min(line_count);
    let vis_start = editor.scroll_offset.min(vis_end);
    editor
        .highlighter
        .ensure(&editor.buffer.lines, vis_start..vis_end);

    let (bg_color, fg_color) = {
        let bg = match editor.config.line_number_bg.as_deref() {
            Some("blue") => Color::Blue,
            Some("black") => Color::Black,
            _ => Color::DarkGray,
        };
        let fg = match editor.config.line_number_fg.as_deref() {
            Some("yellow") => Color::Yellow,
            _ => Color::White,
        };
        (bg, fg)
    };

    let soft_wrap = editor.soft_wrap && text_width > 0;

    // ── render visual rows ────────────────────────────────────────────────────

    let mut visual_row = 0usize;
    let mut buf_y = editor.scroll_offset;

    while visual_row < viewport_height {
        let has_content = buf_y < editor.buffer.lines.len();

        if has_content && soft_wrap {
            let line = editor.buffer.lines[buf_y].clone();
            let chunks = wrap_chunks(&line, text_width);

            for (chunk_idx, &(cs, ce)) in chunks.iter().enumerate() {
                if visual_row >= viewport_height {
                    break;
                }
                let row_y = area.y + visual_row as u16;

                if show_line_numbers {
                    let num_area = Rect {
                        x: area.x,
                        y: row_y,
                        width: line_number_width,
                        height: 1,
                    };
                    let ta_x = area.x + line_number_width + 1;
                    let ta_w = area.width.saturating_sub(line_number_width + 1);
                    let ta = Rect {
                        x: ta_x,
                        y: row_y,
                        width: ta_w,
                        height: 1,
                    };

                    if chunk_idx == 0 {
                        let num_str = right_align(buf_y + 1, line_number_digits);
                        let is_current = buf_y == editor.cursor.y;
                        let line_fg = if is_current { fg_color } else { Color::Gray };
                        let mut style = Style::default().bg(bg_color).fg(line_fg);
                        if is_current {
                            style = style.add_modifier(Modifier::BOLD);
                        }
                        f.render_widget(Paragraph::new(num_str).style(style), num_area);
                    } else {
                        f.render_widget(
                            Paragraph::new("").style(Style::default().bg(bg_color)),
                            num_area,
                        );
                    }
                    f.render_widget(render_line_range(editor, buf_y, cs, ce), ta);
                } else {
                    let la = Rect {
                        x: area.x,
                        y: row_y,
                        width: area.width,
                        height: 1,
                    };
                    f.render_widget(render_line_range(editor, buf_y, cs, ce), la);
                }

                visual_row += 1;
            }
        } else {
            let row_y = area.y + visual_row as u16;

            if show_line_numbers {
                let num_area = Rect {
                    x: area.x,
                    y: row_y,
                    width: line_number_width,
                    height: 1,
                };
                let ta_x = area.x + line_number_width + 1;
                let ta_w = area.width.saturating_sub(line_number_width + 1);
                let ta = Rect {
                    x: ta_x,
                    y: row_y,
                    width: ta_w,
                    height: 1,
                };

                // Always show line number for every viewport row (including beyond buffer)
                let num_str = right_align(buf_y + 1, line_number_digits);
                let is_current = buf_y == editor.cursor.y;
                let line_fg = if is_current { fg_color } else { Color::Gray };
                let mut style = Style::default().bg(bg_color).fg(line_fg);
                if is_current {
                    style = style.add_modifier(Modifier::BOLD);
                }
                f.render_widget(Paragraph::new(num_str).style(style), num_area);

                if has_content {
                    f.render_widget(render_line(editor, buf_y), ta);
                } else {
                    f.render_widget(Paragraph::new(""), ta);
                }
            } else if has_content {
                let la = Rect {
                    x: area.x,
                    y: row_y,
                    width: area.width,
                    height: 1,
                };
                f.render_widget(render_line(editor, buf_y), la);
            } else {
                let la = Rect {
                    x: area.x,
                    y: row_y,
                    width: area.width,
                    height: 1,
                };
                f.render_widget(Paragraph::new(""), la);
            }

            visual_row += 1;
        }

        buf_y += 1;
    }

    // ── cursor positioning ────────────────────────────────────────────────────

    if editor.mode == EditorMode::Edit || editor.mode == EditorMode::Glide {
        let cursor_y = editor.cursor.y;
        let cursor_x = editor.cursor.x;
        let mut vrow = 0usize;
        let mut by = editor.scroll_offset;

        'find: while vrow < viewport_height {
            if by >= editor.buffer.lines.len() {
                break;
            }
            let line = &editor.buffer.lines[by];

            if by == cursor_y {
                if soft_wrap {
                    let chunks = wrap_chunks(line, text_width);
                    let n = chunks.len();
                    for (cidx, &(cs, _)) in chunks.iter().enumerate() {
                        if vrow >= viewport_height {
                            break;
                        }
                        let next_start = if cidx + 1 < n {
                            chunks[cidx + 1].0
                        } else {
                            line.len() + 1
                        };
                        if cursor_x >= cs && cursor_x < next_start {
                            let end = cursor_x.min(line.len());
                            let before = &line[cs..end];
                            let dcol = crate::utils::unicode::str_display_width(before) as u16;
                            let (cx, cy) = clamp_cursor(
                                f.area(),
                                area.x + text_x_offset + dcol,
                                area.y + vrow as u16,
                            );
                            f.set_cursor_position((cx, cy));
                            break 'find;
                        }
                        vrow += 1;
                    }
                } else {
                    let dcol = display_width_up_to(line, cursor_x) as u16;
                    let (cx, cy) = clamp_cursor(
                        f.area(),
                        area.x + text_x_offset + dcol,
                        area.y + vrow as u16,
                    );
                    f.set_cursor_position((cx, cy));
                }
                break;
            } else {
                vrow += if soft_wrap {
                    visual_row_count(line, text_width)
                } else {
                    1
                };
                by += 1;
            }
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn right_align(num: usize, digits: usize) -> String {
    format!(" {:>width$} ", num, width = digits)
}

fn render_line(editor: &EditorState, y: usize) -> Line<'static> {
    let len = editor.buffer.lines[y].len();
    render_line_range(editor, y, 0, len)
}

fn render_line_range(
    editor: &EditorState,
    y: usize,
    byte_start: usize,
    byte_end: usize,
) -> Line<'static> {
    let line_text = &editor.buffer.lines[y];
    let mut spans: Vec<Span<'static>> = Vec::new();

    let is_light = editor.config.theme.as_deref() == Some("light");
    let default_style = if is_light {
        Style::default().fg(Color::Black).bg(Color::White)
    } else {
        Style::default()
    };

    let line_matches: Vec<(usize, usize)> = editor
        .search_matches
        .iter()
        .filter(|&&(my, _, _)| my == y)
        .map(|&(_, s, e)| (s, e))
        .collect();
    let current_match = editor
        .search_matches
        .get(editor.search_current)
        .filter(|&&(my, _, _)| my == y)
        .map(|&(_, s, e)| (s, e));

    let mut has_content = false;
    // ⚠️ **描く範囲の外も桁を進める。** TAB の幅は論理行の先頭から数えるので、
    // 折り返した 2 行目だけを描くときも、そこまでに何桁使ったかを知っている必要がある。
    let mut col = 0usize;

    for (byte_pos, ch) in line_text.char_indices() {
        let in_range = if byte_end > byte_start {
            byte_pos >= byte_start && byte_pos < byte_end
        } else {
            true
        };

        if in_range {
            // Base style comes from the precomputed highlight cache (tree-sitter
            // or regex fallback), patched onto the theme default so modifier-only
            // highlights (e.g. Markdown bold/italic, which set no fg) keep the
            // theme's foreground/background; the search/yank overlays go on top.
            let mut final_style =
                default_style.patch(editor.highlighter.style_at(y, byte_pos).unwrap_or_default());
            // Yank flash (green): shown until the next keypress so you can see
            // what was just copied. A live search match still wins over it.
            if let Some(hl) = &editor.yank_highlight {
                if hl.contains(y, byte_pos) {
                    final_style = final_style.bg(Color::Green).fg(Color::Black);
                }
            }
            if let Some(&(ms, me)) = line_matches
                .iter()
                .find(|&&(s, e)| byte_pos >= s && byte_pos < e)
            {
                if current_match == Some((ms, me)) {
                    final_style = final_style
                        .bg(Color::Yellow)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD);
                } else {
                    final_style = final_style.bg(Color::Rgb(100, 80, 0)).fg(Color::White);
                }
            }
            // 🚨 **他人のバイトを端末へ渡す関門。** 制御文字はここで `^X` になる ——
            // 素のまま渡すと、ファイルの中身が端末制御を奪う（`ROADMAP.md`
            // 「Stop handing the terminal whatever the file says」）。
            // ⭐ 幅を数える `char_display_width` と**同じ関数**に訊いているので、
            // カーソルの列と描かれた字がずれない。
            spans.push(Span::styled(
                crate::utils::unicode::visible_char_at(ch, col),
                final_style,
            ));
            has_content = true;
        }
        col += crate::utils::unicode::char_display_width_at(ch, col);
    }

    if !has_content {
        spans.push(Span::styled(" ", default_style));
    }

    Line::from(spans)
}

#[cfg(all(test, feature = "treesitter"))]
mod render_color_tests {
    use super::*;
    use crate::state::EditorState;
    use crate::state::buffer::TextBuffer;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn function_name_gets_color_through_render() {
        let mut editor = EditorState::new(Some("probe.rs".to_string()));
        editor.buffer = TextBuffer::from_lines(vec!["fn foo() {}".to_string()]);
        editor
            .highlighter
            .set_file(Some(std::path::Path::new("probe.rs")));
        editor.highlighter.mark_dirty();
        editor.mode = EditorMode::Edit;

        let backend = TestBackend::new(40, 4);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render_text_buffer(&mut editor, f, Rect::new(0, 0, 40, 4)))
            .unwrap();
        let buf = term.backend().buffer();
        let mut fgs = std::collections::HashSet::new();
        for x in 0..40u16 {
            if let Some(cell) = buf.cell((x, 0)) {
                fgs.insert(cell.fg);
            }
        }
        assert!(
            fgs.contains(&Color::Magenta),
            "fn keyword should be magenta"
        );
        assert!(
            fgs.contains(&Color::LightBlue),
            "function name should be LightBlue; got {:?}",
            fgs
        );
    }
}

/// 🚨 **描いた字そのものを固定する網。**
///
/// ⚠️ 幅の側（`utils::unicode`）だけを網で押さえていた間、**描画を元に戻しても
/// テストは全部緑のまま**だった（2026-08-28 に陰性対照で発覚）。
/// ⭐ 幅と描画は**別々に壊れる**ので、**別々に固定する**必要がある ——
/// 片方だけ直すと「画面と内部が別々に正しい」状態になり、それがまさに `#8` の症状。
#[cfg(test)]
mod control_char_rendering_tests {
    use super::*;
    use crate::state::EditorState;
    use crate::state::buffer::TextBuffer;

    /// 1 行を描いて、画面に出る文字列を組み立てる。
    fn rendered(line: &str) -> String {
        let mut editor = EditorState::new(None);
        editor.buffer = TextBuffer::from_lines(vec![line.to_string()]);
        render_line(&editor, 0)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    /// 🚨 **本丸** —— ESC が端末へ渡らないこと。生で渡ると、ファイルの中身が
    /// cozy の画面（行番号・フッタ・ステータス行）を書き換えられる。
    #[test]
    fn escape_never_reaches_the_terminal() {
        assert_eq!(rendered("a\u{1b}[2Jb"), "a^[[2Jb");
        assert_eq!(rendered("a\u{1b}]0;t\u{7}b"), "a^[]0;t^Gb");
    }

    #[test]
    fn control_chars_are_drawn_as_caret_notation() {
        assert_eq!(rendered("a\rb\rc"), "a^Mb^Mc");
        assert_eq!(rendered("a\u{8}b"), "a^Hb");
        assert_eq!(rendered("a\u{7f}b"), "a^?b");
    }

    /// 陽性対照 —— 普通の行は**1 文字も変わらない**。
    /// ⭐ これが無いと「全部 `^X` にする」実装が緑で通る。
    #[test]
    fn ordinary_lines_are_drawn_unchanged() {
        assert_eq!(rendered("abc"), "abc");
        assert_eq!(rendered("あい"), "あい");
        // ⚠️ TAB だけは「変わらない」の例外 —— **cozy が空白へ展開する**。
        // 🚨 素通しすると端末が自分のタブストップで桁を進め、cozy の計算とずれる。
        assert_eq!(
            rendered("a\tb"),
            "a       b",
            "TAB は次のタブストップまでの空白に展開される（`a` が 1 桁 → 7 つ）"
        );
    }

    /// ⭐ 描いた桁数と、幅の計算が一致すること。**この 2 つを繋ぐ網**が無いと、
    /// 片方だけ直した状態が緑で通る（実際に一度そうなった）。
    #[test]
    fn drawn_columns_match_the_counted_width() {
        for line in [
            "a\rb",
            "a\u{1b}[31mX",
            "abc",
            "あい",
            "a\tb",
            "\tx",
            "12345678\ty",
        ] {
            let drawn = rendered(line);
            // ⭐ 描いた結果は TAB を含まない（空白に展開済み）ので、単純に足せる。
            let drawn_cols: usize = drawn
                .chars()
                .map(|c| unicode_width::UnicodeWidthChar::width(c).unwrap_or(0))
                .sum();
            assert!(!drawn.contains('\t'), "端末に TAB を渡している: {drawn:?}");
            assert_eq!(
                drawn_cols,
                crate::utils::unicode::str_display_width(line),
                "{line:?} を描くと {drawn:?}（{drawn_cols} 桁）だが、幅は別の値と数えている"
            );
        }
    }
}
