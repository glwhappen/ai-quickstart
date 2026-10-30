//! 工具（tool）抽象：每个 AI 编码工具自带「检测 / 准备 / 配置写入 / 验证命令」。
//!
//! 加一个新工具（例如 Claude Code）应当是**新增一个 impl**，不改主流程：
//! 在这里 `new` 一个结构体、实现 `Tool`、加进 `all()` 即可。
//! 界面主路径仍然只服务「当前这一个」工具，多工具不是让用户做选择。

pub mod codex;

use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize, Clone, Debug, Default)]
pub struct ToolStatus {
    /// 工具标识（codex / claude / …）
    pub id: String,
    /// 展示名（不带品牌也行，用工具本名即可）
    pub name: String,
    /// 是否已经准备好（可以运行）
    pub prepared: bool,
    pub version: Option<String>,
    /// 可执行文件绝对路径
    pub path: Option<String>,
    /// 工具专属的数据目录（例如 Codex 的解压目录）
    pub dir: Option<String>,
    /// 工具专属的配置目录（例如 Codex 的 CODEX_HOME 隔离目录）
    pub home: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PrepareOutcome {
    pub version: String,
    pub path: String,
}

/// 一个 AI 编码工具的全部差异化行为。
pub trait Tool: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;

    /// 工具专属数据根目录（解压/下载产物）。
    fn dir(&self) -> PathBuf;
    /// 工具专属配置目录（启动时作为该工具的环境变量注入）。
    fn home(&self) -> PathBuf;
    /// 启动/运行时需要注入的环境变量名与值（Codex 是 `CODEX_HOME`）。
    fn env_var(&self) -> Option<(&'static str, PathBuf)> {
        None
    }
    /// 配置文件路径（我们写的那一份）。
    fn config_path(&self) -> PathBuf;

    /// 检测：已准备好的话返回 (版本, 可执行文件)。
    fn detect(&self) -> Option<(String, PathBuf)>;
    /// 状态快照（给界面/`--headless status` 用）。
    fn status(&self) -> ToolStatus;
    /// 下载 + 解压（不安装、不写 PATH）。
    fn prepare(&self, log: &(dyn Fn(&str, &str) + Sync)) -> Result<PrepareOutcome, String>;

    /// 生成该工具的配置文本（把 Key 写进去）。
    fn render_config(&self, key: &str) -> String;
    /// 验证命令：真的跑一次，返回要执行的程序与参数。
    fn verify_args(&self, model: &str) -> (Vec<String>, PathBuf);
    /// 启动命令（终端里跑）要带的额外参数（Codex 需要显式 `-m`）。
    fn launch_args(&self, model: &str) -> Vec<String>;
    /// 默认模型（没有写配置时用）。
    fn default_model(&self) -> &'static str;
    /// 上游 base_url。
    fn base_url(&self) -> &'static str;
}

/// 当前界面主路径服务的那一个工具。
pub fn primary() -> &'static dyn Tool {
    &codex::CodexTool
}

/// 所有已知工具（目前只有 Codex）。用于「顺手提一句」级别的增强。
pub fn all() -> Vec<&'static dyn Tool> {
    vec![&codex::CodexTool]
}
