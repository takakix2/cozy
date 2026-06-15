use std::path::PathBuf;

pub(crate) fn current_working_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub(crate) fn startup_args() -> Vec<String> {
    std::env::args().collect()
}
