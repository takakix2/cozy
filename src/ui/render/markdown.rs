use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::state::EditorState;

#[derive(Clone, Copy)]
enum CodeBlock {
    Plain,
    Mermaid { label_idx: usize },
}

pub fn render_markdown(editor: &mut EditorState, f: &mut Frame, area: Rect) {
    editor.markdown_view_height = area.height;
    let lines = markdown_lines(&editor.buffer.lines);
    let max_scroll = lines.len().saturating_sub(area.height as usize);
    let max_line = lines.len().saturating_sub(1);
    editor.markdown_cursor_line = editor.markdown_cursor_line.min(max_line as u16);

    let cursor = editor.markdown_cursor_line as usize;
    let top = editor.markdown_scroll_offset as usize;
    let height = (area.height as usize).max(1);
    if cursor < top {
        editor.markdown_scroll_offset = cursor as u16;
    } else if cursor >= top.saturating_add(height) {
        editor.markdown_scroll_offset = cursor.saturating_sub(height - 1) as u16;
    }
    editor.markdown_scroll_offset = editor.markdown_scroll_offset.min(max_scroll as u16);

    for row in 0..area.height {
        let idx = editor.markdown_scroll_offset as usize + row as usize;
        let mut line = lines.get(idx).cloned().unwrap_or_else(|| Line::from(""));
        if idx == editor.markdown_cursor_line as usize {
            f.render_widget(
                Block::default().style(Style::default().bg(Color::DarkGray)),
                Rect {
                    x: area.x,
                    y: area.y + row,
                    width: area.width,
                    height: 1,
                },
            );
            line = line.style(Style::default().bg(Color::DarkGray));
        }
        let row_area = Rect {
            x: area.x,
            y: area.y + row,
            width: area.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(line), row_area);
    }
}

pub fn markdown_line_count(editor: &EditorState) -> usize {
    markdown_lines(&editor.buffer.lines).len()
}

fn markdown_lines(source: &[String]) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let mut code_block: Option<CodeBlock> = None;

    for raw in source {
        let line = raw.as_str();
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            match code_block {
                Some(_) => {
                    code_block = None;
                    out.push(fence_line(trimmed));
                }
                None => {
                    if is_mermaid_fence(trimmed) {
                        let label_idx = out.len();
                        out.push(mermaid_label(None));
                        code_block = Some(CodeBlock::Mermaid { label_idx });
                    } else {
                        code_block = Some(CodeBlock::Plain);
                        out.push(fence_line(trimmed));
                    }
                }
            }
            continue;
        }

        match code_block {
            Some(CodeBlock::Plain) => {
                out.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(Color::Yellow),
                )));
                continue;
            }
            Some(CodeBlock::Mermaid { label_idx }) => {
                if let Some(kind) = mermaid_kind(trimmed) {
                    out[label_idx] = mermaid_label(Some(kind));
                }
                out.push(Line::from(mermaid_spans(line)));
                continue;
            }
            None => {}
        }

        if trimmed.is_empty() {
            out.push(Line::from(""));
        } else if let Some((level, text)) = heading(trimmed) {
            let marker = " ".repeat(level.saturating_sub(1).min(3));
            out.push(Line::from(vec![
                Span::raw(marker),
                Span::styled(
                    text.to_string(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if let Some(text) = trimmed.strip_prefix("> ") {
            out.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled(text.to_string(), Style::default().fg(Color::Gray)),
            ]));
        } else if let Some(text) = list_item(trimmed) {
            out.push(Line::from(vec![
                Span::styled("• ", Style::default().fg(Color::Cyan)),
                Span::raw(text.to_string()),
            ]));
        } else {
            out.push(Line::from(inline_spans(line)));
        }
    }

    out
}

fn fence_line(line: &str) -> Line<'static> {
    Line::from(Span::styled(
        line.to_string(),
        Style::default().fg(Color::DarkGray),
    ))
}

fn is_mermaid_fence(line: &str) -> bool {
    fence_language(line)
        .map(|lang| lang.eq_ignore_ascii_case("mermaid"))
        .unwrap_or(false)
}

fn fence_language(line: &str) -> Option<&str> {
    line.strip_prefix("```")?.trim().split_whitespace().next()
}

fn mermaid_label(kind: Option<&str>) -> Line<'static> {
    let text = match kind {
        Some(kind) => format!("[Mermaid: {}]", kind),
        None => "[Mermaid]".to_string(),
    };
    Line::from(Span::styled(
        text,
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ))
}

fn mermaid_kind(line: &str) -> Option<&str> {
    let first = line.split_whitespace().next()?;
    match first {
        "flowchart" | "graph" | "sequenceDiagram" | "classDiagram" | "stateDiagram"
        | "stateDiagram-v2" | "erDiagram" | "gantt" | "pie" | "journey" | "mindmap"
        | "timeline" | "gitGraph" => Some(line),
        _ => None,
    }
}

fn mermaid_spans(line: &str) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());
    let mut spans = vec![Span::styled(
        "  ".to_string(),
        Style::default().fg(Color::DarkGray),
    )];
    if indent > 0 {
        spans.push(Span::raw(" ".repeat(indent)));
    }

    let style = if trimmed.starts_with("%%") {
        Style::default().fg(Color::DarkGray)
    } else if mermaid_kind(trimmed).is_some() {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else if has_mermaid_edge(trimmed) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Magenta)
    };

    if trimmed.is_empty() {
        spans.push(Span::raw(" "));
    } else {
        spans.push(Span::styled(trimmed.to_string(), style));
    }
    spans
}

fn has_mermaid_edge(line: &str) -> bool {
    ["-->", "---", "-.->", "==>", "--", "===", "-.-"]
        .iter()
        .any(|token| line.contains(token))
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = line.get(hashes..)?;
    rest.strip_prefix(' ').map(|text| (hashes, text))
}

fn list_item(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
}

fn inline_spans(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = line;

    while let Some(start) = rest.find('`') {
        let (before, after_start) = rest.split_at(start);
        if !before.is_empty() {
            spans.push(Span::raw(before.to_string()));
        }

        let after_tick = &after_start[1..];
        if let Some(end) = after_tick.find('`') {
            let (code, after_end) = after_tick.split_at(end);
            spans.push(Span::styled(
                code.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            rest = &after_end[1..];
        } else {
            spans.push(Span::raw(after_start.to_string()));
            return spans;
        }
    }

    if !rest.is_empty() {
        spans.push(Span::raw(rest.to_string()));
    }

    if spans.is_empty() {
        spans.push(Span::raw(" "));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn mermaid_fence_gets_labeled_from_first_directive() {
        let lines = markdown_lines(&[
            "```mermaid".to_string(),
            "flowchart LR".to_string(),
            "  A --> B".to_string(),
            "```".to_string(),
        ]);

        assert_eq!(line_text(&lines[0]), "[Mermaid: flowchart LR]");
        assert_eq!(line_text(&lines[1]), "  flowchart LR");
        assert_eq!(line_text(&lines[2]), "    A --> B");
        assert_eq!(line_text(&lines[3]), "```");
    }

    #[test]
    fn non_mermaid_code_block_stays_plain() {
        let lines = markdown_lines(&[
            "```rust".to_string(),
            "fn main() {}".to_string(),
            "```".to_string(),
        ]);

        assert_eq!(line_text(&lines[0]), "```rust");
        assert_eq!(line_text(&lines[1]), "  fn main() {}");
        assert_eq!(line_text(&lines[2]), "```");
    }
}
