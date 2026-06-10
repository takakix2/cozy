use crate::state::{EditorState, EditorMode, TextBuffer, YankHighlight};
use crate::action::Action;
use crate::reducer::editor::apply_editor_event;
use crate::reducer::reduce;

#[test]
fn test_edit_mode_navigation_unaffected() {
    let mut editor = EditorState::new(None);
    // Default resting mode is Edit (new(None) starts at Welcome; force Edit + content)
    editor.enter_mode(EditorMode::Edit);
    editor.buffer = TextBuffer::from_lines(vec!["main 1".to_string(), "main 2".to_string()]);
    
    apply_editor_event(&mut editor, &Action::MoveDown);
    assert_eq!(editor.cursor.y, 1);
}

#[test]
fn test_toggle_case_with_count() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["Hello".to_string()]);
    editor.cursor.x = 0;
    // `3~` toggles the first three chars and advances the cursor.
    editor.glide_count.push('3');
    apply_editor_event(&mut editor, &Action::ToggleCase);
    assert_eq!(editor.buffer.lines[0], "hELlo");
    assert_eq!(editor.cursor.x, 3);
    assert!(editor.modified);
}

#[test]
fn test_toggle_case_is_undoable() {
    // Regression: ~ must snapshot before mutating, or undo restores the
    // already-toggled buffer (a no-op) instead of the original.
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["Hello".to_string()]);
    editor.cursor.x = 0;
    editor.glide_count.push('3');
    reduce(&mut editor, Action::ToggleCase);
    assert_eq!(editor.buffer.lines[0], "hELlo");
    reduce(&mut editor, Action::Undo);
    assert_eq!(editor.buffer.lines[0], "Hello"); // fully reverted in one step
}

#[test]
fn test_markdown_preview_toggles_back_to_home_mode() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Edit);

    reduce(&mut editor, Action::ToggleMarkdownPreview);
    assert_eq!(editor.mode, EditorMode::Markdown);

    reduce(&mut editor, Action::ToggleMarkdownPreview);
    assert_eq!(editor.mode, EditorMode::Edit);
}

#[test]
fn test_markdown_preview_scrolls_without_moving_cursor() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    editor.buffer = TextBuffer::from_lines(vec![
        "# Title".to_string(),
        "first".to_string(),
        "second".to_string(),
    ]);

    reduce(&mut editor, Action::MoveDown);
    assert_eq!(editor.markdown_cursor_line, 1);
    assert_eq!((editor.cursor.y, editor.cursor.x), (0, 0));

    reduce(&mut editor, Action::MoveUp);
    assert_eq!(editor.markdown_cursor_line, 0);
    assert_eq!((editor.cursor.y, editor.cursor.x), (0, 0));
}

#[test]
fn test_markdown_preview_screen_motions_move_highlight() {
    use crate::state::key::{KeyCode, KeyModifiers};
    use crate::ui::Keymap;

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    editor.page_size = 10;
    editor.markdown_view_height = 7;
    editor.markdown_scroll_offset = 20;
    editor.buffer = TextBuffer::from_lines((1..=50).map(|n| n.to_string()).collect());

    let middle = Keymap::map_key_to_action(&editor, KeyCode::Char('M'), KeyModifiers::NONE).unwrap();
    reduce(&mut editor, middle);
    assert_eq!(editor.markdown_cursor_line, 23);

    let bottom = Keymap::map_key_to_action(&editor, KeyCode::Char('L'), KeyModifiers::NONE).unwrap();
    reduce(&mut editor, bottom);
    assert_eq!(editor.markdown_cursor_line, 26);

    let top = Keymap::map_key_to_action(&editor, KeyCode::Char('H'), KeyModifiers::NONE).unwrap();
    reduce(&mut editor, top);
    assert_eq!(editor.markdown_cursor_line, 20);

    let lower_middle = Keymap::map_key_to_action(&editor, KeyCode::Char('m'), KeyModifiers::NONE).unwrap();
    reduce(&mut editor, lower_middle);
    assert_eq!(editor.markdown_cursor_line, 23);
}

