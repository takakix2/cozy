//! Syntax highlighting front-end shared by the renderer.
//!
//! The renderer never computes highlights per line per frame anymore. Instead it
//! asks this `Highlighter` to `ensure` the visible window is up to date, then
//! reads precomputed per-line spans via `style_at`. With the `treesitter`
//! feature (default) highlighting comes from a real parse tree, so multi-line
//! strings and block comments are correct; without it, the regex engine in
//! `super::syntax` fills the same cache line by line.

use ratatui::style::Style;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

/// Per-line highlight spans: `(start_byte, end_byte, style)`, may overlap.
type LineSpans = Vec<(usize, usize, Style)>;

pub struct Highlighter {
    ext: Option<String>,
    /// Buffer changed since the cache was built; the next `ensure` recomputes.
    dirty: bool,
    /// line index -> spans (only the `cached_range` window is populated).
    cache: HashMap<usize, LineSpans>,
    cached_range: Range<usize>,
    #[cfg(feature = "treesitter")]
    ts: ts::TsBackend,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self {
            ext: None,
            dirty: true,
            cache: HashMap::new(),
            cached_range: 0..0,
            #[cfg(feature = "treesitter")]
            ts: ts::TsBackend::new(),
        }
    }
}

impl Highlighter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve the language from a file path. Cheap; safe to call on every open.
    pub fn set_file(&mut self, path: Option<&Path>) {
        let ext = path
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase());
        if ext != self.ext {
            self.ext = ext;
            #[cfg(feature = "treesitter")]
            self.ts.set_language(self.ext.as_deref());
        }
        self.mark_dirty();
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Make sure the cache covers `visible`. Reparses only when the buffer is
    /// dirty; re-queries when the visible window moved.
    pub fn ensure(&mut self, lines: &[String], visible: Range<usize>) {
        if !self.dirty && self.cached_range == visible {
            return;
        }

        #[cfg(feature = "treesitter")]
        {
            if self.dirty {
                self.ts.reparse(lines);
            }
            self.cache.clear();
            self.ts.spans_for(lines, &visible, &mut self.cache);
        }

        #[cfg(not(feature = "treesitter"))]
        {
            self.cache.clear();
            let hl = super::syntax::SyntaxHighlighter::new(self.ext.as_deref());
            for y in visible.clone() {
                if let Some(line) = lines.get(y) {
                    self.cache.insert(y, regex_line_spans(&hl, line));
                }
            }
        }

        self.cached_range = visible;
        self.dirty = false;
    }

    /// Base style for the character at `byte` on line `y`, if highlighted.
    /// Later spans win (highlight precedence), so search from the end.
    pub fn style_at(&self, y: usize, byte: usize) -> Option<Style> {
        self.cache.get(&y).and_then(|spans| {
            spans
                .iter()
                .rev()
                .find(|&&(s, e, _)| byte >= s && byte < e)
                .map(|&(_, _, style)| style)
        })
    }
}

#[cfg(not(feature = "treesitter"))]
fn regex_line_spans(hl: &super::syntax::SyntaxHighlighter, line: &str) -> LineSpans {
    let mut spans = Vec::new();
    let mut pos = 0usize;
    for (text, style) in hl.highlight(line) {
        let len = text.len();
        if style != Style::default() {
            spans.push((pos, pos + len, style));
        }
        pos += len;
    }
    spans
}

#[cfg(all(test, feature = "treesitter"))]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn to_lines(s: &str) -> Vec<String> {
        s.lines().map(|l| l.to_string()).collect()
    }

    fn fg_at(h: &Highlighter, y: usize, byte: usize) -> Option<Color> {
        h.style_at(y, byte).and_then(|s| s.fg)
    }

    #[test]
    fn rust_keyword_comment_and_multiline_string() {
        // line0: comment, line1: `fn` keyword, lines 2-3: a string literal that
        // spans the newline — the exact case the old regex highlighter got wrong.
        let src = "// note\nfn main() {\n    let s = \"a\n b\";\n}";
        let buf = to_lines(src);
        let mut h = Highlighter::new();
        h.set_file(Some(Path::new("x.rs")));
        h.ensure(&buf, 0..buf.len());

        assert_eq!(fg_at(&h, 0, 0), Some(Color::DarkGray), "comment");
        assert_eq!(fg_at(&h, 1, 0), Some(Color::Magenta), "fn keyword");

        let quote = buf[2].find('"').unwrap();
        assert_eq!(fg_at(&h, 2, quote), Some(Color::Green), "string open");
        // Continuation line: the leading space is *inside* the string literal.
        assert_eq!(fg_at(&h, 3, 0), Some(Color::Green), "string continues");
    }

    #[test]
    fn unknown_extension_has_no_highlight() {
        let buf = to_lines("fn main() {}");
        let mut h = Highlighter::new();
        h.set_file(Some(Path::new("notes.txt")));
        h.ensure(&buf, 0..buf.len());
        assert_eq!(h.style_at(0, 0), None);
    }
}

