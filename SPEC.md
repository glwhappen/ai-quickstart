# Codex 快速配置（codex-quickstart）SPEC

> 2026-10-30 郭老师立的新项目。旧项目 `/root/ccs-brand`（fork CC Switch）**只作经验参考，不复用代码形态**。

## 0. 一句话

**只服务 Codex 的极简安装/配置器**：打开 → 检查有没有 Codex → 没有就装上 → 有就引导配置 → 测一次能不能用 → 结束。四个步骤，全部在同一页完成。

## 1. 为什么另起一个项目（郭老师的判断）

有人问：「对于 codex 不是加个配置文件就可以了么，还需要 ccswitch 吗？」

答案是对的。CC Switch 那种 12 工具、供应商管理、MCP/Skills/路由/看板的形态，对我们当前需求（只推 Codex CLI + Codex 桌面版）是**纯负担**：
- 用户要做的事其实只有三件：装 Codex、写 `~/.codex/config.toml`、确认能用。
- 上游功能越多 → 小白越晕、我们跟版成本越高、安装包越大（45MB）。

所以新项目定位：**功能砍到只剩一条主路径，界面只有一屏**。

## 1.5 免安装原则（2026-10-30 郭老师追加，优先级最高）

原话意译：「能不能尽量的不走安装呢，直接运行就行吧，这个也就用一次两次的」。

这条把整个设计从「装东西」改成「**绿色版、解压即用、删了就干净**」：

### 1.5.1 我们的程序本身
- 出 **portable / 免安装** 产物：Windows 用绿色版 exe/zip（双击就跑，不进 `Program Files`、不写开始菜单、不注册卸载项）；
  macOS 直接 .app 拖走即用（不装 LaunchDaemon、不装 /usr/local）。**不要跑安装向导。**
- 「卸载」= 删掉目录。界面上给一个「清理并退出」按钮（删掉我们自己产生的全部文件）。

### 1.5.2 Codex 本体也不"安装"
- **不写 PATH、不建软链、不装进系统目录、不动 `~/.local/bin`。**
- 做法：下载 tarball → 解压到我们自己的数据目录 `<appdata>/codex-quickstart/codex/<version>/`，直接以该路径运行。
- 官方之所以要 `~/.codex/packages/standalone/...`，是为了让官方 installer 能识别版本、自动升级；我们不装、不管升级，**不需要那套布局**（升级=重新解压一个新版本目录）。

### 1.5.3 关键：用 `CODEX_HOME` 隔离，完全不污染用户的 `~/.codex`
- Codex 支持 `CODEX_HOME` 环境变量指定配置目录。我们把它设成自己的目录：
  ```
  <appdata>/codex-quickstart/codex-home/
      config.toml        ← 我们生成的（我们的 provider + key）
      (会话历史等也落这里，与用户原有环境隔离)
  ```
- 好处：**用户的 `~/.codex/config.toml` 一个字节都不动**，不需要备份、不需要外科手术式改 TOML，也没有"改坏用户配置"的风险。
- 代价：与用户已有 Codex 环境的会话/历史分离 —— 对我们这个「用一两次」的场景是优点不是缺点。

### 1.5.4 「启动 Codex」而不是「装好了去终端自己敲」
- 界面上直接给一个 **「启动 Codex」** 按钮：在正确工作目录里、带上 `CODEX_HOME` 起一个终端窗口（Windows 用 `cmd /k` 或 Windows Terminal；macOS 用 Terminal.app）。
- 用户不需要知道 PATH、不需要重开终端。用完关掉即可。

### 1.5.5 唯一需要碰用户目录的情况（可选，且默认不做）
- 只有用户显式勾选「**让 Codex 桌面版也能用这个 Key**」时才写 `~/.codex/config.toml`：
  必须先备份 `config.toml.bak.<时间戳>`，且只增改 `model` / `model_provider` / `[model_providers.hsfp]` 几项，
  用户原有的 `[desktop]`、`[projects.*]` 等段一个字符都不能动。
