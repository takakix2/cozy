# Changelog

## v0.2.25

### Fixes

- **A file can no longer repaint cozy's own screen.** Control characters in an opened
  file were handed to the terminal raw, so the terminal obeyed them instead of showing
  them. Measured before the fix, inside an isolated pane: a file containing an escape
  sequence could **erase the line-number gutter**, **write text onto a different line**
  of cozy's display, and **change the terminal's title** — just by being opened. cozy's
  screen is a gutter, a footer, and an `Edit:` / `Saved:` status line, so all of it was
  writable by the contents of the file being edited. That is UI spoofing, not a cosmetic
  glitch: a file could in principle paint a `Saved:` line that never happened.

  Control characters are now drawn in caret notation — `^M` for a carriage return, `^[`
  for an escape, `^H` for a backspace, `^?` for delete — and nothing but the notation
  reaches the terminal. A carriage return inside a line no longer overwrites what came
  before it or spills outside cozy's frame.

  ⭐ This is the other half of the byte-level work in 0.2.24. Keeping a lone `\r` as an
  ordinary character is what lets mixed-ending and Classic-Mac files come back
  byte-identical; drawing it as `^M` is what stops that same byte from lying about what
  is on screen. The bytes are unchanged by this release — the eight round-trip files from
  0.2.24, plus one containing an escape sequence, all still save identically.

- **The cursor lands on the character it appears to be on.** Three different width
  calculations existed side by side, disagreeing about control characters — the wrap used
  one column, the cursor used zero. On a line holding a carriage return, the wrap and the
  cursor already disagreed with each other, before any of the drawing changed. They now
  share a single rule, and the drawing asks that same rule what to draw. Pressing
  `Ctrl+E` on a line whose content is `a` followed by a carriage return now stops at
  column 7, matching the `a^M` that is actually drawn; it used to stop at 6,
  indistinguishable from an ordinary two-character line.

  ⚠️ **Tabs are deliberately unchanged.** Unlike the other control characters a tab
  appears in ordinary text, so drawing it as `^I` would change how every file with a tab
  in it looks. What a tab should be worth is tracked separately.

## v0.2.24

### Fixes

- **A file cozy opens now comes back with the same bytes.** Opening a file and pressing
  save — with no edits — used to rewrite it in two ways, both of them inside perfectly
  valid UTF-8. Every `\r` was dropped, so a CRLF file shrank (`a\r\nb\r\n`, 6 bytes,
  came back as 4); and a newline was appended to any file that did not already end with
  one (`no final newline`, 16 bytes, came back as 17). Between them, that is where *"I
  changed one line and the diff is the whole file"* came from.

  The buffer only ever recorded *what the lines said*, never *how the file wrote them
  down*, so both facts were lost at open and invented at save. cozy now measures them
  when it reads and restores them when it writes. Seven shapes were verified in the real
  binary, not just in tests: CRLF, **mixed endings**, a file whose only line break is a
  lone `\r`, no trailing newline, a 0-byte file, LF, and a one-line CRLF file — all
  byte-identical after open-and-save.

  ⭐ **Files with mixed line endings are not normalised.** A file counts as CRLF only if
  *every* `\n` in it is preceded by `\r`; a single bare `\n` puts the whole file in LF,
  where the remaining `\r`s are ordinary characters and get written back exactly where
  they were. Editing such a file keeps each line's own ending: typing into
  `a\r\nb\nc\r\n` leaves the `b\n` line alone.

  ⚠️ **Two deliberate departures from other editors.** cozy does not add a trailing
  newline to a file that lacked one — vim adds it (`fixendofline`), nano adds it, and
  cozy chooses to return the file rather than tidy it. And a lone `\r` is not read as a
  line break, so a Classic Mac file opens as one line rather than three; nano reads it
  as three and appends a `\r` of its own. Both are consequences of the same rule: a file
  cozy opens is a file it gives back. New files are unaffected and still end with a
  newline.

## v0.2.23

### Fixes