#[test]
fn test_markdown_preview_page_keys_use_visible_height() {
    use crate::state::key::{KeyCode, KeyModifiers};
    use crate::ui::Keymap;

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    editor.page_size = 20;
    editor.markdown_view_height = 7;
    editor.buffer = TextBuffer::from_lines((1..=50).map(|n| n.to_string()).collect());

    let page_down = Keymap::map_key_to_action(&editor, KeyCode::PageDown, KeyModifiers::NONE).unwrap();
    reduce(&mut editor, page_down);
    assert_eq!(editor.markdown_cursor_line, 7);

    let page_up = Keymap::map_key_to_action(&editor, KeyCode::PageUp, KeyModifiers::NONE).unwrap();
    reduce(&mut editor, page_up);
    assert_eq!(editor.markdown_cursor_line, 0);
}

#[test]
fn test_markdown_preview_counted_line_jumps() {
    use crate::state::key::{KeyCode, KeyModifiers};
    use crate::ui::Keymap;

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    editor.buffer = TextBuffer::from_lines((1..=20).map(|n| n.to_string()).collect());

    for code in [KeyCode::Char('5'), KeyCode::Char('g')] {
        let action = Keymap::map_key_to_action(&editor, code, KeyModifiers::NONE).unwrap();
        reduce(&mut editor, action);
    }
    assert_eq!(editor.glide_count, "5");
    assert_eq!(editor.glide_prefix, Some('g'));

    let second_g = Keymap::map_key_to_action(&editor, KeyCode::Char('g'), KeyModifiers::NONE).unwrap();
    reduce(&mut editor, second_g);
    assert_eq!(editor.markdown_cursor_line, 4);
    assert!(editor.glide_count.is_empty());
    assert_eq!(editor.glide_prefix, None);

    for code in [KeyCode::Char('1'), KeyCode::Char('2'), KeyCode::Char('G')] {
        let action = Keymap::map_key_to_action(&editor, code, KeyModifiers::NONE).unwrap();
        reduce(&mut editor, action);
    }
    assert_eq!(editor.markdown_cursor_line, 11);
    assert!(editor.glide_count.is_empty());
}

#[test]
fn test_markdown_preview_counted_vertical_move() {
    use crate::state::key::{KeyCode, KeyModifiers};
    use crate::ui::Keymap;

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Markdown);
    editor.buffer = TextBuffer::from_lines((1..=20).map(|n| n.to_string()).collect());

    for code in [KeyCode::Char('5'), KeyCode::Char('j')] {
        let action = Keymap::map_key_to_action(&editor, code, KeyModifiers::NONE).unwrap();
        reduce(&mut editor, action);
    }
    assert_eq!(editor.markdown_cursor_line, 5);
    assert!(editor.glide_count.is_empty());
}

#[test]
fn test_glide_page_keys_move_by_page_size() {
    use crate::state::key::{KeyCode, KeyModifiers};
    use crate::ui::Keymap;

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Glide);
    editor.page_size = 5;
    editor.buffer = TextBuffer::from_lines((1..=20).map(|n| n.to_string()).collect());

    let page_down = Keymap::map_key_to_action(&editor, KeyCode::PageDown, KeyModifiers::NONE).unwrap();
    reduce(&mut editor, page_down);
    assert_eq!(editor.cursor.y, 5);

    let page_up = Keymap::map_key_to_action(&editor, KeyCode::PageUp, KeyModifiers::NONE).unwrap();
    reduce(&mut editor, page_up);
    assert_eq!(editor.cursor.y, 0);
}

