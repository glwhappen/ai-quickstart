# Codex 快速配置（codex-quickstart）

只服务 Codex 的极简配置器。**绿色版：不安装、不写 PATH、不改你已有的 Codex 环境。**

需求唯一来源：[`SPEC.md`](./SPEC.md)（§1.5「免安装原则」优先级最高）。

## 它做什么

打开后只有一条路，一屏四步：

1. **检查环境** —— 看本工具自己的目录里有没有备好的 Codex。
2. **准备 Codex** —— 从 npmmirror 镜像下载官方原生二进制，解压到
   `<数据目录>/codex-quickstart/codex/<版本>/`，直接以该路径运行。
   **不装进系统目录、不建软链、不写 PATH、不碰 `~/.local/bin`。**
3. **填入 Key** —— 写进 `<数据目录>/codex-quickstart/codex-home/config.toml`。
   这个目录就是启动 Codex 时的 `CODEX_HOME`，**用户自己的 `~/.codex` 一个字节都不动**。
4. **测一下** —— 用准备好的 Codex 真的跑一次
   `codex exec --skip-git-repo-check "reply with only the word OK"`，
   成功显示 ✅ + 模型 + 耗时 + 真实回复；失败显示 ❌ + 具体原因（鉴权失败 / 模型无渠道 / 网络不通 / 还没准备）。

测通之后给「启动 Codex」按钮：在新终端窗口里带上 `CODEX_HOME` 启动，用户不用知道 PATH。
底部「高级 / 清理」里有「清理并退出」——删掉本工具产生的全部文件，等于卸载。

## deep link：从网页一键回填 Key

程序注册了 URL scheme **`codexquick`**。官网（cc.hsfp.cn）的 Key 卡片上点「填入 Codex 快速配置」，
会跳到：

```
codexquick://set-key?key=sk-xxxx&group=半价
```

本程序收到后**自动填入输入框**并提示「已从网页填入 Key，点『测一下』验证」，用户不用复制粘贴。

- 参数名固定：scheme=`codexquick`、host/path=`set-key`、参数 `key` / `group`（官网侧按这个对齐）。
- 幂等：重复点只覆盖同一个 Key；程序已在运行时会复用同一个窗口（`single-instance`）。
- 安全：**日志里只有「尾号 xxxx（长度 n）」，`rg 'sk-' 日志` 是空的**；非法/缺参数的链接忽略并给中性提示，不崩。
- 手动粘贴的兜底路径一直都在。

### 注册与撤销

scheme 由程序自己注册（不依赖安装器，绿色版也能用）：

- **Windows**：`HKCU\Software\Classes\codexquick`（当前用户，不需要管理员）
- **Linux**：`~/.local/share/applications/codex-quickstart-handler.desktop` + `xdg-mime default`
- **macOS**：由 `.app` 的 `CFBundleURLTypes` 提供（打包时写入）

「清理并退出」会把它一起撤掉；也可以手动：
`codex-quickstart --headless register-scheme` / `--headless unregister-scheme`。

命令行自检：`codex-quickstart --headless deeplink 'codexquick://set-key?key=sk-xxx&group=半价'`

## 写进配置的东西

`<数据目录>/codex-quickstart/codex-home/config.toml`：

```toml
model = "deepseek-flash"
model_provider = "hsfp"

[model_providers.hsfp]
name = "HSFP"
base_url = "https://new-api.hsfp.cn/v1"
wire_api = "responses"

[model_providers.hsfp.http_headers]
Authorization = "Bearer sk-..."
```

用 `http_headers` 内联 Key，**不用 `env_key`**：桌面版 App 不继承 shell 环境变量，用 `env_key` 会静默失效。

### 桌面版（可选，默认不做）

桌面版读的是固定的 `~/.codex/config.toml`，没法用 `CODEX_HOME` 隔离，这是唯一例外。
只有勾选「桌面版也要用这个 Key」时才会写它：先备份 `config.toml.bak.<时间戳>`，
再按 TOML 段做外科式修改，只增改 `model` / `model_provider` / `[model_providers.hsfp]`，
用户原有的 `[desktop]`、`[projects.*]`、`[sandbox_workspace_write]` 等段逐字符保留。

