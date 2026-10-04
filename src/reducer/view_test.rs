//! `View`（`czv`）の面の検体（`#14`）。
//!
//! 🚨 **この面の約束は 3 つ**で、どれも「黙って破れる」形をしている:
//! ① **編集できない**（構造で止まる ＝ 腕が編集アクションを産まない）
//! ② **出口が 3 つとも効く**（`Ctrl+Q` / `Esc` / `q`）
//! ③ **`/` で検索に入れる**
//!
//! ⭐ ①は検査ではなく**構造**で止めているので、検体は「押しても何も起きない」を見る。
//! 📌 ②③は鍵の対応表なので、**陽性対照**（別の面では違う結果になる）を必ず添える ——
//! 無いと「常に Quit を返す」実装が緑で通る。

use crate::action::Action;
use crate::reducer::{EventResult, reduce};
use crate::state::key::{KeyCode, KeyModifiers};
use crate::state::{EditorMode, EditorState, TextBuffer};
use crate::ui::keymap::Keymap;

fn view_editor() -> EditorState {
    let mut editor = EditorState::new(None);
    editor.buffer =
        TextBuffer::from_lines((1..=50).map(|n| format!("line {n}")).collect::<Vec<_>>());
    editor.enter_mode(EditorMode::View);
    editor.read_only = true;
    editor
}

fn key(editor: &EditorState, code: KeyCode) -> Option<Action> {
    Keymap::map_key_to_action(editor, code, KeyModifiers::NONE)
}

// ── ① 編集できない（構造で止まる）────────────────────────────────────────────

#[test]
fn view_produces_no_editing_action_for_the_usual_edit_keys() {
    let editor = view_editor();
    // `i`/`a`/`o` は挿入、`x` は削除、`c`/`y` はオペレータ。Glide ではどれも効く。
    // ⚠️ `d` は Glide では Delete オペレータだが、View では `#18` で less の半ページ送り
    // （`Action::HalfPageDown`）として引き受けた（下の検体で検証）。
    for code in ['i', 'a', 'o', 'x', 'c', 'y', 'p', 'D', 'J', '~'] {
        let action = key(&editor, KeyCode::Char(code));
        assert!(
            action.is_none(),
            "View で `{code}` がアクションを産んだ: {action:?} —— 編集面の鍵が漏れている"
        );
    }
}

/// 🚨 **陽性対照。** これが無いと「View では何を押しても `None`」という
/// 実装（＝移動もできない面）が上のテストを緑で通してしまう。
#[test]
fn the_same_keys_do_produce_actions_in_glide() {
    let mut editor = view_editor();
    editor.enter_mode(EditorMode::Glide);
    for code in ['i', 'x', 'd'] {
        assert!(
            key(&editor, KeyCode::Char(code)).is_some(),
            "Glide で `{code}` が効かない —— 陽性対照が壊れている"
        );
    }
}

#[test]
fn typing_a_printable_char_does_not_modify_the_buffer_in_view() {
    let mut editor = view_editor();
    let before = editor.buffer.lines.clone();
    for code in ['z', 'i', 'x'] {
        if let Some(action) = key(&editor, KeyCode::Char(code)) {
            reduce(&mut editor, action);
        }
    }
    assert_eq!(editor.buffer.lines, before, "View でバッファが変わった");
    assert!(!editor.modified, "View で modified が立った");
}

// ── ② 出口が 3 つとも効く ──────────────────────────────────────────────────

#[test]
fn q_quits_the_view() {
    let mut editor = view_editor();
    let action = key(&editor, KeyCode::Char('q')).expect("View で `q` が無反応");
    assert!(matches!(reduce(&mut editor, action), EventResult::Exit));
}

#[test]
fn esc_quits_the_view() {
    let mut editor = view_editor();
    let action = Keymap::map_key_to_action(&editor, KeyCode::Esc, KeyModifiers::NONE)
        .expect("View で `Esc` が無反応");
    assert!(matches!(reduce(&mut editor, action), EventResult::Exit));
}

#[test]
fn ctrl_q_quits_the_view() {
    let mut editor = view_editor();
    let action = Keymap::map_key_to_action(&editor, KeyCode::Char('q'), KeyModifiers::CONTROL)
        .expect("View で `Ctrl+Q` が無反応");
    assert!(matches!(reduce(&mut editor, action), EventResult::Exit));
}