#[test]
fn test_glide_counted_gg_jumps_to_line_from_key_sequence() {
    use crate::state::key::{KeyCode, KeyModifiers};
    use crate::ui::Keymap;

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec![
        "1".to_string(),
        "2".to_string(),
        "3".to_string(),
        "4".to_string(),
        "5".to_string(),
        "6".to_string(),
    ]);

    for code in [KeyCode::Char('5'), KeyCode::Char('g'), KeyCode::Char('g')] {
        let action = Keymap::map_key_to_action(&editor, code, KeyModifiers::NONE).unwrap();
        reduce(&mut editor, action);
    }

    assert_eq!((editor.cursor.y, editor.cursor.x), (4, 0));
    assert!(editor.glide_count.is_empty());
    assert_eq!(editor.glide_prefix, None);
}

#[test]
fn test_dot_comma_repeat_last_find() {
    use crate::ui::Keymap;
    use crate::state::key::{KeyCode, KeyModifiers};
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["axbxcx".to_string()]); // x at 1,3,5
    editor.cursor.x = 0;
    // `>x`: jump onto the first 'x' (index 1), recording last_find.
    reduce(&mut editor, Action::GlideMove(crate::glide::Motion::FindChar('x')));
    assert_eq!(editor.cursor.x, 1);
    // `.` repeats forward -> next 'x' at index 3.
    let dot = Keymap::map_key_to_action(&editor, KeyCode::Char('.'), KeyModifiers::NONE).unwrap();
    reduce(&mut editor, dot);
    assert_eq!(editor.cursor.x, 3);
    // `,` repeats backward -> previous 'x' at index 1.
    let comma = Keymap::map_key_to_action(&editor, KeyCode::Char(','), KeyModifiers::NONE).unwrap();
    reduce(&mut editor, comma);
    assert_eq!(editor.cursor.x, 1);
}

#[test]
fn test_bare_till_jump_moves_cursor() {
    // Bare `t)` (no operator pending) moves the cursor to just before ')'.
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["hello)world".to_string()]);
    editor.cursor.x = 0;
    reduce(&mut editor, Action::GlideMove(crate::glide::Motion::TillChar(')')));
    assert_eq!(editor.cursor.x, 4); // one char before ')'
}

#[test]
fn test_glide_backspace_deletes_char_before_cursor() {
    // Regression: Glide `X` (Action::Backspace) must route through reduce() to
    // handle_backspace, not fall into the no-op default arm.
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["abc".to_string()]);
    editor.cursor.x = 2; // before 'c'
    reduce(&mut editor, Action::Backspace);
    assert_eq!(editor.buffer.lines[0], "ac"); // 'b' removed
    assert_eq!(editor.cursor.x, 1);
}

