use crate::browse::BrowseTree;
use crate::state::{Cursor, EditorState, TextBuffer};
use atomicwrites::{AllowOverwrite, AtomicFile};
use std::fs::File;
use std::io::{self, BufWriter, Write};
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

/// バッファをファイルへ書き出す。
///
/// 以前はここで `File::create`（＝ `O_TRUNC`）してから1行ずつ書いていた。つまり
/// **ユーザーのファイルを先に空にしてから**書き直していたので、途中で死ぬと
/// **前半だけのファイル**が残る。空になるより悪い —— 空なら壊れたと一目で分かるが、
/// 200行だけのファイルは普通に開けてしまい、後半が消えたことに当分気づかない。
/// `fsync` も無かったので、書いたつもりの行が電源断で消えることもあった。
/// cozy は hsh-ios にも同梱される＝**OS がいつアプリを殺してもおかしくない**環境で走る。
///
/// ## 素朴な「tmp に書いて rename」ではいけない
///
/// エディタは**他人のファイルを預かる**。rename は新しい inode を持ち込むので、素直に
/// やると **symlink を実体ファイルに化けさせる**。`~/.hshrc` を dotfiles リポへ symlink
/// している人が cozy で編集すると、リンクが切れてリポジトリ側が取り残され、しかも編集は
/// 成功して見える（実測で確認した）。ハードリンクや、元ファイルの mode も同様に飛ぶ。
///
/// nano / vim / neovim を実際に測ったところ、**3つとも tmp→rename をしていない**
/// （その場で `O_TRUNC` して書き、`fsync` する。inode・mode・symlink はすべて保たれる）。
/// 安全性は別口——vim の swap ファイル、nano の緊急保存——で買っている。
///
/// ## ここでの方針（vim の `backupcopy=auto` 相当）
///
/// 1. **symlink は先に解決**し、実体のパスを相手にする。リンクは生き残る。
///    **リンク先がまだ無くても**（消した直後・dotfiles リポ未 clone）リンクは潰さない。
/// 2. 通常は**不可分に置換**（tmp を隣に書く → fsync → 親ディレクトリも fsync → rename）。
///    元ファイルの mode は rename の**前に**一時ファイルへ移すので、権限の緩い瞬間が無い。
/// 3. **置換してはいけない/できない**ときは、その場書き + `fsync` に落ちる（nano と同じ）:
///    - ハードリンクが張られている（`st_nlink > 1`）——rename は別 inode になり、リンクが切れる。
///    - 親ディレクトリに書込権が無い（ファイルは書けるがディレクトリは書けない。`/etc` 等）
///      ——rename にはディレクトリの書込権が要るので失敗する。
fn write_buffer(editor: &EditorState, target: &std::path::Path, context: &str) -> io::Result<()> {
    ensure_parent_dir(target)?;

    // symlink の実体を相手にする（リンク自体を rename で潰さないため）。
    let real = resolve_symlink(target);
    if real != target {
        // リンク先の親が無ければ、そこには書けない（黙って作らない）。
        ensure_parent_dir(&real)?;
    }
    let existing = std::fs::metadata(&real).ok();

    if replaceable(existing.as_ref()) {
        match write_atomically(editor, &real, existing.as_ref()) {
            Ok(()) => return Ok(()),
            // ディレクトリに書込権が無いと rename できない。ファイル自体は書けることが
            // あるので、諦めずにその場書きへ落ちる（まだ target には触れていない）。
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {}
            Err(e) => return Err(annotate(e, context)),
        }
    }

    write_in_place(editor, &real).map_err(|e| annotate(e, context))
}

fn annotate(e: io::Error, context: &str) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {}", context, e))
}

/// Replace a file cozy owns (the swap journal) with `bytes`, atomically.
///
/// This is the state-file case, not the user's-document case: no symlink or mode
/// to preserve, because we created the file. A torn swap is worse than no swap —
/// it looks recoverable — so it goes through the same tmp/fsync/rename.
pub(crate) fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    AtomicFile::new(path, AllowOverwrite)
        .write(|f| f.write_all(bytes))
        .map_err(|e| match e {
            atomicwrites::Error::Internal(e) | atomicwrites::Error::User(e) => e,
        })
}

