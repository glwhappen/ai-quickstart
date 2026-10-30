use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::{codex_dir, detect};

/// 把 npmmirror 上的 Codex 原生二进制解压到**我们自己的目录**，直接以该路径运行：
/// 不写 PATH、不建软链、不装进系统目录、不碰 `~/.local/bin`、不用官方的
/// `~/.codex/packages/standalone/...` 布局（那套只为官方 installer 的版本识别/自动升级服务，
/// 我们不管升级——换版本就是再解压一个目录）。
///
/// 下载技巧沿用 `/root/ccs-brand/src-tauri/src/commands/misc.rs` 的镜像直下实现
/// （npmmirror + 取 `/latest` 动态版本 + curl/tar，绕过国内不可达的官方 installer 和 Node 依赖），
/// 只是去掉了软链/junction 部分。
#[cfg(not(target_os = "windows"))]
const PREPARE_UNIX: &str = r#"bash -c '
set -e
M=https://registry.npmmirror.com
V=$(curl -fsSL "$M/@openai/codex/latest" | sed -n "s/.*\"version\":\"\([^\"]*\)\".*/\1/p" | head -1)
[ -n "$V" ] || exit 20
case "$(uname -s)" in Linux) O=linux ;; Darwin) O=darwin ;; *) exit 21 ;; esac
case "$(uname -m)" in x86_64|amd64) A=x64 ;; arm64|aarch64) A=arm64 ;; *) exit 22 ;; esac
DEST="$QK_CODEX_DIR/$V"
TMP=$(mktemp -d)
curl -fsSL --retry 3 --connect-timeout 20 "$M/@openai/codex/-/codex-$V-$O-$A.tgz" -o "$TMP/p.tgz" || { rm -rf "$TMP"; exit 23; }
tar xzf "$TMP/p.tgz" -C "$TMP" || { rm -rf "$TMP"; exit 24; }
SRC=$(find "$TMP/package/vendor" -mindepth 1 -maxdepth 1 -type d | head -1)
[ -n "$SRC" ] || { rm -rf "$TMP"; exit 25; }
mkdir -p "$QK_CODEX_DIR"
rm -rf "$DEST"
mv "$SRC" "$DEST"
rm -rf "$TMP"
chmod +x "$DEST/bin/codex" 2>/dev/null || true
"$DEST/bin/codex" --version'"#;

