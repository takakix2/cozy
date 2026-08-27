use crate::state::key::{KeyCode, KeyModifiers};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditorAction {
    EnterSave,
    EnterSaveAs,
    EnterOpen,
    EnterBrowse,
    EnterExit,
    ForceQuit, // Ctrl+Q - Quit immediately without saving
    EnterSearch,
    EnterReplace,
    ReplaceAll,
    Enter,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    PageTop,
    PageBottom,
    FileTop,
    FileBottom,
    Home,
    End,
    ToggleSearchMode,
    ReloadConfig,
    EnterHelp,
    Undo,
    Redo,
    Cancel,
    ToggleLineNumbers,
    ToggleWrap,
    ToggleFooter,
    DeleteLine,
    Paste,
    EnterGoto,
    EnterGlide,
    ToggleMarkdownPreview,
    ToggleDiffReview,
    EnterCommand,
}

#[derive(Clone)]
pub struct Shortcut {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
    pub action: EditorAction,
    /// `"Ctrl+H Help"` のような、**画面に出すための一言**。
    ///
    /// 🚨 **いまは誰も読んでいない。** 帯（`#2` 段③）も Help（`#9`）も、
    /// キー名は `key_for` / `keys_for` から引き、**説明の文言は呼び出し側が持つ**
    /// 形に落ち着いた —— そちらの方が、幅による綴りの出し分けや
    /// `Ctrl+Z / Ctrl+Y  Undo / Redo` のような 2 アクション 1 行と噛み合う。
    ///
    /// ⚠️ ∴ **この欄は「Help を直すときに使う」と書いて残したが、使わなかった。**
    /// 消していないのは、`get_shortcuts()` の一覧を読む人にとって
    /// 「この鍵は何のためか」がその場で分かる注記として働いているから。
    /// 📌 読み手が現れないままなら、次に触る人が消してよい。
    #[allow(dead_code)]
    pub label: &'static str,
    /// この鍵は「主」ではなく、**主の鍵が届かない環境のために置いた 2 本目**か。
    ///
    /// 🚨 これが構造に無かった間、`[keys]` の上書きは**そのアクションの割り当てを
    /// 全部**消していた。∴ `enter_help = "f5"` と書いただけで、
    /// **書いてもいない `F1` まで消えた** —— しかも `F1` は
    /// 「`Ctrl+H` は端末によっては Backspace(0x08) として飲まれる」という
    /// **理由つきで**置かれていた（`#2`）。⭐ 上書きは、書き留められた理由ごと
    /// 消していたことになる。
    ///
    /// ⚠️ **印は理由と対で置く。** ここが `true` のものは、すぐ上のコメントに
    /// 「なぜ主の鍵が届かないことがあるか」が書いてある。理由の無い 2 本目
    /// （`Alt+\` と `Ctrl+Home` のような**別々の流儀**）は `false` のまま ——
    /// あれは保険ではなく、どちらも主。
    pub fallback: bool,
}

// Shortcut constructor helper - makes definitions more compact
fn sc(
    key: KeyCode,
    modifiers: KeyModifiers,
    action: EditorAction,
    label: &'static str,
) -> Shortcut {
    Shortcut {
        key,
        modifiers,
        action,
        label,
        fallback: false,
    }
}

/// **主の鍵が届かない環境のために置く 2 本目。**
///
/// ⭐ `[keys]` の上書きはこれを消さない —— 上書きした人が置き換えたいのは
/// **主の鍵**であって、届かないときの逃げ道ではない（`Shortcut::fallback` を見よ）。
fn sc_fallback(
    key: KeyCode,
    modifiers: KeyModifiers,
    action: EditorAction,
    label: &'static str,
) -> Shortcut {
    Shortcut {
        fallback: true,
        ..sc(key, modifiers, action, label)
    }
}

