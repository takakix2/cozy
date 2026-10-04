//! Markdown プレビュー内のインライン画像（cozy `#20` Phase 2）。
//!
//! フルスクリーンの [`super`] ビューアと**同じ描画の芯**（`ratatui-image` の Picker ／
//! sixel は 4096 交渉つき）を使うが、置き場が違う: スクロールする仮想文書の中の、
//! 予約した行範囲へブリットする。
//!
//! # 端末の能力は「起動時に 1 度」だけ測る
//!
//! Picker は端末へ問い合わせる（raw モードと stdin が要る）。プレビューは**描画ループの
//! 最中**に呼ばれるので、そこで問い合わせると画面が壊れる。∴ CLI 起動時（cozy が端末を
//! 握った直後）に [`capture_caps`] で 1 度測って仕舞う。⚠️ **埋め込みホスト（argotty）は
//! cozy が端末を握らない**ので caps は空のまま ＝ 画像は出さず従来の文字フォールバックに
//! 落ちる（Phase 2 は CLI の面だけ・argotty 側は `#110` の addon-image が別途受ける）。
//!
//! # スクロールと端 —— 「丸ごと見えている画像だけ」出す（v1）
//!
//! 行ベースのプロトコル（sixel/kitty）は 1 セルに逃がして自分のピクセル高を占める。
//! ビューポートの上端で**途中から**描くのは、プロトコルの素直な機能では出せない。
//! ∴ v1 は: **高さは常に予約**（下の文字が跳ねない）し、画像の予約範囲が**丸ごと**
//! ビューポートに入っているときだけブリットする。はみ出す間は予約行を空ける。
//! ⏭ 端のクリップは follow-up（`#20` のコメント参照）。

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::{DynamicImage, GenericImageView, ImageReader, RgbaImage};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    widgets::Widget,
};
use ratatui_image::{
    CropOptions, Image as ImageWidget, Resize,
    picker::{Picker, ProtocolType},
    protocol::Protocol,
};

/// 起動時に測った端末の能力。
struct Caps {
    picker: Picker,
    /// sixel の受け手が 4096 レジスタを認めたか。
    sixel_hi: bool,
}

/// どちらの経路で描くか。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lane {
    /// 自前 4096 色 sixel（切り出した RGBA を自分でエンコード）。
    Hi,
    /// `ratatui-image` のプロトコル（Kitty / iTerm2 / 256-sixel）。クロップはライブラリの
    /// `Resize::Crop` に任せる。
    Lib,
}

/// デコード＋幅フィットまで済ませた画像（重い所・(path, 幅) ごとに 1 度）。
#[derive(Clone)]
struct Fitted {
    image: DynamicImage,
    /// 画像が占めるセル数（予約行数 = `full_cells.height`）。
    full_cells: Size,
    lane: Lane,
    font_h: u32,
}

/// `(解決済みパス, セル幅)`。
type FitKey = (PathBuf, u16);
/// `(パス, 幅, 上に隠す行, 描く行)`。
type SliceKey = (PathBuf, u16, u16, u16);

thread_local! {
    static CAPS: RefCell<Option<Caps>> = const { RefCell::new(None) };
    /// フィット済み画像。デコード＋縮小は 1 度だけ。
    static FITTED: RefCell<HashMap<FitKey, Option<Fitted>>> = RefCell::new(HashMap::new());
    /// 切り出して描く中身。スクロールで同じスライスを何度も描いても再エンコードしない覚え書き。
    static RENDER: RefCell<HashMap<SliceKey, Option<Content>>> = RefCell::new(HashMap::new());
}

#[derive(Clone)]
enum Content {
    Proto(Protocol),
    Escape(String),
}

/// CLI 起動時に 1 度呼ぶ（cozy が raw モードに入った直後）。端末へ問い合わせて能力を仕舞う。
/// ⚠️ 失敗（問い合わせに答えない端末・埋め込みホスト）なら caps は空のまま ＝ 画像は出ない。
pub(crate) fn capture_caps() {
    let Ok(picker) = Picker::from_query_stdio() else {
        return;
    };
    let sixel_hi = picker.protocol_type() == ProtocolType::Sixel
        && super::negotiate_sixel_registers().is_some_and(|n| n >= 4096);
    CAPS.with(|c| *c.borrow_mut() = Some(Caps { picker, sixel_hi }));
}