#[cfg(target_os = "windows")]
const PREPARE_WINDOWS: &str = r#"
$ErrorActionPreference = 'Stop'
$M = 'https://registry.npmmirror.com'
$V = (Invoke-RestMethod -Uri "$M/@openai/codex/latest" -TimeoutSec 25).version
if (-not $V) { throw 'mirror: version parse failed' }
if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { $A = 'arm64' } else { $A = 'x64' }
$DEST = Join-Path $env:QK_CODEX_DIR $V
$T = Join-Path $env:TEMP ('codex-quickstart-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $T -Force | Out-Null
try {
  $tgz = Join-Path $T 'p.tgz'
  Invoke-WebRequest -Uri "$M/@openai/codex/-/codex-$V-win32-$A.tgz" -OutFile $tgz -TimeoutSec 900
  tar.exe -xzf $tgz -C $T
  if ($LASTEXITCODE -ne 0) { throw "mirror: tar failed ($LASTEXITCODE)" }
  $vendor = Get-ChildItem -Path (Join-Path $T 'package\vendor') -Directory -ErrorAction Stop |
            Select-Object -First 1
  if (-not $vendor) { throw 'mirror: vendor dir not found' }
  New-Item -ItemType Directory -Path (Split-Path $DEST -Parent) -Force | Out-Null
  if (Test-Path $DEST) { Remove-Item $DEST -Recurse -Force }
  Move-Item $vendor.FullName $DEST
  & (Join-Path $DEST 'bin\codex.exe') --version
} finally {
  Remove-Item $T -Recurse -Force -ErrorAction SilentlyContinue
}
"#;

#[cfg(target_os = "windows")]
fn build_command(dest_dir: &str) -> Command {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let utf16: Vec<u8> = PREPARE_WINDOWS
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let encoded = STANDARD.encode(utf16);
    let mut cmd = Command::new("powershell");
    cmd.env("QK_CODEX_DIR", dest_dir);
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-EncodedCommand",
        &encoded,
    ]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

#[cfg(not(target_os = "windows"))]
fn build_command(dest_dir: &str) -> Command {
    let mut cmd = Command::new("bash");
    cmd.env("QK_CODEX_DIR", dest_dir);
    cmd.arg("-c").arg(PREPARE_UNIX);
    cmd
}

fn emit(app: Option<&AppHandle>, level: &str, line: impl Into<String>) {
    if let Some(app) = app {
        let _ = app.emit(
            "prepare-log",
            serde_json::json!({ "level": level, "line": line.into() }),
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
    // 已经解压过就直接用，不重复下 100MB。
    if let Some((ver, exe)) = detect::prepared_version() {
        emit(app.as_ref(), "phase", format!("已备好 Codex {ver}，跳过下载"));
        return Ok(format!("{ver} ({})", exe.display()));
    }
    let dest = codex_dir();
    std::fs::create_dir_all(&dest).map_err(|e| format!("创建目录失败：{e}"))?;
    emit(app.as_ref(), "phase", "正在从国内镜像下载 Codex（约 100MB，请保持网络）…");

    let mut cmd = build_command(&dest.display().to_string());
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| format!("无法启动下载程序：{e}"))?;

    let mut handles = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let a = app.clone();
        handles.push(std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    emit(a.as_ref(), "out", line);
                }
            }
        }));
    }
    if let Some(stderr) = child.stderr.take() {
        let a = app.clone();
        handles.push(std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    emit(a.as_ref(), "err", line);
                }
            }
        }));
    }

    let start = Instant::now();
    let timeout = Duration::from_secs(30 * 60);
    let mut last_tick = 0u64;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                let elapsed = start.elapsed();
                if elapsed > timeout {
                    let _ = child.kill();
                    return Err("下载超时（30 分钟）".to_string());
                }
                if elapsed.as_secs() / 10 > last_tick {
                    last_tick = elapsed.as_secs() / 10;
                    emit(
                        app.as_ref(),
                        "tick",
                        format!("已等待 {} 秒…", elapsed.as_secs()),
                    );
                }
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(e) => return Err(format!("等待下载进程失败：{e}")),
        }
    };
    for h in handles {
        let _ = h.join();
    }

    if !status.success() {
        return Err(format!(
            "下载或解压失败（退出码 {}），请检查网络后重试",
            status.code().unwrap_or(-1)
        ));
    }

    let (ver, exe) = detect::prepared_version()
        .ok_or_else(|| "解压完成但没找到 codex 可执行文件，请重试".to_string())?;
    emit(app.as_ref(), "phase", format!("准备完成：{ver}"));
    Ok(format!("{ver} ({})", exe.display()))
}

/// 删除本工具产生的全部文件（数据目录），并把发现的情况回报给界面。
pub fn cleanup() -> Result<String, String> {
    let root = super::data_root();
    if !root.exists() {
        return Ok(format!("没有需要清理的文件（{} 不存在）", root.display()));
    }
    // 用户自己装了系统级 Codex 的，那部分不是我们产生的，不碰。
    std::fs::remove_dir_all(&root)
        .map_err(|e| format!("删除 {} 失败：{e}", root.display()))?;
    let mut msg = format!("已删除 {}", root.display());
    // 曾经勾选过「桌面版也能用」的话，用户自己的 config.toml 里有我们写的段；
    // 这里只提示备份位置，不擅自改回（用户可能已经自己改过）。
    if let Ok(entries) = std::fs::read_dir(super::user_codex_home()) {
        let backups: Vec<String> = entries
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("config.toml.bak.")
            })
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
