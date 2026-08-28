#!/usr/bin/env bash
# 把样例仓库复制到一个独立的临时目录并初始化 git。
#
# 必须复制出去：留在 repolish 仓库里跑，git 探测会找到父仓库的 remote，
# 报告标题就会显示成 asale-ai/repolish —— 演示里出现一个错的仓库名，
# 比没有演示更糟。
set -euo pipefail

DEST="${1:-${TMPDIR:-/tmp}/taskvault}"
rm -rf "$DEST"
mkdir -p "$(dirname "$DEST")"
cp -r "$(dirname "$0")/sample" "$DEST"

cd "$DEST"
git init -q .
git remote add origin https://github.com/acme/taskvault.git
git add -A

# 提交时间**故意不写死**，用「现在」。
#
# 试过写死，为的是让录屏逐字节可复现（commit 哈希会出现在报告抬头）。
# 那是错的，而且错得比原问题更糟：写死之后这个样例仓库就成了一个「一年多
# 没人动」的仓库，`activity` 直接判 P1，演示要展示的前后对比被一条与演示
# 无关的扣分盖过去；更要命的是输出里那句「last commit N days ago」**每天都在
# 变**，比哈希每次运行变一次还频繁。
#
# 结论：这段录屏做不到既确定又与时间无关。所以它不进每次 push 的 CI，
# 改成手动触发重录，见 .github/workflows/demo.yml。
git -c user.email=demo@example.com -c user.name=demo commit -qm "initial commit"

echo "$DEST"