## 数据目录在哪

| 平台 | 路径 |
|---|---|
| Windows | `%LOCALAPPDATA%\codex-quickstart` |
| macOS | `~/Library/Application Support/codex-quickstart` |
| Linux | `${XDG_DATA_HOME:-~/.local/share}/codex-quickstart` |

可用环境变量 `CODEX_QUICKSTART_HOME` 覆盖（自动化验收就用它 + 假 `HOME`）。

## 下载哪个、解压后点哪个

下载页：<https://github.com/glwhappen/codex-quickstart/releases/latest>

Release 资产命名固定为 **`codex-quickstart-<版本>-<平台>-<架构>.zip`**，平台取 `windows` / `macos` / `linux`，
架构取 `x86_64` / `aarch64`：

| 你的机器 | 下载 | 解压后点 |
|---|---|---|
| Windows 10/11（绝大多数） | `codex-quickstart-<版本>-windows-x86_64.zip` | `codex-quickstart.exe`（双击即跑，无需安装） |
| Apple 芯片 Mac（M1/M2/M3…） | `codex-quickstart-<版本>-macos-aarch64.zip` | `codex-quickstart.app`（拖到任意目录，首次右键 → 打开） |
| Intel Mac | 暂未出（可在 Actions 里加 `macos-13`） | — |
| Linux x86_64 | `codex-quickstart-<版本>-linux-x86_64.zip` | `codex-quickstart`（`chmod +x` 后运行） |

不需要下载 `.deb` / `.dmg` / 安装器——我们只出绿色版，解压即用，删目录即卸载。

- **Windows** 依赖系统自带的 WebView2（Win11 与较新的 Win10 已内置）。
- **macOS** 未签名，首次打开会被 Gatekeeper 拦一下：右键 → 打开，或 `xattr -dr com.apple.quarantine codex-quickstart.app`。
- **Linux** 需要 webkit2gtk-4.1（多数桌面发行版已有）。

## 开发 / 构建

```bash
pnpm install
pnpm dev                       # 前端
pnpm tauri dev                 # 整个应用（开发）
pnpm tauri build               # 出安装包（本项目主推绿色版，见下）
./scripts/package-portable.sh  # 出绿色版 zip（不打包安装向导）
cargo test --manifest-path src-tauri/Cargo.toml --lib   # 配置外科式修改的单元测试
```

### 无界面模式（自动化验收用）

```
codex-quickstart --headless check
codex-quickstart --headless prepare
codex-quickstart --headless config --key sk-... [--also-desktop]
codex-quickstart --headless test --key sk-...
codex-quickstart --headless launch
codex-quickstart --headless cleanup
```

与界面调用的是同一套函数，输出 JSON。

## 平台说明

- **Windows**：绿色版就是那个 `codex-quickstart.exe`，双击即跑。
  依赖系统的 **WebView2 Runtime**（Win10 2004+/Win11 一般已自带）。
- **macOS**：`.app` 拖走即用（不装 LaunchDaemon、不写 /usr/local）。
- **Linux**：单文件可执行，需要 webkit2gtk-4.1。

## 不做的事

不做供应商列表、不做路由/故障转移、不做 MCP/Skills/看板、不做多工具管理、不做第二个选择分支。

## 实测踩到的两个 Codex 行为（已在代码里规避）

1. **`model` 写进 `config.toml` 不等于 Codex 会用**：Codex 会把「上次在 TUI 里选的模型」持久化在它自己的状态里，优先级高于 `config.toml`，
   连 `codex exec` 都跟着走，于是拿着别的模型去打网关 → `503 No available channel`。所以「测一下」和「启动 Codex」都**显式带 `-m <模型>`**。
2. **第一次进 TUI 会问「是否信任当前文件夹」**：这是 Codex 自己的安全确认，按回车同意即可（我们没有替你自动设 trusted）。
