use crate::action::MatchAlign;
use crate::reducer::EventResult;
use crate::state::{EditorState, SearchMode};
use regex::Regex;

// ── buffer editing ────────────────────────────────────────────────────────────

pub fn update_search_buffer(editor: &mut EditorState, c: char) {
    let pos = editor.search_cursor;
    let buf = &mut editor.search_buffer;
    if pos <= buf.len() && buf.is_char_boundary(pos) {
        buf.insert(pos, c);
        editor.search_cursor = pos + c.len_utf8();
    }
    recompute_matches(editor);
    focus_nearest_match(editor);
}

pub fn insert_str_to_search_buffer(editor: &mut EditorState, s: &str) {
    let clean: String = s.chars().filter(|&c| c != '\n' && c != '\r').collect();
    if clean.is_empty() {
        return;
    }
    let pos = editor.search_cursor;
    let buf = &mut editor.search_buffer;
    if pos <= buf.len() && buf.is_char_boundary(pos) {
        buf.insert_str(pos, &clean);
        editor.search_cursor = pos + clean.len();
    }
    recompute_matches(editor);
    focus_nearest_match(editor);
}

pub fn delete_from_search_buffer(editor: &mut EditorState) {
    let pos = editor.search_cursor;
    if pos == 0 {
        return;
    }
    let buf = &mut editor.search_buffer;
    let prev = buf[..pos]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    buf.remove(prev);
    editor.search_cursor = prev;
    recompute_matches(editor);
    focus_nearest_match(editor);
}

pub fn delete_search_char_at_cursor(editor: &mut EditorState) {
    let pos = editor.search_cursor;
    let buf = &mut editor.search_buffer;
    if pos < buf.len() && buf.is_char_boundary(pos) {
        buf.remove(pos);
    }
    recompute_matches(editor);
    focus_nearest_match(editor);
}

pub fn move_search_cursor_left(editor: &mut EditorState) {
    let pos = editor.search_cursor;
    if pos == 0 {
        return;
    }
    editor.search_cursor = editor.search_buffer[..pos]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
}

pub fn move_search_cursor_right(editor: &mut EditorState) {
    let pos = editor.search_cursor;
    let len = editor.search_buffer.len();
    if pos >= len {
        return;
    }
    editor.search_cursor = editor.search_buffer[pos..]
        .chars()
        .next()
        .map(|c| pos + c.len_utf8())
        .unwrap_or(pos);
}

pub fn move_search_cursor_home(editor: &mut EditorState) {
    editor.search_cursor = 0;
}

pub fn move_search_cursor_end(editor: &mut EditorState) {
    editor.search_cursor = editor.search_buffer.len();
}

// ── match computation ─────────────────────────────────────────────────────────

/// Collect all byte-range matches of `query` in `line`.
fn find_in_line(line: &str, query: &str, mode: &SearchMode) -> Vec<(usize, usize)> {
    if query.is_empty() || line.is_empty() {
        return vec![];
    }
    match mode {
        SearchMode::MatchCase => line
            .match_indices(query)
            .map(|(i, s)| (i, i + s.len()))
            .collect(),
        SearchMode::ByWord => {
            let q = query.to_lowercase();
            let lower = line.to_lowercase();
            lower
                .match_indices(q.as_str())
                .map(|(i, s)| (i, i + s.len()))
                .collect()
        }
        SearchMode::Regex => Regex::new(query)
            .map(|re| re.find_iter(line).map(|m| (m.start(), m.end())).collect())
            .unwrap_or_default(),
    }
}

/// Rebuild editor.search_matches from the current search_buffer.
pub fn recompute_matches(editor: &mut EditorState) {
    editor.search_matches.clear();
    let query = editor.search_buffer.clone();
    if query.is_empty() {
        return;
    }
    for (y, line) in editor.buffer.lines.iter().enumerate() {
        for (s, e) in find_in_line(line, &query, &editor.search_mode) {
            editor.search_matches.push((y, s, e));
        }
    }
}

/// Set search_current to the first match at-or-after the current cursor.
pub fn focus_nearest_match(editor: &mut EditorState) {
    if editor.search_matches.is_empty() {
        editor.search_current = 0;
        return;
    }
    let cy = editor.cursor.y;
    let cx = editor.cursor.x;
    editor.search_current = editor
        .search_matches
        .iter()
        .position(|&(y, s, _)| y > cy || (y == cy && s >= cx))
        .unwrap_or(0);
    jump_to_current(editor);
    update_status(editor);
}

/// Move cursor to the currently focused match.
fn jump_to_current(editor: &mut EditorState) {
    if let Some(&(y, s, _)) = editor.search_matches.get(editor.search_current) {
        editor.cursor.y = y;
        editor.cursor.x = s;
    }
}

fn update_status(editor: &mut EditorState) {
    if editor.search_matches.is_empty() {
        crate::reducer::status::set_error(editor, &format!("'{}' not found", editor.search_buffer));
    } else {
        editor.status_message = None;
    }
}

// ── navigation ────────────────────────────────────────────────────────────────

pub fn apply_search_next(editor: &mut EditorState) -> EventResult {
    if editor.search_matches.is_empty() {
        recompute_matches(editor);
    }
    if editor.search_matches.is_empty() {
        crate::reducer::status::set_error(editor, &format!("'{}' not found", editor.search_buffer));
        return EventResult::Continue;
    }
    let total = editor.search_matches.len();
    editor.search_current = (editor.search_current + 1) % total;
    jump_to_current(editor);
    update_status(editor);
    EventResult::Continue
}