- **A file cozy cannot read is no longer opened — and no longer saved over.** Opening a
  file that is not valid UTF-8 (a Shift_JIS note, say) gave a blank buffer that looked
  exactly like a new file: same empty screen, same `Edit:` status line, no warning. cozy
  kept the filename, so the next `Ctrl+S` reported `Saved:` and wrote the buffer over the
  original. Measured before the fix: 16 bytes became 6, and the only message on screen
  said the save had succeeded.

  An editor holds someone else's file, so this is fixed structurally rather than with a
  better warning: cozy no longer accepts the *name* of a file it could not read. With no
  filename there is no save target, and the original bytes are out of reach even if the
  user never notices anything is wrong. `NotFound` and `InvalidData` had been sharing one
  branch — only `NotFound` means "a new file", and opening those empty is the point of the
  program, so that half is unchanged.

  The reason now appears on the Welcome screen itself, which has no status bar, and both
  entrances say the same thing: `Ctrl+O` used to surface `stream did not contain valid
  UTF-8`, which is true and tells you nothing about your file.

- **The open prompt starts empty.** `Ctrl+O` used to arrive with the current filename
  already in the field and the cursor at the end, so anyone who treated it as an empty
  prompt got the two names joined: editing `ok.txt` and typing `sjis.txt` asked for
  `ok.txtsjis.txt`, and cozy correctly answered `File not found`. The user typed one name,
  was told it does not exist, and retyped it to the same answer.

  Save prefills for a reason — saving to the same name is its default. Opening is the
  opposite operation, so there is nothing to pre-load, and nano's `Ctrl+R` starts empty
  too. `Ctrl+B` remains the way to pick a file rather than name it.

## v0.2.22

### Fixes

- **The in-app help now names every function key — including one that was nowhere.**
  On a narrow terminal the help listed **no** function key at all: not `F1`, not `F2`,
  not `F3`. And `F4` appeared in neither layout, even though `F4` is the **only** key
  bound to the diff review — a feature with no discoverable way in and no line in the
  help describing it.

  This matters most for `F1`. It exists because `Ctrl+H` does not always arrive: some
  terminals deliver it as the Backspace byte and swallow it, and a tmux configured to
  select panes with `Ctrl+H` consumes it before cozy ever sees it. In both cases `F1` is
  the only remaining way into the help — which is not much use if the help is the only
  place it is written down. The wide layout now says why it is there, the way the Browse
  row already says `(F3 for tmux)`.

