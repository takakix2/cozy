/// 開いたファイルの**形** —— 中身ではなく、中身がどう書き下されていたか。
///
/// `TextBuffer` が持つ `Vec<String>` は「行が**何と言っていたか**」しか覚えていない。
/// **どう書かれていたか**（末尾に改行があったか、行末が LF か CRLF か、どの符号化か）は
/// 読んだ瞬間に落ち、書くときに発明される。それが `ROADMAP.md` の不変条件
/// —— *開いたファイルは、編集した分を除いてバイト単位でそのまま返る* —— を破る経路だった。
///
/// ∴ **読んだときに測って、書くときに戻す**。この型はその器で、欄は増える予定にある。
///
/// | 欄 | 状態 |
/// |---|---|
/// | `final_newline` | ✅ ここ（`#6` 段①） |
/// | `line_ending` | ✅ ここ（`#6` 段②） |
/// | 符号化・BOM | ⏸ `#4` —— `encoding_rs` と判定の設計が要る |
///
/// ⚠️ **欄を増やすときは 2 箇所を同時に直す**: 測る側（`detect`）と戻す側
/// （`file_io::write_lines`）。片方だけでは、覚えているのに戻さない／戻すのに覚えていない
/// という**どちらも静かな**壊れ方をする。
/// ファイルが行を区切っていた綴り。
///
/// ⚠️ **2 値しか無い。`Cr` は無い。** CR だけで改行する古い Mac のファイルを cozy は
/// **行末として知らない** —— `\r` は行の中のただの文字として素通りし、そのまま書き戻る。
/// ⭐ 知らないことと壊すことは別で、cozy が引き受けているのは後者だけ
/// （`ROADMAP.md`「The line cozy holds」）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix / macOS / Linux。**新規ファイルの既定**。
    #[default]
    Lf,
    /// Windows・DOS。`core.autocrlf` を使う git の作業ツリーにも現れる。
    CrLf,
}

impl LineEnding {
    /// 書き出すときの綴り。
    pub fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Lf => b"\n",
            Self::CrLf => b"\r\n",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileFormat {
    /// 最後の行が改行で終わっていたか。
    ///
    /// 🚨 これが無いと、`no final newline`（16 B）を無編集で保存して 17 B になる。
    /// git はそれを全行の差分として見せるので、*「1 行直しただけなのに diff がファイル全体」*
    /// になる（2026-08-27 に 0.2.23 で実測）。
    pub final_newline: bool,

    /// 行を区切っていた綴り。
    ///
    /// 🚨 判定は**全か無か** —— `\n` が 1 つでも `\r` を伴わなければ `Lf` になり、
    /// **残った `\r` は本文の文字として扱われる**（vim の `'fileformats'` と同じ規則）。
    /// ⭐ これで**行末が混在したファイルも 1 欄で返る**: 寄せないので、`\r` は在った場所に
    /// 在ったまま書き戻る。行ごとに覚える必要は無い。
    pub line_ending: LineEnding,
}

impl Default for FileFormat {
    /// **新規ファイルの既定**。
    ///
    /// ⚠️ `false` ではない —— 不変条件は「**開いた**ファイルを返す」であって、まだ存在しない
    /// ファイルには言うことが無い。だからここは慣習で決めてよく、Unix の慣習は
    /// **テキストファイルは改行で終わる**（cozy で書いたシェルスクリプトや `.conf` が
    /// 末尾改行なしで生まれると、連結や `read` を使う側で困る）。
    fn default() -> Self {
        Self {
            final_newline: true,
            line_ending: LineEnding::Lf,
        }
    }
}

impl FileFormat {
    /// 読んだ内容から形を測る。
    ///
    /// ⭐ **0 バイトのファイルは `false`** になる（`ends_with` が偽）。これで空ファイルは
    /// 「行 0 個・終端なし」として 0 バイトのまま返り、`\n` 1 バイトのファイルは
    /// 「空行 1 個・終端あり」として 1 バイトのまま返る。**この 2 つは `lines` だけでは
    /// 区別が付かない**（どちらも `[""]`）ので、区別しているのはこの欄である。
    pub fn detect(content: &str) -> Self {
        Self {
            final_newline: content.ends_with('\n'),
            line_ending: Self::detect_line_ending(content),
        }
    }

    /// 行末の綴りを測る。**全ての `\n` が `\r\n` のときだけ** `CrLf`。
    ///
    /// 🚨 「多数決」でも「最初に見つけた方」でもない。⭐ 全か無かにすると、混在した
    /// ファイルが `Lf` に落ちて **`\r` が本文として素通りする** ——
    /// ∴ **寄せずに済み、バイトがそのまま返る**。多数決にすると、少数派の行を
    /// 書き換えることになる（GNU nano 7.2 はそちらで、混在を 8 B → 9 B にする・実測）。
    ///
    /// ⚠️ `\n` が 1 つも無いファイル（1 行だけ・0 バイト・CR のみ）は `Lf`。
    /// 綴りを名乗る証拠が無いので、既定に落とす。
    fn detect_line_ending(content: &str) -> LineEnding {
        let bytes = content.as_bytes();
        let mut saw_any = false;
        for (i, b) in bytes.iter().enumerate() {
            if *b == b'\n' {
                saw_any = true;
                if i == 0 || bytes[i - 1] != b'\r' {
                    return LineEnding::Lf;
                }
            }
        }
        if saw_any {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_trailing_newline() {
        assert!(FileFormat::detect("a\nb\n").final_newline);
        assert!(!FileFormat::detect("a\nb").final_newline);
    }

    #[test]
    fn empty_and_one_newline_are_different_shapes() {
        // どちらも `lines()` では `[""]` に潰れる。分けているのはこの欄だけ。
        assert!(!FileFormat::detect("").final_newline);
        assert!(FileFormat::detect("\n").final_newline);
    }

    #[test]
    fn a_lone_cr_is_not_a_terminator() {
        // CR のみで改行する古い Mac のファイル。cozy は `\r` を行の区切りとして
        // 読まないので、これは「1 行・終端なし」＝ 足さずに返せば元のバイトに戻る。
        assert!(!FileFormat::detect("a\rb\rc").final_newline);
    }

    #[test]
    fn a_new_file_ends_with_a_newline() {
        assert!(FileFormat::default().final_newline);
    }
}