// File operation shortcuts
fn file_shortcuts() -> Vec<Shortcut> {
    vec![
        sc(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL,
            EditorAction::EnterSave,
            "Ctrl+S Save...",
        ),
        sc(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            EditorAction::EnterSaveAs,
            "Ctrl+Shift+S Save As...",
        ),
        sc(
            KeyCode::Char('o'),
            KeyModifiers::CONTROL,
            EditorAction::EnterOpen,
            "Ctrl+O Open...",
        ),
        sc(
            KeyCode::Char('b'),
            KeyModifiers::CONTROL,
            EditorAction::EnterBrowse,
            "Ctrl+B Browse...",
        ),
        // F3 is a tmux-safe Browse fallback: tmux's default prefix is Ctrl+B,
        // which it swallows before cozy ever sees it. Same shape as the F1/Ctrl+H
        // and F2/Ctrl+D fallbacks for keys terminals or multiplexers intercept.
        sc_fallback(
            KeyCode::F(3),
            KeyModifiers::NONE,
            EditorAction::EnterBrowse,
            "F3 Browse...",
        ),
        sc(
            KeyCode::Char('x'),
            KeyModifiers::CONTROL,
            EditorAction::EnterExit,
            "Ctrl+X Exit...",
        ),
    ]
}

// Search and replace shortcuts
fn search_shortcuts() -> Vec<Shortcut> {
    vec![
        sc(
            KeyCode::Char('f'),
            KeyModifiers::CONTROL,
            EditorAction::EnterSearch,
            "Ctrl+F Find...",
        ),
        sc(
            KeyCode::Char('r'),
            KeyModifiers::CONTROL,
            EditorAction::EnterReplace,
            "Ctrl+R Replace...",
        ),
        sc(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
            EditorAction::ForceQuit,
            "Ctrl+Q Quit without saving",
        ),
        sc(
            KeyCode::Char('t'),
            KeyModifiers::CONTROL,
            EditorAction::ToggleSearchMode,
            "Ctrl+T Toggle",
        ),
    ]
}

// Navigation shortcuts
fn navigation_shortcuts() -> Vec<Shortcut> {
    vec![
        sc(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL,
            EditorAction::Home,
            "Ctrl+A Home",
        ),
        // File top / bottom, spelled the way nano spells them: `M-\` and `M-/`, with
        // `Ctrl+Home` / `Ctrl+End` as the alternate. ⭐ nano itself carries both, so
        // carrying both is the faithful thing rather than a belt-and-braces choice.
        // ⚠️ Neither is producible on a phone; Glide's `gg`/`G` is that side's answer.
        sc(
            KeyCode::Char('\\'),
            KeyModifiers::ALT,
            EditorAction::FileTop,
            "Alt+\\ FileTop",
        ),
        sc(
            KeyCode::Home,
            KeyModifiers::CONTROL,
            EditorAction::FileTop,
            "Ctrl+Home FileTop",
        ),
        sc(
            KeyCode::Char('/'),
            KeyModifiers::ALT,
            EditorAction::FileBottom,
            "Alt+/ FileBottom",
        ),
        sc(
            KeyCode::End,
            KeyModifiers::CONTROL,
            EditorAction::FileBottom,
            "Ctrl+End FileBottom",
        ),
        sc(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL,
            EditorAction::End,
            "Ctrl+E End",
        ),
        sc(
            KeyCode::PageUp,
            KeyModifiers::NONE,
            EditorAction::PageUp,
            "PgUp Prev",
        ),
        sc(
            KeyCode::PageDown,
            KeyModifiers::NONE,
            EditorAction::PageDown,
            "PgDn Next",
        ),
    ]
}

// Edit operation shortcuts
fn edit_shortcuts() -> Vec<Shortcut> {
    vec![
        sc(
            KeyCode::Char('z'),
            KeyModifiers::CONTROL,
            EditorAction::Undo,
            "Ctrl+Z Undo",
        ),
        sc(
            KeyCode::Char('y'),
            KeyModifiers::CONTROL,
            EditorAction::Redo,
            "Ctrl+Y Redo",
        ),
        sc(
            KeyCode::Char('v'),
            KeyModifiers::CONTROL,
            EditorAction::Paste,
            "Ctrl+V Paste",
        ),
    ]
}