/// 書く相手の実体パスを返す（symlink はリンクでなくリンク先を書く）。
///
/// `canonicalize` は**リンク先が存在しないと失敗する**ので、それだけに頼ると
/// **リンク先を消した直後・dotfiles リポをまだ clone していないマシン**で、
/// rename が symlink を実体ファイルに化けさせる（＝守ったはずのリンクをそこだけ潰す）。
/// リンクが在る限りは `read_link` で行き先を出し、**リンクが指していた場所を書く**。
/// 存在しない普通のパス（新規ファイル）はそのまま返す。
fn resolve_symlink(target: &Path) -> PathBuf {
    if let Ok(real) = std::fs::canonicalize(target) {
        return real;
    }
    // リンクは在るがリンク先が無い。行き先を解決して、そこを書きに行く。
    if target.is_symlink() {
        if let Ok(link) = std::fs::read_link(target) {
            if link.is_absolute() {
                return link;
            }
            if let Some(parent) = target.parent() {
                return parent.join(link);
            }
            return link;
        }
    }
    target.to_path_buf()
}

/// rename で置き換えてよいか（ハードリンクの共有先を切り離さないか）。
#[cfg(unix)]
fn replaceable(existing: Option<&std::fs::Metadata>) -> bool {
    use std::os::unix::fs::MetadataExt;
    // 新規ファイル（None）は置換して構わない。既存はリンク数で判断する。
    existing.map(|m| m.nlink() <= 1).unwrap_or(true)
}

#[cfg(not(unix))]
fn replaceable(_existing: Option<&std::fs::Metadata>) -> bool {
    true
}

/// tmp → fsync → 親ディレクトリ fsync → rename。読み手には保存前か保存後しか見えない。
fn write_atomically(
    editor: &EditorState,
    real: &Path,
    existing: Option<&std::fs::Metadata>,
) -> io::Result<()> {
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        // `st_mode` にはファイル種別のビットも乗っている。Linux/macOS の chmod は黙って
        // 落とすが、POSIX では permission bits 以外の扱いは未規定なので手前で削る。
        existing.map(|m| m.permissions().mode() & 0o7777)
    };
    #[cfg(not(unix))]
    let _ = existing;

    AtomicFile::new(real, AllowOverwrite)
        .write(|file| {
            {
                let mut out = BufWriter::new(&mut *file);
                for line in &editor.buffer.lines {
                    writeln!(out, "{}", line)?;
                }
                out.flush()?;
            }
            // rename の**前に** mode を移す。後から chmod すると、その一瞬だけ
            // 権限の緩いファイルが見える（0600 のファイルを預かっている場合に困る）。
            #[cfg(unix)]
            if let Some(mode) = mode {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(std::fs::Permissions::from_mode(mode))?;
            }
            Ok(())
        })
        .map_err(|e| match e {
            atomicwrites::Error::Internal(e) | atomicwrites::Error::User(e) => e,
        })
}

