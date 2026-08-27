use crate::state::Config;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG_TOML: &str = r##"# cozy config

page_size = 20
theme = "dark"
show_line_numbers = true
status_duration = 3

line_number_bg = "darkgray"
line_number_fg = "white"

footer_bg = "#222226"
footer_key_fg = "cyan"
footer_fg = "gray"

status_bar_bg = "darkgray"
status_bar_fg = "white"

cursor_blink = true

# Resting mode after actions: "edit" or "glide".
default_mode = "edit"

# Override only the shortcuts you want to change.
# [keys]
# enter_exit = "ctrl+x"
# toggle_markdown = "f2"
"##;

pub fn load() -> Config {
    load_from(None)
}

pub fn load_from(config_dir: Option<&PathBuf>) -> Config {
    let paths = candidate_paths(config_dir);

    if let Some(path) = default_config_path(config_dir) {
        if !path.exists() {
            if let Err(e) = write_default_config(&path) {
                // ⚠️ ここも同じ理由で画面には出さない。既定 config が作れなくても
                // 動作には効かない（既定値で走る）ので、黙って進む。
                let _ = e;
            }
        }
    }

    // 🚨 **苦情は `eprintln!` にしない。** cozy はこの端末で TUI を描いているので、
    // 標準エラーはエディタの絵の上に混ざる（`#2` を測っている最中に、1 行目が
    // 読めない状態になった）。⭐ 集めて持ち歩き、画面に出させる。
    let mut warnings = Vec::new();
    for path in &paths {
        if path.exists() {
            match load_from_path(path) {
                Ok(mut config) => {
                    config.load_warnings = warnings;
                    return config;
                }
                Err(e) => {
                    warnings.push(format!(
                        "Config ignored: {} ({e}) — using defaults",
                        crate::file_io::shorten_home(&path.display().to_string())
                    ));
                }
            }
        }
    }

    let mut config = Config::default_values();
    config.load_warnings = warnings;
    config
}

pub fn user_config_path(config_dir: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(dir) = config_dir {
        return Some(dir.join("config.toml"));
    }
    dirs::config_dir()
        .map(|p| p.join("cozy/config.toml"))
        .or_else(|| dirs::home_dir().map(|p| p.join(".cozy/config.toml")))
}

pub fn load_from_path(path: &Path) -> io::Result<Config> {
    let content = std::fs::read_to_string(path)?;
    toml::from_str::<Config>(&content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

pub fn ensure_default_config_file(config_dir: Option<&PathBuf>) -> io::Result<PathBuf> {
    let path = default_config_path(config_dir).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not resolve config directory",
        )
    })?;
    if !path.exists() {
        write_default_config(&path)?;
    }
    Ok(path)
}

fn candidate_paths(config_dir: Option<&PathBuf>) -> Vec<PathBuf> {
    if let Some(dir) = config_dir {
        return vec![dir.join("cozy.toml"), dir.join("config.toml")];
    }

    vec![
        dirs::config_dir()
            .map(|p| p.join("cozy/config.toml"))
            .unwrap_or_default(),
        PathBuf::from("config.toml"),
        dirs::home_dir()
            .map(|p| p.join(".cozy/config.toml"))
            .unwrap_or_default(),
    ]
}

fn default_config_path(config_dir: Option<&PathBuf>) -> Option<PathBuf> {
    user_config_path(config_dir)
}

fn write_default_config(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, DEFAULT_CONFIG_TOML)
}

/// 🚨 **設定は「書いた分だけ」効く。**
///
/// ⚠️ `page_size` が必須フィールドだった間、その行を書いていない `config.toml` は
/// **丸ごとパースに失敗**し、`[keys]` も配色も全部落ちていた。⭐ しかも落ちたことは
/// 標準エラーの 1 行でしか出ず、それが **TUI の絵の上に混ざって読めなかった**
/// （`#2` を測っている最中に踏んだ）。
#[cfg(test)]
mod partial_config {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("cozy_cfg_test_{}_{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn write_config(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("config.toml");
        fs::write(&path, body).unwrap();
        path
    }

    /// 🚨 **本丸** —— `[keys]` だけの config が、`[keys]` として効くこと。
    #[test]
    fn a_config_with_only_keys_still_carries_the_keys() {
        let dir = scratch("keys_only");
        let path = write_config(&dir, "[keys]\nenter_help = \"f5\"\n");
        let config = load_from_path(&path).expect("[keys] だけの config が読めない");
        let keys = config.keys.expect("keys が落ちている");
        assert_eq!(keys.get("enter_help").map(String::as_str), Some("f5"));
        // 書いていない欄は既定値で埋まる。
        assert_eq!(config.page_size, 20);
    }

    /// ⭐ 陽性対照 —— 書いた値は**書いたとおり**に効く（既定で上書きしない）。
    #[test]
    fn what_is_written_wins_over_the_default() {
        let dir = scratch("written");
        let path = write_config(&dir, "page_size = 7\n");
        assert_eq!(load_from_path(&path).unwrap().page_size, 7);
    }

    /// 🚨 **苦情は標準エラーではなく、持ち歩いて画面に出す。**
    /// ⚠️ cozy はその端末で TUI を描いているので、`eprintln!` は絵の上に混ざる。
    #[test]
    fn a_broken_config_is_reported_through_the_struct() {
        let dir = scratch("broken");
        write_config(&dir, "page_size = \"not a number\"\n");
        let config = load_from(Some(&dir));
        assert!(
            !config.load_warnings.is_empty(),
            "壊れた config を黙って無視している"
        );
        assert!(
            config.load_warnings[0].contains("Config ignored"),
            "苦情が理由を名乗っていない: {:?}",
            config.load_warnings
        );
        // 既定値で走れること。
        assert_eq!(config.page_size, 20);
    }

    /// ⭐ 陽性対照 —— 正しい config では苦情が出ない。
    /// これが無いと「常に苦情を出す」実装が緑で通る。
    #[test]
    fn a_good_config_says_nothing() {
        let dir = scratch("good");
        write_config(&dir, "page_size = 12\n");
        let config = load_from(Some(&dir));
        assert!(
            config.load_warnings.is_empty(),
            "正しい config に苦情が出ている: {:?}",
            config.load_warnings
        );
        assert_eq!(config.page_size, 12);
    }
}