// Utility shortcuts
fn utility_shortcuts() -> Vec<Shortcut> {
    vec![
        sc(
            KeyCode::Char('h'),
            KeyModifiers::CONTROL,
            EditorAction::EnterHelp,
            "Ctrl+H Help",
        ),
        sc(
            KeyCode::Char('p'),
            KeyModifiers::CONTROL,
            EditorAction::EnterCommand,
            "Ctrl+P Command",
        ),
        // F1 is an unambiguous Help fallback: Ctrl+H sends the Backspace byte
        // (0x08) on some terminals and can be swallowed there.
        sc_fallback(
            KeyCode::F(1),
            KeyModifiers::NONE,
            EditorAction::EnterHelp,
            "F1 Help",
        ),
        sc(
            KeyCode::Char('l'),
            KeyModifiers::CONTROL,
            EditorAction::ToggleLineNumbers,
            "Ctrl+L LineNo",
        ),
        sc(
            KeyCode::Char('w'),
            KeyModifiers::CONTROL,
            EditorAction::ToggleWrap,
            "Ctrl+W Wrap",
        ),
        sc(
            KeyCode::Char('u'),
            KeyModifiers::CONTROL,
            EditorAction::ToggleFooter,
            "Ctrl+U Footer",
        ),
        sc(
            KeyCode::Char('k'),
            KeyModifiers::CONTROL,
            EditorAction::DeleteLine,
            "Ctrl+K Cut Line",
        ),
        sc(
            KeyCode::Char('j'),
            KeyModifiers::CONTROL,
            EditorAction::EnterGoto,
            "Ctrl+J Jump",
        ),
        sc(
            KeyCode::Char('g'),
            KeyModifiers::CONTROL,
            EditorAction::EnterGlide,
            "Ctrl+G Glide",
        ),
        sc_fallback(
            KeyCode::F(2),
            KeyModifiers::NONE,
            EditorAction::ToggleMarkdownPreview,
            "F2 Markdown",
        ),
        sc(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
            EditorAction::ToggleMarkdownPreview,
            "Ctrl+D Markdown",
        ),
        sc(
            KeyCode::F(4),
            KeyModifiers::NONE,
            EditorAction::ToggleDiffReview,
            "F4 Diff",
        ),
    ]
}

// Cancel/Exit shortcuts
fn cancel_shortcuts() -> Vec<Shortcut> {
    vec![
        sc(
            KeyCode::Esc,
            KeyModifiers::NONE,
            EditorAction::Cancel,
            "Esc Cancel",
        ),
        // Ctrl+[ == Esc at the byte level: under the legacy protocol it already
        // arrives as Esc, but under the kitty keyboard protocol it splits off as
        // Ctrl+[. Bind it so the vim-style cancel keeps working either way.
        sc_fallback(
            KeyCode::Char('['),
            KeyModifiers::CONTROL,
            EditorAction::Cancel,
            "Ctrl+[ Cancel",
        ),
    ]
}

// Internal shortcuts (arrow keys, Enter, Esc)
fn internal_shortcuts() -> Vec<Shortcut> {
    vec![
        sc(
            KeyCode::Up,
            KeyModifiers::NONE,
            EditorAction::MoveUp,
            "↑ Up",
        ),
        sc(
            KeyCode::Down,
            KeyModifiers::NONE,
            EditorAction::MoveDown,
            "↓ Down",
        ),
        sc(
            KeyCode::Left,
            KeyModifiers::NONE,
            EditorAction::MoveLeft,
            "← Left",
        ),
        sc(
            KeyCode::Right,
            KeyModifiers::NONE,
            EditorAction::MoveRight,
            "→ Right",
        ),
        sc(
            KeyCode::Enter,
            KeyModifiers::NONE,
            EditorAction::Enter,
            "Enter",
        ),
    ]
}

pub fn get_shortcuts() -> Vec<Shortcut> {
    [
        file_shortcuts(),
        search_shortcuts(),
        navigation_shortcuts(),
        edit_shortcuts(),
        utility_shortcuts(),
        cancel_shortcuts(),
        internal_shortcuts(),
    ]
    .concat()
}

pub fn shortcut_map() -> HashMap<(KeyCode, KeyModifiers), EditorAction> {
    let mut map = HashMap::new();
    for shortcut in get_shortcuts() {
        map.insert((shortcut.key, shortcut.modifiers), shortcut.action);
    }
    map
}

/// 帯での綴り方。⚠️ 幅で変わる（広ければ `Ctrl+S`、狭ければ `^S`）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyStyle {
    /// `^S` —— 狭い帯。
    Caret,
    /// `Ctrl+S` —— 広い帯。
    Spelled,
}