#[test]
fn test_save_bare_filename_uses_current_dir() {
    // Regression: a filename without a directory component has parent() == Some(""),
    // and `"".exists()` is false. That empty parent means the current directory and
    // must NOT be rejected as "Directory not found" — otherwise every bare-name save fails.
    use std::io::Read;
    let dir = std::env::temp_dir().join(format!("cozy_save_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Edit);
    editor.buffer = TextBuffer::from_lines(vec!["hello".to_string()]);
    editor.filename = Some(std::path::PathBuf::from("bare.txt"));

    let result = editor.save();

    // Restore CWD before asserting so a failure can't leave the test process in temp.
    std::env::set_current_dir(&prev).unwrap();

    assert!(result.is_ok(), "bare filename save must succeed, got: {:?}", result.err());
    let mut contents = String::new();
    std::fs::File::open(dir.join("bare.txt")).unwrap()
        .read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "hello\n");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_save_success_status_survives_into_edit_mode() {
    // Regression: enter_mode(Edit) clears status_message. Save set "Saved" BEFORE the
    // mode switch, so it was wiped instantly and the user saw no confirmation. The
    // success message must be set AFTER entering Edit so it persists.
    // Uses an absolute path (no CWD change) so it can't race the bare-filename test.
    let dir = std::env::temp_dir().join(format!("cozy_save_status_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("note.txt");

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Edit);
    editor.buffer = TextBuffer::from_lines(vec!["x".to_string()]);
    editor.filename = Some(path.clone());

    reduce(&mut editor, Action::Save(path.to_string_lossy().to_string()));

    assert_eq!(editor.mode, EditorMode::Edit);
    assert!(
        editor.status_message.as_deref().unwrap_or("").contains("Saved"),
        "expected a 'Saved' status after save, got: {:?}", editor.status_message
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_save_and_exit_saves_then_exits() {
    // Regression: Ctrl+X "Save and Exit" mapped Enter to Action::Quit, which exited
    // WITHOUT saving (data loss). SaveAndExit must write the file, then exit.
    use std::io::Read;
    let dir = std::env::temp_dir().join(format!("cozy_save_exit_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bye.txt");

    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Quit);
    editor.buffer = TextBuffer::from_lines(vec!["data".to_string()]);
    editor.filename = Some(path.clone());

    let result = reduce(&mut editor, Action::SaveAndExit(path.to_string_lossy().to_string()));

    assert!(matches!(result, crate::reducer::EventResult::Exit), "must exit on successful save");
    let mut contents = String::new();
    std::fs::File::open(&path).unwrap().read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "data\n");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_save_and_exit_stays_on_failure() {
    // An empty filename with no current file can't be saved → must NOT exit (avoid
    // data loss), so the user can fix the name or discard explicitly with Ctrl+Q.
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Quit);
    editor.buffer = TextBuffer::from_lines(vec!["data".to_string()]);
    editor.filename = None;

    let result = reduce(&mut editor, Action::SaveAndExit(String::new()));

    assert!(matches!(result, crate::reducer::EventResult::Continue), "must not exit when save fails");
}

#[test]
fn test_yank_highlight_cleared_on_next_keypress() {
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["abc".to_string()]);
    // Simulate a yank having armed the flash.
    editor.yank_highlight = Some(YankHighlight { start: (0, 0), end: (0, 3), linewise: false });
    // Any subsequent action goes through reduce(), which clears the flash.
    reduce(&mut editor, Action::MoveRight);
    assert!(editor.yank_highlight.is_none());
}

// --- Browse mode (folder tree) -------------------------------------------------

/// Build a throwaway directory tree for Browse tests. `name` must be unique per
/// test — tests run in parallel and would otherwise wipe each other's scratch dir.
fn browse_scratch(name: &str) -> std::path::PathBuf {
    use std::fs;
    let base = std::env::temp_dir().join(format!("cozy_browse_reducer_{}_{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(base.join("src")).unwrap();
    fs::write(base.join("README.md"), "readme").unwrap();
    fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();
    base
}

#[test]
fn test_cozy_dir_arg_opens_browse_not_edit() {
    let base = browse_scratch("dir_arg");
    let editor = EditorState::new(Some(base.to_string_lossy().to_string()));
    assert_eq!(editor.mode, EditorMode::Browse, "a directory arg must open Browse");
    assert!(editor.filename.is_none(), "directory must not become the edit filename");
    assert!(editor.browse_tree.is_some(), "tree must be built on launch");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_cozy_file_arg_still_opens_edit() {
    let base = browse_scratch("file_arg");
    let file = base.join("README.md");
    let editor = EditorState::new(Some(file.to_string_lossy().to_string()));
    assert_eq!(editor.mode, EditorMode::Edit);
    assert!(editor.filename.is_some());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_browse_open_file_enters_edit() {
    let base = browse_scratch("open_file");
    let mut editor = EditorState::new(Some(base.to_string_lossy().to_string()));
    let main_path = base.join("src/main.rs");
    // Preselect the file, then "open" it via the reducer.
    editor.browse_tree.as_mut().unwrap().select_path(&main_path);
    reduce(&mut editor, Action::BrowseExpandOrOpen);
    assert_eq!(editor.mode, EditorMode::Edit);
    assert_eq!(editor.filename.as_ref().unwrap(), &main_path);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_browse_filter_round_trip() {
    let base = browse_scratch("filter");
    let mut editor = EditorState::new(Some(base.to_string_lossy().to_string()));
    reduce(&mut editor, Action::BrowseStartFilter);
    reduce(&mut editor, Action::BrowseFilterChar('m'));
    reduce(&mut editor, Action::BrowseFilterChar('a'));
    let tree = editor.browse_tree.as_ref().unwrap();
    assert!(tree.filtering);
    assert_eq!(tree.filter, "ma");
    // main.rs matches; README.md does not.
    let names: Vec<&str> = tree.visible_nodes().iter().map(|&i| tree.nodes[i].name.as_str()).collect();
    assert!(names.contains(&"main.rs"));
    assert!(!names.contains(&"README.md"));
    // Esc clears the filter but stays in Browse.
    reduce(&mut editor, Action::Cancel);
    assert_eq!(editor.mode, EditorMode::Browse);
    assert!(!editor.browse_tree.as_ref().unwrap().filtering);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_save_prompt_defaults_to_untitled_for_new_buffer() {
    let dir = std::env::temp_dir().join(format!("cozy_untitled_empty_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut editor = EditorState::new(None);
    editor._working_dir = dir.clone();
    editor.enter_mode(EditorMode::Save);
    assert_eq!(editor.save_filename_buffer, "untitled.txt");
    assert_eq!(editor.filename_cursor, "untitled.txt".len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_save_default_name_counts_up_on_collision() {
    let dir = std::env::temp_dir().join(format!("cozy_untitled_collide_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("untitled.txt"), "x").unwrap();

    let mut editor = EditorState::new(None);
    editor._working_dir = dir.clone();
    editor.enter_mode(EditorMode::Save);
    assert_eq!(editor.save_filename_buffer, "untitled (1).txt");

    std::fs::write(dir.join("untitled (1).txt"), "y").unwrap();
    editor.enter_mode(EditorMode::Save);
    assert_eq!(editor.save_filename_buffer, "untitled (2).txt");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_save_resolves_relative_name_against_working_dir() {
    // A relative filename writes into the anchored working dir, not the process
    // cwd — so the collision check and the write always agree (future cross-folder Browse).
    let dir = std::env::temp_dir().join(format!("cozy_anchor_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut editor = EditorState::new(None);
    editor._working_dir = dir.clone();
    editor.enter_mode(EditorMode::Edit);
    editor.buffer = TextBuffer::from_lines(vec!["note".to_string()]);
    editor.filename = Some(std::path::PathBuf::from("memo.txt"));

    assert!(editor.save().is_ok());
    assert!(dir.join("memo.txt").exists(), "relative name must land in _working_dir");

    let _ = std::fs::remove_dir_all(&dir);
}

// ---- default_mode (Edit ⇄ Glide home inversion, Lv2) ----

/// Flip the loaded config to Glide-home for the duration of a test.
fn set_glide_home(editor: &mut EditorState) {
    editor.config.default_mode = Some("glide".to_string());
}

#[test]
fn test_home_mode_resolves_from_config() {
    let mut editor = EditorState::new(None);
    editor.config.default_mode = Some("edit".to_string());
    assert_eq!(editor.home_mode(), EditorMode::Edit);
    editor.config.default_mode = Some("glide".to_string());
    assert_eq!(editor.home_mode(), EditorMode::Glide);
    editor.config.default_mode = Some("nonsense".to_string());
    assert_eq!(editor.home_mode(), EditorMode::Edit, "unknown value falls back to Edit");
    editor.config.default_mode = None;
    assert_eq!(editor.home_mode(), EditorMode::Edit, "missing value defaults to Edit");
}

#[test]
fn test_edit_home_returns_to_edit() {
    // Default (config.toml ships default_mode="edit"): resting points are Edit.
    let mut editor = EditorState::new(None);
    editor.enter_mode(EditorMode::Edit);
    editor.buffer = TextBuffer::from_lines(vec!["a".to_string(), "b".to_string()]);
    reduce(&mut editor, Action::GotoLine(2));
    assert_eq!(editor.mode, EditorMode::Edit);
    editor.enter_mode(EditorMode::Search);
    reduce(&mut editor, Action::Cancel);
    assert_eq!(editor.mode, EditorMode::Edit);
}

#[test]
fn test_glide_home_cancel_returns_to_glide() {
    let mut editor = EditorState::new(None);
    set_glide_home(&mut editor);
    editor.enter_mode(EditorMode::Search);
    reduce(&mut editor, Action::Cancel);
    assert_eq!(editor.mode, EditorMode::Glide, "Esc lands in Glide when it is home");
}

#[test]
fn test_glide_home_gotoline_returns_to_glide() {
    let mut editor = EditorState::new(None);
    set_glide_home(&mut editor);
    editor.enter_mode(EditorMode::Edit);
    editor.buffer = TextBuffer::from_lines(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    reduce(&mut editor, Action::GotoLine(3));
    assert_eq!(editor.mode, EditorMode::Glide);
}

#[test]
fn test_glide_home_browse_open_enters_glide() {
    let base = browse_scratch("glide_open");
    let mut editor = EditorState::new(Some(base.to_string_lossy().to_string()));
    set_glide_home(&mut editor);
    let main_path = base.join("src/main.rs");
    editor.browse_tree.as_mut().unwrap().select_path(&main_path);
    reduce(&mut editor, Action::BrowseExpandOrOpen);
    assert_eq!(editor.mode, EditorMode::Glide, "opening a file rests in Glide when it is home");
    assert_eq!(editor.filename.as_ref().unwrap(), &main_path);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_glide_insert_verbs_stay_edit_under_glide_home() {
    // `i`/`a`/`o` exist to ENTER Edit; they must ignore default_mode.
    let mut editor = EditorState::new(None);
    set_glide_home(&mut editor);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["hello".to_string()]);
    reduce(&mut editor, Action::GlideInsert);
    assert_eq!(editor.mode, EditorMode::Edit, "i must enter Edit even with Glide home");
}

#[test]
fn test_change_stays_edit_under_glide_home() {
    // `cc` deletes the line then drops into insert — always Edit, even Glide-home.
    use crate::glide::{Operator, Motion};
    let mut editor = EditorState::new(None);
    set_glide_home(&mut editor);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["hello".to_string()]);
    crate::reducer::operator::apply_operator(&mut editor, Operator::Change, Motion::CurrentLine, None);
    assert_eq!(editor.mode, EditorMode::Edit, "change must enter Edit even with Glide home");
}

#[test]
fn test_glide_home_esc_round_trip() {
    // The vim round-trip: Glide → i → Edit → Esc → Glide.
    let mut editor = EditorState::new(None);
    set_glide_home(&mut editor);
    editor.enter_mode(EditorMode::Glide);
    editor.buffer = TextBuffer::from_lines(vec!["x".to_string()]);
    reduce(&mut editor, Action::GlideInsert);
    assert_eq!(editor.mode, EditorMode::Edit);
    reduce(&mut editor, Action::Cancel);
    assert_eq!(editor.mode, EditorMode::Glide);
}

#[test]
fn test_glide_home_startup_from_file_arg() {
    // `cozy <file>` with default_mode="glide" in the config dir starts in Glide.
    let base = browse_scratch("glide_startup");
    std::fs::write(base.join("config.toml"), "page_size = 20\ndefault_mode = \"glide\"\n").unwrap();
    let file = base.join("README.md");
    let editor = EditorState::new_with_config_dir(
        Some(file.to_string_lossy().to_string()),
        Some(&base),
    );
    assert_eq!(editor.mode, EditorMode::Glide, "file arg rests in Glide when it is home");
    assert!(editor.filename.is_some());
    let _ = std::fs::remove_dir_all(&base);
}
