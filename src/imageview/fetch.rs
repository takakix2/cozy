//! URL の画像を取りに行く係（Markdown プレビューの `i`・feature `remote-images`）。
//!
//! # 約束
//!
//! - **頼まれない限り通信しない。** プレビューを描くだけでは何も起きない —— 取りに行くのは
//!   利用者が `i` を押したときだけ（[`request`]）。`.md` を開いたことが画像のサーバに伝わる
//!   （メールが外の画像を既定で止めるのと同じ理由）ので、既定は止めておく。
//! - **覚えるのは開いている間だけ。** 取ったバイト列はこのプロセスのメモリにだけ持ち、
//!   ディスクには一度も書かない（何を見たかが残らない）。
//! - **`curl` を呼ぶ。** 依存を増やさず、プロキシや証明書の設定も OS のまま効く。
//!   無い台では「curl not found」と出すだけ。
//! - `http(s)` だけ（リダイレクト先も）・10 秒で諦める・1 枚 20 MB まで。
//!
//! 取得は裏のスレッド（同時に [`MAX_WORKERS`] 本まで）。終わるたびに [`generation`] が進むので、
//! 描画ループはそれを見て描き直す。

use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// 1 枚の上限（バイト）。
pub(crate) const MAX_BYTES: usize = 20 * 1024 * 1024;
/// 同時に走らせる curl の数。
const MAX_WORKERS: usize = 4;

/// 1 つの URL の取得の状態（描く側が見る形）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Pending,
    Ready,
    Failed(String),
}

enum Entry {
    Pending,
    Ready(Arc<Vec<u8>>),
    Failed(String),
}

fn table() -> &'static Mutex<HashMap<String, Entry>> {
    static TABLE: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

fn lock() -> std::sync::MutexGuard<'static, HashMap<String, Entry>> {
    // 取得係のスレッドが panic しても表は使い続ける（中身は 1 件ずつ独立）。
    table().lock().unwrap_or_else(|e| e.into_inner())
}

/// その URL を取りに行ったことがあるか・どうなったか。頼まれていなければ `None`。
pub(crate) fn status(url: &str) -> Option<Status> {
    lock().get(url).map(|e| match e {
        Entry::Pending => Status::Pending,
        Entry::Ready(_) => Status::Ready,
        Entry::Failed(why) => Status::Failed(why.clone()),
    })
}

/// 取れた画像のバイト列。
pub(crate) fn bytes(url: &str) -> Option<Arc<Vec<u8>>> {
    match lock().get(url) {
        Some(Entry::Ready(b)) => Some(Arc::clone(b)),
        _ => None,
    }
}

/// まだ終わっていない取得が 1 つでもあるか。
pub(crate) fn any_pending() -> bool {
    lock().values().any(|e| matches!(e, Entry::Pending))
}

/// 取得が 1 つ終わるたびに進む数。描画ループが「描き直す理由ができたか」を見るのに使う。
pub(crate) fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// `urls` のうち、まだ頼んでいない物を裏で取りに行く。始めた数を返す
/// （同じ URL は 2 度取りに行かない —— 失敗した物も、開き直すまでそのまま）。
pub(crate) fn request(urls: &[String]) -> usize {
    request_with(&[OsString::from("curl")], urls)
}

/// `command` は撃つプログラムと、その先頭の引数（本番は `["curl"]`）。
fn request_with(command: &[OsString], urls: &[String]) -> usize {
    let mut fresh: VecDeque<String> = VecDeque::new();
    {
        let mut t = lock();
        for url in urls {
            if !t.contains_key(url) {
                t.insert(url.clone(), Entry::Pending);
                fresh.push_back(url.clone());
            }
        }
    }
    let started = fresh.len();
    if started == 0 {
        return 0;
    }
    let queue = Arc::new(Mutex::new(fresh));
    let command: Arc<[OsString]> = command.into();
    for _ in 0..started.min(MAX_WORKERS) {
        let queue = Arc::clone(&queue);
        let command = Arc::clone(&command);
        std::thread::spawn(move || {
            loop {
                let next = queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front();
                let Some(url) = next else { break };
                let entry = match fetch_with(&command, &url) {
                    Ok(bytes) => Entry::Ready(Arc::new(bytes)),
                    Err(why) => Entry::Failed(why),
                };
                lock().insert(url, entry);
                GENERATION.fetch_add(1, Ordering::AcqRel);
            }
        });
    }
    started
}

