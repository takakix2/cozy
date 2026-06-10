# Intro Post Draft

## Short

I released cozy, a small Rust terminal editor: type like nano, navigate like vim. It supports open/save, search/replace, undo/redo, clipboard paste, syntax highlighting, config, a folder browser, and optional Glide mode for vim-like motions.

https://crates.io/crates/cozy

## Longer

I published cozy, a small terminal text editor written in Rust with ratatui and crossterm.

The goal is a practical `nano` alternative: direct typing by default, with optional vim-like navigation when you need it. It already supports file open/save, search/replace, undo/redo, clipboard paste, line numbers, line wrap, syntax highlighting for common file types, TOML configuration, and a full-screen folder browser.

Install:

```bash
cargo install cozy
```

The internals use a reducer-based architecture so editor behavior can be tested without driving a real terminal.

https://crates.io/crates/cozy

