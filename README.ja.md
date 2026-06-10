# cozy

[![crates.io](https://img.shields.io/crates/v/cozy.svg)](https://crates.io/crates/cozy)
![license](https://img.shields.io/crates/l/cozy.svg)

[English](README.md) | **Japanese**

**Comfort First な TUI — nano のように打って、vim のように動く。**

![cozy welcome screen](https://raw.githubusercontent.com/takakix2/cozy/main/docs/assets/screenshot.png)

cozy は Rust 製の小さなターミナルテキストエディタです。普段は `nano` のようにそのまま入力でき、必要なときだけ Glide モードで vim 風の移動や編集を使えます。

## インストール

```bash
cargo install cozy
```

ソースからビルドする場合:

```bash
cargo build --release
```

## 使い方

```bash
# 新規バッファ
cozy

# ファイルを開く
cozy <ファイル名>

# ディレクトリをブラウズ
cozy <フォルダ名>
```

## 特徴

- 既定は直接入力: モーダル操作を覚えなくてもすぐ編集できる
- ファイルを開く、保存、名前を付けて保存、未保存終了確認、フォルダブラウズ
- 通常検索、大文字小文字区別、単語境界、正規表現検索と置換
- Undo/Redo、行カット、クリップボード貼り付け、行番号、行折り返し、行番号ジャンプ
- Rust / Python / JavaScript / JSON / TOML のシンタックスハイライト
- 高速に読める Markdown プレビューモード
- Markdown プレビューでの Mermaid fenced block 検出
- Glide モードによる vim 風の移動、operator、yank、change、delete、join、paste
- TOML 設定とアクション単位のキーバインド上書き
- reducer ベースの構成と、カーソル、motion、編集、置換、clipboard、browse mode のテスト

## エディタモード

- **Edit**: 既定モード。`nano` のようにそのまま入力。
- **Glide**: vim 風のモーダルナビゲーションと編集 (`Ctrl+G`)。
- **Search**: インクリメンタル検索 (`Ctrl+F`)。
- **Replace**: 検索と置換 (`Ctrl+R`)。
- **Goto**: 行番号へジャンプ (`Ctrl+J`)。
- **Save**: 保存ダイアログ (`Ctrl+S`)。
- **Open**: ファイルを開くダイアログ (`Ctrl+O`)。
- **Browse**: フルスクリーンのファイルツリー (`Ctrl+B`)。
- **Markdown**: Markdown を整形表示して読むモード (`F2`)。
- **Help**: ヘルプ画面 (`Ctrl+H` または `F1`)。

## キーバインド

キーバインドの正は `src/shortcuts.rs` (`get_shortcuts()`) です。既定値は以下の通りです。`config.toml` の `[keys]` セクションでアクション単位に上書きできます。

### ファイル

- `Ctrl+S`: 保存
- `Ctrl+Shift+S`: 名前を付けて保存
- `Ctrl+O`: ファイルを開く
- `Ctrl+B`: ファイルブラウズ
- `Ctrl+X`: 終了。変更があれば保存確認
- `Ctrl+Q`: 保存せず即終了

### ナビゲーション

- `Up` / `Down` / `Left` / `Right`: カーソル移動
- `Ctrl+A`: 行頭
- `Ctrl+E`: 行末
- `PageUp` / `PageDown`: ページ移動
- `Ctrl+J`: 行番号へジャンプ
- `Ctrl+G`: Glide モードへ

### 編集

- `Enter`: 改行
- `Backspace` / `Delete`: カーソル前またはカーソル位置を削除
- `Ctrl+K`: 現在行をカット
- `Ctrl+V`: システムクリップボードから貼り付け
- `Ctrl+Z`: 元に戻す
- `Ctrl+Y`: やり直し

### 検索と置換

- `Ctrl+F`: 検索
- `Ctrl+N`: 次の一致
- `Ctrl+P`: 前の一致
- `Ctrl+T`: 検索オプション切替
- `Ctrl+R`: 置換モード。もう一度押すと全置換
- `Tab` in replace mode: 検索欄と置換欄の切替
- `Enter` in replace mode: 現在の一致を置換

### 表示とヘルプ

- `Ctrl+H` / `F1`: ヘルプ
- `Ctrl+L`: 行番号の表示切替
- `Ctrl+W`: 行折り返しの切替
- `F2`: Markdown プレビュー切替
- `Esc` / `Ctrl+[`: 現在の操作をキャンセル、または現在のモードを抜ける

## Markdown プレビュー

`F2` で Markdown プレビューに入ります。README、実装計画、メモなどの Markdown を素早く読むための読み取り専用ビューです。

- 移動: `j`/`k` または `Up`/`Down`
- ページ移動: `PageUp` / `PageDown`
- ジャンプ: `gg`/`G`, `Ngg`/`NG`
- 画面内移動: `H`/`M`/`L` で表示範囲の上/中央/下へ
- 数字付き移動: `5j`, `5k`, `5gg`, `5G`
- Mermaid: ` ```mermaid ` ブロックを図のソースとしてラベル表示・強調
- `Esc`: 設定されたホームモードへ戻る

## Glide モード

`Ctrl+G` で Glide モードに入ります。数字を前置すると motion や行単位操作を繰り返します。

- 移動: `h` `j` `k` `l`, `w`/`b`/`e`, `W`/`B`/`E`, `0`/`^`/`$`, `gg`/`G`, `H`/`M`/`L`
- find/till: `>`/`<`, `t`/`T`, 直前ジャンプの繰り返し `.`/`,`
- operator: `d`/`c`/`y` + motion。例: `dw`, `de`, `d$`, `dj`, `cw`, `yw`, `d3w`
- 行単位: `dd`/`cc`/`yy`, `3dd` のような count
- 編集: `x`, `X`, `~`, `J`
- 貼り付け: `p`/`P`
- 挿入: `i`/`I`, `a`/`A`, `o`/`O`
- `Esc`: Edit モードへ戻る

## 設定

設定は最初に見つかったパスから読み込まれます。

1. `./config.toml`
2. `~/.config/cozy/config.toml`
3. `~/.cozy/config.toml`

例:

```toml
page_size = 20
theme = "dark"
show_line_numbers = true
status_duration = 3
line_number_bg = "darkgray"
line_number_fg = "white"
cursor_blink = true

# 定常モード（戻り先の「家」）: "edit"（既定・nano のように打つ）か
# "glide"（vim のように動く）。起動時だけでなく、あらゆる操作の戻り先が
# このモードになる。初心者は "edit"（隠れ状態ゼロ）のまま、vim 派は
# "glide" に切替（Glide からは i/a/o で Edit に入る）。
default_mode = "edit"
```

キーバインドはアクション名で上書きできます。

```toml
[keys]
enter_browse = "ctrl+b"
enter_glide = "ctrl+g"
enter_help = "f1"
toggle_markdown = "f2"
```

## アーキテクチャ

cozy は Redux 風の構成です。

```text
Input -> Keymap -> Action -> Reducer -> State -> UI
```

中心となる状態は `EditorState` にあり、テキストは行ベースの `TextBuffer` で保持します。エディタの挙動は reducer に分離されているため、ターミナル入力、状態遷移、描画を切り分けてテストできます。

## 開発

```bash
cargo test
cargo fmt
```

現在のテストは editor reducer、cursor movement、word/screen motion、replace、clipboard/register、browse mode を対象にしています。

## ライセンス

以下のいずれかのライセンスで利用できます。

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
