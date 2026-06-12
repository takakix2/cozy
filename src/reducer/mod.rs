pub mod editor;
pub mod buffer;
pub mod insert;
pub mod delete;
pub mod search;
pub mod replace;
pub mod helper;
pub mod cursor;
pub mod clipboard;
pub mod operator;
pub mod file;
pub mod status;
pub mod browse;
pub mod command;

#[cfg(test)]
mod editor_test;

use crate::state::EditorState;
use crate::action::Action;

pub enum EventResult {
    Continue,
    Exit,
}

fn markdown_line_count(editor: &EditorState) -> usize {
    editor.buffer.lines.len().max(1)
}

fn markdown_page_step(editor: &EditorState) -> usize {
    if editor.markdown_view_height == 0 {
        editor.page_size
    } else {
        editor.markdown_view_height as usize
    }.max(1)
}

fn set_markdown_cursor(editor: &mut EditorState, line: usize) {
    let last = markdown_line_count(editor).saturating_sub(1);
    let y = line.min(last);
    editor.markdown_cursor_line = y as u16;

    let top = editor.markdown_scroll_offset as usize;
    let page = markdown_page_step(editor);
    if y < top {
        editor.markdown_scroll_offset = y as u16;
    } else if y >= top.saturating_add(page) {
        editor.markdown_scroll_offset = y.saturating_sub(page - 1) as u16;
    }
}

fn move_markdown_cursor(editor: &mut EditorState, delta: isize) {
    let current = editor.markdown_cursor_line as usize;
    let next = if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize)
    };
    set_markdown_cursor(editor, next);
}

fn take_markdown_count_opt(editor: &mut EditorState) -> Option<usize> {
    let n = if editor.glide_count.is_empty() {
        None
    } else {
        editor.glide_count.parse::<usize>().ok().filter(|&n| n >= 1)
    };
    editor.glide_count.clear();
    n
}

fn markdown_screen_motion(editor: &mut EditorState, motion: crate::glide::Motion) -> EventResult {
    editor.glide_prefix = None;
    let top = editor.markdown_scroll_offset as usize;
    let page = markdown_page_step(editor);
    let count = take_markdown_count_opt(editor);
    let last = markdown_line_count(editor).saturating_sub(1);
    let line = match motion {
        crate::glide::Motion::FileTop => match count {
            Some(line) => line.saturating_sub(1).min(last),
            None => 0,
        },
        crate::glide::Motion::FileBottom => match count {
            Some(line) => line.saturating_sub(1).min(last),
            None => last,
        },
        crate::glide::Motion::ScreenTop => top,
        crate::glide::Motion::ScreenMiddle => top.saturating_add(page / 2),
        crate::glide::Motion::ScreenBottom => top.saturating_add(page.saturating_sub(1)),
        _ => return editor::apply_editor_event(editor, &Action::GlideMove(motion)),
    };
    set_markdown_cursor(editor, line);
    EventResult::Continue
}

