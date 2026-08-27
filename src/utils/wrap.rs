use crate::utils::unicode::{char_display_width_at, str_display_width, width_from};

/// Split `line` into byte ranges that each fit within `width` display columns.
/// Always returns at least one element.
pub fn wrap_chunks(line: &str, width: usize) -> Vec<(usize, usize)> {
    if width == 0 || line.is_empty() {
        return vec![(0, line.len())];
    }
    let mut chunks = Vec::new();
    let mut chunk_start = 0usize;
    let mut col = 0usize;

    for (i, ch) in line.char_indices() {
        let w = char_display_width_at(ch, col);
        if col + w > width && col > 0 {
            chunks.push((chunk_start, i));
            chunk_start = i;
            col = w;
        } else {
            col += w;
        }
    }
    chunks.push((chunk_start, line.len()));
    chunks
}

/// Number of visual rows a line occupies when wrapped to `width` columns.
pub fn visual_row_count(line: &str, width: usize) -> usize {
    wrap_chunks(line, width).len()
}

/// Byte offset (char boundary) within chunk `[cs, ce)` at visual column
/// `target_vcol` (0 = chunk start). Honors wide chars via unicode-width.
/// When `target_vcol` is past the chunk's content it clamps:
///   - `is_last_chunk`: returns `ce` (the line end), so the cursor may sit one
///     past the final char like a normal end-of-line position.
///   - otherwise: returns the start byte of the last char in the chunk, keeping
///     the cursor on this sub-row instead of spilling onto the next one.
pub fn byte_at_visual_col(
    line: &str,
    cs: usize,
    ce: usize,
    target_vcol: usize,
    is_last_chunk: bool,
) -> usize {
    let chunk = &line[cs..ce];
    if chunk.is_empty() {
        return cs;
    }
    // ⚠️ **TAB の幅は論理行の先頭からの桁で決まる。** ∴ chunk 内だけを見ても
    // 答えられない —— まず chunk がどの桁から始まるかを測る。
    let chunk_start_col = str_display_width(&line[..cs]);
    let mut w = 0usize;
    let mut last_char_start = cs;
    for (i, ch) in chunk.char_indices() {
        let cw = char_display_width_at(ch, chunk_start_col + w);
        let byte = cs + i;
        if target_vcol < w + cw {
            return byte;
        }
        last_char_start = byte;
        w += cw;
    }
    // target_vcol is at or past the chunk's width: clamp.
    if is_last_chunk { ce } else { last_char_start }
}

/// Display column of byte offset `cx` within `line` (whole-line, ignoring wrap).
/// Used by non-wrapped vertical movement to keep a width-aware goal column.
pub fn visual_col(line: &str, cx: usize) -> usize {
    let end = cx.min(line.len());
    str_display_width(&line[..end])
}

/// Returns `(sub_row, visual_col)` for byte offset `cx` within `line`.
/// `sub_row` is which wrapped chunk (0-indexed); `visual_col` is the display
/// column within that chunk.
pub fn cursor_visual_pos(line: &str, cx: usize, width: usize) -> (usize, usize) {
    let chunks = wrap_chunks(line, width);
    let last = chunks.len().saturating_sub(1);
    for (idx, &(s, e)) in chunks.iter().enumerate() {
        let in_chunk = if idx == last {
            cx >= s
        } else {
            cx >= s && cx < e
        };
        if in_chunk {
            let end = cx.min(line.len());
            let before = if s <= end { &line[s..end] } else { "" };
            // ⚠️ chunk の途中の桁も、TAB があるので**行頭からの桁**を起点に測る。
            return (idx, width_from(before, str_display_width(&line[..s])));
        }
    }
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_at_col_ascii() {
        let line = "abcdef";
        assert_eq!(byte_at_visual_col(line, 0, 6, 0, true), 0);
        assert_eq!(byte_at_visual_col(line, 0, 6, 3, true), 3);
        // Past the end of the last chunk clamps to line end (one past final char).
        assert_eq!(byte_at_visual_col(line, 0, 6, 10, true), 6);
    }

    #[test]
    fn byte_at_col_non_last_chunk_clamps_to_last_char() {
        // chunk [0,3) = "abc"; overshooting must stay on this sub-row (start of 'c').
        assert_eq!(byte_at_visual_col("abcdef", 0, 3, 10, false), 2);
    }

    #[test]
    fn byte_at_col_wide_chars() {
        // "あいう": each width 2, bytes 0..3, 3..6, 6..9.
        let line = "あいう";
        assert_eq!(byte_at_visual_col(line, 0, 9, 0, true), 0); // あ
        assert_eq!(byte_at_visual_col(line, 0, 9, 2, true), 3); // い
        assert_eq!(byte_at_visual_col(line, 0, 9, 3, true), 3); // mid of い snaps to い start
        assert_eq!(byte_at_visual_col(line, 0, 9, 4, true), 6); // う
        // Past end: last chunk -> line end; non-last -> start of last char (う @ 6).
        assert_eq!(byte_at_visual_col(line, 0, 9, 99, true), 9);
        assert_eq!(byte_at_visual_col(line, 0, 9, 99, false), 6);
    }

    /// 🚨 **幅を数える場所が割れていた**（2026-08-28 に統合）。折り返し
    /// （`wrap_chunks`）とカーソル（`visual_col` / `cursor_visual_pos`）が、
    /// 同じ行を**別の幅**として数えていた —— 制御文字を `wrap_chunks` は 1、
    /// `UnicodeWidthStr` 系は 0 としていたため。
    ///
    /// ⭐ ここは**値そのもの**ではなく **3 者が一致すること**を見る。
    /// 幅の規則を変えたときに、片方だけ追随し損ねたら落ちる。
    #[test]
    fn every_counter_agrees_on_a_line_with_control_chars() {
        let line = "a\rb\u{1b}c"; // a, CR, b, ESC, c
        // 制御文字は `^X` の 2 桁 → 1 + 2 + 1 + 2 + 1 = 7
        assert_eq!(visual_col(line, line.len()), 7);
        assert_eq!(
            crate::utils::unicode::display_width_up_to(line, line.len()),
            7,
            "display_width_up_to が visual_col と食い違っている"
        );

        // 折り返さない幅なら 1 行。`cursor_visual_pos` の列も同じ数え方になる。
        let (row, col) = cursor_visual_pos(line, line.len(), 80);
        assert_eq!((row, col), (0, 7), "cursor_visual_pos が別の幅を使っている");

        // 幅 7 にちょうど収まり、8 文字目で折り返す（＝ wrap も同じ規則）。
        assert_eq!(visual_row_count(line, 7), 1);
        assert_eq!(visual_row_count(line, 6), 2);
    }

    /// 陽性対照 —— 制御文字を含まない行は**今までどおり**。
    #[test]
    fn ordinary_lines_are_unaffected() {
        assert_eq!(visual_col("abc", 3), 3);
        assert_eq!(visual_col("あい", 6), 4);
        assert_eq!(visual_row_count("abcdef", 3), 2);
        assert_eq!(cursor_visual_pos("あい", 6, 80), (0, 4));
    }

    #[test]
    fn byte_at_col_empty_chunk() {
        assert_eq!(byte_at_visual_col("", 0, 0, 0, true), 0);
        assert_eq!(byte_at_visual_col("abc", 3, 3, 5, true), 3);
    }
}