/// 検体用: curl を撃たずに、取れた／取れなかった状態を表に置く。
#[cfg(test)]
pub(crate) fn settle_for_test(url: &str, result: Result<Vec<u8>, String>) {
    let entry = match result {
        Ok(b) => Entry::Ready(Arc::new(b)),
        Err(why) => Entry::Failed(why),
    };
    lock().insert(url.to_string(), entry);
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

/// 1 枚を `command`（本番は `["curl"]`）で取る。失敗は 1 行の理由で返す。
fn fetch_with(command: &[OsString], url: &str) -> Result<Vec<u8>, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("not an http(s) URL".to_string());
    }
    let (program, lead) = command
        .split_first()
        .ok_or_else(|| "no command".to_string())?;
    let mut child = Command::new(program)
        .args(lead)
        .args([
            "-sSfL",
            "--max-time",
            "10",
            "--connect-timeout",
            "5",
            "--max-filesize",
            "20M",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "-o",
            "-",
            "--url",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "curl not found".to_string(),
            _ => format!("cannot run curl: {e}"),
        })?;

    let mut body = Vec::new();
    let read = child
        .stdout
        .take()
        .map(|out| out.take(MAX_BYTES as u64 + 1).read_to_end(&mut body));
    if body.len() > MAX_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err("larger than 20 MB".to_string());
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let status = child.wait().map_err(|e| format!("curl: {e}"))?;
    if let Some(Err(e)) = read {
        return Err(format!("curl: {e}"));
    }
    if !status.success() {
        let line = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        return Err(if line.is_empty() {
            match status.code() {
                Some(code) => format!("curl exited with {code}"),
                None => "curl was stopped".to_string(),
            }
        } else {
            line.trim().to_string()
        });
    }
    if body.is_empty() {
        return Err("empty response".to_string());
    }
    Ok(body)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::{Duration, Instant};

    /// curl の代わりに撃つ小さなスクリプト（引数は見ない）。
    ///
    /// ⚠️ **exec せず `sh <script>` で走らせる** —— 書いたばかりのスクリプトを直に exec すると、
    /// 並列の他の検体が fork している間は書き込みの fd を握られていて `Text file busy` で
    /// 落ちる（単体では緑でも、全体を回すと赤になる）。
    fn fake_curl(dir: &Path, name: &str, body: &str) -> Vec<OsString> {
        let path = dir.join(name);
        std::fs::write(&path, format!("{body}\n")).unwrap();
        vec![OsString::from("/bin/sh"), path.into_os_string()]
    }

    fn wait_until_done(url: &str) -> Status {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match status(url) {
                Some(Status::Pending) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Some(s) => return s,
                None => panic!("{url} は頼まれていない"),
            }
        }
    }

    #[test]
    fn a_fetch_hands_back_what_curl_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nrest").unwrap();
        let curl = fake_curl(dir.path(), "curl", &format!("cat '{}'", png.display()));
        let url = "https://example.test/fetch-ok.png".to_string();

        assert_eq!(request_with(&curl, std::slice::from_ref(&url)), 1);
        assert_eq!(wait_until_done(&url), Status::Ready);
        assert_eq!(bytes(&url).unwrap().as_slice(), b"\x89PNG\r\n\x1a\nrest");
    }

    #[test]
    fn the_same_url_is_fetched_once() {
        let dir = tempfile::tempdir().unwrap();
        let curl = fake_curl(dir.path(), "curl", "printf x");
        let url = "https://example.test/fetch-once.png".to_string();
        let twice = vec![url.clone(), url.clone()];

        assert_eq!(request_with(&curl, &twice), 1);
        wait_until_done(&url);
        assert_eq!(request_with(&curl, &twice), 0);
    }

    #[test]
    fn a_curl_failure_says_what_curl_said() {
        let dir = tempfile::tempdir().unwrap();
        let curl = fake_curl(
            dir.path(),
            "curl",
            "echo 'curl: (22) The requested URL returned error: 404' >&2; exit 22",
        );
        assert_eq!(
            fetch_with(&curl, "https://example.test/404.png"),
            Err("curl: (22) The requested URL returned error: 404".to_string())
        );
    }

    #[test]
    fn more_than_the_limit_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let curl = fake_curl(
            dir.path(),
            "curl",
            &format!("head -c {} /dev/zero", MAX_BYTES + 1),
        );
        assert_eq!(
            fetch_with(&curl, "https://example.test/huge.png"),
            Err("larger than 20 MB".to_string())
        );
    }

    /// 対照: ちょうど上限なら受ける（上の検体が「何でも断る」で緑になっていないこと）。
    #[test]
    fn exactly_the_limit_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let curl = fake_curl(
            dir.path(),
            "curl",
            &format!("head -c {MAX_BYTES} /dev/zero"),
        );
        let got = fetch_with(&curl, "https://example.test/limit.png").unwrap();
        assert_eq!(got.len(), MAX_BYTES);
    }

    #[test]
    fn a_missing_curl_is_named() {
        assert_eq!(
            fetch_with(
                &[OsString::from("/nonexistent/cozy-test/curl")],
                "https://example.test/x.png"
            ),
            Err("curl not found".to_string())
        );
    }

    #[test]
    fn only_http_and_https_are_fetched() {
        let dir = tempfile::tempdir().unwrap();
        let curl = fake_curl(dir.path(), "curl", "printf x");
        for url in ["file:///etc/passwd", "ftp://example.test/a.png", "-K/etc/x"] {
            assert_eq!(
                fetch_with(&curl, url),
                Err("not an http(s) URL".to_string()),
                "{url}"
            );
        }
    }
}
