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
/// タブ幅。端末の既定と同じ 8。
///
/// 📌 **cozy 自身が空白へ展開する**ので、この数は cozy が決めてよい ——
/// 端末に TAB バイトを渡さないから、端末の設定と食い違いようがない。
pub const TAB_WIDTH: usize = 8;

/// 桁 `col`（**論理行の先頭から 0 始まり**）に置かれた 1 文字が占める桁数。
///
/// 🚨 **TAB は位置で幅が変わる。** ∴ 幅は「1 文字」では決まらず、
/// **そこまでに何桁使ったか**を知らなければ答えられない。
/// ⚠️ これが `char_display_width(ch)` を消した理由 —— 1 文字だけ渡せる形を残すと、
/// TAB を渡されたときに**黙って嘘の幅**を返す。
pub fn char_display_width_at(ch: char, col: usize) -> usize {
    // ⭐ 次のタブストップまで。`col` が既にタブストップ上なら丸ごと 1 つ分。
    if ch == '\t' {
        return TAB_WIDTH - (col % TAB_WIDTH);
    }
    // ⭐ **描く側と同じ関数に訊く。** `^X` として描かれる字だけが 2 桁。
    // ここを `is_control()` で判定すると、`^` 表記を持たない字まで 2 桁になり、
    // **幅だけがずれる**（描画は 1 文字のまま）。
    if caret_notation(ch).is_some() {
        return 2;
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
pub fn visible_char_at(ch: char, col: usize) -> String {
    // 🚨 **TAB も端末に渡さない。** 渡すと端末が自分のタブストップで桁を進め、
    // cozy が数えた桁と食い違う —— `/etc/hosts` の 12 行目で **7 桁**ずれた
    // （行番号の欄があるぶん、端末の絶対桁と行内の桁が別物になるため）。
    // ⭐ 自分で空白に展開すれば、桁は cozy が決めるので**定義上ずれない**。
    if ch == '\t' {
        return " ".repeat(char_display_width_at('\t', col));
    }
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

/// 文字列が画面で占める桁数。**行頭から数える**。
///
/// ⚠️ TAB があるので `map(..).sum()` にはできない —— 各文字の幅が
/// **それまでの桁**に依存する。
pub fn str_display_width(s: &str) -> usize {
    width_from(s, 0)
}

/// 桁 `start_col` から書き始めたときに、`s` が使う桁数。
///
/// ⭐ 折り返した先や、行の途中から測るときに使う。TAB のタブストップは
/// **論理行の先頭**から数えるので、そこまでの桁を渡す必要がある。
pub fn width_from(s: &str, start_col: usize) -> usize {
    let mut col = start_col;
    for ch in s.chars() {
        col += char_display_width_at(ch, col);
    }
    col - start_col
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
            let drawn = visible_char_at(ch, 0);
            let drawn_cols: usize = drawn
                .chars()
                .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                .sum();
            assert_eq!(
                drawn_cols,
                char_display_width_at(ch, 0),
                "U+{code:04X}: 描いた字は {drawn_cols} 桁なのに、幅は {} と数えている",
                char_display_width_at(ch, 0)
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
        assert_eq!(visible_char_at('\r', 0), "^M");
        assert_eq!(visible_char_at('\u{1b}', 0), "^[");
        assert_eq!(visible_char_at('\u{7f}', 0), "^?");
    }

    /// 陽性対照 —— 普通の字は**何も変えない**。
    /// ⭐ これが無いと「全部 `^X` にする」実装が緑で通る。
    #[test]
    fn ordinary_chars_pass_through_untouched() {
        assert_eq!(visible_char_at('a', 0), "a");
        assert_eq!(visible_char_at('あ', 0), "あ");
        assert_eq!(caret_notation('a'), None);
        assert_eq!(
            caret_notation('\t'),
            None,
            "TAB は `^I` ではなく空白へ展開する"
        );
    }

    #[test]
    fn control_chars_are_two_columns() {
        // `^M` / `^[` / `^H` として描かれるので 2 桁。
        assert_eq!(char_display_width_at('\r', 0), 2);
        assert_eq!(char_display_width_at('\u{1b}', 0), 2);
        assert_eq!(char_display_width_at('\u{8}', 0), 2);
        assert_eq!(char_display_width_at('\u{7f}', 0), 2); // DEL → `^?`
    }

    /// 🚨 **TAB の幅は位置で変わる** —— これを 1 文字だけで答えようとしたのが
    /// `/etc/hosts` のずれの原因だった。
    #[test]
    fn a_tab_reaches_the_next_stop() {
        assert_eq!(
            char_display_width_at('\t', 0),
            8,
            "タブストップ上なら丸ごと 1 つ分"
        );
        assert_eq!(char_display_width_at('\t', 1), 7);
        assert_eq!(
            char_display_width_at('\t', 7),
            1,
            "次の桁がちょうどタブストップ"
        );
        assert_eq!(char_display_width_at('\t', 8), 8);
        assert_eq!(char_display_width_at('\t', 15), 1);
    }

    /// ⭐ **展開した空白の数と、数えた幅が一致すること。**
    /// 🚨 これが割れると、端末に渡さないようにした意味が無くなる。
    #[test]
    fn a_tab_is_drawn_as_exactly_as_many_spaces_as_it_is_wide() {
        for col in 0..24 {
            let drawn = visible_char_at('\t', col);
            assert!(
                drawn.chars().all(|c| c == ' '),
                "col={col}: TAB が空白以外で描かれている: {drawn:?}"
            );
            assert_eq!(
                drawn.chars().count(),
                char_display_width_at('\t', col),
                "col={col}: 描いた空白の数と幅が食い違う"
            );
        }
    }

    /// 📏 **報告された `/etc/hosts` の形をそのまま固定する。**
    /// 行番号の欄は含めない（タブストップは論理行の先頭から数える）。
    #[test]
    fn the_etc_hosts_shape_lines_up() {
        // `255.255.255.255` は 15 桁 → 次のタブストップは 16 → TAB は 1 桁。
        assert_eq!(char_display_width_at('\t', 15), 1);
        assert_eq!(
            str_display_width("255.255.255.255\tbroadcasthost"),
            15 + 1 + 13
        );
        // `127.0.0.1` は 9 桁 → 次は 16 → TAB は 7 桁。
        assert_eq!(str_display_width("127.0.0.1\tlocalhost"), 9 + 7 + 9);
    }

    #[test]
    fn ordinary_and_wide_chars_are_unchanged() {
        assert_eq!(char_display_width_at('a', 0), 1);
        assert_eq!(char_display_width_at('あ', 0), 2);
        assert_eq!(str_display_width("abあ"), 4);
    }

    /// 🚨 **これが割れていたのが元の不具合。** 折り返しに使う数え方と、
    /// カーソルに使う数え方が、同じ行に対して同じ値を返すこと。
    #[test]
    fn one_rule_for_every_counter() {
        let line = "a\rb";
        // 1 文字ずつ桁を進めた和と、文字列まるごとの幅が一致する。
        let mut col = 0usize;
        for ch in line.chars() {
            col += char_display_width_at(ch, col);
        }
        let by_char = col;
        assert_eq!(by_char, str_display_width(line));
        assert_eq!(
            str_display_width(line),
            display_width_up_to(line, line.len())
        );
        // `a`(1) + `\r`(2) + `b`(1)
        assert_eq!(by_char, 4);
    }
}
