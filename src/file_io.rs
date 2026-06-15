use crate::browse::BrowseTree;
use crate::state::{Cursor, EditorState, TextBuffer};
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(crate) enum StartupDocument {
    Empty,
    File { path: PathBuf, lines: Vec<String> },
    Directory { tree: BrowseTree },
}

pub(crate) fn load_startup_document(filename: Option<&str>) -> StartupDocument {
    let Some(path) = filename else {
        return StartupDocument::Empty;
    };

    let path_ref = Path::new(path);
    if path_ref.is_dir() {
        return StartupDocument::Directory {
            tree: BrowseTree::build(path_ref),
        };
    }

    let lines = std::fs::read_to_string(path_ref)
        .map(|content| lines_from_content(&content))
        .unwrap_or_else(|_| vec![String::new()]);

    StartupDocument::File {
        path: PathBuf::from(path),
        lines,
    }
}

pub(crate) fn build_browse_tree(filename: Option<&PathBuf>, working_dir: &Path) -> BrowseTree {
    let root = existing_browse_root(filename, working_dir);
    let mut tree = BrowseTree::build(&root);
    if let Some(file) = filename {
        tree.select_path(file);
    }
    tree
}

pub fn save(editor: &mut EditorState) -> io::Result<()> {
    let Some(path) = editor.filename.clone() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No filename set. Use Save As to specify a filename.",
        ));
    };
    let target = editor.resolve_in_working_dir(&path);
    write_buffer(
        editor,
        &target,
        &format!("Failed to save '{}'", target.display()),
    )?;
    mark_saved(editor);
    Ok(())
}

pub fn save_as(editor: &mut EditorState, path: &str) -> io::Result<()> {
    if path.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Filename is empty",
        ));
    }

    let path_buf = PathBuf::from(path);
    let target = editor.resolve_in_working_dir(&path_buf);
    write_buffer(editor, &target, &format!("Failed to save '{}'", path))?;

    editor.filename = Some(path_buf);
    mark_saved(editor);
    Ok(())
}

pub fn open_file(editor: &mut EditorState, path: &str) -> io::Result<()> {
    if path.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Filename is empty",
        ));
    }

    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("File not found: {}", path),
        ));
    }
    if !path_buf.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Not a file: {}", path),
        ));
    }

    let content = std::fs::read_to_string(&path_buf).map_err(|e| {
        let kind = if e.kind() == io::ErrorKind::PermissionDenied {
            io::ErrorKind::PermissionDenied
        } else {
            e.kind()
        };
        io::Error::new(kind, format!("Failed to open '{}': {}", path, e))
    })?;

    let lines = lines_from_content(&content);
    editor.buffer = TextBuffer::from_lines(lines);
    editor.filename = Some(path_buf);
    editor.cursor = Cursor::default();
    editor.modified = false;
    editor.scroll_offset = 0;
    Ok(())
}

fn write_buffer(editor: &EditorState, target: &std::path::Path, context: &str) -> io::Result<()> {
    ensure_parent_dir(target)?;
    let mut file = File::create(target).map_err(|e| {
        let kind = if e.kind() == io::ErrorKind::PermissionDenied {
            io::ErrorKind::PermissionDenied
        } else {
            e.kind()
        };
        io::Error::new(kind, format!("{}: {}", context, e))
    })?;

    for line in &editor.buffer.lines {
        writeln!(file, "{}", line)
            .map_err(|e| io::Error::new(io::ErrorKind::WriteZero, format!("Write error: {}", e)))?;
    }
    Ok(())
}

fn ensure_parent_dir(target: &std::path::Path) -> io::Result<()> {
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Directory not found:{}", parent.display()),
            ));
        }
    }
    Ok(())
}

fn mark_saved(editor: &mut EditorState) {
    editor.last_saved_id = editor.undo_stack.len();
    editor.modified = false;
}

fn existing_browse_root(filename: Option<&PathBuf>, working_dir: &Path) -> PathBuf {
    let mut current = filename
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| working_dir.to_path_buf());

    while !current.exists() {
        if !current.pop() || current.as_os_str().is_empty() {
            return working_dir.to_path_buf();
        }
    }

    current
}

fn lines_from_content(content: &str) -> Vec<String> {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
