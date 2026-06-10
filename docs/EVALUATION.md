# cozy Evaluation

## Goal

cozy is intended to be a practical terminal editor for users who want the immediacy of `nano` with optional vim-like navigation when useful.

It is not trying to replace a full IDE or a mature modal editor. The current target is a small, dependable editor for quick edits, config files, notes, scripts, and terminal-first workflows.

## Current Strengths

- Direct editing by default: users can type immediately without entering insert mode.
- File operations: open, save, save-as, save prompts, quit, and directory browsing.
- Editing basics: insert, delete, backspace, enter, line cut, undo, redo, and paste.
- Navigation: arrow keys, page movement, line start/end, goto-line, and Glide mode.
- Search/replace: next/previous match, regex mode, case sensitivity, word-boundary matching, and replacement capture expansion.
- View support: line numbers, line wrap, status/footer rendering, and syntax highlighting for common file types.
- Architecture: state, actions, reducers, key mapping, and rendering are separated enough to keep focused behavior tests.

## Current Gaps

- Multiple open buffers are not implemented yet.
- Mouse support is intentionally minimal.
- Selection-based copy/cut is not a primary workflow yet; Glide yanking and line operations cover part of that space.
- Encoding support is focused on UTF-8 text.
- Syntax highlighting is lightweight and not a full language-server experience.

## Positioning

cozy is strongest as:

- a quick terminal editor installed with `cargo install cozy`
- a more navigation-friendly alternative to `nano`
- a small Rust/ratatui editor codebase that can be maintained and tested incrementally

It is not yet strongest as:

- a multi-file project editor
- a full vim replacement
- an IDE-like programming environment

## Maintenance Focus

Near-term maintenance should prioritize:

1. Polishing the `nano`-like default editing path.
2. Keeping Glide mode predictable and well-tested.
3. Improving file browsing and save/open workflows.
4. Expanding tests when reducer behavior changes.
5. Keeping README, crates.io metadata, and release notes accurate.

