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
    let Rendered {
        lines,
        placements,
        remote,
    } = rendered_markdown_with_images(
        &editor.buffer.lines,
        area.width,
        area.height,
        editor.filename.as_deref().and_then(|p| p.parent()),
    );
    editor.markdown_remote_images = remote;
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
                &p.source,
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

/// 文書座標での画像の置き場（開始行・高さ・出どころ）。
/// feature 無しのときは一度も構築されない（placements は常に空）ので dead を許す。
#[cfg_attr(not(feature = "imageview"), allow(dead_code))]
struct ImagePlacement {
    start: usize,
    height: usize,
    #[cfg(feature = "imageview")]
    source: crate::imageview::inline::ImageSource,
}

/// プレビュー 1 回分の組み立て結果。
struct Rendered {
    lines: Vec<Line<'static>>,
    placements: Vec<ImagePlacement>,
    /// 文書に出てくる URL の画像（重複なし・出てきた順）。`i` が取りに行く相手。
    remote: Vec<String>,
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
/// URL の画像は、取ってきてあれば手元の物と同じく行を予約して描き、まだなら 1 行の
/// 文字（「`i` で取る」・取得中・取れなかった理由）に差し替える。⚠️ **ここでは取りに行かない**
/// （取るのは `i`・`imageview::fetch`）。
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
) -> Rendered {
    let markdown = source.join("\n");
    let renderer = MarkdownRenderer::new(width.max(1) as usize);
    let blocks = renderer.parse(&markdown);

    #[cfg(not(feature = "imageview"))]
    {
        Rendered {
            lines: renderer.render(&blocks, &ThemeConfig::default()),
            placements: Vec::new(),
            remote: Vec::new(),
        }
    }

    #[cfg(feature = "imageview")]
    {
        let (mut lines, marks) = renderer.render_tracking_images(&blocks, &ThemeConfig::default());
        let mut placements = Vec::new();
        #[cfg_attr(not(feature = "remote-images"), allow(unused_mut))]
        let mut remote: Vec<String> = Vec::new();
        if !crate::imageview::inline::available() {
            return Rendered {
                lines,
                placements,
                remote,
            };
        }
        // 1 枚の画像がプレビューを食い尽くさないよう、高さは view の**半分**までに抑える。
        // 文章を読む面なので、画像の前後の文が同じ画面に残る方を取る（8 割では 512px の
        // ロゴが画面の大半を取った）。
        let max_rows = ((height as u32 / 2).max(1) as u16).max(1);
        let mut offset: isize = 0;
        #[cfg_attr(not(feature = "remote-images"), allow(unused_variables))]
        for (mark_line, src, alt) in marks {
            let Some(resolved) = crate::imageview::inline::resolve(doc_dir, &src) else {
                continue;
            };
            let idx = (mark_line as isize + offset) as usize;
            if idx >= lines.len() {
                continue;
            }
            #[cfg(feature = "remote-images")]
            if let crate::imageview::inline::ImageSource::Url(url) = &resolved {
                if !remote.contains(url) {
                    remote.push(url.clone());
                }
                let status = crate::imageview::fetch::status(url);
                if status != Some(crate::imageview::fetch::Status::Ready) {
                    lines[idx] = remote_image_line(&alt, url, status.as_ref());
                    continue;
                }
            }
            let Some(rows) = crate::imageview::inline::reserve_rows(&resolved, width, max_rows)
            else {
                #[cfg(feature = "remote-images")]
                if let crate::imageview::inline::ImageSource::Url(url) = &resolved {
                    lines[idx] = remote_image_line(
                        &alt,
                        url,
                        Some(&crate::imageview::fetch::Status::Failed(
                            "not an image".to_string(),
                        )),
                    );
                }
                continue;
            };
            // 1 行のフォールバックを rows 行の空行へ差し替える。
            let blanks = std::iter::repeat_with(|| Line::from("")).take(rows);
            lines.splice(idx..idx + 1, blanks);
            placements.push(ImagePlacement {
                start: idx,
                height: rows,
                source: resolved,
            });
            offset += rows as isize - 1;
        }
        Rendered {
            lines,
            placements,
            remote,
        }
    }
}

