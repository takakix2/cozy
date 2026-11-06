//! マウスホイール（`Action::Wheel`）の検体 —— argotty `#27`。
//!
//! ⭐ 約束は 2 つ:
//! ① ホイールは**表示位置**を動かす。現在行のハイライトは文書の同じ行に残り、
//!    画面から出るときだけ端で押される（vim の `Ctrl-E` / `Ctrl-Y`）。
//! ② `j`/`k` は今までどおり**ハイライト**を動かす（意味を変えていない）。
//!
//! 📌 ②は①の対照 —— 「ホイールも `j`/`k` も同じことをする」実装なら、どちらかが赤になる。
//!
//! 読むモードの **↑/↓ はホイールの側**（表示位置を動かす・less と同じ）。
//! 端末のホイールはマウス報告をやめて alternate scroll（`?1007`）で ↑/↓ として届くようにした
//! ので、↑/↓ がホイールそのもの（`event_loop::WHEEL_ON`）。キーからの道は末尾の検体。

use crate::action::Action;
use crate::reducer::reduce;
use crate::state::{EditorMode, EditorState, TextBuffer};

/// 50 行の Markdown を 7 行の画面で読んでいる。表示は 21 行目（index 20）から、
/// ハイライトは 26 行目（index 25）。
fn markdown_editor() -> EditorState {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    editor.markdown_view_height = 7;
    editor.markdown_rendered_line_count = 50;
    editor.buffer = TextBuffer::from_lines((1..=50).map(|n| n.to_string()).collect());
    editor.markdown_scroll_offset = 20;
    editor.markdown_cursor_line = 25;
    editor
}

fn position(editor: &EditorState) -> (usize, usize) {
    (editor.markdown_scroll_offset, editor.markdown_cursor_line)
}

#[test]
fn a_wheel_notch_scrolls_the_text_and_leaves_the_highlight_on_its_line() {
    let mut editor = markdown_editor();
    reduce(&mut editor, Action::Wheel(3));
    assert_eq!(
        position(&editor),
        (23, 25),
        "表示だけが 3 行進み、ハイライトは同じ行のまま"
    );
}

/// ② の対照: 同じ状態から `j`（`MoveDown`）はハイライトを動かし、表示は動かさない。
#[test]
fn j_still_moves_the_highlight_not_the_text() {
    let mut editor = markdown_editor();
    reduce(&mut editor, Action::MoveDown);
    assert_eq!(position(&editor), (20, 26));
}

#[test]
fn the_highlight_is_pushed_along_at_the_top_edge() {
    let mut editor = markdown_editor();
    reduce(&mut editor, Action::Wheel(3)); // 23..=29, 25 は見えている
    reduce(&mut editor, Action::Wheel(3)); // 26..=32, 25 は上へ出る → 26 へ
    assert_eq!(position(&editor), (26, 26));
}

#[test]
fn the_highlight_is_pushed_along_at_the_bottom_edge() {
    let mut editor = markdown_editor();
    reduce(&mut editor, Action::Wheel(-3)); // 17..=23, 25 は下へ出る → 23 へ
    assert_eq!(position(&editor), (17, 23));
}

#[test]
fn the_wheel_stops_at_both_ends_of_the_document() {
    let mut editor = markdown_editor();
    reduce(&mut editor, Action::Wheel(-100));
    assert_eq!(
        position(&editor),
        (0, 6),
        "先頭で止まり、ハイライトは下端に押される"
    );

    reduce(&mut editor, Action::Wheel(100));
    // 50 行・7 行の画面 → 最後の表示は 43..=49
    assert_eq!(position(&editor), (43, 43));
}

#[test]
fn a_wheel_notch_abandons_a_pending_g() {
    let mut editor = markdown_editor();
    editor.glide_prefix = Some('g');
    reduce(&mut editor, Action::Wheel(3));
    assert_eq!(editor.glide_prefix, None);
}

#[test]
fn the_help_screen_scrolls_the_same_way() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Help);
    editor.help_view_height = 7;
    editor.help_rendered_line_count = 50;
    editor.help_scroll_offset = 20;
    editor.help_cursor_line = 25;
    reduce(&mut editor, Action::Wheel(3));
    assert_eq!(
        (editor.help_scroll_offset, editor.help_cursor_line),
        (23, 25)
    );
}

/// `czv` の閲覧（カーソルを持たない）は、矢印と同じく表示位置を送る。
#[test]
fn the_view_mode_scrolls_its_text() {
    let mut editor = EditorState::new(None);
    editor.buffer =
        TextBuffer::from_lines((1..=50).map(|n| format!("line {n}")).collect::<Vec<_>>());
    editor.enter_mode(EditorMode::View);
    editor.read_only = true;
    reduce(&mut editor, Action::Wheel(3));
    assert_eq!(editor.scroll_offset, 3);
}

