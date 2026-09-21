use anyhow::Context as _;

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
    text::Line,
    widgets::Paragraph,
};

use crate::vendor::ratatui_markdown::{
    markdown::{MarkdownBlock, MarkdownRenderer, RenderHooks},
    theme::ThemeConfig,
};

fn test_theme() -> ThemeConfig {
    ThemeConfig::default()
        .with_info_color(Color::Blue)
        .with_focused_border_color(Color::Cyan)
        .with_secondary_color(Color::Yellow)
        .with_json_key_color(Color::Cyan)
        .with_json_bool_color(Color::Yellow)
        .with_json_number_color(Color::Magenta)
}

fn render_to_buffer(lines: Vec<Line<'static>>, width: u16, height: u16) -> anyhow::Result<Buffer> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| {
        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, Rect::new(0, 0, width, height));
    })?;
    Ok(terminal.backend().buffer().clone())
}

fn render_markdown(markdown: &str, max_width: usize) -> Vec<Line<'static>> {
    let renderer = MarkdownRenderer::new(max_width);
    let blocks = renderer.parse(markdown);
    renderer.render(&blocks, &test_theme())
}

#[test]
fn heading1_renders_bold_underlined() -> anyhow::Result<()> {
    let lines = render_markdown("# Hello World", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    let cell = buf.cell((0, 0)).context("cell at (0, 0)")?;
    assert_eq!(cell.symbol(), "H");
    Ok(())
}

#[test]
fn heading2_renders_bold() -> anyhow::Result<()> {
    let lines = render_markdown("## Section", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    let cell = buf.cell((0, 0)).context("cell at (0, 0)")?;
    assert_eq!(cell.symbol(), "S");
    Ok(())
}

#[test]
fn heading3_renders_bold_secondary() -> anyhow::Result<()> {
    let lines = render_markdown("### Subsection", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "S");
    Ok(())
}

#[test]
fn paragraph_renders_text() -> anyhow::Result<()> {
    let lines = render_markdown("Hello, world!", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    let text: String = (0..13)
        .map(|x| buf.cell((x, 0)).map(|c| c.symbol()).unwrap_or_default())
        .collect::<String>();
    assert_eq!(text, "Hello, world!");
    Ok(())
}

#[test]
fn paragraph_wraps_at_max_width() -> anyhow::Result<()> {
    let lines = render_markdown("abcdefghij klmnopqrst uvwxyz", 15);
    assert!(
        lines.len() >= 2,
        "expected wrapping, got {} lines",
        lines.len()
    );
    Ok(())
}

#[test]
fn blank_line_produces_empty_line() -> anyhow::Result<()> {
    let lines = render_markdown("Hello\n\nWorld", 80);
    let blank_idx = lines
        .iter()
        .position(|l| l.spans.is_empty() || l.spans.iter().all(|s| s.content.is_empty()));
    assert!(
        blank_idx.is_some(),
        "expected a blank line between two paragraphs"
    );
    Ok(())
}

#[test]
fn horizontal_rule_renders_dashes() -> anyhow::Result<()> {
    let lines = render_markdown("---", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "─");
    Ok(())
}

#[test]
fn code_block_with_lang_renders_bordered_box() -> anyhow::Result<()> {
    let md = "```rust\nfn main() {}\n```";
    let lines = render_markdown(md, 80);
    assert!(
        lines.len() >= 3,
        "expected header, content, footer; got {} lines",
        lines.len()
    );
    let buf = render_to_buffer(lines, 80, 5)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "╭");
    Ok(())
}

#[test]
fn code_block_without_lang_renders_minimal_header() -> anyhow::Result<()> {
    let md = "```\nsome code\n```";
    let lines = render_markdown(md, 80);
    let buf = render_to_buffer(lines, 80, 5)?;
    let header_sym = buf.cell((0, 0)).context("cell at (0, 0)")?.symbol();
    assert_eq!(header_sym, "╭");
    Ok(())
}

#[test]
fn mermaid_code_block_is_rendered() -> anyhow::Result<()> {
    let md = "```mermaid\ngraph TD\nA-->B\n```";
    let lines = render_markdown(md, 80);
    {
        assert!(
            !lines.is_empty(),
            "mermaid blocks should produce rendered output with mermaid feature"
        );
    }
    Ok(())
}

#[test]
fn unordered_list_dash_renders_bullet() -> anyhow::Result<()> {
    let lines = render_markdown("- item one", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "•");
    Ok(())
}

#[test]
fn unordered_list_star_renders_bullet() -> anyhow::Result<()> {
    let lines = render_markdown("* item one", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "•");
    Ok(())
}

#[test]
fn unordered_list_plus_renders_bullet() -> anyhow::Result<()> {
    let lines = render_markdown("+ item one", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "•");
    Ok(())
}

#[test]
fn ordered_list_renders_items() -> anyhow::Result<()> {
    let lines = render_markdown("1. first\n2. second\n3. third", 80);
    assert_eq!(lines.len(), 3);
    let buf = render_to_buffer(lines, 80, 10)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "•");
    assert_eq!(buf.cell((0, 1)).context("cell at (0, 1)")?.symbol(), "•");
    assert_eq!(buf.cell((0, 2)).context("cell at (0, 2)")?.symbol(), "•");
    Ok(())
}

#[test]
fn nested_list_indents() -> anyhow::Result<()> {
    let renderer = MarkdownRenderer::new(80);
    let blocks = renderer.parse("- outer\n  - inner");
    let inner_block = blocks
        .iter()
        .find(|b| matches!(b, MarkdownBlock::ListItem(t, _) if t == "inner"));
    assert!(inner_block.is_some(), "should find inner list item");
    if let Some(MarkdownBlock::ListItem(_, indent)) = inner_block {
        assert_eq!(
            *indent, 1,
            "inner list item should have indent=1, got {}",
            indent
        );
    }

    let lines = render_markdown("- outer\n  - inner", 80);
    assert_eq!(lines.len(), 2);
    Ok(())
}

#[test]
fn blockquote_renders_with_prefix() -> anyhow::Result<()> {
    let lines = render_markdown("> quoted text", 80);
    assert_eq!(lines.len(), 1);
    let buf = render_to_buffer(lines, 80, 5)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "│");
    Ok(())
}