/// その場で切って書き、`fsync` する（nano と同じ）。不可分ではないが、inode・mode・
/// ハードリンクを保つ。**置換が使えない相手にだけ**使う。
fn write_in_place(editor: &EditorState, real: &Path) -> io::Result<()> {
    let mut file = File::create(real)?;
    {
        let mut out = BufWriter::new(&mut file);
        for line in &editor.buffer.lines {
            writeln!(out, "{}", line)?;
        }
        out.flush()?;
    }
    // 書いたバイトをディスクに載せる。cozy にはこれすら無かった。
    file.sync_all()
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
    // The file on disk now says what the buffer says: nothing left to recover.
    crate::swap::remove(editor);
    // Save As points at a different file, so the swap must follow the target.
    editor.swap_path = crate::swap::path_for(editor);
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

#[cfg(all(test, unix))]
mod write_buffer_tests {
    use super::*;
    use crate::state::{EditorState, TextBuffer};
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    /// 一時ディレクトリ（tempfile 非依存・テストごとに一意。browse の慣習に揃える）。
    fn scratch(name: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("cozy_write_test_{}_{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn editor_with(lines: &[&str]) -> EditorState {
        let mut editor = EditorState::new(None);
        editor.buffer = TextBuffer::from_lines(lines.iter().map(|s| s.to_string()).collect());
        editor
    }

    fn save_to(path: &Path, lines: &[&str]) {
        write_buffer(&editor_with(lines), path, "test").unwrap();
    }

    /// リンク先がまだ無い symlink（リンク先を消した直後・dotfiles リポ未 clone）でも、
    /// **リンクを食い潰さずリンク先に書く**。`canonicalize` は dangling で失敗するので、
    /// それだけに頼っているとここだけ素朴な rename に落ちてリンクが実体化する。
    #[test]
    fn editing_through_a_dangling_symlink_keeps_the_link() {
        let dir = scratch("dangling");
        fs::create_dir_all(dir.join("dotfiles")).unwrap();
        let real = dir.join("dotfiles/hshrc"); // まだ存在しない
        let link = dir.join("hshrc");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        save_to(&link, &["edited"]);

        assert!(
            fs::symlink_metadata(&link).unwrap().file_type().is_symlink(),
            "symlink が実体ファイルに置き換わった"
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "edited\n", "リンク先に書けていない");
    }

    /// 相対リンクも同じ（リンクのある場所を基準に解決する）。
    #[test]
    fn editing_through_a_relative_dangling_symlink_keeps_the_link() {
        let dir = scratch("dangling_relative");
        fs::create_dir_all(dir.join("dotfiles")).unwrap();
        let link = dir.join("hshrc");
        std::os::unix::fs::symlink("dotfiles/hshrc", &link).unwrap();

        save_to(&link, &["edited"]);

        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(dir.join("dotfiles/hshrc")).unwrap(), "edited\n");
    }

    /// リンク先の**親ディレクトリ**が無いときは、黙って作らずに断る
    /// （`ensure_parent_dir` の流儀＝エディタは勝手にディレクトリを生やさない）。
    #[test]
    fn a_dangling_symlink_into_a_missing_directory_is_refused() {
        let dir = scratch("dangling_no_parent");
        let link = dir.join("hshrc");
        std::os::unix::fs::symlink(dir.join("no_such_dir/hshrc"), &link).unwrap();

        let err = write_buffer(&editor_with(&["edited"]), &link, "test").unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(
            fs::symlink_metadata(&link).unwrap().file_type().is_symlink(),
            "断ったのに symlink を壊している"
        );
    }

    /// `~/.hshrc` を dotfiles リポへ symlink している人が cozy で編集しても、
    /// **リンクが実体ファイルに化けない**（素朴な rename はここで壊す）。
    #[test]
    fn editing_through_a_symlink_keeps_the_link() {
        let dir = scratch("symlink");
        fs::create_dir_all(dir.join("dotfiles")).unwrap();
        let real = dir.join("dotfiles/hshrc");
        fs::write(&real, "original\n").unwrap();
        let link = dir.join("hshrc");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        save_to(&link, &["edited"]);

        assert!(
            fs::symlink_metadata(&link).unwrap().file_type().is_symlink(),
            "symlink が実体ファイルに置き換わった"
        );
        // 編集がリンク先＝リポジトリ側に届いていること（届かないと黙って乖離する）
        assert_eq!(fs::read_to_string(&real).unwrap(), "edited\n");
    }

    /// 預かったファイルの mode を変えない（0600 の rc が 0664 になって漏れない）。
    #[test]
    fn preserves_the_mode_of_the_file_it_was_given() {
        let dir = scratch("mode");
        let path = dir.join("secret.conf");
        fs::write(&path, "old\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        save_to(&path, &["new"]);

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "mode が rename で飛んだ");
    }

    /// ハードリンクされたファイルは rename すると相方が取り残される。
    /// その場書きへ落ちて inode を保つ（nano と同じ振る舞い）。
    #[test]
    fn hardlinked_file_stays_the_same_inode() {
        let dir = scratch("hardlink");
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        fs::write(&a, "old\n").unwrap();
        fs::hard_link(&a, &b).unwrap();
        let ino = fs::metadata(&a).unwrap().ino();

        save_to(&a, &["new"]);

        assert_eq!(fs::metadata(&a).unwrap().ino(), ino, "inode が変わった＝リンクが切れた");
        assert_eq!(fs::read_to_string(&b).unwrap(), "new\n", "相方に反映されていない");
    }

    /// 普通のファイルは rename で置換される（＝保存中の姿が観測されない）。
    #[test]
    fn plain_file_is_replaced_by_rename() {
        let dir = scratch("plain");
        let path = dir.join("note.md");
        fs::write(&path, "old\n").unwrap();
        let ino = fs::metadata(&path).unwrap().ino();

        save_to(&path, &["new", "lines"]);

        assert_ne!(fs::metadata(&path).unwrap().ino(), ino, "その場書きに落ちている");
        assert_eq!(fs::read_to_string(&path).unwrap(), "new\nlines\n");
    }

    /// 一時ファイルを置き去りにしない（`.note.md.tmp` が残ると壊れて見える）。
    #[test]
    fn leaves_no_temp_file_behind() {
        let dir = scratch("strays");
        save_to(&dir.join("note.md"), &["x"]);

        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != "note.md")
            .collect();
        assert!(names.is_empty(), "残骸: {names:?}");
    }

    /// 新規ファイルもちゃんと作れる（存在しないので canonicalize は失敗する経路）。
    #[test]
    fn creates_a_new_file() {
        let dir = scratch("new");
        let path = dir.join("fresh.md");
        save_to(&path, &["hello"]);
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
    }
}
