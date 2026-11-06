use crate::host_backend::HostBackend;
use crate::input::{self, EventSource, InputEvent};
use crate::reducer::{EventResult, reduce};
use crate::state::{EditorMode, EditorState};
use crate::ui::Renderer;
use ratatui::Terminal;
use std::io::{self, Write};
use std::time::Duration;

/// Alternate scroll (`1007`): the terminal turns the wheel into ↑/↓ while the
/// alternate screen is up, and the reading views scroll on ↑/↓ (`keymap`).
///
/// 🚨 **Not mouse reporting (`1000`/`1006`).** That takes the mouse away from the
/// terminal: drag-select to copy stops working unless Shift is held, and nothing
/// says so. `1007` leaves the mouse with the terminal, so selecting text just works.
const WHEEL_ON: &[u8] = b"\x1b[?1007h";
pub(crate) const WHEEL_OFF: &[u8] = b"\x1b[?1007l";

/// How long the loop waits for a key. Normally a second (the idle swap write and
/// the cursor blink run on it); while Browse is still indexing in the background,
/// a fifth of one, so the `indexing N` count and the newly found entries show up
/// without a key press (`#25`). Back to a second once the index is absorbed.
const IDLE_WAIT: Duration = Duration::from_millis(1000);
const INDEXING_WAIT: Duration = Duration::from_millis(200);

/// Markdown プレビューで、`i` で頼んだ URL の画像がまだ取れていないか。取得中も Browse の
/// 索引と同じく `INDEXING_WAIT` で起き、取れた物をキーを待たずに描く。
fn images_are_arriving(editor: &EditorState) -> bool {
    #[cfg(feature = "remote-images")]
    {
        editor.mode == EditorMode::Markdown && crate::imageview::fetch::any_pending()
    }
    #[cfg(not(feature = "remote-images"))]
    {
        let _ = editor;
        false
    }
}

/// 取得が 1 つでも終わったら進む数（feature 無しでは常に 0）。
fn fetch_generation() -> u64 {
    #[cfg(feature = "remote-images")]
    {
        crate::imageview::fetch::generation()
    }
    #[cfg(not(feature = "remote-images"))]
    {
        0
    }
}

fn browse_is_indexing(editor: &EditorState) -> bool {
    editor.mode == EditorMode::Browse
        && editor
            .browse_tree
            .as_ref()
            .is_some_and(|tree| tree.is_indexing())
}

/// The modes a wheel scrolls (↑/↓ → `Action::Wheel`). Elsewhere the terminal
/// keeps the wheel for its own scrollback. The mouse stays with the terminal
/// everywhere, so text can always be selected.
fn wants_wheel(mode: &EditorMode) -> bool {
    matches!(
        mode,
        EditorMode::Markdown | EditorMode::Help | EditorMode::View
    )
}

pub fn run<W: Write>(
    terminal: &mut Terminal<HostBackend<W>>,
    editor: &mut EditorState,
    event_src: &mut dyn EventSource,
    report_wheel: bool,
) -> io::Result<()> {
    let mut wheel_on = false;
    let result = run_loop(terminal, editor, event_src, report_wheel, &mut wheel_on);
    if wheel_on {
        let backend = terminal.backend_mut();
        let _ = backend.write_all(WHEEL_OFF);
        let _ = Write::flush(backend);
    }
    result
}