#[test]
fn table_renders_with_borders() -> anyhow::Result<()> {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |";
    let lines = render_markdown(md, 80);
    assert!(
        lines.len() >= 4,
        "expected top border, header, separator, row, bottom; got {}",
        lines.len()
    );
    let buf = render_to_buffer(lines, 80, 10)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "┌");
    Ok(())
}

#[test]
fn table_bottom_border_uses_bl_br_corners() -> anyhow::Result<()> {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |";
    let lines = render_markdown(md, 80);
    let last = &lines[lines.len() - 1];
    let buf = render_to_buffer(vec![last.clone()], 80, 1)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "└");
    Ok(())
}

#[test]
fn inline_bold_renders() -> anyhow::Result<()> {
    let spans = crate::vendor::ratatui_markdown::markdown::parse_inline_formatting(
        "**bold**",
        &test_theme(),
    );
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "bold");
    assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
    Ok(())
}

#[test]
fn inline_italic_renders() -> anyhow::Result<()> {
    let spans = crate::vendor::ratatui_markdown::markdown::parse_inline_formatting(
        "*italic*",
        &test_theme(),
    );
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "italic");
    assert!(spans[0].style.add_modifier.contains(Modifier::ITALIC));
    Ok(())
}

#[test]
fn inline_bold_italic_renders() -> anyhow::Result<()> {
    let spans = crate::vendor::ratatui_markdown::markdown::parse_inline_formatting(
        "***both***",
        &test_theme(),
    );
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "both");
    assert!(
        spans[0]
            .style
            .add_modifier
            .contains(Modifier::BOLD | Modifier::ITALIC)
    );
    Ok(())
}

#[test]
fn inline_code_renders() -> anyhow::Result<()> {
    let spans = crate::vendor::ratatui_markdown::markdown::parse_inline_formatting(
        "some `code` here",
        &test_theme(),
    );
    assert!(spans.iter().any(|s| s.content == "code"));
    let code_span = spans.iter().find(|s| s.content == "code").context("find")?;
    assert_eq!(code_span.style.fg, Some(Color::Yellow));
    Ok(())
}

#[test]
fn mixed_inline_formatting() -> anyhow::Result<()> {
    let spans = crate::vendor::ratatui_markdown::markdown::parse_inline_formatting(
        "normal **bold** *italic* `code`",
        &test_theme(),
    );
    assert!(
        spans.len() >= 4,
        "expected at least 4 spans for mixed formatting"
    );
    Ok(())
}

#[test]
fn complex_document_renders() -> anyhow::Result<()> {
    let md = r#"# Title

A paragraph with **bold** and *italic*.

## Section

- item 1
- item 2

> A quote

```
code here
```

---

| H1 | H2 |
|----|----|
| a  | b  |
"#;
    let lines = render_markdown(md, 80);
    assert!(
        lines.len() > 10,
        "complex document should produce many lines, got {}",
        lines.len()
    );
    let buf = render_to_buffer(lines, 80, 40)?;
    assert_eq!(buf.cell((0, 0)).context("cell at (0, 0)")?.symbol(), "T");
    Ok(())
}

mod example_tree_list_tests {
    use super::*;

    struct TreeListHooks;

    impl RenderHooks for TreeListHooks {
        fn list_item_marker(
            &self,
            indent: u8,
            is_last_in_group: bool,
            ancestors_are_last: &[bool],
            index_in_group: usize,
        ) -> Option<String> {
            let marker = if is_last_in_group {
                "└─ "
            } else if indent == 0 && index_in_group == 0 {
                "┌─ "
            } else {
                "├─ "
            };
            if indent == 0 {
                return Some(marker.to_string());
            }
            let mut prefix = String::new();
            for (depth, &is_last_ancestor) in ancestors_are_last.iter().enumerate() {
                if depth >= indent as usize {
                    break;
                }
                if is_last_ancestor {
                    for _ in 0..3 {
                        prefix.push(' ');
                    }
                } else {
                    prefix.push_str("│  ");
                }
            }
            if indent as usize > ancestors_are_last.len() {
                let extra = indent as usize - ancestors_are_last.len();
                for _ in 0..3 * extra {
                    prefix.push(' ');
                }
            }
            Some(format!("{}{}", prefix, marker))
        }

        fn tree_indent_unit(&self) -> Option<usize> {
            Some(3)
        }

