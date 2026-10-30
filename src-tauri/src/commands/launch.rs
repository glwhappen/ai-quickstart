use std::path::PathBuf;
use std::process::Command;

use super::detect;

/// 生成一个启动脚本，再交给系统终端打开：
/// 这样不用在命令里拼一堆引号，Windows 的 cmd 与 macOS/Linux 的 shell 行为都可控。
fn write_launcher(exe: &PathBuf, codex_home: &PathBuf) -> Result<PathBuf, String> {
    let dir = super::data_root();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败：{e}"))?;

    #[cfg(target_os = "windows")]
    {
        let path = dir.join("launch-codex.cmd");
        let script = format!(
            "@echo off\r\nset \"CODEX_HOME={}\"\r\ncd /d \"%USERPROFILE%\"\r\n\"{}\"\r\n",
            codex_home.display(),
            exe.display()
        );
        std::fs::write(&path, script).map_err(|e| format!("写入启动脚本失败：{e}"))?;
        Ok(path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let path = dir.join("launch-codex.sh");
        let script = format!(
            "#!/bin/sh\n# 由「Codex 快速配置」生成\nexport CODEX_HOME='{}'\ncd \"$HOME\"\nexec '{}'\n",
            codex_home.display(),
            exe.display()
        );
        std::fs::write(&path, script).map_err(|e| format!("写入启动脚本失败：{e}"))?;
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
        }
        Ok(path)
    }
}

/// 「启动 Codex」：带上我们的 CODEX_HOME，在用户主目录里开一个终端窗口跑 Codex。
pub fn launch_codex() -> Result<String, String> {
    let Some((ver, exe)) = detect::prepared_version() else {
        return Err("Codex 还没准备好，请先点上面的准备按钮".to_string());
    };
    let home = super::codex_home();
    let launcher = write_launcher(&exe, &home)?;

    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/c", "start", "", &launcher.display().to_string()]);
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        cmd.spawn()
            .map_err(|e| format!("打开终端失败：{e}"))?;
        return Ok(format!("已在新窗口启动 Codex {ver}"));
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        cmd.args(["-a", "Terminal", &launcher.display().to_string()]);
        cmd.spawn().map_err(|e| format!("打开终端失败：{e}"))?;
        return Ok(format!("已在终端窗口启动 Codex {ver}"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let p = launcher.display().to_string();
        let candidates: Vec<(&str, Vec<&str>)> = vec![
            ("x-terminal-emulator", vec!["-e", &p]),
            ("gnome-terminal", vec!["--", &p]),
            ("konsole", vec!["-e", &p]),
            ("xfce4-terminal", vec!["-x", &p]),
            ("xterm", vec!["-e", &p]),
        ];
        for (bin, args) in candidates {
            let mut cmd = Command::new(bin);
            cmd.args(args);
            if cmd.spawn().is_ok() {
                return Ok(format!("已在 {bin} 窗口启动 Codex {ver}"));
            }
        }
        Err("没找到可用的终端程序（试过 x-terminal-emulator / gnome-terminal / konsole / xfce4-terminal / xterm）".to_string())
    }
}

/// 「清理并退出」：删掉我们自己产生的全部文件。用户自己的 `~/.codex` 不碰。
pub fn cleanup() -> Result<String, String> {
    super::prepare::cleanup()
}
