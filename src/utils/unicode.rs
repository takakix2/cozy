use unicode_width::UnicodeWidthChar;

/// 1 文字が画面で占める桁数。**幅を数える場所は、必ずここを通す。**
///
/// 🚨 **以前は 3 通りに割れていた**（2026-08-28 に統合）。`wrap_chunks` は
/// `unwrap_or(1)`、`display_width_up_to` は `unwrap_or(0)`、`visual_col` は
/// `UnicodeWidthStr`（＝制御文字は 0）を使っていた。∴ **制御文字を含む行では、
/// 折り返しとカーソル位置が互いにずれていた** —— 同じ行を、同じフレームの中で
/// 別の幅として数えていたことになる。
///
/// ⚠️ **描き方と数え方は同じ規則から出さなければならない。** 制御文字を `^X` の
/// 2 桁で描くなら、幅も 2 でなければカーソルが文字の上に乗らない
/// （`ui::render::body` の可視化と対になっている）。
pub fn char_display_width(ch: char) -> usize {
    // ⭐ **描く側と同じ関数に訊く。** `^X` として描かれる字だけが 2 桁。
    // ここを `is_control()` で判定すると、`^` 表記を持たない字まで 2 桁になり、
    // **幅だけがずれる**（描画は 1 文字のまま）。
    if caret_notation(ch).is_some() {
        return 2;
    }
    // ⚠️ TAB だけは別扱い。他の制御文字と違い**普通のテキストに入っている**ので、
    // `^I` として 2 桁で描くと既存のファイルの見た目が変わる。⭐ 桁を合わせて
    // 展開するのか `^I` と描くのかは `#8` の外で決める。ここでは
    // **`wrap_chunks` が元から使っていた 1** を保つ（挙動を変えない側に倒す）。
    if ch == '\t' {
        return 1;
    }
    // ⚠️ ここでの `None` は「幅が未定義」＝ 結合文字など。0 でよい。
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// `^X` 表記で描くべき字なら、その `X` を返す。**描画と幅の唯一の判定**。
///
/// 🚨 これが分かれると、画面に 1 文字しか出ていない場所をカーソルが 2 桁ぶん
/// 進む（`#8` の症状そのもの）。∴ **描く側 (`ui::render::body`) も
/// 数える側 (`char_display_width`) も、必ずここに訊く。**
///
/// | 字 | 表記 |
/// |---|---|
/// | `\r` (0x0D) | `^M` |
/// | `ESC` (0x1B) | `^[` |
/// | `\b` (0x08) | `^H` |
/// | `DEL` (0x7F) | `^?` |
///
/// ⚠️ **TAB は返さない** —— 普通のテキストに入っており、`^I` にすると既存ファイルの
/// 見た目が変わる（`#8` の外で決める）。
///
/// ⚠️ **C1（U+0080–U+009F）も返さない。** `^` 表記は ASCII 用で、`0x80 ^ 0x40` は
/// 非 ASCII になる。⭐ そして UTF-8 のテキストでは C1 は 2 バイトに符号化されるので、
/// **端末は制御として解釈しない** ＝ この issue の危険（端末制御を渡す）に当たらない。
/// 見えないままではあるので、扱うなら `<80>` のような別の表記と、それに合う幅が要る。
pub fn caret_notation(ch: char) -> Option<char> {
    match ch {
        // TAB は素通し（上記）。
        '\t' => None,
        // C0 制御文字。`^` は 0x40 との XOR（`+` ではない —— DEL が壊れる）。
        c if (c as u32) < 0x20 => Some(((c as u8) ^ 0x40) as char),
        '\u{7f}' => Some('?'),
        _ => None,
    }
}

/// 1 文字を**画面に出す形**にする。制御文字はここで `^X` になる。
///
/// 🚨 **他人のバイトを端末へ渡す唯一の関門。** 行を描く側は必ずここを通す ——
/// 通さないと、ファイルの中身が端末制御を奪える（`ROADMAP.md`「Stop handing the
/// terminal whatever the file says」）。
pub fn visible_char(ch: char) -> String {
    match caret_notation(ch) {
        Some(c) => {
            let mut s = String::with_capacity(2);
            s.push('^');
            s.push(c);
            s
        }
        None => ch.to_string(),
    }
}

/// 文字列が画面で占める桁数。
pub fn str_display_width(s: &str) -> usize {
    s.chars().map(char_display_width).sum()
}

/// Returns the display column width of the substring `s[..byte_pos]`.
/// Byte pos must be on a char boundary.
pub fn display_width_up_to(s: &str, byte_pos: usize) -> usize {
    str_display_width(&s[..byte_pos.min(s.len())])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🚨 **これが `#8` の芯** —— 描いた字の桁数と、数えた幅が一致すること。
    /// 割れると、画面に 1 文字しか出ていない場所をカーソルが 2 桁ぶん進む。
    ///
    /// ⭐ **全 C0 制御文字と DEL を回す。** 1 文字ずつ書いた表だと、
    /// 次に表記を足したとき（C1 など）に**その字だけ抜ける**。
    #[test]
    fn drawing_and_counting_agree_for_every_control_char() {
        for code in (0u32..0x20).chain(std::iter::once(0x7f)) {
            let ch = char::from_u32(code).unwrap();
            if ch == '\t' {
                continue; // ⚠️ TAB は `#8` の外（端末が桁を送るので別の話）
            }
            let drawn = visible_char(ch);
            let drawn_cols: usize = drawn
                .chars()
                .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                .sum();
            assert_eq!(
                drawn_cols,
                char_display_width(ch),
                "U+{code:04X}: 描いた字は {drawn_cols} 桁なのに、幅は {} と数えている",
                char_display_width(ch)
            );
        }
    }

    #[test]
    fn caret_notation_uses_xor_not_addition() {
        // ⚠️ `+ 0x40` だと DEL(0x7F) が 0xBF になって壊れる。
        assert_eq!(caret_notation('\r'), Some('M'));
        assert_eq!(caret_notation('\u{1b}'), Some('['));
        assert_eq!(caret_notation('\u{8}'), Some('H'));
        assert_eq!(caret_notation('\u{7f}'), Some('?'));
        assert_eq!(caret_notation('\0'), Some('@'));
    }

    #[test]
    fn visible_char_draws_caret_notation() {
        assert_eq!(visible_char('\r'), "^M");
        assert_eq!(visible_char('\u{1b}'), "^[");
        assert_eq!(visible_char('\u{7f}'), "^?");
    }

    /// 陽性対照 —— 普通の字は**何も変えない**。
    /// ⭐ これが無いと「全部 `^X` にする」実装が緑で通る。
    #[test]
    fn ordinary_chars_pass_through_untouched() {
        assert_eq!(visible_char('a'), "a");
        assert_eq!(visible_char('あ'), "あ");
        assert_eq!(visible_char('\t'), "\t");
        assert_eq!(caret_notation('a'), None);
        assert_eq!(caret_notation('\t'), None, "TAB は `#8` の外");
    }

    #[test]
    fn control_chars_are_two_columns() {
        // `^M` / `^[` / `^H` として描かれるので 2 桁。
        assert_eq!(char_display_width('\r'), 2);
        assert_eq!(char_display_width('\u{1b}'), 2);
        assert_eq!(char_display_width('\u{8}'), 2);
        assert_eq!(char_display_width('\u{7f}'), 2); // DEL → `^?`
    }

    #[test]
    fn tab_keeps_its_old_width() {
        // ⚠️ `#8` の外。ここを変えると既存ファイルの見た目が動く。
        assert_eq!(char_display_width('\t'), 1);
    }

    #[test]
    fn ordinary_and_wide_chars_are_unchanged() {
        assert_eq!(char_display_width('a'), 1);
        assert_eq!(char_display_width('あ'), 2);
        assert_eq!(str_display_width("abあ"), 4);
    }

    /// 🚨 **これが割れていたのが元の不具合。** 折り返しに使う数え方と、
    /// カーソルに使う数え方が、同じ行に対して同じ値を返すこと。
    #[test]
    fn one_rule_for_every_counter() {
        let line = "a\rb";
        // 1 文字ずつ足した和と、文字列まるごとの幅が一致する。
        let by_char: usize = line.chars().map(char_display_width).sum();
        assert_eq!(by_char, str_display_width(line));
        assert_eq!(
            str_display_width(line),
            display_width_up_to(line, line.len())
        );
        // `a`(1) + `\r`(2) + `b`(1)
        assert_eq!(by_char, 4);
    }
}
