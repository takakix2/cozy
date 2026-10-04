use crate::vendor::ratatui_markdown::{markdown::MarkdownRenderer, theme::ThemeConfig};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::state::EditorState;

pub fn render_markdown(editor: &mut EditorState, f: &mut Frame, area: Rect) {
    editor.markdown_view_height = area.height as usize;
    // 画像は解決できたものだけ予約行へ差し替わる（`#20` Phase 2）。feature 無し／端末が
    // 画像非対応なら `placements` は空で、`lines` は従来どおり文字フォールバックのまま。
    let (lines, placements) = rendered_markdown_with_images(
        &editor.buffer.lines,
        area.width,
        area.height,
        editor.filename.as_deref().and_then(|p| p.parent()),
    );
    editor.markdown_rendered_line_count = lines.len().max(1);
    let max_scroll = lines.len().saturating_sub(area.height as usize);
    let max_line = lines.len().saturating_sub(1);
    editor.markdown_cursor_line = editor.markdown_cursor_line.min(max_line);

    let cursor = editor.markdown_cursor_line;
    let top = editor.markdown_scroll_offset;
    let height = (area.height as usize).max(1);
    if cursor < top {
        editor.markdown_scroll_offset = cursor;
    } else if cursor >= top.saturating_add(height) {
        editor.markdown_scroll_offset = cursor.saturating_sub(height - 1);
    }
    editor.markdown_scroll_offset = editor.markdown_scroll_offset.min(max_scroll);

    for row in 0..area.height {
        let idx = editor.markdown_scroll_offset + row as usize;
        let mut line = lines.get(idx).cloned().unwrap_or_else(|| Line::from(""));
        if idx == editor.markdown_cursor_line {
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

    // ⭐ 文字行を描いた**後に**画像をブリット（予約行は空なので上書きで出る）。
    // スクロールで端にかかった画像は、見えている縦スライスだけを描く（`inline.rs` がクロップ）。
    blit_visible_images(&placements, editor.markdown_scroll_offset, area, f);
}

/// 予約済み画像のうち、スクロール窓に少しでも入っているものを、見えている分だけ描く。
#[cfg_attr(not(feature = "imageview"), allow(unused_variables))]
fn blit_visible_images(placements: &[ImagePlacement], scroll: usize, area: Rect, f: &mut Frame) {
    #[cfg(feature = "imageview")]
    {
        let view_h = area.height as usize;
        for p in placements {
            let Some((skip, draw, screen_row)) = visible_slice(p.start, p.height, scroll, view_h)
            else {
                continue;
            };
            let rect = Rect::new(area.x, area.y + screen_row as u16, area.width, draw as u16);
            crate::imageview::inline::blit(
                &p.path,
                area.width,
                skip as u16,
                draw as u16,
                rect,
                f.buffer_mut(),
            );
        }
    }
}

/// 予約範囲 `[start, start+height)` と スクロール窓 `[scroll, scroll+view_h)` の重なりを
/// `(skip, draw, screen_row)` で返す（重なりが無ければ `None`）:
/// - `skip` … 画像の上から窓の外に隠れている行数（上端クリップ量）
/// - `draw` … 実際に描く行数
/// - `screen_row` … 窓内の描画開始行（`area.y` からの相対）
///
/// feature 無しでは呼び手が消えるが、検体は両構成で走る。
#[cfg_attr(not(feature = "imageview"), allow(dead_code))]
fn visible_slice(
    start: usize,
    height: usize,
    scroll: usize,
    view_h: usize,
) -> Option<(usize, usize, usize)> {
    let vis_start = start.max(scroll);
    let vis_end = (start + height).min(scroll + view_h);
    if vis_end <= vis_start {
        return None;
    }
    Some((vis_start - start, vis_end - vis_start, vis_start - scroll))
}

/// 文書座標での画像の置き場（開始行・高さ・解決済みパス）。
/// feature 無しのときは一度も構築されない（placements は常に空）ので dead を許す。
#[cfg_attr(not(feature = "imageview"), allow(dead_code))]
struct ImagePlacement {
    start: usize,
    height: usize,
    path: std::path::PathBuf,
}

pub fn markdown_line_count(editor: &EditorState) -> usize {
    editor
        .markdown_rendered_line_count
        .max(editor.buffer.lines.len())
        .max(1)
}

/// 平坦化だけの経路（テストが使う）。本番は `rendered_markdown_with_images`。
#[cfg(test)]
fn rendered_markdown_lines(source: &[String], width: u16) -> Vec<Line<'static>> {
    let markdown = source.join("\n");
    let renderer = MarkdownRenderer::new(width.max(1) as usize);
    let blocks = renderer.parse(&markdown);
    renderer.render(&blocks, &ThemeConfig::default())
}

/// 行の平坦化に加えて、解決できたインライン画像の置き場を返す（`#20` Phase 2）。
///
/// 解決できた画像は、1 行の文字フォールバックを **N 行の予約空行**へ差し替える
/// （N は端末のセル寸法と幅から `inline::reserve_rows` が決める）。差し替えで後続の行が
/// ずれるので、オフセットを持ち越しながら前から処理する。feature 無しのときは
/// 画像処理ごと消え、出力は `rendered_markdown_lines` と 1 バイト違わない。
fn rendered_markdown_with_images(
    source: &[String],
    width: u16,
    #[cfg_attr(not(feature = "imageview"), allow(unused_variables))] height: u16,
    #[cfg_attr(not(feature = "imageview"), allow(unused_variables))] doc_dir: Option<
        &std::path::Path,
    >,
) -> (Vec<Line<'static>>, Vec<ImagePlacement>) {
    let markdown = source.join("\n");
    let renderer = MarkdownRenderer::new(width.max(1) as usize);
    let blocks = renderer.parse(&markdown);

    #[cfg(not(feature = "imageview"))]
    {
        (
            renderer.render(&blocks, &ThemeConfig::default()),
            Vec::new(),
        )
    }

    #[cfg(feature = "imageview")]
    {
        let (mut lines, marks) = renderer.render_tracking_images(&blocks, &ThemeConfig::default());
        let mut placements = Vec::new();
        if !crate::imageview::inline::available() {
            return (lines, placements);
        }
        // 1 枚の画像がプレビューを食い尽くさないよう、高さは view の 8 割までに抑える。
        let max_rows = ((height as u32 * 4 / 5).max(1) as u16).max(1);
        let mut offset: isize = 0;
        for (mark_line, src, _alt) in marks {
            let Some(resolved) = crate::imageview::inline::resolve(doc_dir, &src) else {
                continue;
            };
            let Some(rows) = crate::imageview::inline::reserve_rows(&resolved, width, max_rows)
            else {
                continue;
            };
            let idx = (mark_line as isize + offset) as usize;
            if idx >= lines.len() {
                continue;
            }
            // 1 行のフォールバックを rows 行の空行へ差し替える。
            let blanks = std::iter::repeat_with(|| Line::from("")).take(rows);
            lines.splice(idx..idx + 1, blanks);
            placements.push(ImagePlacement {
                start: idx,
                height: rows,
                path: resolved,
            });
            offset += rows as isize - 1;
        }
        (lines, placements)
    }
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
    fn heading_renders_without_hash_marker() {
        let lines = rendered_markdown_lines(&["# Title".to_string()], 80);
        assert_eq!(line_text(&lines[0]), "Title");
    }

    #[test]
    fn inline_code_renders_without_backticks() {
        let lines = rendered_markdown_lines(&["Use `cozy` now".to_string()], 80);
        assert_eq!(line_text(&lines[0]), "Use cozy now");
    }

    #[test]
    fn fenced_code_block_preserves_code_content() {
        let lines = rendered_markdown_lines(
            &[
                "```rust".to_string(),
                "fn main() {}".to_string(),
                "```".to_string(),
            ],
            80,
        );

        assert!(lines.iter().any(|line| line_text(line).contains("rust")));
        assert!(
            lines
                .iter()
                .any(|line| line_text(line).contains("fn main() {}"))
        );
    }

    #[test]
    fn paragraph_wraps_to_rendered_lines() {
        let lines = rendered_markdown_lines(
            &["alpha beta gamma delta epsilon zeta eta theta".to_string()],
            20,
        );

        assert!(lines.len() > 1);
    }

    /// 見えているスライスの計算 —— 全見え・上端クリップ・下端クリップ・窓外のオフバイワン。
    #[test]
    fn visible_slice_cases() {
        // 窓 = 行 10..30（scroll=10, view_h=20）。
        // 全見え: 行 12..18 → 隠れ 0・描く 6・画面行 2。
        assert_eq!(visible_slice(12, 6, 10, 20), Some((0, 6, 2)));
        // 下端クリップ: 行 25..33 だが窓は 30 まで → 隠れ 0・描く 5・画面行 15。
        assert_eq!(visible_slice(25, 8, 10, 20), Some((0, 5, 15)));
        // 上端クリップ: 行 7..15、窓は 10 から → 上 3 行隠れ・描く 5・画面行 0。
        assert_eq!(visible_slice(7, 8, 10, 20), Some((3, 5, 0)));
        // 窓の上にすっかり外れた。
        assert_eq!(visible_slice(0, 5, 10, 20), None);
        // 窓の下にすっかり外れた（行 30 は窓外）。
        assert_eq!(visible_slice(30, 5, 10, 20), None);
    }

    /// caps が無い（＝実端末を握っていない CI / 埋め込みホスト）なら、画像入りの Markdown でも
    /// 予約は 1 件も起きず、出力は平坦化そのまま ＝ **退行なし**。Phase 2 の安全弁。
    #[test]
    fn without_terminal_caps_images_fall_back_to_text_unchanged() {
        let src = vec![
            "# Title".to_string(),
            String::new(),
            "![alt](img/a.png)".to_string(),
            String::new(),
            "body".to_string(),
        ];
        let plain = rendered_markdown_lines(&src, 80);
        let (lines, placements) = rendered_markdown_with_images(&src, 80, 24, None);
        assert!(placements.is_empty(), "caps 無しで予約が起きた");
        assert_eq!(lines.len(), plain.len(), "行数が平坦化と違う");
        for (a, b) in lines.iter().zip(plain.iter()) {
            assert_eq!(line_text(a), line_text(b), "行の中身が平坦化と違う");
        }
    }

    #[test]
    fn mermaid_block_renders_diagram_content() {
        let lines = rendered_markdown_lines(
            &[
                "```mermaid".to_string(),
                "graph TD".to_string(),
                "A[Start] --> B[End]".to_string(),
                "```".to_string(),
            ],
            80,
        );
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("mermaid"));
        assert!(text.contains("Start"));
        assert!(text.contains("End"));
    }
}
