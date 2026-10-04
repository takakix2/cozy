//! 4096 色 sixel エンコーダ（cozy `#20` Phase 1・🧑「エンコーダは 4096 色ディザで」）。
//!
//! # なぜ自前か
//!
//! `ratatui-image` の sixel は `icy_sixel` に `EncodeOptions::default()` 固定で、
//! `max_colors` の上限が **256**（icy_sixel 0.5.1 `encoder.rs`）。4096 レジスタの sixel は
//! このエコシステムのどこからも出せないので、ここが唯一の焼き場になる。
//! ⚠️ **受け手が 4096 レジスタを持つときだけ使うこと** —— 256 の端末に流すと
//! レジスタ番号が折り返して色が壊れる。受け手の容量は XTSMGRAPHICS で交渉する
//! （`super::negotiate_sixel_registers`）。交渉が立たなければ呼び手は
//! `ratatui-image` の既定（icy の 256 色適応・Floyd–Steinberg 入り）へ落ちる。
//!
//! # 方式 —— 16³ の均等キューブ ＋ Floyd–Steinberg
//!
//! 4096 = 16×16×16。パレットを**画像から学習しない**（均等キューブ固定）のは:
//! - 量子化パスが丸ごと消える（1 パス＝ディザしながら直接レジスタへ落ちる）
//! - 16 階調/チャネル ＋ 誤差拡散は、写真でも縞の見えない水準（4 bit + dither）
//! - 決定的 ＝ 検体がバイト列で釘付けできる
//!
//! sixel の色定義は **0–100 のパーセント**なので（255 階調ではない）、
//! 16 階調はパーセント側の解像度（101 段）に十分収まる。

use std::collections::HashMap;
use std::fmt::Write as _;

/// 1 チャネルの階調数。4096 = LEVELS³。
const LEVELS: u16 = 16;
/// 階調間の距離（255 / 15 = 17 ちょうど）。
const STEP: i32 = 17;