/// 🚨 **陽性対照。** `q` は **View だけ**の出口。編集面で `q` が終了したら、
/// 文字が打てないどころか**書きかけが消える**。
#[test]
fn bare_q_is_not_an_exit_in_edit_or_glide() {
    let mut editor = view_editor();
    for mode in [EditorMode::Edit, EditorMode::Glide] {
        editor.enter_mode(mode);
        let action = key(&editor, KeyCode::Char('q'));
        let quits = matches!(action, Some(Action::Quit));
        assert!(!quits, "{mode:?} で裸の `q` が終了している");
    }
}

// ── ③ `/` で検索に入れる ──────────────────────────────────────────────────

#[test]
fn slash_enters_search_in_view_and_glide() {
    let mut editor = view_editor();
    for mode in [EditorMode::View, EditorMode::Glide] {
        editor.enter_mode(mode);
        assert!(
            matches!(
                key(&editor, KeyCode::Char('/')),
                Some(Action::EnterMode(EditorMode::Search))
            ),
            "{mode:?} で `/` が検索に入らない"
        );
    }
}

/// 🚨 **陽性対照その 1。** Edit モードの `/` は**文字**。ここが検索になると、
/// パスも URL も打てなくなる（大域表に入れてはいけない理由）。
#[test]
fn slash_is_a_character_in_edit_mode() {
    let mut editor = view_editor();
    editor.enter_mode(EditorMode::Edit);
    assert!(
        matches!(
            key(&editor, KeyCode::Char('/')),
            Some(Action::InsertChar('/'))
        ),
        "Edit で `/` が文字として入らない"
    );
}

/// 🚨 **陽性対照その 2。** Help / Markdown には `/` を**効かせない** ——
/// あの 2 つは専用の中身を描いているのに、検索が見るのは**バッファ**なので、
/// 画面に無い物を探し始める。
#[test]
fn slash_does_not_enter_search_in_help_or_markdown() {
    let mut editor = view_editor();
    for mode in [EditorMode::Help, EditorMode::Markdown] {
        editor.enter_mode(mode);
        assert!(
            key(&editor, KeyCode::Char('/')).is_none(),
            "{mode:?} で `/` が効いている —— 画面に無い物を探す口が開いた"
        );
    }
}

// ── 移動は効く（vim の綴りのまま）──────────────────────────────────────────

#[test]
fn vim_motions_still_work_in_view() {
    let editor = view_editor();
    for code in ['j', 'k', 'g', 'G', 'H', 'M', 'L', 'b', ' '] {
        assert!(
            key(&editor, KeyCode::Char(code)).is_some(),
            "View で移動の `{code}` が効かない"
        );
    }
}

#[test]
fn j_moves_down_in_view() {
    let mut editor = view_editor();
    let before = editor.cursor.y;
    let action = key(&editor, KeyCode::Char('j')).expect("`j` が無反応");
    reduce(&mut editor, action);
    assert!(editor.cursor.y > before, "View で `j` が進んでいない");
}

// ── 戻る先は閲覧面（実機で踏んだ穴・`#14`）───────────────────────────────────

/// 🚨 **`czv` → `/` → `Esc` で編集面に降りていた**（2026-09-12・実機で発見）。
///
/// ⭐ 原因は `home_mode()` が `config.default_mode` しか見ていなかったこと。
/// **モードは「いまどこに居るか」しか言わない** —— 検索やヘルプは一時的に離れる面なので、
/// 戻る先は**席の性質**（`view_session`）が持つ必要がある。
///
/// ⚠️ `read_only` が保存を止めるので**ファイルは無事**だったが、
/// 「誤操作で汚さない」という約束は**画面の上で破れていた**（帯が `Ctrl+S Save` に変わる）。
#[test]
fn leaving_search_returns_to_view_not_edit() {
    let mut editor = view_editor();
    editor.view_session = true;

    let enter = key(&editor, KeyCode::Char('/')).expect("`/` が無反応");
    reduce(&mut editor, enter);
    assert_eq!(editor.mode, EditorMode::Search, "`/` で検索に入っていない");

    let leave = Keymap::map_key_to_action(&editor, KeyCode::Esc, KeyModifiers::NONE)
        .expect("検索から `Esc` が無反応");
    reduce(&mut editor, leave);
    assert_eq!(
        editor.mode,
        EditorMode::View,
        "検索を抜けたら編集面に降りた —— 閲覧の席が編集面へ落ちている"
    );
}