/// 鍵を**画面に出す綴り**にする（`^S` / `Ctrl+S` / `F1` / `Esc` / `Alt+\`）。
///
/// 📌 綴りは footer が元々べた書きしていた形に合わせてある —— 帯の見た目を変えずに、
/// 出どころだけを「文字列リテラル」から「実際のキーマップ」へ移すため（`#2`）。
pub fn display_key(key: KeyCode, mods: KeyModifiers, style: KeyStyle) -> String {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let alt = mods.contains(KeyModifiers::ALT);
    match key {
        // ⚠️ 帯は幅で綴りを変える —— 広ければ `Ctrl+S`、狭ければ `^S`。
        KeyCode::Char(c) if ctrl => match style {
            KeyStyle::Caret => format!("^{}", c.to_ascii_uppercase()),
            KeyStyle::Spelled => format!("Ctrl+{}", c.to_ascii_uppercase()),
        },
        KeyCode::Char(c) if alt => format!("Alt+{c}"),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "BS".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Home if ctrl => match style {
            KeyStyle::Caret => "^Home".to_string(),
            KeyStyle::Spelled => "Ctrl+Home".to_string(),
        },
        KeyCode::End if ctrl => match style {
            KeyStyle::Caret => "^End".to_string(),
            KeyStyle::Spelled => "Ctrl+End".to_string(),
        },
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
    }
}

/// そのアクションを**いま**呼び出す鍵の綴り。案内はここに訊く。
///
/// 🚨 footer はキー名を**文字列リテラルで持っていた**ので、`[keys]` で上書きしても
/// `^H Help` と言い続け、その `^H` は効かなかった（`#2`）。
/// ⭐ 電話ではこの帯が唯一の発見手段なので、嘘をつくと入口ごと失われる。
///
/// ⚠️ **主の鍵を出す。** 同じアクションに複数あるときは `fallback` でない方
/// （`Ctrl+H` が生きているなら `F1` ではなくそちらを出す）。⭐ 順序は
/// `get_shortcuts()` の並びで決めるので、`HashMap` の走査順に左右されない。
pub fn key_for(
    map: &HashMap<(KeyCode, KeyModifiers), EditorAction>,
    action: EditorAction,
    style: KeyStyle,
) -> Option<String> {
    let defaults = get_shortcuts();
    let is_default =
        |k: KeyCode, m: KeyModifiers| defaults.iter().any(|s| s.key == k && s.modifiers == m);

    // ① 🚨 **利用者が `[keys]` で指定した鍵が最優先。** ここを後回しにすると、
    // 守ったフォールバックの方を案内してしまう —— `enter_browse = "f6"` と書いたのに
    // 帯が `F3 Browse` と言う状態になった（実測で踏んだ）。
    // ⭐ 決定的にするため綴りで並べて先頭を取る（`HashMap` の順に左右されない）。
    let mut chosen: Vec<String> = map
        .iter()
        .filter(|(_, v)| **v == action)
        .filter(|((k, m), _)| !is_default(*k, *m))
        .map(|((k, m), _)| display_key(*k, *m, style))
        .collect();
    chosen.sort();
    if let Some(first) = chosen.into_iter().next() {
        return Some(first);
    }

    // ② 既定のまま生きている鍵。主 → フォールバックの順（`Ctrl+H` が生きているなら
    // `F1` ではなくそちらを案内する）。
    let live = |k: KeyCode, m: KeyModifiers| map.get(&(k, m)) == Some(&action);
    for want_fallback in [false, true] {
        for sc in defaults.iter() {
            if sc.action == action && sc.fallback == want_fallback && live(sc.key, sc.modifiers) {
                return Some(display_key(sc.key, sc.modifiers, style));
            }
        }
    }
    None
}