- 因为桌面版读的是固定的 `~/.codex/config.toml`，**没法用 CODEX_HOME 隔离**，这是它唯一的例外。

## 2. 目标用户与唯一路径

不懂命令行的国内小白，想用 Codex（CLI 或桌面版）接我们的半价模型。

打开程序后**只有一条路**：

```
① 检查环境（自动，无需操作）
      ↓
② 没有 Codex → 点「安装」→ 镜像直下装好（约 300MB，带进度）
   已经有了   → 直接进下一步
      ↓
③ 配置：把 Key 填进来（或点「去领一个 Key」跳 cc.hsfp.cn）
      ↓
④ 测试：点「测一下」→ 真实发一次请求 → 显示 ✅ 可用 / ❌ 具体原因
      ↓
⑤ 完成：一个大按钮「开始使用」，下面一行字告诉用户怎么打开
```

**不做的事**：不做供应商列表、不做路由/故障转移、不做 MCP/Skills/看板、不做多工具管理、不做 Clash 式界面。一个都不做。

## 3. 功能明细

### 3.1 检查（自动）
- **Codex CLI**：查 `~/.codex/packages/standalone/current/bin/codex`（官方独立安装布局）以及 PATH 里的 `codex`，取版本。
- **Codex 桌面版**：Windows 查 `Get-AppxPackage`（`OpenAI.Codex*` / `*ChatGPT*`）；macOS 查 `/Applications/Codex.app`、`/Applications/ChatGPT.app`。
- 两者都**没装**也算正常情况，直接给安装按钮。

### 3.2 安装
- 装 **Codex CLI**：npmmirror 镜像直下（**已经有实测可用的实现，直接从 `/root/ccs-brand/src-tauri/src/commands/misc.rs` 的 `CODEX_MIRROR_INSTALL_WINDOWS_SCRIPT` / `CODEX_MIRROR_INSTALL_UNIX` 搬**，不要重写）：
  - 布局必须与官方 install.sh 一致：`~/.codex/packages/standalone/releases/<ver>-<triple>/` + `current` 软链（Windows 用 junction）+ `~/.local/bin/codex` 入口。
  - 国内不用官方 installer（`chatgpt.com/codex/install.sh` 不可达）。
- 装 **Codex 桌面版**：**不代下**（Windows MSIX ≈958MB、macOS zip ≈729MB），只给三样：Microsoft Store 搜 `Codex` 的说明、官方下载页链接、本站镜像链接（`https://cc.hsfp.cn/`）。

### 3.3 配置
写 `~/.codex/config.toml`（Windows `%USERPROFILE%\.codex\config.toml`）：

```toml
model = "deepseek-flash"
model_provider = "hsfp"

[model_providers.hsfp]
name = "HSFP"
base_url = "https://new-api.hsfp.cn/v1"
wire_api = "responses"

[model_providers.hsfp.http_headers]
Authorization = "Bearer <user-key>"
```

硬约束：
- **写入前必须备份** `config.toml.bak.<YYYYMMDDHHMMSS>`。
- **只增改上面这几项**，用户原有的其它段（`[desktop]`、`[projects.*]`、`[sandbox_workspace_write]` 等）**一个字符都不能动**——按 TOML 段做外科式处理，不许整文件重写。
- **必须用 `http_headers`，不能用 `env_key`**：桌面版 App 不继承 shell 环境变量，用 env_key 会静默失效（踩过的坑）。
- `wire_api` 只能是 `responses`（我们的网关已验证支持，`/v1/responses` 200）。
- 幂等：重复点配置不会写出重复段、不会叠加。

### 3.4 Key 从哪来（2026-10-30 追加：要能「快速拉取」）
郭老师追加要求：「也可以快速拉取 key」——不要逼用户「去网页复制 → 回来粘贴」两趟。

