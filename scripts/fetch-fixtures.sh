#!/usr/bin/env bash
# 拉取用于人工验收的真实开源仓库。
#
# 这些仓库不入库（体积大、且是第三方内容），用完即弃。
# 选型标准：覆盖不同语言、不同 README 风格，且每个都曾暴露过至少一个真实缺陷——
# 见 docs/06-路线图.md 的「M1 验收」「M2 验收」两节。
#
# 注意：这里是浅克隆（--depth 1），与 CI 里 actions/checkout 的默认行为一致。
# 因此 release-hygiene 会一律判 Inconclusive——这是有意保留的真实场景。
#
#   ./scripts/fetch-fixtures.sh [目标目录]
#   cargo run -- check <目标目录>/ripgrep

set -euo pipefail

DEST="${1:-target/fixtures}"
mkdir -p "$DEST"

REPOS=(
  # 仓库                      # 这个样本卡住过什么
  "BurntSushi/ripgrep"        # setext 标题（下划线式，级别是 h2）
  "astral-sh/ruff"            # 标题下的徽章行 + 导航行；子标题截断父区块
  "serde-rs/serde"            # LICENSE-MIT/-APACHE 双协议；标题内嵌徽章
  "tokio-rs/tokio"            # tests-integration/src/bin 让纯库被判成 CLI
  "psf/requests"              # CI 步骤名与命令不在同一行
  "pallets/flask"             # pyproject 的 console_scripts
  "axios/axios"               # 开头是赞助商 HTML，全文没有标题
  "chalk/chalk"               # logo 包在 HTML h1 里；说明写成引用块
  "junegunn/fzf"              # 裸 logo 图片作标题；块内混有 Sponsors 徽章
  "koajs/koa"                 # 空的 Getting started 与有命令的 Installation 并存
  "sindresorhus/awesome"      # 真 logo 在前、赞助位 h2 在后
  "doocs/advanced-java"       # 中文标题；根绝对路径链接 /docs/x.md
)

for entry in "${REPOS[@]}"; do
  repo="${entry%% *}"
  name="${repo##*/}"
  if [ -d "$DEST/$name" ]; then
    echo "  skip  $name"
    continue
  fi
  if git clone --depth 1 -q "https://github.com/$repo.git" "$DEST/$name" 2>/dev/null; then
    echo "  ok    $name"
  else
    echo "  FAIL  $name" >&2
  fi
done

echo
echo "完成。跑一遍："
echo "  for d in $DEST/*/; do cargo run -q -- check \"\$d\" --format json; done"