fn run_loop<W: Write>(
    terminal: &mut Terminal<HostBackend<W>>,
    editor: &mut EditorState,
    event_src: &mut dyn EventSource,
    report_wheel: bool,
    wheel_on: &mut bool,
) -> io::Result<()> {
    let mut needs_redraw = true;
    let mut seen_fetches = fetch_generation();
    loop {
        let want = report_wheel && wants_wheel(&editor.mode);
        if want != *wheel_on {
            let backend = terminal.backend_mut();
            backend.write_all(if want { WHEEL_ON } else { WHEEL_OFF })?;
            Write::flush(backend)?;
            *wheel_on = want;
        }
        if needs_redraw {
            terminal.draw(|f| {
                let chunks = Renderer::editor_layout(f.area(), editor);
                Renderer::render_body(editor, f, chunks[0]);
                Renderer::render_shortcuts(editor, f, chunks[1]);
                Renderer::render_status_bar(editor, f, chunks[2]);
            })?;
            needs_redraw = false;
        }

        // Poll with longer timeout to avoid clearing IME composition overlays.
        // crossterm receives no events during IME composition (macOS intercepts
        // keystrokes at the Cocoa layer), so longer polling means the IME inline
        // display persists undisturbed until the user confirms the composition.
        // ⚠️ Shorter only while Browse indexes (`INDEXING_WAIT`): each wake redraws,
        // which is what clears a composing IME — acceptable for the few seconds a
        // large folder takes, not as the normal cadence.
        let indexing = browse_is_indexing(editor);
        let arriving = images_are_arriving(editor);
        let wait = if indexing || arriving {
            INDEXING_WAIT
        } else {
            IDLE_WAIT
        };
        if event_src.poll(wait)? {
            match input::map_event(editor, event_src.read()?) {
                InputEvent::Action(action) => {
                    if let EventResult::Exit = reduce(editor, action) {
                        // A deliberate exit: whatever the user wanted to keep is
                        // saved, and whatever they discarded they discarded on
                        // purpose. The swap exists for the exits nobody chose.
                        crate::swap::remove(editor);
                        break;
                    }
                    if editor.modified {
                        editor.swap_dirty = true;
                        editor.swap_edits += 1;
                        // Do not let a long uninterrupted burst of typing outrun
                        // the journal — the idle tick below never fires while the
                        // keys keep coming.
                        if editor.swap_edits >= crate::swap::EDITS_PER_WRITE {
                            crate::swap::write(editor);
                        }
                    }
                    editor.cursor_blink = true;
                    // MVP: any action may have mutated the buffer; reparse on the
                    // next render. Buffer mutations are not funneled through one
                    // method, so a precise dirty signal (revision / InputEdit) is
                    // a follow-up — see cozy-notes treesitter-integration.md.
                    editor.highlighter.mark_dirty();
                    needs_redraw = true;
                }
                InputEvent::Resize(cols, rows) => {
                    // The size first: `resize` asks the backend for it while clearing,
                    // and an embedding host's backend answers with what it was told.
                    terminal.backend_mut().note_size(cols, rows);
                    let _ = terminal.resize(ratatui::layout::Rect::new(0, 0, cols, rows));
                    needs_redraw = true;
                }
                InputEvent::Flush => {
                    // The one second of typing the idle tick has not written yet
                    // is exactly what a kill would take. Write it while we still
                    // hold the thread.
                    if editor.swap_dirty {
                        crate::swap::write(editor);
                    }
                }
                InputEvent::Ignore => {}
            }
        } else {
            // No key for a second: the user stopped typing, so write the swap
            // now. This is the whole cadence — an iOS kill is announced to
            // nobody, so "on the way out" is not a moment we are given.
            if editor.swap_dirty {
                crate::swap::write(editor);
            }

            // An image asked for with `i` arrived: draw it without waiting for a key.
            let fetches = fetch_generation();
            if fetches != seen_fetches {
                seen_fetches = fetches;
                needs_redraw = true;
            }
            if indexing {
                // Take in what the background index found and show the new count —
                // including the last time, which removes `indexing` from the footer.
                // The blink waits: it would toggle five times a second here.
                if let Some(tree) = editor.browse_tree.as_mut() {
                    tree.tick();
                }
                needs_redraw = true;
            } else if editor.config.cursor_blink.unwrap_or(false) {
                // Timeout: toggle cursor blink only if enabled
                editor.cursor_blink = !editor.cursor_blink;
                needs_redraw = true;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::Event;
    use ratatui::layout::Rect;
    use ratatui::{TerminalOptions, Viewport};

    /// 1 回目の poll で失敗する入力 —— ループは途中で `Err` で抜ける。
    /// ⭐ 正常終了ではなく失敗の道で測る: `?` で抜けても解除が出ることも同時に見られる。
    struct Fails;
    impl EventSource for Fails {
        fn poll(&mut self, _: Duration) -> io::Result<bool> {
            Err(io::Error::other("stop"))
        }
        fn read(&mut self) -> io::Result<Event> {
            unreachable!()
        }
    }

    fn bytes_written(mode: EditorMode, report_wheel: bool) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let backend = HostBackend::new(&mut out, Some((40, 10)));
            let mut terminal = Terminal::with_options(
                backend,
                TerminalOptions {
                    viewport: Viewport::Fixed(Rect::new(0, 0, 40, 10)),
                },
            )
            .unwrap();
            let mut editor = EditorState::new(None);
            editor.enter_mode(mode);
            let _ = run(&mut terminal, &mut editor, &mut Fails, report_wheel);
        }
        out
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn a_reading_mode_asks_for_the_wheel_and_gives_it_back() {
        let out = bytes_written(EditorMode::Help, true);
        let on = find(&out, WHEEL_ON).expect("読むモードでホイールを頼んでいない");
        let off = find(&out, WHEEL_OFF).expect("抜けるときに解除していない");
        assert!(on < off);
    }

    /// 読むモードでもマウスは端末に残す（ドラッグで選んでコピーできるように）——
    /// マウス報告（`1000`/`1002`/`1003`/`1006`）はどのモードでも頼まない。
    #[test]
    fn no_mode_takes_the_mouse_from_the_terminal() {
        for mode in [
            EditorMode::Help,
            EditorMode::Markdown,
            EditorMode::View,
            EditorMode::Edit,
        ] {
            let out = bytes_written(mode, true);
            for report in [
                &b"\x1b[?1000h"[..],
                b"\x1b[?1002h",
                b"\x1b[?1003h",
                b"\x1b[?1006h",
            ] {
                assert_eq!(
                    find(&out, report),
                    None,
                    "{mode:?} がマウス報告 {:?} を頼んだ",
                    String::from_utf8_lossy(report)
                );
            }
        }
    }

    /// 対照: 編集のモードでは頼まない（マウスでの文字の選択を端末に残す）。
    #[test]
    fn an_editing_mode_leaves_the_mouse_to_the_terminal() {
        let out = bytes_written(EditorMode::Edit, true);
        assert_eq!(find(&out, WHEEL_ON), None);
    }

    /// Records how long each poll was asked to wait; the second poll ends the loop.
    struct Waits(Vec<Duration>);
    impl EventSource for Waits {
        fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
            self.0.push(timeout);
            if self.0.len() >= 2 {
                return Err(io::Error::other("stop"));
            }
            Ok(false)
        }
        fn read(&mut self) -> io::Result<Event> {
            unreachable!()
        }
    }

    fn waits_with_a_stalled_index(
        mode: EditorMode,
        finished: bool,
    ) -> (Vec<Duration>, EditorState) {
        let mut out = Vec::new();
        let backend = HostBackend::new(&mut out, Some((40, 10)));
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, 40, 10)),
            },
        )
        .unwrap();
        let mut editor = EditorState::new(None);
        let tree =
            crate::browse::BrowseTree::stalled(std::path::Path::new("/nonexistent-cozy-root"));
        if finished {
            tree.finish_stalled_index();
        }
        editor.browse_tree = Some(tree);
        editor.mode = mode;
        let mut src = Waits(Vec::new());
        let _ = run(&mut terminal, &mut editor, &mut src, false);
        (src.0, editor)
    }

    /// `#25`: Browse の索引中は 200 ms で起きる（キーを待たずに件数を描き直す）。
    #[test]
    fn browse_wakes_often_while_it_indexes() {
        let (waits, editor) = waits_with_a_stalled_index(EditorMode::Browse, false);
        assert_eq!(waits, vec![INDEXING_WAIT, INDEXING_WAIT]);
        assert!(editor.browse_tree.as_ref().unwrap().is_indexing());
    }

    /// 索引が完成したら、取り込んだ回を最後に 1 秒へ戻る（待機中の CPU を増やさない）。
    #[test]
    fn browse_goes_back_to_idle_once_the_index_is_absorbed() {
        let (waits, editor) = waits_with_a_stalled_index(EditorMode::Browse, true);
        assert_eq!(waits, vec![INDEXING_WAIT, IDLE_WAIT]);
        assert!(!editor.browse_tree.as_ref().unwrap().is_indexing());
    }

    /// 対照: Browse 以外では、索引が途中でも 1 秒のまま。
    #[test]
    fn other_modes_keep_the_idle_wait() {
        let (waits, _) = waits_with_a_stalled_index(EditorMode::Edit, false);
        assert_eq!(waits, vec![IDLE_WAIT, IDLE_WAIT]);
    }

    /// 対照: 埋め込み（`report_wheel = false`）では読むモードでも頼まない。
    #[test]
    fn an_embedding_host_is_never_asked() {
        let out = bytes_written(EditorMode::Help, false);
        assert_eq!(find(&out, WHEEL_ON), None);
    }
}