/// RGBA ピクセル列を 4096 色 sixel（DCS q … ST）へ焼く。
///
/// - `rgba`: 8bit RGBA・長さ `w * h * 4`。アルファは**黒に合成**する
///   （sixel に透過は無く、端末の地は暗い前提の道具なので黒が最も自然）。
/// - 出力は ASCII のみ（`ESC P 0;0;0 q` 〜 `ESC \`）。
pub(crate) fn encode(rgba: &[u8], w: u32, h: u32) -> String {
    assert_eq!(rgba.len(), (w * h * 4) as usize, "rgba length mismatch");
    let (w_us, h_us) = (w as usize, h as usize);

    // ── 1 パス目: 合成 ＋ Floyd–Steinberg で各ピクセルをレジスタ番号へ ──
    // 誤差は次行へ持ち越すので、行バッファ 2 本（現行・次行）だけ持つ。
    let mut reg_of: Vec<u16> = vec![0; w_us * h_us];
    let mut err_cur: Vec<[f32; 3]> = vec![[0.0; 3]; w_us + 2];
    let mut err_next: Vec<[f32; 3]> = vec![[0.0; 3]; w_us + 2];
    let mut used = [false; (LEVELS * LEVELS * LEVELS) as usize];

    for y in 0..h_us {
        for e in err_next.iter_mut() {
            *e = [0.0; 3];
        }
        for x in 0..w_us {
            let p = (y * w_us + x) * 4;
            let a = rgba[p + 3] as i32;
            let mut lvl = [0u16; 3];
            for (c, l) in lvl.iter_mut().enumerate() {
                // 黒への合成（a=255 で素通し）＋ 持ち越した誤差。
                let base = (rgba[p + c] as i32 * a) / 255;
                let v = base as f32 + err_cur[x + 1][c];
                // 最近傍の階調へ。丸めは「17 で割って丸め」＝ level*17 が代表値。
                let q = ((v / STEP as f32).round() as i32).clamp(0, (LEVELS - 1) as i32);
                let diff = v - (q * STEP) as f32;
                // Floyd–Steinberg: 右 7/16・左下 3/16・下 5/16・右下 1/16。
                err_cur[x + 2][c] += diff * (7.0 / 16.0);
                err_next[x][c] += diff * (3.0 / 16.0);
                err_next[x + 1][c] += diff * (5.0 / 16.0);
                err_next[x + 2][c] += diff * (1.0 / 16.0);
                *l = q as u16;
            }
            let reg = lvl[0] * LEVELS * LEVELS + lvl[1] * LEVELS + lvl[2];
            reg_of[y * w_us + x] = reg;
            used[reg as usize] = true;
        }
        std::mem::swap(&mut err_cur, &mut err_next);
    }

    // ── 2 パス目: 出力 ──
    let mut out = String::with_capacity(rgba.len() / 4);
    // P2=0（背景色の扱いは端末既定）。ラスタ属性でピクセル寸法を名乗る。
    let _ = write!(out, "\x1bP0;0;0q\"1;1;{w};{h}");

    // 使ったレジスタだけ定義（最悪 4096 本 ≈ 64KB・本文に比べれば端数）。
    for (reg, _) in used.iter().enumerate().filter(|(_, u)| **u) {
        let r = percent((reg as u16) / (LEVELS * LEVELS) % LEVELS);
        let g = percent((reg as u16) / LEVELS % LEVELS);
        let b = percent((reg as u16) % LEVELS);
        let _ = write!(out, "#{reg};2;{r};{g};{b}");
    }

    // 6 行ずつの帯。帯ごとに「色 → 列ビットマスク」を 1 パスで作り、色ごとに RLE で吐く。
    let mut masks: HashMap<u16, Vec<u8>> = HashMap::new();
    for y0 in (0..h_us).step_by(6) {
        masks.clear();
        let rows = (h_us - y0).min(6);
        for dy in 0..rows {
            let bit = 1u8 << dy;
            let row = &reg_of[(y0 + dy) * w_us..(y0 + dy) * w_us + w_us];
            for (x, &reg) in row.iter().enumerate() {
                masks.entry(reg).or_insert_with(|| vec![0u8; w_us])[x] |= bit;
            }
        }
        // 出力順を決定的に（検体がバイト列で釘付けできるように）。
        let mut regs: Vec<&u16> = masks.keys().collect();
        regs.sort_unstable();
        let last = regs.len().saturating_sub(1);
        for (i, reg) in regs.iter().enumerate() {
            let _ = write!(out, "#{reg}");
            emit_rle(&mut out, &masks[reg]);
            // 同じ帯のまま次の色へ戻るのは `$`（CR）。最後の色は `-`（LF）で次の帯へ。
            out.push(if i == last { '-' } else { '$' });
        }
    }
    out.push_str("\x1b\\");
    out
}

/// レベル（0..16）→ sixel の色パーセント（0..100）。0→0・15→100 が厳密に出る丸め。
fn percent(level: u16) -> u16 {
    ((level as u32 * STEP as u32 * 100 + 127) / 255) as u16
}

