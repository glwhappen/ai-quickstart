//! Codex（OpenAI Codex CLI）的工具定义：检测 / 镜像下载解压 / 配置生成 / 验证命令。
//!
//! 绿色版原则（SPEC §1.5）：不写 PATH、不建软链、不碰 `~/.codex`，
//! 一切落在 `<数据目录>/tools/codex/` 下，配置目录用 `CODEX_HOME` 隔离。

use std::path::PathBuf;
use std::io::{BufRead, BufReader};
use std::process::Command;

use super::{PrepareOutcome, Tool, ToolStatus};

pub const BASE_URL: &str = "https://new-api.hsfp.cn/v1";
pub const DEFAULT_MODEL: &str = "deepseek-flash";
pub const PROVIDER_ID: &str = "hsfp";
pub const PROVIDER_NAME: &str = "HSFP";

pub struct CodexTool;

impl CodexTool {
    fn root(&self) -> PathBuf {
        super::super::data_root().join("tools").join("codex")
    }
}

fn exe_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "codex.exe"
    } else {
        "codex"
    }
}

fn version_of(exe: &PathBuf) -> Option<String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok()?;
    let text = if out.stdout.is_empty() {
        String::from_utf8_lossy(&out.stderr).to_string()
    } else {
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    text.lines().map(str::trim).find(|l| !l.is_empty()).map(|s| s.to_string())
}

/// npmmirror 镜像直下 + 解压到我们自己的目录（不安装）。
/// 技巧沿用 `/root/ccs-brand/.../misc.rs` 的实现：官方 installer 在国内不可达，npm 包里的
/// 平台分包本身就是原生二进制，curl + tar 一次搞定。
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
$T = Join-Path $env:TEMP ('aiquick-' + [guid]::NewGuid().ToString('N'))
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

fn prepare_command(dest_dir: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let utf16: Vec<u8> = PREPARE_WINDOWS.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let encoded = STANDARD.encode(utf16);
        let mut cmd = Command::new("powershell");
        cmd.env("QK_CODEX_DIR", dest_dir);
        cmd.args([
            "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded,
        ]);
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = Command::new("bash");
        cmd.env("QK_CODEX_DIR", dest_dir);
        cmd.arg("-c").arg(PREPARE_UNIX);
        cmd
    }
}

impl Tool for CodexTool {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn dir(&self) -> PathBuf {
        self.root().join("packages")
    }

    fn home(&self) -> PathBuf {
        self.root().join("home")
    }

    fn env_var(&self) -> Option<(&'static str, PathBuf)> {
        Some(("CODEX_HOME", self.home()))
    }

    fn config_path(&self) -> PathBuf {
        self.home().join("config.toml")
    }

    fn detect(&self) -> Option<(String, PathBuf)> {
        let dir = self.dir();
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
                Some((bv, _)) => super::super::version_cmp(&ver, bv) == std::cmp::Ordering::Greater,
            };
            if better {
                best = Some((ver, exe));
            }
        }
        best
    }

    fn status(&self) -> ToolStatus {
        match self.detect() {
            Some((ver, exe)) => ToolStatus {
                id: self.id().to_string(),
                name: self.name().to_string(),
                prepared: true,
                version: version_of(&exe).or(Some(ver)),
                dir: exe.parent().and_then(|p| p.parent()).map(|p| p.display().to_string()),
                path: Some(exe.display().to_string()),
                home: Some(self.home().display().to_string()),
            },
            None => ToolStatus {
                id: self.id().to_string(),
                name: self.name().to_string(),
                dir: Some(self.dir().display().to_string()),
                home: Some(self.home().display().to_string()),
                ..Default::default()
            },
        }
    }

    fn prepare(&self, log: &(dyn Fn(&str, &str) + Sync)) -> Result<PrepareOutcome, String> {
        if let Some((ver, exe)) = self.detect() {
            log("phase", &format!("已备好 Codex {ver}，跳过下载"));
            return Ok(PrepareOutcome { version: ver, path: exe.display().to_string() });
        }
        let dest = self.dir();
        std::fs::create_dir_all(&dest).map_err(|e| format!("创建目录失败：{e}"))?;
        log("phase", "正在从国内镜像下载 Codex（约 100MB，请保持网络）…");

        let mut cmd = prepare_command(&dest.display().to_string());
        use std::process::Stdio;
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| format!("无法启动下载程序：{e}"))?;

        let status = std::thread::scope(|scope| -> Result<std::process::ExitStatus, String> {
            if let Some(p) = child.stdout.take() {
                scope.spawn(move || {
                    for line in BufReader::new(p).lines().map_while(Result::ok) {
                        if !line.trim().is_empty() {
                            log("out", &line);
                        }
                    }
                });
            }
            if let Some(p) = child.stderr.take() {
                scope.spawn(move || {
                    for line in BufReader::new(p).lines().map_while(Result::ok) {
                        if !line.trim().is_empty() {
                            log("err", &line);
                        }
                    }
                });
            }
            let start = std::time::Instant::now();
            let mut last_tick = 0u64;
            loop {
                match child.try_wait() {
                    Ok(Some(st)) => return Ok(st),
                    Ok(None) => {
                        let el = start.elapsed();
                        if el.as_secs() > 30 * 60 {
                            let _ = child.kill();
                            return Err("下载超时（30 分钟）".to_string());
                        }
                        if el.as_secs() / 10 > last_tick {
                            last_tick = el.as_secs() / 10;
                            log("tick", &format!("已等待 {} 秒…", el.as_secs()));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                    Err(e) => return Err(format!("等待下载进程失败：{e}")),
                }
            }
        })?;
        if !status.success() {
            return Err(format!(
                "下载或解压失败（退出码 {}），请检查网络后重试",
                status.code().unwrap_or(-1)
            ));
        }
        let (ver, exe) = self.detect().ok_or_else(|| "解压完成但没找到 codex 可执行文件，请重试".to_string())?;
        log("phase", &format!("准备完成：{ver}"));
        Ok(PrepareOutcome { version: ver, path: exe.display().to_string() })
    }

    fn render_config(&self, key: &str) -> String {
        // 必须用 http_headers 内联 Key，不能用 env_key：
        // 桌面版 App 不继承 shell 环境变量，用 env_key 会静默失效。
        format!(
            r#"# 由「AI 快速配置」生成（绿色版，只作用于本工具启动的 Codex）
model = "{DEFAULT_MODEL}"
model_provider = "{PROVIDER_ID}"

[model_providers.{PROVIDER_ID}]
name = "{PROVIDER_NAME}"
base_url = "{BASE_URL}"
wire_api = "responses"

[model_providers.{PROVIDER_ID}.http_headers]
Authorization = "Bearer {key}"
"#
        )
    }

    fn verify_args(&self, model: &str) -> (Vec<String>, PathBuf) {
        (
            vec![
                "exec".into(),
                "-m".into(),
                model.into(),
                "--skip-git-repo-check".into(),
                "reply with only the word OK".into(),
            ],
            self.detect().map(|(_, e)| e).unwrap_or_default(),
        )
    }

    fn launch_args(&self, model: &str) -> Vec<String> {
        // 必须显式 `-m`：Codex 会把它自己持久化的「上次选的模型」排在 config.toml 之前。
        vec!["-m".into(), model.into()]
    }

    fn default_model(&self) -> &'static str {
        DEFAULT_MODEL
    }

    fn base_url(&self) -> &'static str {
        BASE_URL
    }
}
