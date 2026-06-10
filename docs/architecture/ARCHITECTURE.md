# Architecture Documentation

## 🏗️ System Architecture

### High-Level Overview

cozy follows a **Redux-inspired architecture** with clear separation between:
- **State**: Immutable data structures representing editor state
- **Actions**: Intentions to change state
- **Reducers**: Pure functions that transform state based on actions
- **UI**: Presentation layer that renders state

### Data Flow

```
User Input (Keyboard/Mouse)
    ↓
Event Loop (event_loop.rs)
    ↓
Keymap (ui/keymap.rs) → Maps keys to Actions
    ↓
Action (action/action.rs) → Represents user intent
    ↓
Reducer (reducer/mod.rs) → Dispatches to specific reducers
    ↓
State Update (state/editor.rs) → Modifies EditorState
    ↓
UI Render (ui/render/) → Renders updated state
    ↓
Terminal Display
```

## 📦 Module Details

### 1. State Module (`src/state/`)

#### `EditorState` (state/editor.rs)
Central state container for the entire editor.

**Key Fields**:
- `buffer: TextBuffer` - Text content (line-based)
- `cursor: Cursor` - Current cursor position
- `mode: EditorMode` - Current mode (Normal, Search, Replace, etc.)
- `filename: Option<PathBuf>` - Current file path
- `modified: bool` - Unsaved changes flag
- `status_message: Option<String>` - Status bar message
- `config: Config` - Configuration settings

**Modes**:
- `Normal` - Default editing mode
- `Glide` - Vim-like modal navigation/editing mode (Ctrl+G)
- `Search` - Search mode
- `Replace` - Replace mode
- `Goto` - Jump-to-line mode (Ctrl+J)
- `Save` - Save dialog
- `Open` - Open file dialog
- `Help` - Help screen
- `Quit` - Quit confirmation
- `Welcome` - Initial welcome screen

#### `TextBuffer` (state/buffer.rs)
Line-based text storage.

**Structure**:
```rust
pub struct TextBuffer {
    pub lines: Vec<String>,
}
```

**Operations**:
- `insert_char()` - Insert character at cursor
- `enter()` - Insert newline
- `backspace()` - Delete character before cursor
- `delete()` - Delete character at cursor
- `delete_line()` - Delete entire line

#### `Cursor` (state/cursor.rs)
Cursor position management.

**Fields**:
- `x: usize` - Column position
- `y: usize` - Row position

**Constraints**:
- Cursor position is clamped to valid buffer bounds
- Handles line wrapping and navigation

### 2. Action Module (`src/action/`)

#### `Action` Enum (action/action.rs)
Represents all possible user actions.

**Categories**:
- **Navigation**: `MoveUp`, `MoveDown`, `PageUp`, `PageDown`, etc.
- **Editing**: `InsertChar`, `Backspace`, `Delete`, `Enter`
- **Mode Switching**: `EnterMode(EditorMode)`
- **File Operations**: `Save`, `Open`, `SaveAndExit`
- **Search/Replace**: `Search`, `SearchNext`, `Replace`, `ReplaceAll`
- **System**: `Quit`, `Cancel`, `ReloadConfig`

### 3. Reducer Module (`src/reducer/`)

#### Main Reducer (`reducer/mod.rs`)
Central dispatcher that routes actions to specific reducers.

**Dispatch Logic**:
- Mode-dependent routing (Normal vs. Search vs. Replace)
- Action-specific handlers
- Returns `EventResult::Continue` or `EventResult::Exit`

#### Specific Reducers

**`editor.rs`**: Editor-level operations
- Mode transitions
- File operations
- Status message management

**`buffer.rs`**: Buffer operations
- Text manipulation
- Line operations

**`insert.rs`**: Insert operations
- Character insertion
- String insertion (paste)
- Newline insertion

**`delete.rs`**: Delete operations
- Character deletion
- Line deletion
- Backspace handling

**`cursor.rs`**: Cursor movement
- Navigation logic
- Boundary checking

**`search.rs`**: Search functionality
- Search buffer management
- Pattern matching (regex, case-sensitive, word-boundary)
- Next/Previous navigation

