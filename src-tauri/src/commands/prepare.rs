use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

use super::tools;

fn emit(app: Option<&AppHandle>, level: &str, line: &str) {
    if let Some(app) = app {
        let _ = app.emit(
            "prepare-log",
            serde_json::json!({ "level": level, "line": line }),
        );
    }
}

pub async fn prepare_codex(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || run_prepare(Some(app)))
        .await
        .map_err(|e| format!("下载线程异常：{e}"))?
}

/// 无界面调用（`--headless prepare`），供自动化验收使用。
pub fn prepare_sync() -> Result<String, String> {
    run_prepare(None)
}

fn run_prepare(app: Option<AppHandle>) -> Result<String, String> {
    let tool = tools::primary();
    let log = move |level: &str, line: &str| emit(app.as_ref(), level, line);
    let out = tool.prepare(&log)?;
    Ok(format!("{} ({})", out.version, out.path))
}

/// 删除本工具产生的全部文件（数据目录）。
pub fn cleanup() -> Result<String, String> {
    let root = super::data_root();
    if !root.exists() {
        return Ok(format!("没有需要清理的文件（{} 不存在）", root.display()));
    }
    // 用户自己装了系统级 Codex 的，那部分不是我们产生的，不碰。
    std::fs::remove_dir_all(&root).map_err(|e| format!("删除 {} 失败：{e}", root.display()))?;
    let mut msg = format!("已删除 {}", root.display());
    // 曾经勾选过「桌面版也能用」的话，用户自己的 config.toml 里有我们写的段；
    // 这里只提示备份位置，不擅自改回（用户可能已经自己改过）。
    if let Ok(entries) = std::fs::read_dir(super::user_codex_home()) {
        let backups: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("config.toml.bak."))
            .map(|e| e.path().display().to_string())
            .collect();
        if let Some(b) = backups.last() {
            msg.push_str(&format!(
                "。注意：你选过「桌面版也能用这个 Key」，它在 {} 留下了备份 {}，需要还原就把它拷回 config.toml。",
                super::user_codex_home().display(),
                b
            ));
        }
    }
    Ok(msg)
}

#[allow(dead_code)]
fn unused(_p: PathBuf) {}