pub fn reduce(editor: &mut EditorState, action: Action) -> EventResult {
    // The yank flash lasts exactly until the next keypress: clear it here, before
    // dispatch, so a fresh yank in this same call can re-arm it for one frame.
    editor.yank_highlight = None;

    // Dispatch to buffer reducer for editing actions
    match action {
        Action::InsertChar(c) => {
            match editor.mode {
                crate::state::EditorMode::Edit => insert::handle_insert_char(editor, c),
                crate::state::EditorMode::Search => {
                    search::update_search_buffer(editor, c);
                    EventResult::Continue
                }
                crate::state::EditorMode::Replace => {
                    replace::update_replace_buffer(editor, c);
                    EventResult::Continue
                }
                crate::state::EditorMode::Save | crate::state::EditorMode::Open | crate::state::EditorMode::Quit => {
                    file::update_filename_buffer(editor, c)
                }
                crate::state::EditorMode::Goto => {
                    if c.is_ascii_digit() {
                        editor.goto_line_buffer.push(c);
                    }
                    EventResult::Continue
                }
                _ => EventResult::Continue,
            }
        }
        Action::InsertString(s) => clipboard::paste_string(editor, &s),
        Action::Enter => {
            // Enter is special, handled in both but depends on mode
            // insert reducer handles Insert/Edit mode Enter
            // editor reducer handles others
            match editor.mode {
                crate::state::EditorMode::Edit => {
                    insert::handle_enter(editor)
                }
                crate::state::EditorMode::Search => {
                    search::apply_search_next(editor)
                }
                crate::state::EditorMode::Replace => {
                    replace::apply_replace_current(editor)
                }
                _ => {
                    editor::apply_editor_event(editor, &action)
                }
            }
        }
        Action::Backspace => {
            match editor.mode {
                // Glide `X` deletes the char before the cursor (mirror of `x`).
                crate::state::EditorMode::Edit | crate::state::EditorMode::Glide => delete::handle_backspace(editor),
                crate::state::EditorMode::Search => {
                    search::delete_from_search_buffer(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Replace => {
                    replace::delete_from_replace_buffer(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Save | crate::state::EditorMode::Open | crate::state::EditorMode::Quit => {
                    file::delete_from_filename_buffer(editor)
                }
                crate::state::EditorMode::Goto => {
                    editor.goto_line_buffer.pop();
                    EventResult::Continue
                }
                _ => EventResult::Continue,
            }
        }
        Action::Delete => {
            match editor.mode {
                crate::state::EditorMode::Save | crate::state::EditorMode::Open | crate::state::EditorMode::Quit => {
                    file::delete_char_at_cursor(editor)
                }
                crate::state::EditorMode::Search => {
                    search::delete_search_char_at_cursor(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Replace => {
                    replace::delete_replace_char_at_cursor(editor);
                    EventResult::Continue
                }
                _ => delete::handle_delete(editor),
            }
        }
        Action::PasteFromClipboard => clipboard::paste_from_clipboard(editor),
        Action::ReplaceCurrent => replace::apply_replace_current(editor),
        Action::ReplaceAll => replace::apply_replace_all(editor),
        Action::SearchNext => search::apply_search_next(editor),
        Action::SearchPrevious => search::apply_search_previous(editor),
        Action::ToggleSearchMode => search::apply_toggle_search_mode(editor),
        Action::SwitchFocus => replace::apply_switch_focus(editor),
        Action::MoveLeft => {
            match editor.mode {
                crate::state::EditorMode::Save | crate::state::EditorMode::Open | crate::state::EditorMode::Quit => {
                    file::move_filename_cursor_left(editor)
                }
                crate::state::EditorMode::Search => {
                    search::move_search_cursor_left(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Replace => {
                    replace::move_replace_cursor_left(editor);
                    EventResult::Continue
                }
                // ← collapses a dir / moves to the parent in the tree.
                crate::state::EditorMode::Browse => browse::collapse_or_parent(editor),
                _ => editor::apply_editor_event(editor, &action),
            }
        }
        Action::MoveRight => {
            match editor.mode {
                crate::state::EditorMode::Save | crate::state::EditorMode::Open | crate::state::EditorMode::Quit => {
                    file::move_filename_cursor_right(editor)
                }
                crate::state::EditorMode::Search => {
                    search::move_search_cursor_right(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Replace => {
                    replace::move_replace_cursor_right(editor);
                    EventResult::Continue
                }
                // → expands a dir / opens a file in the tree.
                crate::state::EditorMode::Browse => browse::expand_or_open(editor),
                _ => editor::apply_editor_event(editor, &action),
            }
        }
        Action::Home => {
            match editor.mode {
                crate::state::EditorMode::Save | crate::state::EditorMode::Open | crate::state::EditorMode::Quit => {
                    file::move_filename_cursor_home(editor)
                }
                crate::state::EditorMode::Search => {
                    search::move_search_cursor_home(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Replace => {
                    replace::move_replace_cursor_home(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Markdown => {
                    set_markdown_cursor(editor, 0);
                    EventResult::Continue
                }
                _ => editor::apply_editor_event(editor, &action),
            }
        }
        Action::End => {
            match editor.mode {
                crate::state::EditorMode::Save | crate::state::EditorMode::Open | crate::state::EditorMode::Quit => {
                    file::move_filename_cursor_end(editor)
                }
                crate::state::EditorMode::Search => {
                    search::move_search_cursor_end(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Replace => {
                    replace::move_replace_cursor_end(editor);
                    EventResult::Continue
                }
                crate::state::EditorMode::Markdown => {
                    let last = markdown_line_count(editor).saturating_sub(1);
                    set_markdown_cursor(editor, last);
                    EventResult::Continue
                }
                _ => editor::apply_editor_event(editor, &action),
            }
        }
        // Browse mode reuses MoveUp/MoveDown for cursor motion and PageTop/PageBottom
        // for gg/G; dispatch those to the tree, leaving every other mode untouched.
        Action::MoveUp => match editor.mode {
            crate::state::EditorMode::Browse => browse::move_up(editor),
            crate::state::EditorMode::Markdown => {
                let n = take_markdown_count_opt(editor).unwrap_or(1);
                move_markdown_cursor(editor, -(n as isize));
                EventResult::Continue
            }
            _ => editor::apply_editor_event(editor, &action),
        },
        Action::MoveDown => match editor.mode {
            crate::state::EditorMode::Browse => browse::move_down(editor),
            crate::state::EditorMode::Markdown => {
                let n = take_markdown_count_opt(editor).unwrap_or(1);
                move_markdown_cursor(editor, n as isize);
                EventResult::Continue
            }
            _ => editor::apply_editor_event(editor, &action),
        },
        Action::PageUp => match editor.mode {
            crate::state::EditorMode::Markdown => {
                let n = take_markdown_count_opt(editor).unwrap_or(1);
                move_markdown_cursor(editor, -((markdown_page_step(editor) * n) as isize));
                EventResult::Continue
            }
            _ => editor::apply_editor_event(editor, &action),
        },
        Action::PageDown => match editor.mode {
            crate::state::EditorMode::Markdown => {
                let n = take_markdown_count_opt(editor).unwrap_or(1);
                move_markdown_cursor(editor, (markdown_page_step(editor) * n) as isize);
                EventResult::Continue
            }
            _ => editor::apply_editor_event(editor, &action),
        },
        Action::PageTop => match editor.mode {
            crate::state::EditorMode::Browse => browse::goto_top(editor),
            crate::state::EditorMode::Markdown => {
                set_markdown_cursor(editor, 0);
                EventResult::Continue
            }
            _ => editor::apply_editor_event(editor, &action),
        },
        Action::PageBottom => match editor.mode {
            crate::state::EditorMode::Browse => browse::goto_bottom(editor),
            crate::state::EditorMode::Markdown => {
                let last = markdown_line_count(editor).saturating_sub(1);
                set_markdown_cursor(editor, last);
                EventResult::Continue
            }
            _ => editor::apply_editor_event(editor, &action),
        },
        Action::GlideMove(motion) => match editor.mode {
            crate::state::EditorMode::Markdown => markdown_screen_motion(editor, motion),
            _ => editor::apply_editor_event(editor, &action),
        },
        Action::Cancel => match editor.mode {
            crate::state::EditorMode::Browse => browse::cancel(editor),
            _ => editor::apply_editor_event(editor, &action),
        },
        Action::BrowseExpandOrOpen => browse::expand_or_open(editor),
        Action::BrowseCollapseOrParent => browse::collapse_or_parent(editor),
        Action::BrowseStartFilter => browse::start_filter(editor),
        Action::BrowseFilterChar(c) => browse::filter_char(editor, c),
        Action::BrowseFilterBackspace => browse::filter_backspace(editor),
        Action::CommandInput(c) => command::input_char(editor, c),
        Action::CommandBackspace => command::backspace(editor),
        Action::CommandMoveUp => command::move_up(editor),
        Action::CommandMoveDown => command::move_down(editor),
        Action::CommandComplete => command::complete(editor),
        Action::CommandExecute => command::execute(editor),

        _ => {
            editor::apply_editor_event(editor, &action)
        }
    }
}