pub fn apply_search_previous(editor: &mut EditorState) -> EventResult {
    if editor.search_matches.is_empty() {
        recompute_matches(editor);
    }
    if editor.search_matches.is_empty() {
        crate::reducer::status::set_error(editor, &format!("'{}' not found", editor.search_buffer));
        return EventResult::Continue;
    }
    let total = editor.search_matches.len();
    editor.search_current = (editor.search_current + total - 1) % total;
    jump_to_current(editor);
    update_status(editor);
    EventResult::Continue
}

// ── mode helpers ──────────────────────────────────────────────────────────────

pub fn apply_toggle_search_mode(editor: &mut EditorState) -> EventResult {
    editor.search_mode = match editor.search_mode {
        SearchMode::MatchCase => SearchMode::Regex,
        SearchMode::Regex => SearchMode::ByWord,
        SearchMode::ByWord => SearchMode::MatchCase,
    };
    recompute_matches(editor);
    focus_nearest_match(editor);
    EventResult::Continue
}

// ── View（`czv`）: 欄を閉じた後も生きる検索（`#18`）─────────────────────────
//
// ⭐ 閲覧の席では、欄の `Enter` が**確定**になり、ヒットとハイライトは閲覧へ持ち帰る。
// そこから `n` / `N` で送り、`zz` / `zt` / `zb` で寄せる（less / vim の作法）。
//
// 🚨 **基準は `search_matches[search_current]` であってカーソルではない** —— View の
// 見えないカーソルは、検索ではヒット行へ動くが、スクロールすると画面の先頭行へ戻される
// （`reducer::view_scroll`）。カーソルを基準にすると、一度スクロールしただけで `zz` が
// 「先頭行を中央へ」に化ける。
//
// 📌 ヒットが無いときはどれも**何もしない** —— 寄せる・送る基準が無いので、動かないのが正直。
// ⚠️ `apply_search_next` と違って**数え直さない**: `Esc` で取り消した検索は欄の文字だけ
// 残っているので、数え直すと取り消したはずのハイライトが `n` で蘇る。

/// 検索欄の `Enter`（閲覧の席だけ）: View へ戻り、ヒットは残す。
pub fn apply_search_confirm(editor: &mut EditorState) -> EventResult {
    editor.glide_prefix = None;
    editor.mode = editor.home_mode();
    // 「'foo' not found」はそのまま残す —— 確定した結果が空振りだったことは言う価値がある。
    if !editor.search_matches.is_empty() {
        editor.status_message = None;
    }
    EventResult::Continue
}

/// 注目中のヒットが画面に見えているか。
fn current_match_visible(editor: &EditorState) -> bool {
    let page = editor.page_size.max(1);
    editor
        .search_matches
        .get(editor.search_current)
        .is_some_and(|&(y, _, _)| y >= editor.scroll_offset && y < editor.scroll_offset + page)
}

/// View の `n` / `N`。
///
/// ⭐ **起点は 2 通り**: 注目中のヒットが画面に見えていれば**そこから 1 つ先へ**、
/// スクロールして見えなくなっていれば**いま見ている所から最寄りへ**（less の作法）。
/// 📌 「ヒットの上に居れば」ではなく「見えていれば」なのは、見えているヒットから
/// 1 行スクロールしただけで `n` が同じヒットに留まる（＝押しても動かない）のを避けるため。
/// 端では折り返す。
pub fn apply_view_search_step(editor: &mut EditorState, forward: bool) -> EventResult {
    editor.glide_prefix = None;
    // ⚠️ 回数（`3n`）は取らずに捨てる —— 1 歩目の後の `scroll_offset` は描画まで動かないので、
    // 2 歩目の「見えているか」が嘘を答える。
    editor.glide_count.clear();
    let total = editor.search_matches.len();
    if total == 0 {
        return EventResult::Continue;
    }
    let next = if current_match_visible(editor) {
        if forward {
            (editor.search_current + 1) % total
        } else {
            (editor.search_current + total - 1) % total
        }
    } else {
        let top = editor.scroll_offset;
        if forward {
            editor
                .search_matches
                .iter()
                .position(|&(y, _, _)| y >= top)
                .unwrap_or(0)
        } else {
            editor
                .search_matches
                .iter()
                .rposition(|&(y, _, _)| y < top)
                .unwrap_or(total - 1)
        }
    };
    editor.search_current = next;
    jump_to_current(editor);
    update_status(editor);
    EventResult::Continue
}

/// View の `zz` / `zt` / `zb`: 注目中のヒット行を画面の中央 / 上 / 下へ。
///
/// ⚠️ 行は**論理行**で数える。折り返しの長い行が挟まると中央は目安になり、`zb` で
/// ヒットが画面の下へはみ出したら描画側の `adjust_scroll` が引き戻す（見えなくはならない）。
pub fn apply_align_match(editor: &mut EditorState, align: MatchAlign) -> EventResult {
    editor.glide_prefix = None;
    editor.glide_count.clear();
    let Some(&(y, _, _)) = editor.search_matches.get(editor.search_current) else {
        return EventResult::Continue;
    };
    let page = editor.page_size.max(1);
    let top = match align {
        MatchAlign::Top => y,
        MatchAlign::Center => y.saturating_sub(page / 2),
        MatchAlign::Bottom => y.saturating_sub(page - 1),
    };
    let last = editor.buffer.lines.len().saturating_sub(1);
    editor.scroll_offset = top.min(last);
    // カーソルはヒットの上に置く —— `adjust_scroll` が寄せた位置を動かさず、
    // 次の `n` も「見えているヒットから」になる。
    jump_to_current(editor);
    EventResult::Continue
}
