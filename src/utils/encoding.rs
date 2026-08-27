//! ファイルの**符号化**を、推測ではなく**検査**で決める。
//!
//! 🚨 検出の難しさは「Shift_JIS と EUC-JP と latin-1 は、同じバイト列が全部妥当に
//! なりうる」ことにある。∴ 当てにいくと、外したときに **`#3` で塞いだ穴
//! （間違った解釈で開いて保存し、壊す）が別の入口から戻る**。
//!
//! ⭐ **往復で確かめれば、当たっているかを測れる** —— 候補で decode し、同じ候補で
//! encode し直して、**元のバイトと一致するか**を見る。一致したものだけを採る。
//! ∴ 開けたファイルは、**保存しても元のバイトに戻ることが保証されている**。
//!
//! ⚠️ **表示が正しいことまでは保証しない。** 短いバイト列は複数の候補で往復しうる
//! （順序で決まる）。⭐ しかしその場合も**バイトは返る**ので、`ROADMAP.md` の
//! 不変条件は破れない。何と解釈したかは画面で名乗る。

use encoding_rs::Encoding;

/// UTF-8 で読めなかったときに試す順。
///
/// 🚨 **latin-1 は入れない。** あれは**全バイト列が妥当**（バイトと文字の全単射）なので、
/// 入れた瞬間に**何でも往復してしまい**、日本語のファイルが化けたまま開く。
/// ⚠️ ロケールの無い vim がまさにそうなる —— 壊しはしないが、読めない。
/// ⭐ cozy は `#3` で「返せない形は開かない」を選んでいるので、**化けたまま開かない**側に倒す。
///
/// 📌 候補を増やすのは後から安全 —— 往復検査を通らないものは採用されないので、
/// 「増やしたら誤検出が増える」という形にならない。
const CANDIDATES: &[&Encoding] = &[encoding_rs::SHIFT_JIS, encoding_rs::EUC_JP];

/// ファイルの中身をどう読んだか。**保存でそのまま戻す**。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileEncoding {
    /// 既定。新規ファイルもこれ。
    #[default]
    Utf8,
    /// UTF-8 では読めず、往復検査を通った符号化。
    Legacy(&'static Encoding),
    /// **読み方が決まらなかった** —— バイナリか、cozy の知らない符号化。
    ///
    /// ⭐ latin-1 として見せる（**1 バイト = 1 文字**）。`ui::render::body` が制御文字を
    /// `^X` に変えるので、**画面の 1 桁が原文の 1 バイト**に対応する ——
    /// バイナリを覗く道具として、これが正しい見え方。
    /// 🚨 保存は**拒む**（`file_io::write_buffer`）。∴ 開けても壊しようがない。
    ViewOnly,
}

impl FileEncoding {
    /// 画面で名乗る名前（`Shift_JIS` / `EUC-JP`）。UTF-8 は名乗らない ——
    /// ⭐ **既定は言わない**。言うのは「いつもと違う」ときだけ。
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Utf8 => None,
            Self::Legacy(enc) => Some(enc.name()),
            // ⚠️ **理由（バイナリ / 未知の符号化）は言わない。** 状態行は狭く、
            // 利用者が知りたいのは**何ができないか**。⭐ なぜかは Help と CHANGELOG に。
            Self::ViewOnly => Some("view only"),
        }
    }

    /// このファイルは保存してよいか。
    ///
    /// 🚨 `ViewOnly` は**読み方が確定していない** ＝ 書き戻す先が無い。
    /// ⭐ `#3` が守っていたのは「読めないファイルを**開かない**」ではなく
    /// 「読めないファイルを**保存させない**」の方だった。∴ 開く方は許し、ここで止める。
    pub fn is_writable(self) -> bool {
        !matches!(self, Self::ViewOnly)
    }

    /// バッファの文字列を、読んだときと同じ符号化のバイト列へ戻す。
    ///
    /// ⚠️ 戻せない文字（その符号化に無い字）が入っていたら `None`。
    /// 🚨 **黙って `?` に落とさない** —— それは「保存できた」と言いながら
    /// 中身を変えることになる（`#3` が塞いだのと同じ形）。
    pub fn encode(self, text: &str) -> Option<Vec<u8>> {
        match self {
            Self::Utf8 => Some(text.as_bytes().to_vec()),
            Self::Legacy(enc) => {
                let (bytes, _, had_errors) = enc.encode(text);
                if had_errors {
                    None
                } else {
                    Some(bytes.into_owned())
                }
            }
            // 🚨 **書かない。** 呼び出し側が `is_writable` で先に止めるが、
            // ここでも `None` を返す —— 止め忘れても壊れない側に倒す。
            Self::ViewOnly => None,
        }
    }
}

