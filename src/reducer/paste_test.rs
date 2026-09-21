//! ペースト（`Action::InsertString` / `Action::PasteFromClipboard`）の検体。
//!
//! 🚨 **Edit 以外の入力欄を持つ全モードへの配線テスト**。
//! これまで `paste_string` は `if editor.mode == EditorMode::Edit` の1分岐しか持たず、
//! フッター入力欄（Search, Replace, Save/Open, Goto, Command 等）でペーストが全滅していた。
//!
//! 各モードの入力バッファに正しく文字列が挿入され、カーソルが移動し、
//! 必要に応じてマッチ再計算等の副作用が正しく起きることを検証する。

use crate::action::Action;
use crate::reducer::reduce;
use crate::state::{EditorMode, EditorState, ReplaceFocus, TextBuffer};

fn editor_with_text(lines: &[&str]) -> EditorState {
    let mut editor = EditorState::new(None);
    editor.buffer = TextBuffer::from_lines(lines.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    editor
}

// ── Search (Find) ─────────────────────────────────────────────────────────────

#[test]
fn test_paste_into_search_buffer() {
    let mut editor = editor_with_text(&["first needle here", "second needle there"]);
    editor.enter_mode(EditorMode::Search);

    reduce(&mut editor, Action::InsertString("needle".to_string()));

    assert_eq!(editor.search_buffer, "needle");
    assert_eq!(editor.search_cursor, 6);
    // マッチが再計算され、2件見つかっていること
    assert_eq!(editor.search_matches.len(), 2);
}

#[test]
fn test_paste_into_search_buffer_at_cursor() {
    let mut editor = editor_with_text(&["abcxyz"]);
    editor.enter_mode(EditorMode::Search);

    reduce(&mut editor, Action::InsertString("ax".to_string()));
    // カーソルを 'a' と 'x' の間に移動 (インデックス 1)
    editor.search_cursor = 1;

    reduce(&mut editor, Action::InsertString("bc".to_string()));

    assert_eq!(editor.search_buffer, "abcx");
    assert_eq!(editor.search_cursor, 3);
}

#[test]
fn test_paste_into_search_buffer_strips_newlines() {
    let mut editor = editor_with_text(&["needle"]);
    editor.enter_mode(EditorMode::Search);

    // クリップボード末尾に改行がついている場合
    reduce(
        &mut editor,
        Action::InsertString("nee\ndle\r\n".to_string()),
    );

    assert_eq!(editor.search_buffer, "needle");
    assert_eq!(editor.search_matches.len(), 1);
}

// ── Replace ───────────────────────────────────────────────────────────────────

#[test]
fn test_paste_into_replace_query_and_replacement() {
    let mut editor = editor_with_text(&["apple and banana"]);
    editor.enter_mode(EditorMode::Replace);

    // 1. Query 側にペースト
    editor.replace_focus = ReplaceFocus::Query;
    reduce(&mut editor, Action::InsertString("apple".to_string()));
    assert_eq!(editor.search_buffer, "apple");
    assert_eq!(editor.search_matches.len(), 1);

    // 2. Replace 側にペースト
    editor.replace_focus = ReplaceFocus::Replace;
    editor.search_cursor = 0;
    reduce(&mut editor, Action::InsertString("orange".to_string()));
    assert_eq!(editor.replace_buffer, "orange");
    assert_eq!(editor.search_cursor, 6);
}

// ── Save / Open ───────────────────────────────────────────────────────────────

#[test]
fn test_paste_into_open_prompt() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Open);

    reduce(
        &mut editor,
        Action::InsertString("my_file.rs\n".to_string()),
    );

    assert_eq!(editor.open_filename_buffer, "my_file.rs");
    assert_eq!(editor.filename_cursor, "my_file.rs".len());
}

#[test]
fn test_paste_into_save_prompt() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Save);
    editor.save_filename_buffer.clear();
    editor.filename_cursor = 0;

    reduce(&mut editor, Action::InsertString("saved.txt".to_string()));

    assert_eq!(editor.save_filename_buffer, "saved.txt");
    assert_eq!(editor.filename_cursor, "saved.txt".len());
}

// ── Goto ──────────────────────────────────────────────────────────────────────

#[test]
fn test_paste_into_goto_line() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Goto);

    reduce(&mut editor, Action::InsertString("line 42\n".to_string()));

    // 数字のみがバッファに入る
    assert_eq!(editor.goto_line_buffer, "42");
}

// ── Command Palette ───────────────────────────────────────────────────────────

#[test]
fn test_paste_into_command_palette() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Command);

    reduce(&mut editor, Action::InsertString("search\n".to_string()));

    assert_eq!(editor.command_query, "search");
}

// ── DiffCommitMsg ─────────────────────────────────────────────────────────────

#[test]
fn test_paste_into_diff_commit_msg() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::DiffCommitMsg);

    reduce(
        &mut editor,
        Action::InsertString("fix: paste bug\n".to_string()),
    );

    assert_eq!(editor.commit_msg_buffer, "fix: paste bug");
}

// ── 陽性対照: View (czv) ではペーストで本文が変更されない ──────────────────────

#[test]
fn test_paste_in_view_mode_does_not_modify_buffer() {
    let mut editor = editor_with_text(&["read only text"]);
    editor.enter_mode(EditorMode::View);

    reduce(&mut editor, Action::InsertString("evil edit".to_string()));

    assert_eq!(editor.buffer.lines, vec!["read only text"]);
}
