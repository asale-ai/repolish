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
git -c user.email=demo@example.com -c user.name=demo commit -qm "initial commit"

echo "$DEST"