/// インライン画像が使えるか（caps が測れている ＝ CLI で端末を握っている）。
pub(crate) fn available() -> bool {
    CAPS.with(|c| c.borrow().is_some())
}

/// 画像を用意して**予約すべき行数**を返す。使えない／読めない／デコード不能なら `None`
/// （呼び手は文字フォールバックのまま）。`max_rows` でプレビューに対して大きすぎる画像を抑える。
/// 🚨 **予約行数は view より必ず低くする**（max_rows は呼び手が view の 8 割で渡す）——
/// こうすると画像は常にビューポートより低く、スクロールで**同時に切れる端は片方だけ**になる。
pub(crate) fn reserve_rows(path: &Path, cols: u16, max_rows: u16) -> Option<usize> {
    if cols == 0 || !available() {
        return None;
    }
    let key = (path.to_path_buf(), cols);
    let fitted = FITTED.with(|c| {
        c.borrow_mut()
            .entry(key)
            .or_insert_with(|| fit(path, cols, max_rows))
            .clone()
    });
    fitted.map(|f| f.full_cells.height as usize)
}

/// 画像の見えている縦スライスだけを `rect` へ描く（`reserve_rows` が `Some` を返したパスに）。
///
/// - `skip`: 画像の上から**ビューポートの外に隠れている**行数（0 なら上端は見えている）。
/// - `draw`: 実際に描く行数（`rect.height` と同じ）。
///
/// 画像は view より低いので、切れるのは上下どちらか一方だけ:
/// `skip > 0` ＝ 上が隠れている → 下側を残す（下アンカー）。`skip == 0` ＝ 上は見えていて
/// 下がはみ出す（または全見え）→ 上側を残す（上アンカー）。
pub(crate) fn blit(path: &Path, cols: u16, skip: u16, draw: u16, rect: Rect, buf: &mut Buffer) {
    if draw == 0 || rect.width == 0 {
        return;
    }
    let rkey = (path.to_path_buf(), cols, skip, draw);
    let content = RENDER.with(|c| {
        c.borrow_mut()
            .entry(rkey)
            .or_insert_with(|| render_slice(path, cols, skip, draw))
            .clone()
    });
    let Some(content) = content else { return };
    let place = Rect::new(rect.x, rect.y, rect.width, draw.min(rect.height));
    match &content {
        Content::Proto(proto) => ImageWidget::new(proto).render(place, buf),
        Content::Escape(data) => super::render_escape(data, place, buf),
    }
}

/// `(skip, draw)` のスライスを切り出して描く中身を作る（RENDER キャッシュの中身）。
fn render_slice(path: &Path, cols: u16, skip: u16, draw: u16) -> Option<Content> {
    let key = (path.to_path_buf(), cols);
    let fitted = FITTED.with(|c| c.borrow().get(&key).cloned().flatten())?;
    let clip_top = skip > 0;
    let draw_cells = Size::new(fitted.full_cells.width, draw);

    match fitted.lane {
        Lane::Hi => {
            // 切り出す画素範囲。上が隠れているなら下 `draw` 行、そうでなければ上 `draw` 行。
            let total_h = fitted.image.height();
            let w = fitted.image.width();
            let draw_px = (draw as u32 * fitted.font_h).min(total_h);
            let y0 = if clip_top {
                total_h.saturating_sub(draw_px)
            } else {
                0
            };
            let cropped = fitted.image.crop_imm(0, y0, w, draw_px);
            let rgba = cropped.to_rgba8();
            Some(Content::Escape(super::sixel4096::encode(
                rgba.as_raw(),
                w,
                draw_px,
            )))
        }
        Lane::Lib => {
            // ⭐ クロップはライブラリに任せる（`Resize::Crop` が上/下アンカーで切る）。
            let opts = CropOptions {
                clip_top,
                clip_left: false,
            };
            let proto = CAPS.with(|c| {
                c.borrow_mut().as_mut().and_then(|caps| {
                    caps.picker
                        .new_protocol(fitted.image.clone(), draw_cells, Resize::Crop(Some(opts)))
                        .ok()
                })
            })?;
            Some(Content::Proto(proto))
        }
    }
}