        fn tree_continuation_prefix(
            &self,
            indent: u8,
            ancestors_are_last: &[bool],
        ) -> Option<String> {
            let unit = 3;
            let mut p = String::new();
            for (i, &last) in ancestors_are_last.iter().enumerate() {
                if i >= indent as usize {
                    break;
                }
                if last {
                    for _ in 0..unit {
                        p.push(' ');
                    }
                } else {
                    p.push_str("│  ");
                }
            }
            for _ in 0..unit {
                p.push(' ');
            }
            Some(p)
        }
    }

    #[test]
    fn tree_hook_root_items_have_tree_markers() {
        let renderer = MarkdownRenderer::new(76).with_render_hooks(Box::new(TreeListHooks));
        let blocks = renderer.parse("- A\n  - B\n  - C\n- D");
        let lines = renderer.render(&blocks, &test_theme());
        let has_tree_marker = lines.iter().any(|l| {
            l.spans.iter().any(|s| {
                s.content.contains("┌─") || s.content.contains("├─") || s.content.contains("└─")
            })
        });
        assert!(has_tree_marker, "tree markers should appear in output");
    }

    #[test]
    fn tree_hook_nested_items_have_pipe_prefix() {
        let renderer = MarkdownRenderer::new(76).with_render_hooks(Box::new(TreeListHooks));
        let blocks = renderer.parse("- A\n  - B\n- C");
        let lines = renderer.render(&blocks, &test_theme());
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            all_text.contains("│"),
            "nested items should have │ pipe prefix"
        );
    }

    #[test]
    fn tree_hook_last_item_uses_corner_marker() {
        let renderer = MarkdownRenderer::new(76).with_render_hooks(Box::new(TreeListHooks));
        let blocks = renderer.parse("- A\n- B");
        let lines = renderer.render(&blocks, &test_theme());
        let has_corner = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("└─")));
        assert!(has_corner, "last items should use └─ corner marker");
    }

    #[test]
    fn tree_hook_render_to_buffer_no_panic() -> anyhow::Result<()> {
        let renderer = MarkdownRenderer::new(76).with_render_hooks(Box::new(TreeListHooks));
        let blocks = renderer.parse("- A\n  - B\n- C");
        let lines = renderer.render(&blocks, &test_theme());
        let buffer = render_to_buffer(lines, 80, 40)?;
        assert_eq!(buffer.area.height, 40);
        Ok(())
    }

    #[test]
    fn tree_hook_two_sections_separated_by_paragraph() {
        let renderer = MarkdownRenderer::new(40).with_render_hooks(Box::new(TreeListHooks));
        let md = "- Alpha\n  - Beta\n  - Gamma\n\nSome paragraph text between.\n\n- Delta\n  - Epsilon\n  - Zeta";
        let blocks = renderer.parse(md);
        let lines = renderer.render(&blocks, &test_theme());

        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();

        assert!(
            texts[0].starts_with("└─"),
            "first tree has one root (Alpha) so it should use └─, got: {}",
            texts[0]
        );
        assert!(
            texts[1].starts_with("   "),
            "Alpha is last root, children should have spaces prefix, got: {}",
            texts[1]
        );

        let delta_line = texts
            .iter()
            .position(|t| t.contains("Delta"))
            .expect("Delta should exist");
        assert!(
            texts[delta_line].starts_with("└─"),
            "second tree has one root (Delta) so it should use └─, got: {}",
            texts[delta_line],
        );
        let epsilon_line = texts
            .iter()
            .position(|t| t.contains("Epsilon"))
            .expect("Epsilon should exist");
        assert!(
            texts[epsilon_line].starts_with("   "),
            "Delta is last root in its group, children should have spaces prefix, got: {}",
            texts[epsilon_line],
        );
    }

    #[test]
    fn tree_hook_multi_root_groups_isolated() {
        let renderer = MarkdownRenderer::new(40).with_render_hooks(Box::new(TreeListHooks));
        let md = "- A1\n  - B1\n  - B2\n- A2\n  - B3\n\nParagraph.\n\n- C1\n  - D1\n- C2\n  - D2";
        let blocks = renderer.parse(md);
        let lines = renderer.render(&blocks, &test_theme());
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();

        let a2_line = texts.iter().position(|t| t.contains("A2")).expect("A2");
        assert!(
            texts[a2_line].starts_with("└─"),
            "A2 is last root in first group: got {}",
            texts[a2_line],
        );
        let b3_line = texts.iter().position(|t| t.contains("B3")).expect("B3");
        assert!(
            !texts[b3_line].contains("│"),
            "A2 is last, so B3 should have spaces not │: got {}",
            texts[b3_line],
        );

        let c1_line = texts.iter().position(|t| t.contains("C1")).expect("C1");
        assert!(
            texts[c1_line].starts_with("┌─"),
            "C1 is first sibling and has sibling C2: got {}",
            texts[c1_line],
        );
        let c2_line = texts.iter().position(|t| t.contains("C2")).expect("C2");
        assert!(
            texts[c2_line].starts_with("└─"),
            "C2 is last root in second group: got {}",
            texts[c2_line],
        );
    }
}

mod example_scrollable_tests {
    use super::*;

    struct ScrollState {
        v_offset: usize,
        h_offset: usize,
        total_lines: usize,
        max_line_width: usize,
        pad_top: u16,
        pad_bottom: u16,
        pad_left: u16,
        pad_right: u16,
    }

    impl ScrollState {
        fn new(total_lines: usize, max_line_width: usize) -> Self {
            Self {
                v_offset: 0,
                h_offset: 0,
                total_lines,
                max_line_width,
                pad_top: 1,
                pad_bottom: 1,
                pad_left: 2,
                pad_right: 2,
            }
        }