/// そのアクションを呼び出す鍵を**全部**、主 → フォールバックの順で。
///
/// ⭐ Help はキーの**全体像**を見る場所なので、`Ctrl+B / F3` のように
/// 主とフォールバックを両方出す（帯は枠が 3〜5 個しか無いので `key_for` の 1 本だけ）。
/// ⚠️ `[keys]` で足された鍵が最優先なのは `key_for` と同じ ——
/// 利用者が書いた鍵を差し置いてフォールバックを先頭に出さない。
pub fn keys_for(
    map: &HashMap<(KeyCode, KeyModifiers), EditorAction>,
    action: EditorAction,
    style: KeyStyle,
) -> Vec<String> {
    let defaults = get_shortcuts();
    let is_default =
        |k: KeyCode, m: KeyModifiers| defaults.iter().any(|s| s.key == k && s.modifiers == m);

    // ① 上書きで足された鍵（綴りで並べて決定的に）。
    let mut out: Vec<String> = map
        .iter()
        .filter(|(_, v)| **v == action)
        .filter(|((k, m), _)| !is_default(*k, *m))
        .map(|((k, m), _)| display_key(*k, *m, style))
        .collect();
    out.sort();

    // ② 既定のまま生きている鍵。主 → フォールバックの順。
    for want_fallback in [false, true] {
        for sc in defaults.iter() {
            if sc.action == action
                && sc.fallback == want_fallback
                && map.get(&(sc.key, sc.modifiers)) == Some(&action)
            {
                out.push(display_key(sc.key, sc.modifiers, style));
            }
        }
    }
    out
}

/// `"ctrl+s"` / `"alt+enter"` / `"pageup"` 等のキー文字列を (KeyCode, KeyModifiers) に変換する
pub fn parse_key_str(s: &str) -> Option<(KeyCode, KeyModifiers)> {
    let lower = s.to_lowercase();
    let parts: Vec<&str> = lower.split('+').collect();
    // split_last() → (last_element, rest): key は末尾、修飾キーは残り
    let (key_token, modifier_tokens) = parts.split_last()?;
    let key_token: &str = key_token;
    let mut mods = KeyModifiers::NONE;
    for m in modifier_tokens {
        match *m {
            "ctrl" => mods |= KeyModifiers::CONTROL,
            "shift" => mods |= KeyModifiers::SHIFT,
            "alt" => mods |= KeyModifiers::ALT,
            _ => return None,
        }
    }
    let code = match key_token {
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "tab" => KeyCode::Tab,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        c if c.chars().count() == 1 => KeyCode::Char(c.chars().next()?),
        _ => return None,
    };
    Some((code, mods))
}

/// アクション名文字列 → EditorAction
pub fn action_from_name(name: &str) -> Option<EditorAction> {
    match name {
        "enter_save" => Some(EditorAction::EnterSave),
        "enter_save_as" => Some(EditorAction::EnterSaveAs),
        "enter_open" => Some(EditorAction::EnterOpen),
        "enter_browse" => Some(EditorAction::EnterBrowse),
        "enter_exit" => Some(EditorAction::EnterExit),
        "force_quit" => Some(EditorAction::ForceQuit),
        "enter_search" => Some(EditorAction::EnterSearch),
        "enter_replace" => Some(EditorAction::EnterReplace),
        "replace_all" => Some(EditorAction::ReplaceAll),
        "enter_help" => Some(EditorAction::EnterHelp),
        "undo" => Some(EditorAction::Undo),
        "redo" => Some(EditorAction::Redo),
        "cancel" => Some(EditorAction::Cancel),
        "page_up" => Some(EditorAction::PageUp),
        "page_down" => Some(EditorAction::PageDown),
        "page_top" => Some(EditorAction::PageTop),
        "file_top" => Some(EditorAction::FileTop),
        "file_bottom" => Some(EditorAction::FileBottom),
        "page_bottom" => Some(EditorAction::PageBottom),
        "home" => Some(EditorAction::Home),
        "end" => Some(EditorAction::End),
        "delete_line" => Some(EditorAction::DeleteLine),
        "toggle_line_numbers" => Some(EditorAction::ToggleLineNumbers),
        "toggle_wrap" => Some(EditorAction::ToggleWrap),
        "toggle_footer" => Some(EditorAction::ToggleFooter),
        "reload_config" => Some(EditorAction::ReloadConfig),
        "enter_goto" => Some(EditorAction::EnterGoto),
        "enter_glide" => Some(EditorAction::EnterGlide),
        "toggle_markdown" => Some(EditorAction::ToggleMarkdownPreview),
        "toggle_diff_review" => Some(EditorAction::ToggleDiffReview),
        "enter_command" => Some(EditorAction::EnterCommand),
        "paste" => Some(EditorAction::Paste),
        _ => None,
    }
}