**`replace.rs`**: Replace functionality
- Replace buffer management
- Focus switching (query/replace fields)
- Replace current/all operations

**`file.rs`**: File I/O
- File loading
- File saving
- Filename buffer management

**`clipboard.rs`**: Clipboard operations
- Paste handling
- Copy operations (future)

**`status.rs`**: Status message management
- Message display
- Timestamp tracking
- Auto-hide logic

### 4. UI Module (`src/ui/`)

#### Renderer (`ui/render/`)

**`screen.rs`**: Layout management
- Calculates screen layout
- Divides terminal into body and footer

**`body.rs`**: Text body rendering
- Renders text content
- Line numbers
- Cursor display
- Scrolling logic

**`footer.rs`**: Status bar rendering
- Mode indicator
- Status messages
- File information

#### Keymap (`ui/keymap.rs`)
Maps keyboard input to actions.

**Key Mapping Logic**:
- Mode-aware key bindings
- Modifier key support (Ctrl, Alt)
- Special key handling (F1-F12, etc.)

#### Help (`ui/help.rs`)
Help screen rendering and content.

### 5. Event Loop (`src/event_loop.rs`)

Main event loop that:
1. Renders UI
2. Polls for events (500ms timeout)
3. Handles keyboard input
4. Handles paste events
5. Manages cursor blink animation
6. Handles terminal resize

**Event Types**:
- `Event::Key` - Keyboard input
- `Event::Paste` - Paste operation
- `Event::Resize` - Terminal resize

### 6. Utilities (`src/utils/`)

**`terminal.rs`**: Terminal utilities
- Bracketed paste mode
- Terminal setup/cleanup

**`file.rs`**: File utilities
- File reading/writing
- Path handling

**`unicode.rs`**: Unicode handling
- Character width calculation
- Unicode-aware operations

## 🔄 State Management Patterns

### Immutability
State is modified through reducers, not directly mutated.

### Single Source of Truth
All state is stored in `EditorState`.

### Predictable Updates
All state changes go through the reducer system.

### Mode-Based Behavior
Different modes handle the same actions differently:
- `InsertChar` in Normal mode → Insert character
- `InsertChar` in Search mode → Update search buffer
- `InsertChar` in Replace mode → Update replace buffer

## 🎨 Rendering Pipeline

1. **Layout Calculation**: Calculate screen layout based on terminal size
2. **Body Rendering**: Render text content with line numbers
3. **Footer Rendering**: Render status bar
4. **Cursor Rendering**: Render cursor (with blink support)
5. **Mode-Specific UI**: Render mode-specific UI (search bar, replace fields, etc.)

## 🔧 Configuration System

### Configuration Loading Order
1. `./config.toml` (current directory)
2. `~/.config/cozy/config.toml`
3. `~/.cozy/config.toml`
4. Default configuration

### Configuration Structure
```toml
page_size = 20              # PageUp/Down scroll amount
theme = "dark"              # Theme (not yet implemented)
show_line_numbers = true    # Show line numbers
status_duration = 3         # Status message display time (seconds)
line_number_width = 6       # Line number panel width
line_number_bg = "darkgray" # Line number background color
line_number_fg = "white"    # Line number foreground color
cursor_blink = true         # Enable cursor blinking
cursor_color = "blue"        # Cursor color (optional)
```

## 🐛 Known Issues

### Configuration Parsing
- `status_duration = 3c` should be `status_duration = 3` (syntax error in config.toml)

### Unicode Handling
- Some Unicode characters may not render correctly
- Character width calculation may be inaccurate for some characters

### Performance
- Large files may have performance issues
- No virtual scrolling for very long files

## 🚀 Future Improvements

### Planned Features
- Syntax highlighting
- Multi-file editing
- Plugin system
- Better Unicode support
- Performance optimizations (virtual scrolling)
- Configuration validation
- Better error messages

### Architecture Improvements
- Add message passing system for async operations
- Implement proper undo/redo with history
- Add command palette
- Improve mode system with better state management