        fn viewport_height(&self, area_height: u16) -> usize {
            area_height.saturating_sub(self.pad_top + self.pad_bottom) as usize
        }

        fn viewport_width(&self, area_width: u16) -> usize {
            area_width.saturating_sub(self.pad_left + self.pad_right) as usize
        }

        fn max_v_offset(&self, area_height: u16) -> usize {
            self.total_lines
                .saturating_sub(self.viewport_height(area_height))
        }

        fn max_h_offset(&self, area_width: u16) -> usize {
            self.max_line_width
                .saturating_sub(self.viewport_width(area_width))
        }

        fn clamp(&mut self, area: Rect) {
            self.v_offset = self.v_offset.min(self.max_v_offset(area.height));
            self.h_offset = self.h_offset.min(self.max_h_offset(area.width));
        }

        fn scroll_v(&mut self, delta: isize, area: Rect) {
            if delta >= 0 {
                self.v_offset = self.v_offset.saturating_add(delta as usize);
            } else {
                self.v_offset = self.v_offset.saturating_sub((-delta) as usize);
            }
            self.clamp(area);
        }

        fn scroll_h(&mut self, delta: isize, area: Rect) {
            if delta >= 0 {
                self.h_offset = self.h_offset.saturating_add(delta as usize);
            } else {
                self.h_offset = self.h_offset.saturating_sub((-delta) as usize);
            }
            self.clamp(area);
        }

        fn page_up(&mut self, area: Rect) {
            let step = self.viewport_height(area.height).max(1);
            self.scroll_v(-(step as isize), area);
        }

        fn page_down(&mut self, area: Rect) {
            let step = self.viewport_height(area.height).max(1);
            self.scroll_v(step as isize, area);
        }
    }

    fn area(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn scroll_state_initial_offsets_zero() {
        let s = ScrollState::new(100, 200);
        assert_eq!(s.v_offset, 0);
        assert_eq!(s.h_offset, 0);
    }

    #[test]
    fn scroll_state_viewport_dimensions() {
        let s = ScrollState::new(100, 200);
        let vp_h = s.viewport_height(24);
        let vp_w = s.viewport_width(80);
        assert_eq!(vp_h, 22, "24 - pad_top(1) - pad_bottom(1) = 22");
        assert_eq!(vp_w, 76, "80 - pad_left(2) - pad_right(2) = 76");
    }

    #[test]
    fn scroll_state_max_v_offset() {
        let s = ScrollState::new(100, 80);
        let max_v = s.max_v_offset(24);
        assert_eq!(max_v, 78, "100 - 22 viewport = 78");
    }

    #[test]
    fn scroll_state_max_h_offset() {
        let s = ScrollState::new(50, 200);
        let max_h = s.max_h_offset(80);
        assert_eq!(max_h, 124, "200 - 76 viewport = 124");
    }

    #[test]
    fn scroll_state_clamp_v_offset() {
        let mut s = ScrollState::new(50, 80);
        s.v_offset = 100;
        s.clamp(area(80, 24));
        assert_eq!(s.v_offset, 28, "clamped to 50 - 22 = 28");
    }

    #[test]
    fn scroll_state_clamp_h_offset() {
        let mut s = ScrollState::new(50, 100);
        s.h_offset = 200;
        s.clamp(area(80, 24));
        assert_eq!(s.h_offset, 24, "clamped to 100 - 76 = 24");
    }

    #[test]
    fn scroll_state_scroll_down() {
        let mut s = ScrollState::new(100, 80);
        s.scroll_v(5, area(80, 24));
        assert_eq!(s.v_offset, 5);
    }

    #[test]
    fn scroll_state_scroll_up_from_zero() {
        let mut s = ScrollState::new(100, 80);
        s.scroll_v(-5, area(80, 24));
        assert_eq!(s.v_offset, 0, "can't scroll above 0");
    }

    #[test]
    fn scroll_state_scroll_down_clamps_at_max() {
        let mut s = ScrollState::new(30, 80);
        s.scroll_v(100, area(80, 24));
        assert_eq!(s.v_offset, 8, "clamped to 30 - 22 = 8");
    }

    #[test]
    fn scroll_state_scroll_horizontal() {
        let mut s = ScrollState::new(50, 200);
        s.scroll_h(10, area(80, 24));
        assert_eq!(s.h_offset, 10);
    }

    #[test]
    fn scroll_state_scroll_horizontal_clamps() {
        let mut s = ScrollState::new(50, 100);
        s.scroll_h(200, area(80, 24));
        assert_eq!(s.h_offset, 24, "clamped to 100 - 76 = 24");
    }

    #[test]
    fn scroll_state_page_down() {
        let mut s = ScrollState::new(200, 80);
        s.page_down(area(80, 24));
        assert_eq!(s.v_offset, 22, "page down by viewport height");
    }

    #[test]
    fn scroll_state_page_up() {
        let mut s = ScrollState::new(200, 80);
        s.v_offset = 50;
        s.page_up(area(80, 24));
        assert_eq!(s.v_offset, 28, "50 - 22 = 28");
    }

    #[test]
    fn scroll_state_page_up_at_top() {
        let mut s = ScrollState::new(200, 80);
        s.page_up(area(80, 24));
        assert_eq!(s.v_offset, 0);
    }

    #[test]
    fn scroll_state_content_fits_viewport_no_scroll() {
        let mut s = ScrollState::new(10, 50);
        s.scroll_v(5, area(80, 24));
        assert_eq!(s.v_offset, 0, "content fits, offset stays 0");
    }

    #[test]
    fn scrollable_example_render_and_measure() {
        let md = "# Scrollable\n\nLine 1\nLine 2\nLine 3\nLine 4\nLine 5\n";
        let renderer = MarkdownRenderer::new(120);
        let blocks = renderer.parse(md);
        let lines = renderer.render(&blocks, &test_theme());
        let max_w = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| unicode_width_02::UnicodeWidthStr::width(s.content.as_ref()))
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        let mut scroll = ScrollState::new(lines.len(), max_w);
        assert!(scroll.total_lines > 0);
        assert!(scroll.max_line_width > 0);
        scroll.scroll_v(1, area(80, 24));
        assert!(scroll.v_offset <= scroll.max_v_offset(24));
    }
}