**主路径：deep link 回填（实现这条）**
1. 本程序注册自定义 URL scheme **`codexquick`**（Tauri `deep-link` 插件）。
2. 「快速领取 Key」按钮 → 用**系统浏览器**打开 `https://cc.hsfp.cn/#key`（那边已有 SSO 与一键领取）。
3. 官网 Key 卡片上加一个按钮「**填入 Codex 快速配置**」，点击跳到：
   `codexquick://set-key?key=sk-xxxx&group=半价`
4. 本程序收到 deep link → **自动填入输入框 + 立即校验 + 提示「已从网页填入 Key，点『测一下』验证」**，用户不用复制粘贴。
5. deep link 处理要幂等：重复点击只覆盖为同一个 Key，不产生多份配置；**key 不写进日志**。
6. 用户在官网没登录时，官网侧会先跳登录，登录后再回来点按钮——这段由官网负责，程序侧只管收 deep link。

**兜底路径（必须同时保留）**：输入框可手动粘贴，校验 `sk-` 前缀；「去领一个 Key」按钮仍打开 `https://cc.hsfp.cn/`。
**不落盘明文以外的位置**（只写进 config.toml 的 Authorization）。

### 3.5 测试（关键一步，必须真实）
点「测一下」→ 用写好的配置**真的跑一次** `codex exec --skip-git-repo-check "reply with only the word OK"`（或直接 `curl https://new-api.hsfp.cn/v1/responses` 带同一个 Key 与模型），
成功显示 ✅ + 耗时 + 用的模型；失败显示 ❌ + **具体原因**（鉴权失败 / 模型没渠道 / 网络不通 / 没装），并给下一步建议。
不能只显示"配置已保存"就算完。

### 3.6 完成
一个大按钮「开始使用」，下面一行说明：
- CLI 用：终端里输入 `codex`（Windows 可能要重开终端让 PATH 生效）。
- 桌面版用：打开应用，左上角切到 **Codex 模式**（普通 ChatGPT 对话不走我们的 Key）。

## 4. 技术形态

- **Tauri 2 + React**（经验现成，安装包 ~5–10MB，远小于 CC Switch 的 45MB）。
- **单窗口、单页、五个状态**（检查中 / 待安装 / 待配置 / 待测试 / 完成），不需要侧栏、不需要路由。
- 白牌：界面叫「**Codex 快速配置**」，不出现任何厂商品牌；不叫 CC Switch、不叫 HSFP Code。
- 白色亮色主题、按钮大、字大、留白多，窗口 480×620 左右就够。
- 失败态必须可复制错误文本，不许无限转圈。

## 5. 验收（必须贴证据）

1. 全新环境（假 HOME/干净用户）打开 → 只有一屏、只有必要的按钮。
2. 点安装 → 真的装出 `codex --version`（截版本号）。
3. 填一把真实的半价 Key → 配置写入 → `cat ~/.codex/config.toml` 贴出来，且原有段未被破坏（造一个带 `[desktop]` 的旧文件测）。
4. 点「测一下」→ 输出 ✅ 与真实回复内容。
5. 故意填错 Key → 显示 ❌ 且原因是鉴权失败（不是"未知错误"）。
6. 安装包体积贴出来（目标 <15MB）。Windows 包在真机跑通（可找我要机器）。

## 6. 参考（可抄，不要照搬架构）

- 镜像安装链实现：`/root/ccs-brand/src-tauri/src/commands/misc.rs`（`CODEX_MIRROR_INSTALL_WINDOWS_SCRIPT`、`codex_mirror_install_windows_command`、`mirror_first`）
- 踩坑记录：`/root/ccs-brand/FORK.md`「为什么必须改安装链」
- 国内下载/协议实测：`/root/.openclaw/workspace/memory/国内分发AI编程CLI的坑与解法.md`
- 已验证事实：半价分组可用模型 `deepseek-flash` / `deepseek-v4.1-flash`；`/v1/responses` 200；`codex-1` 无渠道会 503。
