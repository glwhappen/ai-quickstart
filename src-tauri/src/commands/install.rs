use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::detect;
use super::exe_name;

/// Codex 走 npmmirror 镜像直下，布局要**模仿官方 install.sh 产物**：
/// `<CODEX_HOME>/packages/standalone/releases/<ver>-<target>/` + `current` 软链，
/// `~/.local/bin/codex` 指向 `current/bin/codex`。
///
/// 实现**原样搬自** `/root/ccs-brand/src-tauri/src/commands/misc.rs`
/// （`CODEX_MIRROR_INSTALL_UNIX`，已实测可装出 codex 0.164.0），不重写：
/// 官方 installer 在国内不可达（`chatgpt.com/codex/install.sh` 连不上），npm 又要求先装 Node，
/// 而 npm 包里的「平台分包」本身就是原生二进制，所以 curl + tar + 摆放，一次装好。
///
/// 注意 tarball 内的 vendor 目录名是 Rust target triple（与 npm 包名的 os-arch 不同）。
#[cfg(not(target_os = "windows"))]
const CODEX_MIRROR_INSTALL_UNIX: &str = r#"bash -c '
set -e
M=https://registry.npmmirror.com
V=$(curl -fsSL "$M/@openai/codex/latest" | sed -n "s/.*\"version\":\"\([^\"]*\)\".*/\1/p" | head -1)
[ -n "$V" ] || exit 20
case "$(uname -s)" in Linux) O=linux ;; Darwin) O=darwin ;; *) exit 21 ;; esac
case "$(uname -m)" in
  x86_64|amd64) A=x64; TRIPLE=x86_64-unknown-linux-musl ;;
  arm64|aarch64) A=arm64; TRIPLE=aarch64-unknown-linux-musl ;;
  *) exit 22 ;;
esac
if [ "$O" = "darwin" ]; then
  case "$A" in x64) TRIPLE=x86_64-apple-darwin ;; *) TRIPLE=aarch64-apple-darwin ;; esac
fi
CH=${CODEX_HOME:-$HOME/.codex}
DEST="$CH/packages/standalone/releases/$V-$TRIPLE"
TMP=$(mktemp -d)
curl -fsSL --retry 3 --connect-timeout 20 "$M/@openai/codex/-/codex-$V-$O-$A.tgz" -o "$TMP/p.tgz" || { rm -rf "$TMP"; exit 23; }
tar xzf "$TMP/p.tgz" -C "$TMP" || { rm -rf "$TMP"; exit 24; }
SRC=$(find "$TMP/package/vendor" -mindepth 1 -maxdepth 1 -type d | head -1)
[ -n "$SRC" ] || { rm -rf "$TMP"; exit 25; }
mkdir -p "$(dirname "$DEST")"
rm -rf "$DEST"
mv "$SRC" "$DEST"
rm -rf "$TMP"
mkdir -p "$CH/packages/standalone"
ln -sfn "$DEST" "$CH/packages/standalone/current"
mkdir -p "$HOME/.local/bin"
ln -sf "$CH/packages/standalone/current/bin/codex" "$HOME/.local/bin/codex"
"$HOME/.local/bin/codex" --version'"#;