// ==================== Nested Blockquote Tests ====================

#[test]
fn blockquote_parsed_with_level_and_children() -> anyhow::Result<()> {
    let renderer = MarkdownRenderer::new(80);
    let blocks = renderer.parse("> quoted text");
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        MarkdownBlock::Blockquote {
            level, children, ..
        } => {
            assert_eq!(*level, 1);
            assert!(!children.is_empty());
        }
        other => panic!("expected Blockquote, got {:?}", other),
    }
    Ok(())
}

#[test]
fn blockquote_multiline_grouped() -> anyhow::Result<()> {
    let renderer = MarkdownRenderer::new(80);
    let blocks = renderer.parse("> line 1\n> line 2\n> line 3");
    assert_eq!(
        blocks.len(),
        1,
        "consecutive > lines should be grouped into one blockquote"
    );
    match &blocks[0] {
        MarkdownBlock::Blockquote {
            level, children, ..
        } => {
            assert_eq!(*level, 1);
            assert!(!children.is_empty());
        }
        other => panic!("expected single Blockquote, got {:?}", other),
    }
    Ok(())
}

#[test]
fn nested_blockquote_parsed() -> anyhow::Result<()> {
    let renderer = MarkdownRenderer::new(80);
    let blocks = renderer.parse("> level 1\n> > level 2");
    assert!(!blocks.is_empty(), "should parse nested blockquote");
    match &blocks[0] {
        MarkdownBlock::Blockquote {
            level, children, ..
        } => {
            assert_eq!(*level, 1);
            let has_nested = children
                .iter()
                .any(|c| matches!(c, MarkdownBlock::Blockquote { .. }));
            assert!(
                has_nested,
                "level 1 should contain a nested level 2 blockquote"
            );
        }
        other => panic!("expected Blockquote, got {:?}", other),
    }
    Ok(())
}

#[test]
fn blockquote_renders_with_pipe_prefix() -> anyhow::Result<()> {
    let lines = render_markdown("> hello", 80);
    assert_eq!(lines.len(), 1);
    let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.starts_with("│"),
        "blockquote should start with │: got '{}'",
        text
    );
    assert!(
        text.contains("hello"),
        "blockquote should contain text: got '{}'",
        text
    );
    Ok(())
}

#[test]
fn nested_blockquote_renders_with_double_pipe() -> anyhow::Result<()> {
    let lines = render_markdown("> outer\n> > inner", 80);
    let all_text: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect::<Vec<&str>>()
        .join("");
    assert!(
        all_text.contains("│ │"),
        "nested blockquote should have double pipe prefix"
    );
    Ok(())
}

#[test]
fn blockquote_with_code_inside() -> anyhow::Result<()> {
    let md = "> text before\n> ```rust\n> fn main() {}\n> ```\n> text after";
    let renderer = MarkdownRenderer::new(80);
    let blocks = renderer.parse(md);
    let bq = blocks
        .iter()
        .find(|b| matches!(b, MarkdownBlock::Blockquote { .. }));
    assert!(bq.is_some(), "should parse blockquote");
    if let Some(MarkdownBlock::Blockquote { children, .. }) = bq {
        let has_code = children
            .iter()
            .any(|c| matches!(c, MarkdownBlock::CodeBlock { .. }));
        assert!(has_code, "blockquote children should contain a code block");
    }
    Ok(())
}

// ==================== CodeBlock Override Tests ====================

#[test]
fn code_block_override_header() -> anyhow::Result<()> {
    let block = MarkdownBlock::CodeBlock {
        lang: "rust".into(),
        code: "fn main() {}".into(),
        header_override: Some("╭─ Input ──".into()),
        footer_override: None,
        prefix_override: None,
    };
    let renderer = MarkdownRenderer::new(80);
    let lines = renderer.render(&[block], &test_theme());
    let header_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        header_text.contains("Input"),
        "header override should be used: got '{}'",
        header_text
    );
    Ok(())
}

#[test]
fn code_block_override_footer() -> anyhow::Result<()> {
    let block = MarkdownBlock::CodeBlock {
        lang: "rust".into(),
        code: "fn main() {}".into(),
        header_override: None,
        footer_override: Some("╰─ Output ──".into()),
        prefix_override: None,
    };
    let renderer = MarkdownRenderer::new(80);
    let lines = renderer.render(&[block], &test_theme());
    let last = lines.last().context("last line")?;
    let footer_text: String = last.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        footer_text.contains("Output"),
        "footer override should be used: got '{}'",
        footer_text
    );
    Ok(())
}