- **The file top/bottom row looks like the rest of the table.** Shipped in 0.2.21, it
  wrote `M-\` and `^Home` where every neighbouring row spells `Ctrl+…` in full, put an
  alternate key in the parentheses this table reserves for explanations, and started its
  description one column late. The keys and what they do are unchanged.

## v0.2.21

### Highlights

- **The file's top and bottom finally have keys: `Alt+\` / `Alt+/`, or `Ctrl+Home` /
  `Ctrl+End`.** Those are nano's own spellings — it carries both, so cozy carries both.

  This is not a new capability so much as a missing wire. `Motion::FileTop` and
  `Motion::FileBottom` have always existed; Glide's `gg` and `G` use them. What was
  missing was any way to reach them from Edit mode, which is where cozy rests by
  default. The README documented `Ctrl+Home` / `Ctrl+End` for exactly this until a
  documentation cleanup removed the line in June — the line described a key that had
  never been written, and deleting it took away the last record that the gap existed.

  ⚠️ Neither spelling can be typed on a phone's software keyboard. That is what Glide's
  `gg` / `G` are for, and they are unchanged. With a hardware keyboard attached, both
  work.

### Fixes

- **`Ctrl+A` and `Ctrl+E` move within the line again, as the README has always said.**
  Both READMEs — and therefore the crates.io page — document them as *Line start* and
  *Line end*, and cozy's pitch is that you type like nano, where that is exactly what
  they are. What they actually did was jump to the **start and end of the whole
  document**: pressing `Ctrl+E` anywhere in a file landed you on the last line.

  The line meaning had no other key, either. Bare `Home`/`End` are unbound by default,
  the in-app help never mentioned `Ctrl+A`/`Ctrl+E` at all, and the command palette has
  no entry for them — so in Edit mode, the default resting mode, there was **no way to
  reach the end of the current line** except by holding the arrow key. That is a
  keyboard's worth of work on a desktop and a real cost on a phone, which is where cozy
  mostly runs.

  Reading views are unaffected: in the help screen and the Markdown preview, `Ctrl+A`
  and `Ctrl+E` still go to the top and bottom of the document, which is what a pager
  should do. The in-app help now lists the pair in both its wide and narrow layouts.

## v0.2.20

### Fixes

- **A half-typed `gg` no longer sticks in the read view.** In the help screen and the
  Markdown preview, pressing `g` — the first half of `gg` — shows `[g]` in the footer
  while cozy waits for the second key. Any arrow or page key left that `[g]` sitting
  there for the rest of the session: the keys that clear it are the ones handled by the
  per-mode branch, and arrows resolve in the global shortcut table and return before
  that branch is ever reached.

  The display has been there since July, but `[g]` normally lives for the few tens of
  milliseconds between two `g` presses, and the only way to strand it was to reach for
  an arrow key mid-motion. It became an everyday sight on a phone, where a finger pan
  arrives as a run of arrow keys.

## v0.2.19

### Highlights

- **The crates.io page now says how to install cozy without `cargo`.** v0.2.18 shipped
  prebuilt binaries and a Homebrew tap, and then told nobody: crates.io renders the
  README **as it was at publish time**, so the crate page kept offering
  `cargo install cozy` as the only way in — the exact gap v0.2.18 existed to close, left
  standing in the first place most people look.

  There is no way to correct that without publishing again, which is what this release
  is. ⚠️ **No code changed** — v0.2.19 is byte-for-byte the same editor as v0.2.18. If
  you already have it, there is nothing to update for.

## v0.2.18

### Highlights

- **cozy can be installed without a Rust toolchain.** Until now there was exactly one
  way in — `cargo install cozy` — which serves the half of the audience that already has
  `cargo`. The pitch is *type like nano*, so the person who most wants cozy is the one
  least likely to have a Rust toolchain at all.

  This release publishes prebuilt binaries for macOS and Linux (Apple Silicon and Intel,
  x86-64 and arm64) on the GitHub release, plus a Homebrew tap:

  ```sh
  brew install takakix2/tap/cozy
  ```

  Homebrew is the macOS answer specifically. A tarball dropped in `~/.local/bin` is on
  `PATH` by default on current Debian/Ubuntu and Fedora, and **not** on macOS — so a
  manual install "succeeds" and then `cozy` is command not found, which reads as *the
  app is broken* rather than *my PATH is short*. Homebrew's prefix is always on `PATH`.
  There is also a shell installer on the release, and it installs to `~/.local/bin`
  rather than `$CARGO_HOME/bin` for the same reason.

  ⚠️ **The code is unchanged from v0.2.17.** Nothing about the editor is different in
  this release; what changed is how you can get it. The Linux binaries keep clipboard
  support, and require glibc 2.35 or newer.

## v0.2.17

### Highlights

- **cozy now answers where its config file lives.** An embedding host could already
  *move* that file — `CozyConfig::config_dir` has always been public — but nothing
  published where it lands when the host does not move it. Half a contract: you could
  override the answer without being able to ask what you were overriding.

  So argo wrote the rule out a second time, and the two copies drifted. One machine
  ended up holding `~/Library/Application Support/cozy/config.toml` *and*
  `~/.hsh/cozy/config.toml`, and editing either one changed only one of the two ways
  of launching cozy — the same editor, the same machine, two settings.

  `cozy::user_config_path(config_dir)` is now exported. Pass the host's override to get
  the file under it; pass `None` to get the default cozy would have used
  (`$XDG_CONFIG_HOME/cozy/config.toml`, falling back to `~/.cozy/config.toml`). The
  override and its default are a pair, and only one side of the pair was reachable.

  ⚠️ **Nothing about cozy's own behaviour changed.** This release adds an export, so a
  desktop `cargo install cozy` sees no difference; it exists so a host can ask instead
  of reimplement.

## v0.2.16

### Highlights

- **Paths outside `$HOME` can be shown from the sandbox root now.** v0.2.14 shortened a
  path under `$HOME` to `~/…`, which covers what people usually open. It did nothing for
  anything above that: inside the phone app, `cozy /notes.md` still answered
  `File: [/private/var/mobile/Containers/…/notes.md]` — the container path the shell
  around it deliberately never shows, and one you cannot usefully type back.

  A host that runs cozy inside a sandbox can now say where the root is by setting
  `COZY_SANDBOX_ROOT` to the physical container path (the same shape as `COZY_COMPACT` —
  cozy cannot work this out for itself). With that, a path under the root displays as
  `/notes.md`, and typing `/notes.md` back resolves under the root, so the two directions
  agree.

  `$HOME` still wins where both apply, because it is the more specific of the two —
  otherwise `~/notes.md` would come back as `/Documents/notes.md` on iOS. A path already
  under the root is left alone rather than prefixed twice, since a host that translates
  before handing the file over will pass one in.

  ⚠️ With no `COZY_SANDBOX_ROOT` set, nothing changes at all — this is invisible on a
  desktop.

## v0.2.15

### Highlights

- **A cramped footer now offers `^Q Quit` where it used to offer `^X Exit`.** The two
  keys look like different answers and are almost the same one: `^X` and `^S` both
  land in the identical `File:` prompt, and only diverge after you press Enter. So on
  a narrow screen one of those two slots was spent saying the same thing twice, while
  the key that does something genuinely different — leave *without* saving — was not
  shown at all.

  That matters most exactly where the room is smallest. cozy runs inside a phone
  terminal, where dropping to a shell to sort it out is not the easy escape it is on
  a desktop, so the way out belongs on screen.

  Only the two cramped layouts changed: the single-row footer (three slots) and the
  narrow two-row one (under 80 columns). At full width `^X Exit` stays — nano's
  muscle memory is worth keeping where there is room for both — and the four-row
  layout already showed both.

  ⚠️ **No key was removed.** `^X` still exits; it is only off the sign.

## v0.2.14

### Highlights

- **The Save and Open prompts show `~` again instead of the resolved path.** cozy
  holds the file it opened as an absolute path, so pressing `Ctrl+S` on a file you
  opened as `~/notes.md` used to answer `File: [/home/you/notes.md]`. On a desktop
  that is merely verbose. Inside the phone app it is a leak: the host rewrites the
  path into the sandbox's real location before cozy sees it, so the prompt was
  spelling out `/data/data/com.hsh.mobile/files/notes.md` — a container path the
  shell around it deliberately never shows, and one you cannot usefully type.

  `~` had only ever worked in the *typing* direction (v0.2.12). This is the other
  half: a path under `$HOME` is shortened for display, and `Enter` expands it back,
  so the string on screen and the file that gets written are the same thing.

  Only whole path components match — with `HOME=/home/al`, `/home/alice/x` stays
  as it is rather than becoming `~ice/x`. A `HOME` of `/` never shortens: turning
  every path into `~/…` would make `~` mean nothing.

  ⚠️ The shortening happens where the prompt's buffer is seeded, not where it is
  drawn. That buffer is what you then edit, so shortening at draw time would leave
  the cursor pointing into a different string than the one on screen.

## v0.2.13

> ⓘ **0.2.12 never reached crates.io.** The version was bumped in the repository but
> not published, so what it described — the swap flushed on focus loss, and `~` in
> cozy's own prompts — ships here instead. Upgrading from 0.2.11 gets both releases
> at once.

### Highlights

- **Saving into a directory that does not exist now asks instead of refusing.**
  `Ctrl+S` on `~/.ssh/config` when there is no `~/.ssh` used to stop with
  `Directory not found` and leave you to quit, run `mkdir -p`, and start again.
  It now offers one line — `Create directory ~/.ssh? — [Enter] create  [Esc] cancel`
  — in the same shape as the swap-recovery offer: one line, two keys, anything
  else ignored.

  The two ends of this were both wrong to pick on their own. Creating it silently
  is what a typo does not deserve: `~/.shh/config` is a slip, and the whole value
  of the question is that you see the directory spelled out before it exists.
  Refusing is what nano and vim do, and it is defensible on a desktop where the
  shell is one `Ctrl+Z` away — but cozy also runs inside a phone app, where the
  shell you would drop to is a few centimetres wide. So it asks.

  Cancelling stays in the Save prompt with the typed name intact, because a wrong
  directory usually means a wrong name. `Ctrl+Q` still quits while the question is
  open — a prompt that swallows the quit key traps you, and the swap already holds
  the buffer.

## v0.2.12

### Highlights

- **The swap is written on the way out, not one tick later.** The cadence writes
  about a second after you stop typing, so a kill landing inside that second took
  the keystrokes with it. Nothing warns before a kill — but something does say when
  the app leaves the screen, and that moment was going unused. `Event::FocusLost`
  now flushes the swap on the spot. No new API was needed: losing focus already
  means "you are about to stop being the thing in front of the user", so a host
  that leaves the screen sends it, and a terminal that reports focus gives the CLI
  the same thing for free.

### Fixes

- **`~` works in the editor's own prompts.** Typing a home-relative path into
  Save As or Open — `~/.hshrc`, `~/notes.md` — failed with `Directory not found`,
  because cozy never expanded the tilde itself. Nothing was wrong on the command
  line: there the shell expands `~` before cozy ever sees it. But cozy's prompts
  have no shell in front of them, so the path was taken literally and read as a
  relative one containing a directory named `~`. `vim` (`:e ~/x`) and `nano` both
  expand it themselves, and cozy now does too — the bare `~`, and `~/…`. A
  `~user` prefix has no resolution here and is left alone, as is a `~` anywhere
  but the front, which turns up in ordinary filenames.

  This had been true since the first commit and shipped in every release. It
  stayed hidden because the common path — `cozy <file>`, then `Ctrl+S` — never
  involves a tilde reaching cozy. It surfaced on iOS, where the editor is called
  in-process and no word expansion happens anywhere, so even the command line
  arrived with the tilde intact.

## v0.2.11

### Highlights

- **Unsaved work survives a kill.** cozy now keeps a swap file — a snapshot of the
  buffer, written about a second after you stop typing (and every 200 keystrokes if
  you never stop). Open the file again after a crash, a `kill -9`, or an iOS app
  eviction, and cozy offers the edits back on one line: `[Enter]` restores,
  `[Esc]` discards. Restoring loads the buffer and leaves it unsaved — the file on
  disk is not touched until you say so.

  The swap is not a `.swp` beside your file: it lives in a state directory
  (`~/.local/state/cozy/swap`, or the host's config dir when embedded), so it
  cannot dirty a git worktree and works when the file's directory is read-only.
  It is removed on save and on a deliberate quit — it exists for the exits you
  did not choose — and an unclaimed one is swept after 14 days.

  This is vim's model, not nano's, and deliberately so: nano writes its emergency
  copy when a signal arrives, but iOS kills a backgrounded app with SIGKILL, where
  no handler runs at all. The only writes that survive are the ones already made.

### Fixes

- Saving through a symlink whose target does not exist yet (a dotfiles repo not
  cloned on this machine, a target deleted a moment ago) replaced the link with a
  regular file. The link is now followed to where it points, and the file is
  written there.

## v0.2.10

### Highlights

- Added a compact / low-spec host mode via the `COZY_COMPACT` environment
  variable (set it to anything but empty or `0`). When on, cozy uses the
  lightweight bordered welcome (a centered, width-capped box — no longer
  stretched edge-to-edge on a wide screen) instead of the block-art logo, and hides line
  numbers by default — both cut the number of cells redrawn each frame, which
  matters on full-repaint GPUs (e.g. an Android tablet's Mali-G52 driven by
  hsh-ios). The host sets it; cozy can't detect the GPU itself. A runtime
  `Ctrl+L` toggle still turns line numbers on even under compact.

## v0.2.9

### Highlights

- Added tree-sitter syntax highlighting for Markdown source (`.md`/`.markdown`)
  in the edit view, so headings, bold/italic emphasis, inline code, list/quote
  markers, and link URLs are colored while you type — not only in the
  `ratatui-markdown` preview. Colors mirror VS Code's Dark+ theme (blue
  headings/bold, purple italics, orange inline code, light-blue list markers,
  green quote markers; fenced-code-block contents stay the default color).
  Markdown uses a two-grammar parser (block + inline) via `tree-sitter-md`; the
  highlighter backend was generalized to drive it. Bumped the core `tree-sitter`
  dependency to 0.26 (required by `tree-sitter-md`).
- Refreshed the edit-mode and glide-mode README screenshots to show the new
  Markdown source highlighting.

## v0.2.8

### Highlights

- Reorganized the Help screen: split the cross-mode shortcuts out of "Edit Mode"
  into new "View" (line numbers, wrap, footer, Markdown preview) and "Global"
  (command palette, help) sections, and added the previously missing `Ctrl+L`,
  `Ctrl+W`, `Ctrl+U`, and `Ctrl+P` entries. Fixed column alignment on the
  multi-key rows.

## v0.2.7

### Highlights

- Added tree-sitter syntax highlighting for TypeScript (`.ts`/`.mts`/`.cts`/
  `.tsx`) and Go (`.go`). TypeScript inherits the JavaScript highlight query so
  keywords, strings, and comments are covered alongside the TS-specific nodes.

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
