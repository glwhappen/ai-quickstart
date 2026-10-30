use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

use super::{codex_dir, codex_home, config_path, data_root, exe_name, user_codex_home, version_cmp};

#[derive(Serialize, Clone, Default)]
pub struct CodexStatus {
    /// 我们已经解压好、可以直接运行的 Codex 版本目录
    pub prepared: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub dir: Option<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct SystemCodex {
    /// 用户系统里另外还装着的 Codex CLI（只读探测，不用它跑）
    pub cli_found: bool,
    pub cli_path: Option<String>,
    pub cli_version: Option<String>,
    /// 桌面版
    pub desktop_supported: bool,
    pub desktop_found: bool,
    pub desktop_detail: String,
}

#[derive(Serialize, Clone, Default)]
pub struct EnvStatus {
    pub codex: CodexStatus,
    pub system: SystemCodex,
    pub os: String,
    pub data_root: String,
    pub codex_home: String,
    pub config_path: String,
    pub config_exists: bool,
    pub user_codex_home: String,
}

/// 扫描我们自己解压的版本目录，取最高版本。
pub fn prepared_version() -> Option<(String, PathBuf)> {
    let dir = codex_dir();
    let mut best: Option<(String, PathBuf)> = None;
    let entries = std::fs::read_dir(&dir).ok()?;
    for e in entries.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let ver = e.file_name().to_string_lossy().to_string();
        let exe = e.path().join("bin").join(exe_name());
        if !exe.is_file() {
            continue;
        }
        let better = match &best {
            None => true,
            Some((bv, _)) => version_cmp(&ver, bv) == std::cmp::Ordering::Greater,
        };
        if better {
            best = Some((ver, exe));
        }
    }
    best
}

fn version_of(exe: &PathBuf) -> Option<String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    let text = if out.stdout.is_empty() {
        String::from_utf8_lossy(&out.stderr).to_string()
    } else {
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|s| s.to_string())
}

fn check_codex() -> CodexStatus {
    match prepared_version() {
        Some((ver, exe)) => CodexStatus {
            prepared: true,
            version: version_of(&exe).or(Some(ver)),
            dir: exe.parent().and_then(|p| p.parent()).map(|p| p.display().to_string()),
            path: Some(exe.display().to_string()),
        },
        None => CodexStatus {
            dir: Some(codex_dir().display().to_string()),
            ..Default::default()
        },
    }
}

fn path_lookup(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn check_system() -> SystemCodex {
    let mut s = SystemCodex::default();
    // 只读探测：用户系统里可能另外装过 Codex，界面里只作提示，不用它运行。
    let mut candidates: Vec<PathBuf> = vec![
        user_codex_home()
            .join("packages")
            .join("standalone")
            .join("current")
            .join("bin")
            .join(exe_name()),
        super::home_dir().join(".local").join("bin").join(exe_name()),
    ];
    if cfg!(target_os = "windows") {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(local)
                    .join("Programs")
                    .join("OpenAI")
                    .join("Codex")
                    .join("bin")
                    .join(exe_name()),
            );
        }
    }
    if let Some(p) = candidates.into_iter().find(|p| p.is_file()) {
        s.cli_found = true;
        s.cli_version = version_of(&p);
        s.cli_path = Some(p.display().to_string());
    } else if let Some(p) = path_lookup(exe_name()) {
        s.cli_found = true;
        s.cli_version = version_of(&p);
        s.cli_path = Some(p.display().to_string());
    }

    let desktop = check_desktop();
    s.desktop_supported = desktop.0;
    s.desktop_found = desktop.1;
    s.desktop_detail = desktop.2;
    s
}

#[cfg(target_os = "windows")]
fn check_desktop() -> (bool, bool, String) {
    let script = "Get-AppxPackage | Where-Object { $_.Name -like 'OpenAI.Codex*' -or $_.Name -like '*ChatGPT*' } | Select-Object -First 1 -ExpandProperty Name";
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let found = cmd
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if found.is_empty() {
        (true, false, "未检测到已安装的桌面版".to_string())
    } else {
        (true, true, found)
    }
}

#[cfg(target_os = "macos")]
fn check_desktop() -> (bool, bool, String) {
    for p in ["/Applications/Codex.app", "/Applications/ChatGPT.app"] {
        if std::path::Path::new(p).exists() {
            return (true, true, p.to_string());
        }
    }
    (true, false, "未检测到 /Applications 下的 Codex.app 或 ChatGPT.app".to_string())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn check_desktop() -> (bool, bool, String) {
    (false, false, "当前系统没有 Codex 桌面版".to_string())
}

pub fn check_environment() -> EnvStatus {
    EnvStatus {
        codex: check_codex(),
        system: check_system(),
        os: std::env::consts::OS.to_string(),
        data_root: data_root().display().to_string(),
        codex_home: codex_home().display().to_string(),
        config_path: config_path().display().to_string(),
        config_exists: config_path().is_file(),
        user_codex_home: user_codex_home().display().to_string(),
    }
}