/// まだ描けない URL の画像の 1 行。`[image: …]`（手元の画像の文字フォールバック）と同じ形に、
/// 次に何が起きるかを足す。
#[cfg(feature = "remote-images")]
fn remote_image_line(
    alt: &str,
    url: &str,
    status: Option<&crate::imageview::fetch::Status>,
) -> Line<'static> {
    use crate::imageview::fetch::Status;
    let label = if alt.is_empty() { url } else { alt }.replace('\t', "    ");
    let note = match status {
        None => "press i to fetch".to_string(),
        Some(Status::Pending) => "fetching…".to_string(),
        Some(Status::Failed(why)) => why.clone(),
        Some(Status::Ready) => String::new(),
    };
    Line::from(ratatui::text::Span::styled(
        format!("[image: {label} · {note}]"),
        Style::default().italic().fg(Color::Gray),
    ))
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
        let Rendered {
            lines, placements, ..
        } = rendered_markdown_with_images(&src, 80, 24, None);
        assert!(placements.is_empty(), "caps 無しで予約が起きた");
        assert_eq!(lines.len(), plain.len(), "行数が平坦化と違う");
        for (a, b) in lines.iter().zip(plain.iter()) {
            assert_eq!(line_text(a), line_text(b), "行の中身が平坦化と違う");
        }
    }

    /// URL の画像（feature `remote-images`）。端末が画像を描ける状態（caps）を立てて撃つ。
    #[cfg(feature = "remote-images")]
    mod remote_images {
        use super::*;
        use crate::imageview::fetch;

        fn with_caps() {
            crate::imageview::inline::set_caps(&crate::ImageCaps {
                protocol: crate::ImageProtocol::Sixel,
                cell_px: (9, 18),
                sixel_registers: 256,
            });
        }

        fn doc(url: &str) -> Vec<String> {
            vec![
                "# Title".to_string(),
                String::new(),
                format!("![logo]({url})"),
                String::new(),
                "body".to_string(),
            ]
        }

        fn png() -> Vec<u8> {
            let mut out = std::io::Cursor::new(Vec::new());
            image::RgbaImage::from_pixel(32, 32, image::Rgba([200, 40, 40, 255]))
                .write_to(&mut out, image::ImageFormat::Png)
                .unwrap();
            out.into_inner()
        }

        fn text(r: &Rendered) -> String {
            r.lines.iter().map(line_text).collect::<Vec<_>>().join("\n")
        }

        /// ⭐ 描くだけでは取りに行かない（キーを押すまで通信しない）—— 取得の表に載らない。
        #[test]
        fn rendering_a_url_image_does_not_fetch_it() {
            with_caps();
            let url = "https://example.test/md-untouched.png";
            let r = rendered_markdown_with_images(&doc(url), 80, 24, None);
            assert_eq!(fetch::status(url), None, "描いただけで取りに行った");
            assert_eq!(r.remote, vec![url.to_string()]);
            assert!(r.placements.is_empty());
            assert!(
                text(&r).contains("[image: logo · press i to fetch]"),
                "{}",
                text(&r)
            );
        }

        #[test]
        fn a_fetched_url_image_is_drawn_like_a_local_one() {
            with_caps();
            let url = "https://example.test/md-ready.png";
            fetch::settle_for_test(url, Ok(png()));
            let r = rendered_markdown_with_images(&doc(url), 80, 24, None);
            assert_eq!(r.placements.len(), 1, "{}", text(&r));
            assert_eq!(
                r.placements[0].source,
                crate::imageview::inline::ImageSource::Url(url.to_string())
            );
            assert!(!text(&r).contains("[image:"), "{}", text(&r));
        }

        #[test]
        fn a_failed_or_pending_fetch_says_so() {
            with_caps();
            let failed = "https://example.test/md-failed.png";
            fetch::settle_for_test(failed, Err("curl: (22) 404".to_string()));
            let r = rendered_markdown_with_images(&doc(failed), 80, 24, None);
            assert!(
                text(&r).contains("[image: logo · curl: (22) 404]"),
                "{}",
                text(&r)
            );
            assert!(r.placements.is_empty());

            // 取れたが絵ではない（HTML のエラーページ等）は描かない。
            let html = "https://example.test/md-html.png";
            fetch::settle_for_test(html, Ok(b"<!doctype html><p>nope".to_vec()));
            let r = rendered_markdown_with_images(&doc(html), 80, 24, None);
            assert!(
                text(&r).contains("[image: logo · not an image]"),
                "{}",
                text(&r)
            );
            assert!(r.placements.is_empty());
        }

        /// 手元の画像は URL の数に入らない（`i` の相手は URL だけ）。
        #[test]
        fn local_images_are_not_remote() {
            with_caps();
            let r = rendered_markdown_with_images(&doc("img/a.png"), 80, 24, None);
            assert!(r.remote.is_empty());
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
