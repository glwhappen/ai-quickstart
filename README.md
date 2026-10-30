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
