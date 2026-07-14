use std::path::PathBuf;

pub(crate) fn current_working_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub(crate) fn startup_args() -> Vec<String> {
    std::env::args().collect()
}

/// Low-spec / mobile host hint. When `COZY_COMPACT` is set (and not empty or
/// "0"), cozy uses the lightweight welcome and hides line numbers by default,
/// cutting per-frame cells on full-repaint GPUs (e.g. an Android tablet's
/// Mali-G52 driven by hsh-ios). The host sets it — cozy can't detect the GPU —
/// and cozy just honours it. Cached: the env doesn't change mid-run.
pub(crate) fn compact() -> bool {
    use std::sync::OnceLock;
    static COMPACT: OnceLock<bool> = OnceLock::new();
    *COMPACT.get_or_init(|| {
        std::env::var("COZY_COMPACT")
            .map(|v| !v.is_empty() && v != "0")
            .unwrap_or(false)
    })
}