/// 🚨 **陽性対照。** 閲覧の席でないときは、これまでどおり `default_mode` に従う。
/// これが無いと「`home_mode` は常に View」という実装が上を緑で通す。
#[test]
fn a_normal_session_still_returns_to_its_default_mode() {
    let mut editor = view_editor();
    editor.view_session = false;
    editor.enter_mode(EditorMode::Glide);

    let enter = key(&editor, KeyCode::Char('/')).expect("Glide で `/` が無反応");
    reduce(&mut editor, enter);
    let leave = Keymap::map_key_to_action(&editor, KeyCode::Esc, KeyModifiers::NONE)
        .expect("検索から `Esc` が無反応");
    reduce(&mut editor, leave);
    assert_ne!(
        editor.mode,
        EditorMode::View,
        "閲覧の席でないのに View へ帰っている"
    );
}

/// ⭐ **`default_mode = "glide"` の人が `czv` を使っても閲覧へ帰る。**
/// 📌 これが `#14` で言っていた「`view` は `default_mode` より強い」の本体。
#[test]
fn view_session_outranks_a_configured_default_mode() {
    let mut editor = view_editor();
    editor.view_session = true;
    editor.config.default_mode = Some("glide".to_string());
    assert_eq!(editor.home_mode(), EditorMode::View);
}

