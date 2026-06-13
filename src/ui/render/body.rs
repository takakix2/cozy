use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

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

    let show_line_numbers = editor
        .show_line_numbers_runtime
        .unwrap_or(editor.config.show_line_numbers.unwrap_or(true));

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
                            let dcol = UnicodeWidthStr::width(before) as u16;
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

    let language = editor
        .filename
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|ext| ext.to_str());
    let highlighter = crate::utils::syntax::SyntaxHighlighter::new(language);
    let highlighted = highlighter.highlight(line_text);

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

    let mut byte_pos = 0usize;
    let mut has_content = false;

    for (text, style) in highlighted {
        for ch in text.chars() {
            let ch_end = byte_pos + ch.len_utf8();
            let in_range = if byte_end > byte_start {
                byte_pos >= byte_start && byte_pos < byte_end
            } else {
                true
            };

            if in_range {
                let mut final_style = style;
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
                spans.push(Span::styled(ch.to_string(), final_style));
                has_content = true;
            }

            byte_pos = ch_end;
        }
    }

    if !has_content {
        spans.push(Span::styled(" ", default_style));
    }

    Line::from(spans)
}