/// デフォルトショートカットマップに config.toml の [keys] 上書きを適用して返す
pub fn build_shortcut_map(
    overrides: Option<&HashMap<String, String>>,
) -> HashMap<(KeyCode, KeyModifiers), EditorAction> {
    let mut map = shortcut_map();
    let Some(ov) = overrides else { return map };
    // 上書きが消してはいけない鍵（`Shortcut::fallback`）。
    let fallback_keys: std::collections::HashSet<(KeyCode, KeyModifiers)> = get_shortcuts()
        .iter()
        .filter(|s| s.fallback)
        .map(|s| (s.key, s.modifiers))
        .collect();
    for (action_name, key_str) in ov {
        let Some(action) = action_from_name(action_name) else {
            eprintln!("warning: unknown action '{}' in [keys] config", action_name);
            continue;
        };
        let Some(key) = parse_key_str(key_str) else {
            eprintln!("warning: cannot parse key '{}' in [keys] config", key_str);
            continue;
        };
        // 🚨 **上書きは主の鍵だけを置き換える。** 以前はここが
        // `map.retain(|_, v| v != &action)` で、そのアクションの割り当てを**全部**消していた。
        // ∴ `enter_help = "f5"` と書いただけで、書いてもいない `F1` まで消えた ——
        // しかも `F1` は「`Ctrl+H` は端末によっては Backspace として飲まれる」という
        // **理由つきで**置かれていた（`#2`）。
        //
        // ⭐ 上書きした人が置き換えたいのは**主の鍵**であって、届かないときの逃げ道ではない。
        // ∴ `fallback` の印が付いた割り当ては残す。
        map.retain(|k, v| v != &action || fallback_keys.contains(k));
        map.insert(key, action);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_preview_has_function_key_and_ctrl_fallback() {
        let map = shortcut_map();

        assert_eq!(
            map.get(&(KeyCode::F(2), KeyModifiers::NONE)),
            Some(&EditorAction::ToggleMarkdownPreview)
        );
        assert_eq!(
            map.get(&(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            Some(&EditorAction::ToggleMarkdownPreview)
        );
    }

    #[test]
    fn browse_has_ctrl_b_and_f3_fallback() {
        let map = shortcut_map();

        assert_eq!(
            map.get(&(KeyCode::Char('b'), KeyModifiers::CONTROL)),
            Some(&EditorAction::EnterBrowse)
        );
        // F3 is reachable inside tmux, where Ctrl+B is the multiplexer prefix.
        assert_eq!(
            map.get(&(KeyCode::F(3), KeyModifiers::NONE)),
            Some(&EditorAction::EnterBrowse)
        );
    }

    #[test]
    fn footer_toggle_has_default_shortcut() {
        let map = shortcut_map();

        assert_eq!(
            map.get(&(KeyCode::Char('u'), KeyModifiers::CONTROL)),
            Some(&EditorAction::ToggleFooter)
        );
    }

    #[test]
    fn footer_toggle_binding_can_be_overridden() {
        let overrides = HashMap::from([("toggle_footer".to_string(), "f6".to_string())]);
        let map = build_shortcut_map(Some(&overrides));

        assert_eq!(
            map.get(&(KeyCode::F(6), KeyModifiers::NONE)),
            Some(&EditorAction::ToggleFooter)
        );
        assert_eq!(map.get(&(KeyCode::Char('u'), KeyModifiers::CONTROL)), None);
    }
}

/// 🚨 **`[keys]` の上書きが、理由つきで置かれた 2 本目を殺していた**（`#2`）。
///
/// ⭐ 網は「消えないこと」だけでなく **消えるべきものは消えること**も見る ——
/// 「何も消さない」実装なら前者だけでは緑で通る。
#[cfg(test)]
mod override_keeps_fallbacks {
    use super::*;
    use std::collections::HashMap;

    fn overridden(action: &str, key: &str) -> HashMap<(KeyCode, KeyModifiers), EditorAction> {
        let mut ov = HashMap::new();
        ov.insert(action.to_string(), key.to_string());
        build_shortcut_map(Some(&ov))
    }

    fn bound(
        map: &HashMap<(KeyCode, KeyModifiers), EditorAction>,
        key: KeyCode,
        mods: KeyModifiers,
    ) -> Option<EditorAction> {
        map.get(&(key, mods)).copied()
    }

    /// 🚨 **本丸** —— `enter_help = "f5"` は `Ctrl+H` を置き換えるが、`F1` は残す。
    /// ⚠️ `F1` は「`Ctrl+H` は端末によっては Backspace(0x08) として飲まれる」という
    /// 理由で置かれている。上書きは、その理由ごと消してはいけない。
    #[test]
    fn overriding_help_keeps_the_f1_fallback() {
        let map = overridden("enter_help", "f5");
        assert_eq!(
            bound(&map, KeyCode::F(5), KeyModifiers::NONE),
            Some(EditorAction::EnterHelp),
            "上書きした鍵が効いていない"
        );
        assert_eq!(
            bound(&map, KeyCode::F(1), KeyModifiers::NONE),
            Some(EditorAction::EnterHelp),
            "書いてもいない F1 が消えた（端末が Ctrl+H を飲む環境で Help に入れなくなる）"
        );
        assert_eq!(
            bound(&map, KeyCode::Char('h'), KeyModifiers::CONTROL),
            None,
            "主の鍵は置き換えられるべき"
        );
    }

    /// ⭐ tmux では `Ctrl+B` が prefix なので、`F3` が唯一の入口になる。
    #[test]
    fn overriding_browse_keeps_the_f3_fallback() {
        let map = overridden("enter_browse", "f6");
        assert_eq!(
            bound(&map, KeyCode::F(6), KeyModifiers::NONE),
            Some(EditorAction::EnterBrowse)
        );
        assert_eq!(
            bound(&map, KeyCode::F(3), KeyModifiers::NONE),
            Some(EditorAction::EnterBrowse),
            "tmux 越しの唯一の入口が消えた"
        );
        assert_eq!(bound(&map, KeyCode::Char('b'), KeyModifiers::CONTROL), None);
    }

    /// 🚨 **陽性対照。** 「何も消さない」実装をここで弾く ——
    /// 主の鍵は**置き換えられなければならない**（上書きの意味が無くなる）。
    #[test]
    fn the_primary_key_is_still_replaced() {
        let map = overridden("enter_save", "f9");
        assert_eq!(
            bound(&map, KeyCode::F(9), KeyModifiers::NONE),
            Some(EditorAction::EnterSave)
        );
        assert_eq!(
            bound(&map, KeyCode::Char('s'), KeyModifiers::CONTROL),
            None,
            "上書きしたのに元の Ctrl+S が残っている"
        );
    }

    /// ⚠️ **理由の無い 2 本目は守らない。** `Alt+\` と `Ctrl+Home` は保険ではなく
    /// **別々の流儀**（vim 風と一般的な綴り）で、どちらも主。
    /// ⭐ 印は理由と対で置く、という線をここで固定する。
    #[test]
    fn two_primaries_are_both_replaced() {
        let map = overridden("file_top", "f10");
        assert_eq!(
            bound(&map, KeyCode::F(10), KeyModifiers::NONE),
            Some(EditorAction::FileTop)
        );
        assert_eq!(
            bound(&map, KeyCode::Home, KeyModifiers::CONTROL),
            None,
            "理由の無い 2 本目まで守っている（印が広すぎる）"
        );
    }

    /// ⭐ 上書きが無いときは、印は何にも影響しない。
    #[test]
    fn without_overrides_every_binding_stands() {
        let map = build_shortcut_map(None);
        assert_eq!(
            bound(&map, KeyCode::Char('h'), KeyModifiers::CONTROL),
            Some(EditorAction::EnterHelp)
        );
        assert_eq!(
            bound(&map, KeyCode::F(1), KeyModifiers::NONE),
            Some(EditorAction::EnterHelp)
        );
    }

    /// 📌 **印が付いているのは、理由がソースに書いてあるものだけ**であること。
    /// ⚠️ 数を固定すると増やすたびに落ちるので、**印と理由の対応**を見る。
    #[test]
    fn every_fallback_has_a_primary_to_fall_back_from() {
        for s in get_shortcuts().iter().filter(|s| s.fallback) {
            let primaries = get_shortcuts()
                .iter()
                .filter(|o| o.action == s.action && !o.fallback)
                .count();
            assert!(
                primaries >= 1,
                "{:?} は fallback だが、主の鍵が無い（印が間違っている）",
                s.action
            );
        }
    }
}
