pub mod config;
pub mod deeplink;
pub mod detect;
pub mod launch;
pub mod prepare;
pub mod scheme;
pub mod sso;
pub mod test;
pub mod tools;

use std::path::PathBuf;

/// 本工具自己的数据根目录（绿色版：全部产物都在这里，删掉即卸载）。
/// `AIQUICK_HOME` 可覆盖，便于用假 HOME 做验收。
pub fn data_root() -> PathBuf {
    if let Ok(v) = std::env::var("AIQUICK_HOME") {
        if !v.trim().is_empty() {
            return PathBuf::from(v);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(v) = std::env::var("LOCALAPPDATA") {
            if !v.trim().is_empty() {
                return PathBuf::from(v).join("ai-quickstart");
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        return home_dir()
            .join("Library")
            .join("Application Support")
            .join("ai-quickstart");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(v) = std::env::var("XDG_DATA_HOME") {
            if !v.trim().is_empty() {
                return PathBuf::from(v).join("ai-quickstart");
            }
        }
        return home_dir().join(".local").join("share").join("ai-quickstart");
    }
    #[allow(unreachable_code)]
    home_dir().join(".ai-quickstart")
}

pub fn home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(v) = std::env::var("USERPROFILE") {
            if !v.trim().is_empty() {
                return PathBuf::from(v);
            }
        }
        let drive = std::env::var("HOMEDRIVE").unwrap_or_default();
        let path = std::env::var("HOMEPATH").unwrap_or_default();
        if !drive.is_empty() || !path.is_empty() {
            return PathBuf::from(format!("{drive}{path}"));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(v) = std::env::var("HOME") {
            if !v.trim().is_empty() {
                return PathBuf::from(v);
            }
        }
    }
    PathBuf::from(".")
}

/// 当前主工具的数据目录（解压产物）。
pub fn codex_dir() -> PathBuf {
    tools::primary().dir()
}

/// 当前主工具的隔离配置目录（启动时作为 `CODEX_HOME` 注入）。
pub fn codex_home() -> PathBuf {
    tools::primary().home()
}

pub fn config_path() -> PathBuf {
    tools::primary().config_path()
}

/// 用户自己的 Codex 配置目录（只有桌面版的可选开关会碰它）。
pub fn user_codex_home() -> PathBuf {
    home_dir().join(".codex")
}

pub fn exe_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "codex.exe"
    } else {
        "codex"
    }
}

/// 版本号按 x.y.z 数字比较（npmmirror 上是语义化版本）。
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<u64> = a
        .split(['.', '-'])
        .map(|s| s.parse::<u64>().unwrap_or(0))
        .collect();
    let pb: Vec<u64> = b
        .split(['.', '-'])
        .map(|s| s.parse::<u64>().unwrap_or(0))
        .collect();
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        if x != y {
            return x.cmp(&y);
        }
    }
    std::cmp::Ordering::Equal
}
