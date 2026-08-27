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
}

impl FileEncoding {
    /// 画面で名乗る名前（`Shift_JIS` / `EUC-JP`）。UTF-8 は名乗らない ——
    /// ⭐ **既定は言わない**。言うのは「いつもと違う」ときだけ。
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Utf8 => None,
            Self::Legacy(enc) => Some(enc.name()),
        }
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
        }
    }
}

/// バイト列を読む。**往復するものだけ**を返す。
///
/// ⭐ 順に:
/// 1. UTF-8 として妥当ならそれ（今までどおり・大多数はここ）
/// 2. 候補で decode → encode し、**元のバイトと一致**したもの
/// 3. どれも通らなければ `None` ＝ **開かない**（`#3` のまま）
pub fn decode(bytes: &[u8]) -> Option<(String, FileEncoding)> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Some((text.to_string(), FileEncoding::Utf8));
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
            return Some((text.into_owned(), FileEncoding::Legacy(enc)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 実物のバイト列（Python の `cp932` / `euc_jp` で作ったものと同じ）。
    const SJIS: &[u8] = &[
        0x82, 0xb1, 0x82, 0xea, 0x82, 0xcd, 0x53, 0x4a, 0x49, 0x53, 0x0a,
    ]; // "これはSJIS\n"
    const EUCJP: &[u8] = &[0xa4, 0xb3, 0xa4, 0xec, 0xa4, 0xcf, 0x45, 0x55, 0x43, 0x0a]; // "これはEUC\n"

    #[test]
    fn utf8_stays_utf8() {
        let (text, enc) = decode("これは UTF-8\n".as_bytes()).unwrap();
        assert_eq!(enc, FileEncoding::Utf8);
        assert_eq!(text, "これは UTF-8\n");
        assert_eq!(enc.label(), None, "既定は名乗らない");
    }

    #[test]
    fn shift_jis_is_recognised_and_named() {
        let (text, enc) = decode(SJIS).expect("Shift_JIS が開けない");
        assert!(text.contains("これは"), "読めていない: {text:?}");
        assert_eq!(enc.label(), Some("Shift_JIS"));
    }

    #[test]
    fn euc_jp_is_recognised() {
        let (_, enc) = decode(EUCJP).expect("EUC-JP が開けない");
        assert_eq!(enc.label(), Some("EUC-JP"));
    }

    /// 🚨 **本丸** —— 開けたものは必ず往復する。
    #[test]
    fn whatever_opens_round_trips() {
        for bytes in [SJIS, EUCJP, "utf8 ok\n".as_bytes()] {
            let (text, enc) = decode(bytes).expect("開けるはず");
            assert_eq!(
                enc.encode(&text).as_deref(),
                Some(bytes),
                "開けたのに往復しない ＝ 保存でファイルが変わる"
            );
        }
    }

    /// 🚨 **これがこの設計の全部** —— *開けたものは、必ず往復する*。
    ///
    /// ⭐ 検体を手で選ぶのをやめ、**2 バイトの全組み合わせ（65,536 通り）**を回す。
    /// ⚠️ 手で選んだ検体だと「decode は通るが往復しない」並びを取り逃がす ——
    /// 実際に一度取り逃がし、往復検査を外した実装が緑で通った。
    #[test]
    fn anything_that_opens_round_trips_for_every_two_byte_sequence() {
        let mut opened = 0usize;
        for a in 0..=255u8 {
            for b in 0..=255u8 {
                let bytes = [a, b, 0x0a];
                if let Some((text, enc)) = decode(&bytes) {
                    opened += 1;
                    assert_eq!(
                        enc.encode(&text).as_deref(),
                        Some(&bytes[..]),
                        "開けたのに往復しない: {bytes:02x?} を {:?} として読んだ",
                        enc.label().unwrap_or("UTF-8")
                    );
                }
            }
        }
        // ⭐ 「1 つも開けていない」なら網が何も測っていない。
        assert!(opened > 1000, "開けた検体が少なすぎる: {opened}");
    }

    /// 🚨 **latin-1 系を候補に入れていないこと。**
    /// ⚠️ あれは全バイト列が妥当なので、入れると**何でも開けるが日本語が化ける**。
    /// ⭐ この検体は latin-1 としては読めるが、Shift_JIS / EUC-JP としては読めない。
    #[test]
    fn latin1_text_is_not_opened() {
        // "café naïve\n" を latin-1 で書いたもの。
        let latin1: &[u8] = &[
            0x63, 0x61, 0x66, 0xe9, 0x20, 0x6e, 0x61, 0xef, 0x76, 0x65, 0x0a,
        ];
        assert!(
            decode(latin1).is_none(),
            "latin-1 を開いている ＝ 候補に入っている（何でも開けるようになる）"
        );
    }

    /// 🚨 **陽性対照。** `#3` が緩んでいないこと —— どの候補でも往復しないバイト列は
    /// **開かない**。⭐ これが無いと「latin-1 を足して何でも開く」実装が緑で通る。
    #[test]
    fn bytes_that_do_not_round_trip_are_refused() {
        // Shift_JIS としても EUC-JP としても妥当でない並び。
        let junk: &[u8] = &[0xff, 0xfe, 0x80, 0x81, 0x0a];
        assert!(
            decode(junk).is_none(),
            "往復しないバイト列を開いている（#3 が緩んでいる）"
        );
    }

    /// ⚠️ **その符号化で書けない字は、保存を断る。**
    /// 🚨 黙って `?` に落とすと「保存できた」と言いながら中身が変わる。
    #[test]
    fn a_character_the_encoding_cannot_hold_is_refused() {
        let (_, enc) = decode(SJIS).unwrap();
        assert!(enc.encode("これはSJIS\n").is_some(), "元の字は書けるはず");
        assert!(
            enc.encode("絵文字 🐚\n").is_none(),
            "Shift_JIS に無い字を黙って落としている"
        );
    }
}
