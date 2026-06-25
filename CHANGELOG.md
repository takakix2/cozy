# Changelog

## v0.2.6

### Highlights

- Replaced the line-by-line regex syntax highlighter with tree-sitter (new
  default `treesitter` feature). Multi-line strings and block comments are now
  highlighted correctly, and highlights are computed for the visible window on
  change instead of per line on every frame.
- Function names are now highlighted (light blue). Disable the `treesitter`
  feature to fall back to the regex highlighter for a lighter embedded build.

## v0.2.5

### Highlights

- Added `F3` as a tmux-safe Browse shortcut. tmux's default prefix is `Ctrl+B`,
  which it swallows before cozy sees it, so Browse was unreachable inside tmux.
  `Ctrl+B` still works outside tmux.
- The in-app Help screen now lists the fallback keys (`Ctrl+B / F3` for Browse,
  `Ctrl+H / F1` for Help) so they are discoverable without crowding the footer.

## v0.2.4

### Highlights

- Fixed sluggish typing and shortcut response when editing highlighted files.
  Syntax highlighter regex sets are now compiled once per language instead of
  on every visible line on every frame.

## v0.2.3

### Highlights

- Added a mobile footer visibility toggle for low-height embedded terminals.
- Tightened one-row and compact footer layouts for iPhone-sized `hsh-ios`
  sessions.
- Kept search, goto, save, open, quit, and replace prompts usable when footer
  height is constrained.

## v0.2.2

### Highlights

- Refactored host boundaries for hsh-ios embedding: input, file I/O, config I/O,
  clipboard I/O, and startup runtime are now isolated from reducer/editor logic.
- Documented the planned session-diff workflow.

## v0.2.1

### Highlights

- Enabled Mermaid diagram rendering in Markdown preview.

## v0.2.0

### Highlights

- Added configurable footer and status bar colors via `config.toml`.
- Color settings accept named terminal colors and `#RRGGBB` true color values.
- Markdown preview now uses `ratatui-markdown` for rendered wrapping and code blocks.

## v0.1.15

### Highlights

- Added Markdown preview mode (`F2`) for rendered, read-only reading of Markdown documents.
- Supported fast reading controls in Markdown preview:
  - Navigation via `j`/`k` or `Up`/`Down`.
  - Page navigation via `PageUp`/`PageDown`.
  - Document jumps via `gg`/`G`, `Ngg`/`NG`.
  - Screen jumps via `H`/`M`/`L` (top/middle/bottom).
  - Counted movements (e.g. `5j`, `5gg`).
  - Easy return to the configured home mode using `Esc`.

## v0.1.8

### Highlights

- Documentation and presentation only (no code changes): lead the READMEs with the **Comfort First TUI** tagline, add a welcome-screen screenshot, and align the crate description with the comfort-first positioning.

## v0.1.7

### Highlights

- Added `default_mode` config option to choose your resting mode: `"edit"` (default, type like nano) or `"glide"` (navigate like vim). It governs every action's return target, not just startup — with Glide home, `Esc` round-trips back to Glide like vim's normal mode. Edit-entry verbs (`i`/`a`/`o`, change) still enter Edit regardless. Opt-in; newcomers keep zero hidden state.

### Validation

- `cargo test`: 79 tests passed.

## v0.1.6

### Highlights

- Added `cozy --version` and `cozy -V` for install verification without opening the TUI.
- Split Browse mode footer shortcuts into separate arrow-key and `hjkl` rows.

### Validation

- `cargo test`: 70 tests passed.

## v0.1.5

### Highlights

- Added Browse mode for opening directories and navigating a file tree.
- Improved save behavior for new buffers and relative filenames.
- Added collision-safe default save names such as `untitled (1).txt`.
- Updated README positioning around cozy as a small `nano` alternative with optional vim-like navigation.

### Validation

- `cargo test`: 70 tests passed.

## v0.1.4

- Added crates.io and license badges to the README files.