#[cfg(feature = "treesitter")]
mod ts {
    use super::LineSpans;
    use ratatui::style::{Color, Style};
    use std::collections::HashMap;
    use std::ops::Range;
    use tree_sitter::{Language, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

    pub struct TsBackend {
        parser: Parser,
        tree: Option<Tree>,
        lang: Option<LangConfig>,
        /// The exact source the current `tree` was parsed from (`lines` joined
        /// with '\n'), reused by `spans_for` so we don't rebuild it twice.
        source: String,
    }

    struct LangConfig {
        query: Query,
        /// capture index -> style.
        styles: Vec<Style>,
    }

    impl TsBackend {
        pub fn new() -> Self {
            Self {
                parser: Parser::new(),
                tree: None,
                lang: None,
                source: String::new(),
            }
        }

        pub fn set_language(&mut self, ext: Option<&str>) {
            self.tree = None;
            self.source.clear();
            self.lang = match resolve(ext) {
                Some((language, query_src)) => {
                    if self.parser.set_language(&language).is_err() {
                        return;
                    }
                    match Query::new(&language, query_src) {
                        Ok(query) => {
                            let styles = capture_styles(&query);
                            Some(LangConfig { query, styles })
                        }
                        Err(_) => None,
                    }
                }
                None => None,
            };
        }

        pub fn reparse(&mut self, lines: &[String]) {
            if self.lang.is_none() {
                self.tree = None;
                return;
            }
            self.source = lines.join("\n");
            self.tree = self.parser.parse(&self.source, None);
        }

        pub fn spans_for(
            &self,
            lines: &[String],
            visible: &Range<usize>,
            out: &mut HashMap<usize, LineSpans>,
        ) {
            let (Some(lc), Some(tree)) = (self.lang.as_ref(), self.tree.as_ref()) else {
                return;
            };
            let mut cursor = QueryCursor::new();
            cursor.set_point_range(Point::new(visible.start, 0)..Point::new(visible.end, 0));
            let mut captures =
                cursor.captures(&lc.query, tree.root_node(), self.source.as_bytes());
            while let Some((m, idx)) = captures.next() {
                let cap = m.captures[*idx];
                let style = lc.styles[cap.index as usize];
                if style == Style::default() {
                    continue;
                }
                push_span(lines, out, cap.node.start_position(), cap.node.end_position(), style, visible);
            }
        }
    }

    /// Split a possibly multi-line capture into per-line byte ranges.
    fn push_span(
        lines: &[String],
        out: &mut HashMap<usize, LineSpans>,
        start: Point,
        end: Point,
        style: Style,
        visible: &Range<usize>,
    ) {
        for row in start.row..=end.row {
            if row < visible.start || row >= visible.end {
                continue;
            }
            let line_len = lines.get(row).map(|l| l.len()).unwrap_or(0);
            let s = if row == start.row { start.column } else { 0 };
            let e = if row == end.row { end.column } else { line_len };
            let e = e.min(line_len);
            let s = s.min(e);
            if s < e {
                out.entry(row).or_default().push((s, e, style));
            }
        }
    }

    fn resolve(ext: Option<&str>) -> Option<(Language, &'static str)> {
        Some(match ext? {
            "rs" => (
                tree_sitter_rust::LANGUAGE.into(),
                tree_sitter_rust::HIGHLIGHTS_QUERY,
            ),
            "py" | "pyi" => (
                tree_sitter_python::LANGUAGE.into(),
                tree_sitter_python::HIGHLIGHTS_QUERY,
            ),
            "js" | "mjs" | "cjs" | "jsx" => (
                tree_sitter_javascript::LANGUAGE.into(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
            ),
            "json" => (
                tree_sitter_json::LANGUAGE.into(),
                tree_sitter_json::HIGHLIGHTS_QUERY,
            ),
            "toml" => (
                tree_sitter_toml_ng::LANGUAGE.into(),
                tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            ),
            _ => return None,
        })
    }

    fn capture_styles(query: &Query) -> Vec<Style> {
        query
            .capture_names()
            .iter()
            .map(|name| style_for_capture(name))
            .collect()
    }

    /// Map a tree-sitter capture name to a terminal style. The base segment
    /// (before the first '.') carries the meaning, e.g. `keyword.control`.
    fn style_for_capture(name: &str) -> Style {
        let base = name.split('.').next().unwrap_or(name);
        match base {
            "keyword" => Style::default().fg(Color::Magenta),
            "string" | "character" | "char" => Style::default().fg(Color::Green),
            "comment" => Style::default().fg(Color::DarkGray),
            "type" | "constructor" => Style::default().fg(Color::Cyan),
            "number" | "float" | "constant" | "boolean" => Style::default().fg(Color::Yellow),
            // LightBlue, not Blue: plain ANSI blue is too dark to read on a dark
            // background (it looks unhighlighted / "white").
            "function" => Style::default().fg(Color::LightBlue),
            "property" | "attribute" => Style::default().fg(Color::Cyan),
            _ => Style::default(),
        }
    }
}