/// 列ビットマスク列を sixel 本文へ（`?`＋mask の文字・4 連続以上は `!n` で RLE）。
/// 末尾の空（mask=0）はそのまま落とす —— 次の `$`/`-` が桁を戻すので位置に影響しない。
fn emit_rle(out: &mut String, cols: &[u8]) {
    let trimmed = cols
        .iter()
        .rposition(|&m| m != 0)
        .map_or(&cols[..0], |p| &cols[..=p]);
    let mut i = 0;
    while i < trimmed.len() {
        let m = trimmed[i];
        let mut n = 1;
        while i + n < trimmed.len() && trimmed[i + n] == m {
            n += 1;
        }
        let ch = (0x3F + m) as char;
        if n >= 4 {
            let _ = write!(out, "!{n}{ch}");
        } else {
            for _ in 0..n {
                out.push(ch);
            }
        }
        i += n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(rgba: [u8; 4], w: u32, h: u32) -> Vec<u8> {
        rgba.repeat((w * h) as usize)
    }

    /// パレット定義（`#reg;2;r;g;b`）の本数。
    /// ⚠️ `";2;"` を素朴に数えないこと —— ラスタ属性 `"1;1;2;2` の中にも居る。
    fn count_defs(out: &str) -> usize {
        out.split('#')
            .skip(1)
            .filter(|part| {
                let digits = part.chars().take_while(|c| c.is_ascii_digit()).count();
                digits > 0 && part[digits..].starts_with(";2;")
            })
            .count()
    }

    /// 単色 2×2: レジスタ定義は 1 本・本文は 1 帯 1 色・包みが正しい。
    #[test]
    fn a_solid_color_uses_exactly_one_register() {
        let out = encode(&solid([255, 0, 0, 255], 2, 2), 2, 2);
        assert!(out.starts_with("\x1bP0;0;0q\"1;1;2;2"), "header: {out:?}");
        assert!(out.ends_with("\x1b\\"), "tail: {out:?}");
        // 赤 255 → level 15 → レジスタ 15*256 = 3840・パーセントは 100;0;0。
        assert_eq!(count_defs(&out), 1, "palette defs: {out:?}");
        assert!(out.contains("#3840;2;100;0;0"), "red def: {out:?}");
        // 2×2 の上 2 行 = mask 0b11 = 0x3F+3 = 'B'。
        assert!(out.contains("BB"), "body: {out:?}");
    }

    /// 端のパーセントは厳密（0→0・255→100）—— 丸めで白が 99 に欠けると全体が暗く転ぶ。
    #[test]
    fn percent_is_exact_at_both_ends() {
        assert_eq!(percent(0), 0);
        assert_eq!(percent(15), 100);
    }

    /// 50% グレー（128）は階調の間（119 と 136 の間）に居るので、
    /// ディザが効いていれば**複数のレジスタ**に散る。1 本しか使っていなければ
    /// 誤差拡散が死んでいる。
    #[test]
    fn dithering_spreads_a_between_levels_gray() {
        let out = encode(&solid([128, 128, 128, 255], 16, 16), 16, 16);
        assert!(
            count_defs(&out) >= 2,
            "flat 128 gray must dither into 2+ registers: {out:?}"
        );
    }

    /// 出力は 7bit ASCII だけ（エスケープ 2 箇所を除く）—— 8bit が混ざると
    /// UTF-8 の層（TauriWriter の carry）が化かす余地が生まれる。
    #[test]
    fn output_is_seven_bit_ascii() {
        let mut img = Vec::new();
        for i in 0..64u32 {
            img.extend_from_slice(&[(i * 4) as u8, 255 - (i * 4) as u8, (i * 2) as u8, 255]);
        }
        let out = encode(&img, 8, 8);
        assert!(
            out.bytes().all(|b| b == 0x1b || (0x20..0x7f).contains(&b)),
            "non-ascii byte in sixel output"
        );
    }

    /// レジスタ番号は 4096 を踏み越えない。
    #[test]
    fn registers_stay_below_4096() {
        let mut img = Vec::new();
        for i in 0..256u32 {
            img.extend_from_slice(&[
                i as u8,
                (i * 7) as u8,
                (255 - i) as u8,
                (i + 64).min(255) as u8,
            ]);
        }
        let out = encode(&img, 16, 16);
        for part in out.split('#').skip(1) {
            let n: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            let reg: u32 = n.parse().expect("register number");
            assert!(reg < 4096, "register {reg} out of range");
        }
    }

    /// 透明ピクセルは黒に合成される（アルファ 0 の白 ＝ 黒・レジスタ 0）。
    #[test]
    fn transparent_pixels_composite_onto_black() {
        let out = encode(&solid([255, 255, 255, 0], 2, 2), 2, 2);
        assert!(out.contains("#0;2;0;0;0"), "transparent → black: {out:?}");
        assert_eq!(count_defs(&out), 1);
    }

    /// RLE: 同じマスクが 4 列以上続けば `!n` に畳まれる。
    #[test]
    fn long_runs_are_rle_compressed() {
        let out = encode(&solid([0, 255, 0, 255], 100, 6), 100, 6);
        assert!(out.contains("!100"), "run of 100 must compress: {out:?}");
    }

    // ── ラウンドトリップ（別実装のデコーダで突き合わせる）─────────────────────
    //
    // 🚨 **帯 / `$`(CR) / `-`(LF) / RLE は手書き**なので、構造が valid でも**意味**（どの色が
    // どこに出るか・寸法）がズレうる。icy_sixel のデコーダ（ratatui-image が使っているのと
    // 同じ実装・ここでは逆向きに使う）で復号し、原画と比べる。
    // ⭐ 2 実装が一致しても「同じ間違い」の危険は残るが、エンコーダとデコーダは別物なので、
    //   少なくとも「自分で書いた帯組みが自分で読み返せる」より強い（公開ベクタではないが
    //   独立実装との一致）。

    fn decode(sixel: &str) -> (Vec<u8>, usize, usize) {
        let img = icy_sixel::SixelImage::decode(sixel.as_bytes()).expect("decode");
        (img.pixels, img.width, img.height)
    }

    /// 寸法が往復で保たれる（ラスタ属性 `"1;1;W;H` が効いている）。
    #[test]
    fn roundtrip_preserves_dimensions() {
        let (w, h) = (24, 18);
        let out = encode(&solid([40, 120, 200, 255], w, h), w, h);
        let (_, dw, dh) = decode(&out);
        assert_eq!((dw, dh), (w as usize, h as usize), "dims drift: {dw}x{dh}");
    }

    /// 単色（階調グリッド上の色）は往復でほぼ同じ色になる —— 帯・CR・LF の組み方が
    /// 1 つでも狂えば、色が別のセルに散るか寸法がずれてここが落ちる。
    #[test]
    fn roundtrip_of_a_solid_is_near_identical() {
        let (w, h) = (16, 12);
        // 51 = level 3（51/17）・153 = level 9・204 = level 12。全部グリッド上。
        let out = encode(&solid([51, 153, 204, 255], w, h), w, h);
        let (px, dw, dh) = decode(&out);
        assert_eq!((dw, dh), (16, 12));
        for p in px.as_chunks::<4>().0.iter() {
            // グリッド上の色なので誤差拡散は 0・パーセント丸めの ±3/255 だけ許す。
            assert!((p[0] as i32 - 51).abs() <= 3, "R off: {}", p[0]);
            assert!((p[1] as i32 - 153).abs() <= 3, "G off: {}", p[1]);
            assert!((p[2] as i32 - 204).abs() <= 3, "B off: {}", p[2]);
        }
    }

    /// 左半分=赤・右半分=青が、往復後も左右に分かれて出る（帯内の桁位置＝RLE と `$`/`-`
    /// の整合）。⭐ ここが「どの色がどこに」を直接釘付けする一点。
    #[test]
    fn roundtrip_keeps_left_right_split() {
        let (w, h) = (16usize, 12usize);
        let mut img = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let p = (y * w + x) * 4;
                let c = if x < w / 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
                img[p..p + 4].copy_from_slice(&c);
            }
        }
        let out = encode(&img, w as u32, h as u32);
        let (px, dw, dh) = decode(&out);
        assert_eq!((dw, dh), (w, h));
        let at = |x: usize, y: usize| -> [u8; 3] {
            let p = (y * dw + x) * 4;
            [px[p], px[p + 1], px[p + 2]]
        };
        // 左端は赤優勢・右端は青優勢。
        let l = at(1, 6);
        let r = at(w - 2, 6);
        assert!(l[0] > 200 && l[2] < 60, "left not red: {l:?}");
        assert!(r[2] > 200 && r[0] < 60, "right not blue: {r:?}");
    }
}
