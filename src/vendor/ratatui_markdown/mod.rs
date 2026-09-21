//! Markdown and Mermaid rendering, copied from `ratatui-markdown` 0.3.6.
//!
//! Source: <https://github.com/celestia-island/ratatui-markdown> (crates.io 0.3.6),
//! by langyo, licensed MIT OR Apache-2.0 — see `LICENSE-APACHE` in this directory.
//!
//! Why a copy: 0.3.6 (and its main branch as of 2026-09-15) require ratatui 0.29, which
//! pins `unicode-width = "=0.2.0"`. argotty embeds cozy next to a mosh client whose
//! `vt100` needs `unicode-width >= 0.2.1`, so no build could hold both. cozy moved to
//! ratatui 0.30 and keeps the Mermaid preview by carrying the parts it uses.
//!
//! Changes from upstream:
//! - only `constants`, `markdown`, `mermaid` and `theme` are kept;
//! - image support (the `image` feature) is removed, and Mermaid is always on;
//! - paths point into this module, and `unicode-width` 0.2 is the renamed
//!   `unicode_width_02` (cozy's own width code stays on 0.1);
//! - ported to ratatui 0.30 and edition 2024.

pub mod constants;
pub mod markdown;
pub mod mermaid;
pub mod theme;