/// 🚨 **閲覧の席で別ファイルを開いても `[read-only]` は消えない**（実機で踏んだ）。
///
/// 📏 `czv README.md` → `Ctrl+O` → `Cargo.toml`（書けるファイル）で、
/// **フッタの `[read-only]` が消えていた**。⭐ 編集は構造で止まったままなので
/// 実害は出ないが、**「見るだけ」と言った利用者に嘘をつく**。
#[test]
fn opening_another_file_in_a_view_session_stays_read_only() {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("cozy_view_open_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let writable = dir.join("writable.txt");
    let mut f = std::fs::File::create(&writable).unwrap();
    writeln!(f, "plain writable file").unwrap();
    drop(f);

    let mut editor = view_editor();
    editor.view_session = true;
    crate::file_io::open_file(&mut editor, writable.to_str().unwrap()).unwrap();
    assert!(
        editor.read_only,
        "閲覧の席で書けるファイルを開いたら read_only が下りた —— 約束の表示が消える"
    );

    // 🚨 **陽性対照。** 閲覧の席でなければ、書けるファイルは書ける印のまま。
    let mut normal = view_editor();
    normal.view_session = false;
    crate::file_io::open_file(&mut normal, writable.to_str().unwrap()).unwrap();
    assert!(
        !normal.read_only,
        "普通の席で書けるファイルが read_only になった"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── 半ページ移動 (Issue #18) ──────────────────────────────────────────────────

/// `View`（`czv`）では `d` / `u` および `Ctrl+D` / `Ctrl+U` で半ページ送り・戻しができる。
#[test]
fn view_mode_half_page_navigation_keys() {
    let editor = view_editor();

    // 1. 素の `d` と `u` (less の流儀・モバイル 1 タップ)
    assert_eq!(
        Keymap::map_key_to_action(&editor, KeyCode::Char('d'), KeyModifiers::NONE),
        Some(Action::HalfPageDown),
        "View で `d` が半ページ送りにならない"
    );
    assert_eq!(
        Keymap::map_key_to_action(&editor, KeyCode::Char('u'), KeyModifiers::NONE),
        Some(Action::HalfPageUp),
        "View で `u` が半ページ戻しにならない"
    );

    // 2. `Ctrl+D` と `Ctrl+U` (vi / vim の流儀)
    assert_eq!(
        Keymap::map_key_to_action(&editor, KeyCode::Char('d'), KeyModifiers::CONTROL),
        Some(Action::HalfPageDown),
        "View で `Ctrl+D` が半ページ送りにならない"
    );
    assert_eq!(
        Keymap::map_key_to_action(&editor, KeyCode::Char('u'), KeyModifiers::CONTROL),
        Some(Action::HalfPageUp),
        "View で `Ctrl+U` が半ページ戻しにならない"
    );

    // 🚨 陽性対照: Edit モードでは文字入力や大域ショートカットとして機能する
    let mut edit = view_editor();
    edit.mode = EditorMode::Edit;
    assert_eq!(
        Keymap::map_key_to_action(&edit, KeyCode::Char('d'), KeyModifiers::NONE),
        Some(Action::InsertChar('d')),
        "Edit で `d` が文字入力にならない"
    );
    assert_eq!(
        Keymap::map_key_to_action(&edit, KeyCode::Char('d'), KeyModifiers::CONTROL),
        Some(Action::ToggleMarkdownPreview),
        "Edit で `Ctrl+D` が ToggleMarkdownPreview にならない"
    );
    assert_eq!(
        Keymap::map_key_to_action(&edit, KeyCode::Char('u'), KeyModifiers::CONTROL),
        Some(Action::ToggleFooter),
        "Edit で `Ctrl+U` が ToggleFooter にならない"
    );

    // 🚨 陽性対照: Glide モードでは `d` は Delete オペレータ
    let mut glide = view_editor();
    glide.mode = EditorMode::Glide;
    assert_eq!(
        Keymap::map_key_to_action(&glide, KeyCode::Char('d'), KeyModifiers::NONE),
        Some(Action::SetOperator(crate::glide::Operator::Delete)),
        "Glide で `d` が Delete オペレータにならない"
    );
}

/// czv（View）の F2 は **md のときだけ**整形プレビュー切替。Ctrl+D は半ページ送りのまま
/// （決定 2026-10-03・`#20` Phase 2）。
#[test]
fn f2_previews_in_view_only_for_markdown() {
    let mut md = view_editor();
    md.filename = Some(std::path::PathBuf::from("notes.md"));
    assert_eq!(
        Keymap::map_key_to_action(&md, KeyCode::F(2), KeyModifiers::NONE),
        Some(Action::ToggleMarkdownPreview),
        "md の czv で F2 が整形切替にならない"
    );
    // 🚨 Ctrl+D は据え置き —— vi/less の半ページ送りを壊さない。
    assert_eq!(
        Keymap::map_key_to_action(&md, KeyCode::Char('d'), KeyModifiers::CONTROL),
        Some(Action::HalfPageDown),
        "md でも View の Ctrl+D は半ページ送りのまま"
    );

    let mut other = view_editor();
    other.filename = Some(std::path::PathBuf::from("main.rs"));
    assert_eq!(
        Keymap::map_key_to_action(&other, KeyCode::F(2), KeyModifiers::NONE),
        None,
        "md でない czv で F2 が何かにマップされた"
    );
}

/// `Action::HalfPageDown` / `HalfPageUp` が半画面（page_size / 2）分スクロールする。
#[test]
fn view_mode_half_page_scrolling() {
    let mut editor = view_editor();
    editor.page_size = 20; // 画面高さ 20 行 -> 半ページは 10 行
    editor.scroll_offset = 0;
    editor.cursor.y = 0;

    // 半ページ送り: 10 行進む
    reduce(&mut editor, Action::HalfPageDown);
    assert_eq!(
        editor.scroll_offset, 10,
        "HalfPageDown で 10 行進んでいない"
    );
    assert_eq!(editor.cursor.y, 10);

    // もう一度半ページ送り: さらに 10 行進んで 20 行目へ
    reduce(&mut editor, Action::HalfPageDown);
    assert_eq!(editor.scroll_offset, 20);
    assert_eq!(editor.cursor.y, 20);

    // 半ページ戻し: 10 行戻って 10 行目へ
    reduce(&mut editor, Action::HalfPageUp);
    assert_eq!(editor.scroll_offset, 10);
    assert_eq!(editor.cursor.y, 10);

    // さらに戻し: 0 行目へ
    reduce(&mut editor, Action::HalfPageUp);
    assert_eq!(editor.scroll_offset, 0);
    assert_eq!(editor.cursor.y, 0);

    // 0 行目からさらに上へ行っても 0 未満にはならない
    reduce(&mut editor, Action::HalfPageUp);
    assert_eq!(editor.scroll_offset, 0);
}

// ── ④ 検索を閲覧へ持ち帰る —— `Enter` で確定・`n`/`N`・`zz`/`zt`/`zb`（`#18`）────────
//
// ⭐ 基準は「注目中のヒット」。カーソルではない（スクロールで先頭行へ戻されるので）。

/// 100 行のうち 10 行おき（9, 19, …, 99 行目 ＝ 0 起点）に `hit` が居るバッファ。
fn hits_editor() -> EditorState {
    let mut editor = view_editor();
    editor.view_session = true;
    editor.buffer = TextBuffer::from_lines(
        (1..=100)
            .map(|n| {
                if n % 10 == 0 {
                    format!("{n} hit")
                } else {
                    format!("{n} plain")
                }
            })
            .collect::<Vec<_>>(),
    );
    editor.page_size = 20;
    editor
}

/// `/hit` と打って `Enter` まで押す（鍵の対応表を通す）。
fn search_hit_and_enter(editor: &mut EditorState) {
    let slash = key(editor, KeyCode::Char('/')).expect("`/` が無反応");
    reduce(editor, slash);
    for c in "hit".chars() {
        let a = key(editor, KeyCode::Char(c)).expect("検索欄で文字が無反応");
        reduce(editor, a);
    }
    let enter = key(editor, KeyCode::Enter).expect("検索欄で `Enter` が無反応");
    reduce(editor, enter);
}

#[test]
fn enter_confirms_the_search_and_keeps_the_hits_in_a_view_session() {
    let mut editor = hits_editor();
    search_hit_and_enter(&mut editor);
    assert_eq!(
        editor.mode,
        EditorMode::View,
        "`Enter` で閲覧へ戻っていない"
    );
    assert_eq!(
        editor.search_matches.len(),
        10,
        "ヒットが閲覧へ持ち帰られていない"
    );
    assert_eq!(editor.search_current, 0);
    assert_eq!(editor.cursor.y, 9, "最初のヒットに居ない");
}

/// 🚨 **陽性対照。** 閲覧の席でなければ `Enter` は従来どおり「次へ」で、欄に居続ける。
#[test]
fn enter_still_means_next_outside_a_view_session() {
    let mut editor = hits_editor();
    editor.view_session = false;
    editor.enter_mode(EditorMode::Glide);
    search_hit_and_enter(&mut editor);
    assert_eq!(
        editor.mode,
        EditorMode::Search,
        "編集面の `Enter` が確定に化けた"
    );
    assert_eq!(
        editor.search_current, 1,
        "編集面の `Enter` が次へ送っていない"
    );
}

#[test]
fn n_and_shift_n_step_through_the_hits_in_view_and_wrap() {
    let mut editor = hits_editor();
    search_hit_and_enter(&mut editor);

    let n = key(&editor, KeyCode::Char('n')).expect("View で `n` が無反応");
    reduce(&mut editor, n);
    assert_eq!((editor.search_current, editor.cursor.y), (1, 19));

    let shift_n = key(&editor, KeyCode::Char('N')).expect("View で `N` が無反応");
    reduce(&mut editor, shift_n.clone());
    assert_eq!((editor.search_current, editor.cursor.y), (0, 9));
    // 先頭から戻ると末尾へ折り返す。
    reduce(&mut editor, shift_n);
    assert_eq!((editor.search_current, editor.cursor.y), (9, 99));
}

/// ⭐ 注目中のヒットが見えなくなるまでスクロールしたら、`n` は**いま見ている所から**最寄りへ。
/// 📌 見えているうちは「そこから 1 つ先」—— 1 行スクロールしただけで留まらない。
#[test]
fn n_starts_from_the_screen_once_the_current_hit_scrolled_away() {
    let mut editor = hits_editor();
    search_hit_and_enter(&mut editor); // 注目 = 9 行目

    // 1 行だけ送る: 9 行目はまだ見えている（先頭 1・page 20）→ 次の 19 行目へ。
    reduce(&mut editor, Action::MoveDown);
    reduce(&mut editor, Action::SearchNext);
    assert_eq!(
        editor.cursor.y, 19,
        "見えているヒットから 1 つ先へ送れていない"
    );

    // 先頭を 55 行目まで送る（注目の 19 行目は見えない）→ 55 以降の最寄り ＝ 59 行目。
    editor.scroll_offset = 55;
    editor.cursor.y = 55;
    reduce(&mut editor, Action::SearchNext);
    assert_eq!(editor.cursor.y, 59, "いま見ている所から送っていない");

    // 同じく先頭 55 から `N` ＝ 55 より上の最寄り ＝ 49 行目。
    editor.scroll_offset = 55;
    editor.cursor.y = 55;
    editor.search_current = 0; // 9 行目 ＝ 見えていない
    reduce(&mut editor, Action::SearchPrevious);
    assert_eq!(editor.cursor.y, 49, "いま見ている所から戻っていない");
}

#[test]
fn zz_zt_zb_put_the_current_hit_at_the_middle_top_and_bottom() {
    let mut editor = hits_editor();
    search_hit_and_enter(&mut editor);
    reduce(&mut editor, Action::SearchNext);
    reduce(&mut editor, Action::SearchNext); // 注目 = 29 行目
    assert_eq!(editor.cursor.y, 29);

    for (second, top) in [('z', 19), ('t', 29), ('b', 10)] {
        editor.scroll_offset = 0;
        let z = key(&editor, KeyCode::Char('z')).expect("View で `z` が無反応");
        reduce(&mut editor, z);
        let a = key(&editor, KeyCode::Char(second)).expect("`z` の後の 2 打鍵目が無反応");
        reduce(&mut editor, a);
        assert_eq!(editor.scroll_offset, top, "`z{second}` の先頭行が違う");
        assert_eq!(editor.cursor.y, 29, "`z{second}` でヒットから離れた");
        assert_eq!(editor.glide_prefix, None, "`z{second}` の後に前置が残った");
    }
}

/// 📌 ヒットが無ければ動かない。🚨 取り消した検索が `n` で蘇らないこと —— 欄の文字は
/// 残っているので、`apply_search_next` のように数え直すと蘇る。
#[test]
fn without_hits_n_and_zz_do_nothing_and_a_cancelled_search_stays_cancelled() {
    let mut editor = hits_editor();
    let slash = key(&editor, KeyCode::Char('/')).unwrap();
    reduce(&mut editor, slash);
    for c in "hit".chars() {
        let a = key(&editor, KeyCode::Char(c)).unwrap();
        reduce(&mut editor, a);
    }
    let esc = Keymap::map_key_to_action(&editor, KeyCode::Esc, KeyModifiers::NONE).unwrap();
    reduce(&mut editor, esc);
    assert_eq!(editor.mode, EditorMode::View);
    assert!(
        editor.search_matches.is_empty(),
        "`Esc` でヒットが消えていない"
    );

    editor.scroll_offset = 40;
    editor.cursor.y = 40;
    reduce(&mut editor, Action::SearchNext);
    reduce(
        &mut editor,
        Action::AlignMatch(crate::action::MatchAlign::Center),
    );
    assert!(
        editor.search_matches.is_empty(),
        "取り消した検索が `n` で蘇った"
    );
    assert_eq!(
        (editor.scroll_offset, editor.cursor.y),
        (40, 40),
        "ヒットが無いのに動いた"
    );
}

/// 🚨 `n` / `z` は View だけ。Help / Markdown は専用の中身を描いており、検索が見るのは
/// バッファなので、効かせると画面に無い物を送り始める（`/` と同じ線）。
#[test]
fn n_and_z_are_view_only() {
    let mut editor = hits_editor();
    for mode in [EditorMode::Help, EditorMode::Markdown] {
        editor.enter_mode(mode);
        assert_eq!(
            key(&editor, KeyCode::Char('n')),
            None,
            "{mode:?} で `n` が効いた"
        );
        assert_eq!(
            key(&editor, KeyCode::Char('z')),
            None,
            "{mode:?} で `z` が効いた"
        );
    }
    // 陽性対照: View では両方とも何かを産む。
    editor.enter_mode(EditorMode::View);
    assert!(key(&editor, KeyCode::Char('n')).is_some());
    assert!(key(&editor, KeyCode::Char('z')).is_some());
}
