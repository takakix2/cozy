# cozy

[![crates.io](https://img.shields.io/crates/v/cozy.svg)](https://crates.io/crates/cozy)
![license](https://img.shields.io/crates/l/cozy.svg)

**English** | [Japanese](README.ja.md)

**A Comfort First TUI — type like nano, navigate like vim.**

![cozy welcome screen](https://raw.githubusercontent.com/takakix2/cozy/main/docs/assets/screenshot.png)

cozy is a small Rust terminal text editor for people who want direct, low-friction editing by default, with optional modal navigation when they need it. It is intended to feel approachable like `nano` while keeping a focused set of vim-like motions in Glide mode.

## Install

```bash
cargo install cozy
```

Or build from source:

```bash
cargo build --release
```

## Usage

```bash
# Open a new buffer
cozy

# Open a file
cozy <file>

# Browse a directory
cozy <folder>
```

## Highlights

- Direct text editing by default: type, save, and exit without learning a modal workflow first
- File open/save/save-as, unsaved-change prompts, and folder browsing
- Search and replace with literal, case-sensitive, word-boundary, and regex modes
- Undo/redo, line cut, clipboard paste, line numbers, line wrap, and goto-line
- Syntax highlighting for Rust, Python, JavaScript, JSON, and TOML
- Markdown preview mode with fast reading controls
- Mermaid fenced block detection in Markdown preview
- Glide mode for vim-like movement, operators, yanking, changing, deleting, joining, and paste
- TOML configuration and per-action key overrides
- Reducer-based architecture with focused tests for cursor behavior, motions, editing, replace, clipboard, and browse mode

## Editor Modes

- **Edit**: Default mode. Type directly, like `nano`.
- **Glide**: Vim-like modal navigation and editing (`Ctrl+G`).
- **Search**: Incremental search (`Ctrl+F`).
- **Replace**: Search and replace (`Ctrl+R`).
- **Goto**: Jump to a line number (`Ctrl+J`).
- **Save**: Save dialog (`Ctrl+S`).
- **Open**: Open-file dialog (`Ctrl+O`).
- **Browse**: Full-screen file tree (`Ctrl+B`).
- **Markdown**: Rendered Markdown reading mode (`F2`).
- **Help**: Help screen (`Ctrl+H` or `F1`).

## Key Bindings

The source of truth for key bindings is `src/shortcuts.rs` (`get_shortcuts()`). Defaults are shown below. Per-action overrides are available through the `[keys]` section of `config.toml`.

### File

- `Ctrl+S`: Save
- `Ctrl+Shift+S`: Save as
- `Ctrl+O`: Open file
- `Ctrl+B`: Browse files
- `Ctrl+X`: Exit, prompting to save when modified
- `Ctrl+Q`: Quit immediately without saving

### Navigation

- `Up` / `Down` / `Left` / `Right`: Move cursor
- `Ctrl+A`: Line start
- `Ctrl+E`: Line end
- `PageUp` / `PageDown`: Scroll page
- `Ctrl+J`: Jump to line number
- `Ctrl+G`: Enter Glide mode

### Editing

- `Enter`: Insert newline
- `Backspace` / `Delete`: Delete before or at cursor
- `Ctrl+K`: Cut current line
- `Ctrl+V`: Paste from system clipboard
- `Ctrl+Z`: Undo
- `Ctrl+Y`: Redo

### Search And Replace

- `Ctrl+F`: Search
- `Ctrl+N`: Next match
- `Ctrl+P`: Previous match
- `Ctrl+T`: Toggle search options
- `Ctrl+R`: Replace mode; press again to replace all
- `Tab` in replace mode: Switch between query and replacement fields
- `Enter` in replace mode: Replace current match

### View And Help

- `Ctrl+H` / `F1`: Help
- `Ctrl+L`: Toggle line numbers
- `Ctrl+W`: Toggle line wrap
- `F2`: Toggle Markdown preview
- `Esc` / `Ctrl+[`: Cancel current operation or leave the current mode

## Markdown Preview

Markdown preview is available with `F2`. It is a read-only view for quickly reading README files, plans, notes, and other Markdown documents.

- Move: `j`/`k` or `Up`/`Down`
- Page: `PageUp` / `PageDown`
- Jump: `gg`/`G`, `Ngg`/`NG`
- Screen: `H`/`M`/`L` for top/middle/bottom of the visible area
- Counted move: `5j`, `5k`, `5gg`, `5G`
- Mermaid: ` ```mermaid ` blocks are labeled and highlighted as diagram source
- `Esc`: Return to your configured home mode

## Glide Mode

Glide mode is available with `Ctrl+G`. A leading number repeats a motion or linewise operation.

- Move: `h` `j` `k` `l`, `w`/`b`/`e`, `W`/`B`/`E`, `0`/`^`/`$`, `gg`/`G`, `H`/`M`/`L`
- Find/till: `>`/`<`, `t`/`T`, then `.`/`,` to repeat the last jump forward/back
- Operators: `d`/`c`/`y` plus a motion, such as `dw`, `de`, `d$`, `dj`, `cw`, `yw`, `d3w`
- Linewise: `dd`/`cc`/`yy`, with counts such as `3dd`
- Edit: `x`, `X`, `~`, `J`
- Paste: `p`/`P`
- Insert: `i`/`I`, `a`/`A`, `o`/`O`
- `Esc`: Return to Edit mode

## Configuration

Configuration is loaded from the first path found:

1. `./config.toml`
2. `~/.config/cozy/config.toml`
3. `~/.cozy/config.toml`

Example:

```toml
page_size = 20
theme = "dark"
show_line_numbers = true
status_duration = 3
line_number_bg = "darkgray"
line_number_fg = "white"
cursor_blink = true

# Which mode you rest in: "edit" (default, type like nano) or "glide"
# (navigate like vim). This is your home — every action returns here, not
# just startup. Newcomers stay in "edit" (zero hidden state); vim users can
# opt into "glide" and enter Edit with i/a/o.
default_mode = "edit"
```

Key bindings can be overridden by action name:

```toml
[keys]
enter_browse = "ctrl+b"
enter_glide = "ctrl+g"
enter_help = "f1"
toggle_markdown = "f2"
```

## Architecture

cozy uses a Redux-inspired architecture:

```text
Input -> Keymap -> Action -> Reducer -> State -> UI
```

The main editor state lives in `EditorState`, text is stored in a line-based `TextBuffer`, and editor behavior is implemented through reducers. This keeps terminal input, state transitions, and rendering separate enough to test core editor behavior directly.

## Development

```bash
cargo test
cargo fmt
```

The current test suite covers editor reducers, cursor movement, word and screen motions, replace behavior, clipboard/register handling, and browse mode behavior.

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
