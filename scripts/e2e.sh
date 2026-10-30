#!/usr/bin/env bash
# 用途：在假 HOME 里跑一遍完整验收（检查 → 准备 Codex → 写配置 → 测一下），不碰真实用户目录
# 用法：./scripts/e2e.sh [--key sk-真实Key]
#       不给 --key 时只跑「错 Key 应显示鉴权失败」这一路（不需要真 Key）
# 入参：--key <sk-...>  用真 Key 验证 ✅ 路径
# 出参：每步的真实输出；退出码 0=全部符合预期
# 依赖：已构建的 src-tauri/target/release/ai-quickstart（或 debug），网络可达 npmmirror / new-api.hsfp.cn
# 关键词：e2e, 验收, 假HOME, fake home, 绿色版, 免安装, 不动用户目录, ai-quickstart, 测试
# 更新：2026-10-30
set -euo pipefail
cd "$(dirname "$0")/.."

KEY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --key) KEY="${2:-}"; shift 2 ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done

BIN="${BIN:-src-tauri/target/release/ai-quickstart}"
[ -x "$BIN" ] || BIN="src-tauri/target/debug/ai-quickstart"
[ -x "$BIN" ] || { echo "先构建：cd src-tauri && cargo build" >&2; exit 1; }

FAKE=$(mktemp -d /tmp/ai-quickstart-e2e.XXXXXX)
export HOME="$FAKE"
export XDG_DATA_HOME="$FAKE/.local/share"
trap 'rm -rf "$FAKE"' EXIT

echo "== 假 HOME：$FAKE  (二进制：$BIN)"
echo
echo "== ① 检查环境（应为 prepared=false，且不碰 \$HOME/.codex）"
"$BIN" --headless check

echo
echo "== ② 造一个带 [desktop] 的用户配置，验证我们写文件时不动它"
mkdir -p "$HOME/.codex"
cat > "$HOME/.codex/config.toml" <<'TOML'
# 用户自己的配置
model = "gpt-5-codex"
approval_policy = "on-request"

[desktop]
theme = "dark"
conversation_detail = "STEPS_COMMANDS"

[projects."/home/me/proj"]
trust_level = "trusted"
TOML
cp "$HOME/.codex/config.toml" "$FAKE/before.toml"

echo
echo "== ③ 准备 Codex（真下载解压，不写 PATH）"
"$BIN" --headless prepare
echo "-- 产物："
ls -1 "$XDG_DATA_HOME/ai-quickstart/codex"/*/bin/
echo "-- 校验：\$HOME 下没有 .local/bin/codex 软链（不该有）"
ls -l "$HOME/.local/bin" 2>/dev/null || echo "   (没有 ~/.local/bin，符合预期)"

echo
echo "== ④ 写配置（隔离目录 + 显式勾选的桌面版写入）"
"$BIN" --headless config --key "${KEY:-sk-wrongkey-e2e-000000}" --also-desktop
echo "-- 隔离配置："
cat "$XDG_DATA_HOME/ai-quickstart/codex-home/config.toml"
echo "-- 用户 ~/.codex/config.toml 的改动（应只有 model/model_provider/[model_providers.hsfp]）："
diff "$FAKE/before.toml" "$HOME/.codex/config.toml" || true

echo
if [ -n "$KEY" ]; then
  echo "== ⑤ 用真 Key 测一下（应 ok=true 且带真实回复）"
  "$BIN" --headless test --key "$KEY"
else
  echo "== ⑤ 故意用错 Key 测一下（应 ok=false / category=auth）"
  "$BIN" --headless test --key sk-wrongkey-e2e-000000
fi

echo
echo "== ⑥ 清理（删掉本工具产生的全部文件）"
"$BIN" --headless cleanup
ls "$XDG_DATA_HOME/ai-quickstart" 2>/dev/null && echo "!! 清理后目录还在" && exit 1 || echo "   数据目录已删除，符合预期"