/// デコード → 幅に合わせて縮小（重い所はここだけ・(path, 幅) ごとに 1 度）。
fn fit(path: &Path, cols: u16, max_rows: u16) -> Option<Fitted> {
    let caps_cell = CAPS.with(|c| {
        c.borrow().as_ref().map(|caps| {
            (
                caps.picker.font_size(),
                caps.sixel_hi,
                caps.picker.protocol_type(),
            )
        })
    });
    let (font, sixel_hi, proto_type) = caps_cell?;
    let font_w = (font.width as u32).max(1);
    let font_h = (font.height as u32).max(1);

    let image = decode(path)?;
    let (ow, oh) = image.dimensions();
    if ow == 0 || oh == 0 {
        return None;
    }

    // 幅はプレビュー幅いっぱいまで・高さは max_rows まで。両方に収める倍率。
    let max_px_w = cols as u32 * font_w;
    let max_px_h = max_rows as u32 * font_h;
    let scale = (max_px_w as f64 / ow as f64)
        .min(max_px_h as f64 / oh as f64)
        .min(1.0);
    let tw = ((ow as f64 * scale).round() as u32).max(1);
    let th = ((oh as f64 * scale).round() as u32).max(1);
    let rgba = super::resize_rgba(&image, tw, th);
    let fitted_image = DynamicImage::ImageRgba8(RgbaImage::from_raw(tw, th, rgba)?);

    let full_cells = Size::new(
        (tw.div_ceil(font_w) as u16).min(cols),
        (th.div_ceil(font_h) as u16).min(max_rows),
    );
    let lane = if sixel_hi && proto_type == ProtocolType::Sixel {
        Lane::Hi
    } else {
        Lane::Lib
    };
    Some(Fitted {
        image: fitted_image,
        full_cells,
        lane,
        font_h,
    })
}

fn decode(path: &Path) -> Option<DynamicImage> {
    // マジックバイトで絵だけ受ける（壊れたファイル・テキストを掴まない）。
    let mut head = [0u8; 16];
    let n = std::fs::File::open(path)
        .and_then(|mut f| std::io::Read::read(&mut f, &mut head))
        .ok()?;
    super::sniff_image(&head[..n])?;
    ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()
}

/// Markdown 内の画像パスを**その .md のディレクトリ基準**で解決する。
/// 絶対パス・`~` はそのまま（`~` は展開）。URL（`http(s)://`）は `None`（ネットは引かない）。
pub(crate) fn resolve(doc_dir: Option<&Path>, src: &str) -> Option<PathBuf> {
    if src.starts_with("http://") || src.starts_with("https://") {
        return None;
    }
    let expanded = crate::file_io::expand_tilde(src);
    if expanded.is_absolute() {
        return Some(expanded);
    }
    doc_dir.map(|d| d.join(expanded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_is_relative_to_the_doc_dir() {
        let doc = Path::new("/home/u/notes");
        assert_eq!(
            resolve(Some(doc), "img/a.png"),
            Some(PathBuf::from("/home/u/notes/img/a.png"))
        );
    }

    #[test]
    fn absolute_paths_pass_through() {
        assert_eq!(
            resolve(Some(Path::new("/home/u/notes")), "/tmp/x.png"),
            Some(PathBuf::from("/tmp/x.png"))
        );
    }

    #[test]
    fn urls_are_not_fetched() {
        assert_eq!(resolve(Some(Path::new("/x")), "https://e.com/a.png"), None);
        assert_eq!(resolve(Some(Path::new("/x")), "http://e.com/a.png"), None);
    }

    #[test]
    fn no_doc_dir_means_only_absolute_resolves() {
        assert_eq!(resolve(None, "img/a.png"), None);
        assert_eq!(
            resolve(None, "/tmp/x.png"),
            Some(PathBuf::from("/tmp/x.png"))
        );
    }

    /// caps 未取得（埋め込みホスト・問い合わせ失敗）では必ず `None` ＝ 文字フォールバックへ。
    #[test]
    fn without_caps_nothing_reserves() {
        CAPS.with(|c| *c.borrow_mut() = None);
        assert!(!available());
        assert_eq!(reserve_rows(Path::new("/tmp/whatever.png"), 40, 20), None);
    }
}