/// 編集中はカーソルを動かす（argotty のスワイプが以前から送っていた ↑/↓ と同じ）。
/// ⚠️ 端末のホイールは編集中には届かない —— 頼むのは読むモードの間だけ（`event_loop`）。
#[test]
fn the_wheel_moves_the_cursor_while_editing() {
    let mut editor = EditorState::new(None);
    editor.buffer = TextBuffer::from_lines((1..=50).map(|n| n.to_string()).collect());
    editor.enter_mode(EditorMode::Edit);
    reduce(&mut editor, Action::Wheel(3));
    assert_eq!(editor.cursor.y, 3);
    reduce(&mut editor, Action::Wheel(-2));
    assert_eq!(editor.cursor.y, 1);
    assert!(!editor.modified);
}

/// 他のモード（ここでは保存の欄）では何もしない。
#[test]
fn the_wheel_does_nothing_in_a_prompt() {
    let mut editor = EditorState::new(None);
    editor.buffer = TextBuffer::from_lines((1..=50).map(|n| n.to_string()).collect());
    editor.enter_mode(EditorMode::Save);
    reduce(&mut editor, Action::Wheel(3));
    assert_eq!((editor.cursor.y, editor.scroll_offset), (0, 0));
    assert_eq!(editor.mode, EditorMode::Save);
}

fn key(editor: &EditorState, code: crate::state::key::KeyCode) -> Option<Action> {
    crate::ui::Keymap::map_key_to_action(editor, code, crate::state::key::KeyModifiers::NONE)
}

/// 読むモードの ↑/↓ はホイールと同じ（表示位置を動かす）—— 端末のホイールは
/// `?1007` で ↑/↓ として届くので、ここが赤ならホイールでスクロールできない。
#[test]
fn the_arrows_scroll_every_reading_view() {
    use crate::state::key::KeyCode;
    for mode in [EditorMode::Markdown, EditorMode::Help, EditorMode::View] {
        let mut editor = EditorState::new(None);
        editor.enter_mode(mode);
        assert_eq!(
            key(&editor, KeyCode::Down),
            Some(Action::Wheel(1)),
            "{mode:?} の ↓"
        );
        assert_eq!(
            key(&editor, KeyCode::Up),
            Some(Action::Wheel(-1)),
            "{mode:?} の ↑"
        );
    }
}

/// 端から端まで: ↓ を押すと表示が進み、ハイライトは同じ行に残る。
#[test]
fn the_down_arrow_scrolls_the_preview() {
    let mut editor = markdown_editor();
    let down = key(&editor, crate::state::key::KeyCode::Down).unwrap();
    reduce(&mut editor, down);
    assert_eq!(position(&editor), (21, 25));
}

/// 対照: `j`/`k` と、編集中の ↑/↓ は今までどおり。
#[test]
fn j_and_k_and_the_editing_arrows_are_unchanged() {
    use crate::state::key::KeyCode;
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    assert_eq!(key(&editor, KeyCode::Char('j')), Some(Action::MoveDown));
    assert_eq!(key(&editor, KeyCode::Char('k')), Some(Action::MoveUp));
    editor.enter_mode(EditorMode::Edit);
    assert_eq!(key(&editor, KeyCode::Down), Some(Action::MoveDown));
    assert_eq!(key(&editor, KeyCode::Up), Some(Action::MoveUp));
}

/// `i` は Markdown プレビューでだけ「URL の画像を取る」。czv の生表示・Help では何もしない。
#[test]
fn i_fetches_images_only_in_the_preview() {
    use crate::state::key::KeyCode;
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    assert_eq!(
        key(&editor, KeyCode::Char('i')),
        Some(Action::FetchRemoteImages)
    );
    for mode in [EditorMode::Help, EditorMode::View] {
        editor.enter_mode(mode);
        assert_ne!(
            key(&editor, KeyCode::Char('i')),
            Some(Action::FetchRemoteImages),
            "{mode:?}"
        );
    }
}

/// 端から端まで: プレビューで `i` を押すと、数えてあった URL を本物の取得係に渡す。
/// ⚠️ 外へ出ないよう相手は `127.0.0.1:9`（discard・つながらない）—— 取れなくても
/// 「頼んだ」ことは表に残る（curl が無い台でも `curl not found` で残る）。
#[cfg(feature = "remote-images")]
#[test]
fn i_in_the_preview_asks_for_the_counted_urls() {
    let url = "http://127.0.0.1:9/cozy-wheel-test.png".to_string();
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    editor.markdown_remote_images = vec![url.clone()];
    reduce(&mut editor, Action::FetchRemoteImages);
    assert!(
        crate::imageview::fetch::status(&url).is_some(),
        "頼んでいない"
    );
    assert_eq!(editor.status_message.as_deref(), Some("Images: fetching 1"));
}

/// 対照: プレビュー以外では、URL が数えてあっても取りに行かない。
#[cfg(feature = "remote-images")]
#[test]
fn outside_the_preview_nothing_is_fetched() {
    let url = "http://127.0.0.1:9/cozy-wheel-test-help.png".to_string();
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Help);
    editor.markdown_remote_images = vec![url.clone()];
    reduce(&mut editor, Action::FetchRemoteImages);
    assert_eq!(crate::imageview::fetch::status(&url), None);
}