/// Codex 的 Windows 镜像直下（原样搬自 `misc.rs` 的 `CODEX_MIRROR_INSTALL_WINDOWS_SCRIPT`）。
/// 两个关键点：
/// 1. `<CODEX_HOME>\packages\standalone\releases\<ver>-<triple>\` 放整份 vendor 目录
///    （`bin\codex.exe` + `codex-resources` + `codex-path`），不能只抄 exe；
/// 2. 入口 `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin` 本身是一个 junction，指向 `current\bin`
///    —— real path 里带 `/packages/standalone/`，上游才认得这是官方独立安装。
/// junction 不需要开发者模式/管理员，比 symlink 稳。
#[cfg(target_os = "windows")]
const CODEX_MIRROR_INSTALL_WINDOWS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$M = 'https://registry.npmmirror.com'
$V = (Invoke-RestMethod -Uri "$M/@openai/codex/latest" -TimeoutSec 25).version
if (-not $V) { throw 'mirror: version parse failed' }
if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
  $A = 'arm64'; $TRIPLE = 'aarch64-pc-windows-msvc'
} else {
  $A = 'x64';   $TRIPLE = 'x86_64-pc-windows-msvc'
}
$CH = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $env:USERPROFILE '.codex' }
$DEST = Join-Path $CH "packages\standalone\releases\$V-$TRIPLE"
$CURRENT = Join-Path $CH 'packages\standalone\current'
$T = Join-Path $env:TEMP ('codex-mirror-' + [guid]::NewGuid().ToString('N'))
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
  if (Test-Path $CURRENT) { Remove-Item $CURRENT -Force -Recurse }
  New-Item -ItemType Junction -Path $CURRENT -Target $DEST | Out-Null
  $binDir = Join-Path $env:LOCALAPPDATA 'Programs\OpenAI\Codex\bin'
  if (Test-Path $binDir) { Remove-Item $binDir -Force -Recurse }
  New-Item -ItemType Directory -Path (Split-Path $binDir -Parent) -Force | Out-Null
  New-Item -ItemType Junction -Path $binDir -Target (Join-Path $DEST 'bin') | Out-Null
  $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  if (-not $userPath) { $userPath = '' }
  if ($userPath -notlike "*$binDir*") {
    [Environment]::SetEnvironmentVariable('Path', (($userPath.TrimEnd(';') + ';' + $binDir).TrimStart(';')), 'User')
  }
  if ($env:Path -notlike "*$binDir*") { $env:Path = $env:Path + ';' + $binDir }
  & (Join-Path $DEST 'bin\codex.exe') --version
} finally {
  Remove-Item $T -Recurse -Force -ErrorAction SilentlyContinue
}
"#;

#[cfg(target_os = "windows")]
fn build_command() -> Command {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let utf16: Vec<u8> = CODEX_MIRROR_INSTALL_WINDOWS_SCRIPT
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let encoded = STANDARD.encode(utf16);
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-EncodedCommand",
        &encoded,
    ]);
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

#[cfg(not(target_os = "windows"))]
fn build_command() -> Command {
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg(CODEX_MIRROR_INSTALL_UNIX);
    cmd
}

fn emit(app: &AppHandle, level: &str, line: impl Into<String>) {
    let _ = app.emit(
        "install-log",
        serde_json::json!({ "level": level, "line": line.into() }),
    );
}

/// 安装 Codex CLI。返回新装出的版本号。
pub async fn install_cli(app: AppHandle) -> Result<String, String> {
    let app2 = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || run_install(app2))
        .await
        .map_err(|e| format!("安装线程异常：{e}"))?;
    res
}

fn run_install(app: AppHandle) -> Result<String, String> {
    emit(&app, "phase", "正在从国内镜像下载 Codex（约 100MB，请保持网络）…");
    let mut cmd = build_command();
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动安装程序：{e}"))?;

    let mut handles = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let a = app.clone();
        handles.push(std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    emit(&a, "out", line);
                }
            }
        }));
    }
    if let Some(stderr) = child.stderr.take() {
        let a = app.clone();
        handles.push(std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    emit(&a, "err", line);
                }
            }
        }));
    }

    let start = Instant::now();
    let timeout = Duration::from_secs(30 * 60);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err("安装超时（30 分钟）".to_string());
                }
                std::thread::sleep(Duration::from_millis(300));
                if start.elapsed().as_secs() % 10 == 0 {
                    let _ = app.emit(
                        "install-log",
                        serde_json::json!({
                            "level": "tick",
                            "line": format!("已等待 {} 秒…", start.elapsed().as_secs())
                        }),
                    );
                }
            }
            Err(e) => return Err(format!("等待安装进程失败：{e}")),
        }
    };
    for h in handles {
        let _ = h.join();
    }

    if !status.success() {
        return Err(format!(
            "安装失败（退出码 {}），请检查网络后重试",
            status.code().unwrap_or(-1)
        ));
    }

    let found = detect::find_cli().ok_or_else(|| {
        format!(
            "安装脚本已结束，但没有找到 {}，请重开终端后重试",
            exe_name()
        )
    })?;
    let version = detect::check_environment()
        .cli
        .version
        .unwrap_or_else(|| "(版本未知)".to_string());
    emit(&app, "phase", format!("安装完成：{}", version));
    Ok(format!("{} ({})", version, found.0.display()))
}