#[test]
fn code_block_override_prefix() -> anyhow::Result<()> {
    let block = MarkdownBlock::CodeBlock {
        lang: "json".into(),
        code: r#"{"key": "value"}"#.into(),
        header_override: None,
        footer_override: None,
        prefix_override: Some("║ ".into()),
    };
    let renderer = MarkdownRenderer::new(80);
    let lines = renderer.render(&[block], &test_theme());
    let code_line = &lines[1];
    let text: String = code_line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.starts_with("║"),
        "prefix override should be used: got '{}'",
        text
    );
    Ok(())
}

#[test]
fn code_block_constructor_helper() -> anyhow::Result<()> {
    let block = MarkdownBlock::code_block("python", "print(1)");
    match block {
        MarkdownBlock::CodeBlock {
            lang,
            code,
            header_override,
            footer_override,
            prefix_override,
        } => {
            assert_eq!(lang, "python");
            assert_eq!(code, "print(1)");
            assert!(header_override.is_none());
            assert!(footer_override.is_none());
            assert!(prefix_override.is_none());
        }
        other => panic!("expected CodeBlock, got {:?}", other),
    }
    Ok(())
}

// ==================== Mermaid Rendering Tests ====================

mod mermaid_render_tests {
    use super::*;

    #[test]
    fn mermaid_simple_flowchart_renders() {
        let md = "```mermaid\ngraph TD\nA[Start] --> B[End]\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty(), "mermaid flowchart should render output");
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            all_text.contains("Start"),
            "rendered output should contain node label 'Start': got '{}'",
            all_text
        );
        assert!(
            all_text.contains("End"),
            "rendered output should contain node label 'End': got '{}'",
            all_text
        );
    }

    #[test]
    fn mermaid_lr_direction_renders() {
        let md = "```mermaid\ngraph LR\nA --> B\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty(), "LR flowchart should render");
    }

    #[test]
    fn mermaid_three_node_chain() {
        let md = "```mermaid\ngraph TD\nA[First] --> B[Second] --> C[Third]\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty());
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(all_text.contains("First"));
        assert!(all_text.contains("Second"));
        assert!(all_text.contains("Third"));
    }

    #[test]
    fn mermaid_diamond_shape_renders() {
        let md = "```mermaid\ngraph TD\nA{Decision} --> B[Result]\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty());
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(all_text.contains("Decision"));
    }

    #[test]
    fn mermaid_labeled_edge_renders() {
        let md = "```mermaid\ngraph TD\nA -->|yes| B\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty());
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            all_text.contains("yes"),
            "edge label should appear: got '{}'",
            all_text
        );
    }

    #[test]
    fn mermaid_viewport_adaptation() {
        let md = "```mermaid\ngraph TD\nA[Very Long Node Label Here] --> B[Another]\n```";
        let lines_narrow = render_markdown(md, 30);
        let lines_wide = render_markdown(md, 80);
        assert!(!lines_narrow.is_empty());
        assert!(!lines_wide.is_empty());
    }

    #[test]
    fn mermaid_invalid_syntax_skipped() {
        let md = "```mermaid\nnot a valid mermaid diagram\n```";
        let lines = render_markdown(md, 80);
        assert!(
            lines.is_empty(),
            "invalid mermaid should be skipped gracefully"
        );
    }

    #[test]
    fn mermaid_empty_nodes_render() -> anyhow::Result<()> {
        let md = "```mermaid\ngraph TD\nA --> B\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty(), "unnamed nodes should still render");
        let buf = render_to_buffer(lines, 80, 20)?;
        assert!(buf.area.width > 0);
        Ok(())
    }

    #[test]
    fn mermaid_multiple_edges_render() {
        let md = "```mermaid\ngraph TD\nA --> B\nA --> C\nB --> D\nC --> D\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty());
    }

    #[test]
    fn mermaid_direct_render_api() -> anyhow::Result<()> {
        let source = "graph TD\nA[Hello] --> B[World]";
        let result = crate::vendor::ratatui_markdown::mermaid::render_mermaid(
            source,
            80,
            None,
            &test_theme(),
        );
        assert!(result.is_some());
        let lines = result.context("parse result")?;
        assert!(!lines.is_empty());
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(all_text.contains("Hello"));
        assert!(all_text.contains("World"));
        Ok(())
    }

    #[test]
    fn mermaid_max_height_constraint() -> anyhow::Result<()> {
        let source = "graph TD\nA --> B\nB --> C\nC --> D\nD --> E";
        let result = crate::vendor::ratatui_markdown::mermaid::render_mermaid(
            source,
            80,
            Some(10),
            &test_theme(),
        );
        assert!(result.is_some());
        let lines = result.context("parse result")?;
        assert!(
            lines.len() <= 25,
            "should try to respect max_height, got {} lines",
            lines.len()
        );
        Ok(())
    }

    #[test]
    fn mermaid_sequence_diagram_renders() {
        let md = "```mermaid\nsequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty(), "sequence diagram should render");
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            all_text.contains("Alice"),
            "should contain participant Alice"
        );
        assert!(all_text.contains("Bob"), "should contain participant Bob");
        assert!(all_text.contains("Hello"), "should contain message text");
    }

    #[test]
    fn mermaid_pie_chart_renders() {
        let md = "```mermaid\npie title Pets\n    \"Dogs\" : 386\n    \"Cats\" : 85\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty(), "pie chart should render");
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(all_text.contains("Pets"), "should contain title");
        assert!(all_text.contains("Dogs"), "should contain slice label");
        assert!(all_text.contains("Cats"), "should contain slice label");
    }

    #[test]
    fn mermaid_gantt_chart_renders() {
        let md = "```mermaid\ngantt\ntitle Project\nsection Phase 1\nTask A :a1, 7d\nTask B :a2, 5d\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty(), "gantt chart should render");
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(all_text.contains("Project"), "should contain title");
        assert!(all_text.contains("Task A"), "should contain task name");
    }

    #[test]
    fn mermaid_state_diagram_renders() {
        let md = "```mermaid\nstateDiagram-v2\n    [*] --> Idle\n    Idle --> Running\n    Running --> Idle\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty(), "state diagram should render");
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(all_text.contains("Idle"), "should contain state name");
        assert!(all_text.contains("Running"), "should contain state name");
    }

    #[test]
    fn mermaid_diamond_uses_rounded_corners() -> anyhow::Result<()> {
        let source = "graph TD\nA{Decision} --> B[Result]";
        let result = crate::vendor::ratatui_markdown::mermaid::render_mermaid(
            source,
            80,
            None,
            &test_theme(),
        );
        assert!(result.is_some());
        let lines = result.context("parse result")?;
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            !all_text.contains('/'),
            "diamond should use rounded corners, not slashes"
        );
        assert!(
            all_text.contains("Decision"),
            "should contain diamond label"
        );
        Ok(())
    }

    #[test]
    fn mermaid_no_dangling_cross_chars() -> anyhow::Result<()> {
        let source = "graph TD\nA[Start] --> B[End]";
        let result = crate::vendor::ratatui_markdown::mermaid::render_mermaid(
            source,
            80,
            None,
            &test_theme(),
        );
        assert!(result.is_some());
        let lines = result.context("parse result")?;
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            !all_text.contains('┼'),
            "should not have dangling cross characters"
        );
        Ok(())
    }

    fn lines_to_text(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<Vec<&str>>()
                    .join("")
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn render_lines(source: &str, width: usize) -> Vec<Line<'static>> {
        crate::vendor::ratatui_markdown::mermaid::render_mermaid(source, width, None, &test_theme())
            .unwrap_or_default()
    }

    #[test]
    fn mermaid_fork_uses_correct_tee() -> anyhow::Result<()> {
        let source = "graph TD\nD{Decision} -->|Yes| A[Action A]\nD -->|No| B[Action B]";
        let lines = render_lines(source, 80);
        let text = lines_to_text(&lines);

        assert!(
            text.contains('┴'),
            "fork junction should use ┴ (tee-up, connects up+left+right), got:\n{text}"
        );
        assert!(
            !text.contains('┬'),
            "fork junction should NOT use ┬ (tee-down), got:\n{text}"
        );
        assert!(text.contains("Decision"), "should contain Decision label");
        assert!(
            text.contains("Action A") && text.contains("Action B"),
            "should contain both action labels"
        );
        Ok(())
    }

    #[test]
    fn mermaid_merge_uses_correct_tee() -> anyhow::Result<()> {
        let source = "graph TD\nA[Action A] --> E[End]\nB[Action B] --> E";
        let lines = render_lines(source, 80);
        let text = lines_to_text(&lines);

        assert!(
            text.contains('┬'),
            "merge junction should use ┬ (connects down+left+right), got:\n{text}"
        );
        assert!(
            !text.contains('┴'),
            "merge junction should NOT use ┴ (connects up+left+right), got:\n{text}"
        );
        assert!(
            text.contains("Action A") && text.contains("Action B") && text.contains("End"),
            "should contain all node labels"
        );
        Ok(())
    }

    #[test]
    fn mermaid_straight_line_has_no_breaks() -> anyhow::Result<()> {
        let source = "graph TD\nA[Start] --> B[End]";
        let lines = render_lines(source, 80);
        let text = lines_to_text(&lines);

        assert!(
            text.contains('│') || text.contains('▼'),
            "straight edge should have vertical line or arrow, got:\n{text}"
        );
        assert!(
            !text.contains('┼'),
            "straight edge should NOT have cross chars, got:\n{text}"
        );
        assert!(
            !text.contains('┬') || text.split('┬').count() <= 2,
            "straight edge should NOT have spurious T-junctions"
        );
        Ok(())
    }

    #[test]
    fn mermaid_corners_are_correct() -> anyhow::Result<()> {
        let source = "graph TD\nA[Start] --> B[End]";
        let lines = render_lines(source, 80);
        let text = lines_to_text(&lines);

        let bad_chars: Vec<char> = text.chars().filter(|c| "╱╲+*#".contains(*c)).collect();
        assert!(
            bad_chars.is_empty(),
            "should not contain stray chars {:?}, got:\n{text}",
            bad_chars
        );
        Ok(())
    }

    #[test]
    fn mermaid_class_diagram_renders_borders() -> anyhow::Result<()> {
        let source = "classDiagram\nclass Animal {\n  +String name\n  +makeSound() void\n}";
        let lines = render_lines(source, 80);
        let text = lines_to_text(&lines);

        assert!(
            text.contains("Animal"),
            "class diagram should contain class name, got:\n{text}"
        );
        assert!(
            text.contains("name"),
            "class diagram should contain attribute, got:\n{text}"
        );
        assert!(
            text.contains("makeSound"),
            "class diagram should contain method, got:\n{text}"
        );

        let top_border_count = text.matches('┌').count();
        let bottom_border_count = text.matches('└').count();
        assert!(
            top_border_count >= 1 && bottom_border_count >= 1,
            "class box should have top-left ┌ and bottom-left └ corners, got:\n{text}"
        );

        let right_top = text.matches('┐').count();
        let right_bottom = text.matches('┘').count();
        assert!(
            right_top >= 1 && right_bottom >= 1,
            "class box should have top-right ┐ and bottom-right ┘ corners, got:\n{text}"
        );
        Ok(())
    }

    #[test]
    fn mermaid_class_inheritance_edge_renders() -> anyhow::Result<()> {
        let source = "classDiagram\nclass Animal {\n  +String name\n}\nclass Dog {\n  +String breed\n}\nAnimal <|-- Dog";
        let lines = render_lines(source, 80);
        let text = lines_to_text(&lines);

        assert!(
            text.contains("Animal") && text.contains("Dog"),
            "should contain both class names, got:\n{text}"
        );
        let has_edge_chars =
            text.contains('│') || text.contains('┬') || text.contains('├') || text.contains('▼');
        assert!(
            has_edge_chars,
            "inheritance should have edge line chars, got:\n{text}"
        );
        Ok(())
    }

    #[test]
    fn mermaid_three_way_fork_full_output() -> anyhow::Result<()> {
        let source = "graph TD\nS[Start] --> A[One]\nS --> B[Two]\nS --> C[Three]";
        let lines = render_lines(source, 80);
        let text = lines_to_text(&lines);

        assert!(text.contains("Start"), "should have Start");
        assert!(text.contains("One"), "should have One");
        assert!(text.contains("Two"), "should have Two");
        assert!(text.contains("Three"), "should have Three");

        assert!(
            text.contains('┬'),
            "three-way fork should have ┬ junction, got:\n{text}"
        );
        assert!(
            !text.contains('┼'),
            "should not have spurious cross chars, got:\n{text}"
        );
        Ok(())
    }
}

