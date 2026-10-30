//! 自己注册 URL scheme `codexquick`（不依赖插件的 GUI-only 注册），
//! 这样「绿色版」也能在无界面模式下注册/撤销，`cleanup` 能把痕迹擦干净。
//!
//! - Windows：写 `HKCU\Software\Classes\codexquick`（当前用户，不需要管理员）
//! - Linux：写用户级 `~/.local/share/applications/*.desktop` + `xdg-mime default`
//! - macOS：`CFBundleURLTypes` 在打包进 .app 时由 bundle 配置负责，这里直接跳过
//!
//! 接收端不依赖注册：冷启动看 argv，运行中由 single-instance 转发 argv。

use std::path::PathBuf;
use std::process::Command;

pub const SCHEME: &str = "codexquick";

fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("取程序路径失败：{e}"))
}

#[cfg(target_os = "windows")]
fn reg(args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new("reg");
    cmd.args(args);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // 不弹黑窗
    }
    let out = cmd.output().map_err(|e| format!("调用 reg.exe 失败：{e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "reg.exe 失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(target_os = "windows")]
pub fn register() -> Result<String, String> {
    let exe = exe()?;
    let exe_s = exe.display().to_string();
    let base = format!("HKCU\\Software\\Classes\\{SCHEME}");
    reg(&[
        "add",
        &base,
        "/ve",
        "/t",
        "REG_SZ",
        "/d",
        &format!("URL:{SCHEME} protocol"),
        "/f",
    ])?;
    reg(&[
        "add",
        &base,
        "/v",
        "URL Protocol",
        "/t",
        "REG_SZ",
        "/d",
        "",
        "/f",
    ])?;
    reg(&[
        "add",
        &format!("{base}\\DefaultIcon"),
        "/ve",
        "/t",
        "REG_SZ",
        "/d",
        &format!("{exe_s},0"),
        "/f",
    ])?;
    reg(&[
        "add",
        &format!("{base}\\shell\\open\\command"),
        "/ve",
        "/t",
        "REG_SZ",
        "/d",
        &format!("\"{exe_s}\" \"%1\""),
        "/f",
    ])?;
    Ok(format!("已注册 {SCHEME}:// → {exe_s}"))
}

#[cfg(target_os = "windows")]
pub fn unregister() -> Result<String, String> {
    let base = format!("HKCU\\Software\\Classes\\{SCHEME}");
    reg(&["delete", &base, "/f"])?;
    Ok(format!("已注销 {SCHEME}://"))
}

#[cfg(target_os = "linux")]
fn desktop_path() -> Result<PathBuf, String> {
    let data = match std::env::var("XDG_DATA_HOME") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => super::home_dir().join(".local").join("share"),
    };
    Ok(data.join("applications").join("codex-quickstart-handler.desktop"))
}

#[cfg(target_os = "linux")]
pub fn register() -> Result<String, String> {
    let exe = exe()?;
    let path = desktop_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=Codex 快速配置\nExec=\"{}\" %u\nTerminal=false\nNoDisplay=true\nMimeType=x-scheme-handler/{SCHEME};\n",
        exe.display()
    );
    std::fs::write(&path, body).map_err(|e| format!("写入 .desktop 失败：{e}"))?;
    // 让桌面环境知道 x-scheme-handler 归我们
    let _ = Command::new("update-desktop-database")
        .arg(path.parent().unwrap_or(&path))
        .output();
    let _ = Command::new("xdg-mime")
        .args([
            "default",
            "codex-quickstart-handler.desktop",
            &format!("x-scheme-handler/{SCHEME}"),
        ])
        .output();
    Ok(format!("已注册 {SCHEME}:// → {}", path.display()))
}

#[cfg(target_os = "linux")]
pub fn unregister() -> Result<String, String> {
    let path = desktop_path()?;
    let existed = path.exists();
    if existed {
        std::fs::remove_file(&path).map_err(|e| format!("删除 .desktop 失败：{e}"))?;
    }
    Ok(if existed {
        format!("已注销 {SCHEME}://")
    } else {
        format!("没有注册过 {SCHEME}://")
    })
}

#[cfg(target_os = "macos")]
pub fn register() -> Result<String, String> {
    Ok("macOS 的 scheme 由 .app 的 CFBundleURLTypes 提供，无需运行时注册".to_string())
}

#[cfg(target_os = "macos")]
pub fn unregister() -> Result<String, String> {
    Ok("macOS 无需注销".to_string())
}

/// 已经是我们的程序处理这个 scheme 吗（重复注册时避免无谓写盘）。
#[cfg(target_os = "windows")]
pub fn is_registered() -> bool {
    let out = Command::new("reg")
        .args([
            "query",
            &format!("HKCU\\Software\\Classes\\{SCHEME}\\shell\\open\\command"),
            "/ve",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).to_lowercase();
            exe()
                .map(|e| s.contains(&e.display().to_string().to_lowercase()))
                .unwrap_or(false)
        }
        _ => false,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_registered() -> bool {
    #[cfg(target_os = "linux")]
    {
        desktop_path().map(|p| p.exists()).unwrap_or(false)
    }
    #[cfg(target_os = "macos")]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_name_is_fixed() {
        // 官网那边按这个拼链接，不能改
        assert_eq!(SCHEME, "codexquick");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_entry_has_scheme_handler() {
        let body = format!(
            "[Desktop Entry]\nType=Application\nExec=\"/tmp/x\" %u\nMimeType=x-scheme-handler/{SCHEME};\n"
        );
        assert!(body.contains("x-scheme-handler/codexquick"));
        assert!(body.contains("%u"));
    }
}