/// バイト列を読む。**往復するものだけ**を返す。
///
/// ⭐ 順に:
/// 1. UTF-8 として妥当ならそれ（今までどおり・大多数はここ）
/// 2. 候補で decode → encode し、**元のバイトと一致**したもの
/// 3. どれも通らなければ `None` ＝ **開かない**（`#3` のまま）
pub fn decode(bytes: &[u8]) -> (String, FileEncoding) {
    // 🚨 **NUL があればテキストではない。** ⭐ `grep` や `git` がバイナリを判定するのに
    // 使う線と同じ —— **利用者が既に知っている**線を使う。
    // ⚠️ 往復するかどうかは利用者から見えないので、そちらを表の判定にはしない
    // （PNG の magic は Shift_JIS として往復してしまい、`臼NG` と化けて開けていた）。
    if !bytes.contains(&0) {
        if let Ok(text) = std::str::from_utf8(bytes) {
            return (text.to_string(), FileEncoding::Utf8);
        }
        for enc in CANDIDATES {
            let (text, _, had_errors) = enc.decode(bytes);
            if had_errors {
                continue;
            }
            // 🚨 **ここが判定**。decode が通っただけでは足りない —— 戻して同じバイトに
            // ならなければ、保存でファイルが変わる。
            let (round, _, enc_errors) = enc.encode(&text);
            if !enc_errors && round.as_ref() == bytes {
                return (text.into_owned(), FileEncoding::Legacy(enc));
            }
        }
    }
    // ⭐ **ここに来たものも開く。** latin-1 は全バイト列が妥当なので、
    // **必ず読める**（1 バイト = 1 文字）。⚠️ 保存は `is_writable` が止める。
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    (text.into_owned(), FileEncoding::ViewOnly)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SJIS: &[u8] = &[
        0x82, 0xb1, 0x82, 0xea, 0x82, 0xcd, 0x53, 0x4a, 0x49, 0x53, 0x0a,
    ]; // "これはSJIS\n"
    const EUCJP: &[u8] = &[0xa4, 0xb3, 0xa4, 0xec, 0xa4, 0xcf, 0x45, 0x55, 0x43, 0x0a]; // "これはEUC\n"
    /// PNG の magic。⭐ **NUL を含み、かつ Shift_JIS として往復してしまう** ——
    /// `#10` の前はこれが `臼NG` と化けて**編集可能なテキストとして**開けていた。
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
    ];
    /// latin-1 の "café naïve"。NUL は無いが、どの候補でも往復しない。
    const LATIN1: &[u8] = &[
        0x63, 0x61, 0x66, 0xe9, 0x20, 0x6e, 0x61, 0xef, 0x76, 0x65, 0x0a,
    ];

    #[test]
    fn utf8_stays_utf8() {
        let (text, enc) = decode("これは UTF-8\n".as_bytes());
        assert_eq!(enc, FileEncoding::Utf8);
        assert_eq!(text, "これは UTF-8\n");
        assert_eq!(enc.label(), None, "既定は名乗らない");
        assert!(enc.is_writable());
    }

    #[test]
    fn legacy_encodings_are_recognised_and_writable() {
        for (bytes, want) in [(SJIS, "Shift_JIS"), (EUCJP, "EUC-JP")] {
            let (text, enc) = decode(bytes);
            assert_eq!(enc.label(), Some(want));
            assert!(enc.is_writable(), "{want} は編集できるべき");
            assert_eq!(
                enc.encode(&text).as_deref(),
                Some(bytes),
                "{want}: 往復しない"
            );
        }
    }

    /// 🚨 **本丸** —— **何でも開く。ただし読み方が決まらないものは書かない。**
    #[test]
    fn everything_opens_but_only_some_of_it_is_writable() {
        for (name, bytes, writable) in [
            ("utf8", "ok\n".as_bytes(), true),
            ("sjis", SJIS, true),
            ("eucjp", EUCJP, true),
            ("png", PNG, false),
            ("latin1", LATIN1, false),
        ] {
            let (text, enc) = decode(bytes);
            assert!(!text.is_empty(), "{name}: 開けていない");
            assert_eq!(enc.is_writable(), writable, "{name}: 書ける/書けないが逆");
        }
    }

    /// 🚨 **NUL を含むものは、往復しても view only。**
    /// ⚠️ PNG の magic は Shift_JIS として往復する ——
    /// `#10` の前はそれで**編集可能なテキストとして開けていた**（`臼NG` と化けて）。
    /// ⭐ 往復するかどうかは利用者から見えないので、判定の表には出さない。
    #[test]
    fn a_nul_byte_wins_over_a_successful_round_trip() {
        let (_, sjis_view) = decode(PNG);
        assert_eq!(
            sjis_view,
            FileEncoding::ViewOnly,
            "NUL があるのに編集可にした"
        );
        // 対照 —— NUL を抜くと Shift_JIS として開ける（往復自体は成立している）。
        let without_nul: Vec<u8> = PNG.iter().copied().filter(|b| *b != 0).collect();
        let (_, enc) = decode(&without_nul);
        assert_eq!(
            enc.label(),
            Some("Shift_JIS"),
            "NUL 以外の理由で view only になっている ＝ 線引きが別物"
        );
    }

    /// ⭐ view only は **1 バイト = 1 文字**で見せる。
    /// これが崩れると、バイナリを覗く道具として意味が無い。
    #[test]
    fn view_only_shows_one_character_per_byte() {
        let (text, enc) = decode(PNG);
        assert_eq!(enc, FileEncoding::ViewOnly);
        assert_eq!(
            text.chars().count(),
            PNG.len(),
            "バイト数と文字数が合わない ＝ 画面の桁と原文のバイトが対応しない"
        );
    }

    /// 🚨 **view only は書かない。** 呼び出し側が止め忘れても壊れない側に倒す。
    #[test]
    fn view_only_refuses_to_encode() {
        let (text, enc) = decode(PNG);
        assert!(!enc.is_writable());
        assert!(enc.encode(&text).is_none(), "view only なのに書けてしまう");
    }

    /// 🚨 **開けて編集できるものは、必ず往復する。**
    /// ⭐ 2 バイトの全組み合わせを回す —— 手で選んだ検体では
    /// 「decode は通るが往復しない」並びを取り逃がす（実際に一度取り逃がした）。
    #[test]
    fn anything_writable_round_trips_for_every_two_byte_sequence() {
        let mut writable = 0usize;
        for a in 0..=255u8 {
            for b in 0..=255u8 {
                let bytes = [a, b, 0x0a];
                let (text, enc) = decode(&bytes);
                if enc.is_writable() {
                    writable += 1;
                    assert_eq!(
                        enc.encode(&text).as_deref(),
                        Some(&bytes[..]),
                        "編集可なのに往復しない: {bytes:02x?}"
                    );
                }
            }
        }
        assert!(writable > 1000, "編集可の検体が少なすぎる: {writable}");
    }

    /// 🚨 **latin-1 は候補に入れない。** 入れると何でも「編集可」になり、
    /// 日本語が化けたまま保存できてしまう。⭐ view only で見せるのとは別の話。
    #[test]
    fn latin1_is_viewable_but_not_editable() {
        let (text, enc) = decode(LATIN1);
        assert_eq!(enc, FileEncoding::ViewOnly, "latin-1 を編集可にしている");
        assert!(text.contains("caf"), "見せられていない: {text:?}");
    }

    #[test]
    fn a_character_the_encoding_cannot_hold_is_refused() {
        let (_, enc) = decode(SJIS);
        assert!(enc.encode("これはSJIS\n").is_some(), "元の字は書けるはず");
        assert!(
            enc.encode("絵文字 🐚\n").is_none(),
            "Shift_JIS に無い字を黙って落としている"
        );
    }
}