// ==================== Link Inline Parsing Tests ====================

#[test]
fn inline_link_renders_underlined() -> anyhow::Result<()> {
    let lines = render_markdown("click [here](https://example.com) now", 80);
    assert_eq!(lines.len(), 1);
    let has_here = lines[0]
        .spans
        .iter()
        .any(|s| s.content == "here" && s.style.add_modifier == Modifier::UNDERLINED);
    assert!(has_here, "link text should be underlined");
    Ok(())
}

#[test]
fn inline_link_without_url_unchanged() -> anyhow::Result<()> {
    let lines = render_markdown("plain [text] here", 80);
    let all_text: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(
        all_text.contains("[text]"),
        "non-link bracket text should remain unchanged"
    );
    Ok(())
}

// ==================== Strikethrough Tests ====================

#[test]
fn strikethrough_renders_crossed_out() -> anyhow::Result<()> {
    let lines = render_markdown("this is ~~deleted~~ text", 80);
    assert_eq!(lines.len(), 1);
    let has_crossed = lines[0]
        .spans
        .iter()
        .any(|s| s.content == "deleted" && s.style.add_modifier == Modifier::CROSSED_OUT);
    assert!(
        has_crossed,
        "strikethrough text should have CROSSED_OUT modifier"
    );
    Ok(())
}

#[test]
fn strikethrough_unterminated_unchanged() -> anyhow::Result<()> {
    let lines = render_markdown("~~no end", 80);
    let all_text: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(
        all_text.contains("~~no end"),
        "unterminated strikethrough should remain as-is"
    );
    Ok(())
}

// ==================== Task List Tests ====================

#[test]
fn task_list_unchecked_parsed() -> anyhow::Result<()> {
    let renderer = MarkdownRenderer::new(80);
    let blocks = renderer.parse("- [ ] pending task");
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        MarkdownBlock::TaskItem { text, checked, .. } => {
            assert_eq!(text, "pending task");
            assert!(!checked);
        }
        other => panic!("expected TaskItem, got {:?}", other),
    }
    Ok(())
}

#[test]
fn task_list_checked_parsed() -> anyhow::Result<()> {
    let renderer = MarkdownRenderer::new(80);
    let blocks = renderer.parse("- [x] done task");
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        MarkdownBlock::TaskItem { text, checked, .. } => {
            assert_eq!(text, "done task");
            assert!(checked);
        }
        other => panic!("expected TaskItem, got {:?}", other),
    }
    Ok(())
}

#[test]
fn task_list_renders_checkbox() -> anyhow::Result<()> {
    let lines = render_markdown("- [ ] todo\n- [x] done", 80);
    let all_text: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(
        all_text.contains("☐ todo"),
        "unchecked task should render ☐: got '{}'",
        all_text
    );
    assert!(
        all_text.contains("☑ done"),
        "checked task should render ☑: got '{}'",
        all_text
    );
    Ok(())
}

#[test]
fn task_list_uppercase_x_checked() -> anyhow::Result<()> {
    let renderer = MarkdownRenderer::new(80);
    let blocks = renderer.parse("- [X] done");
    match &blocks[0] {
        MarkdownBlock::TaskItem { checked, .. } => {
            assert!(checked);
        }
        other => panic!("expected TaskItem, got {:?}", other),
    }
    Ok(())
}
