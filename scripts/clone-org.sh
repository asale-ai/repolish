#!/usr/bin/env bash
# 把一个 GitHub 组织（或用户）下的所有仓库并排克隆到一个目录，供 `repolish scan` 评分。
#
# 为什么这一步在脚本里而不在二进制里：评分是离线优先的，`repolish` 自己
# 不 clone、不需要网络也不需要 git。把仓库弄到本地是 git 的事。
#
#   ./scripts/clone-org.sh asale-ai [目标目录]
#   repolish scan target/orgs/asale-ai --remote
#
# 用 --filter=blob:none 而不是 --depth 1：浅克隆一个 tag 都拉不到，
# release-hygiene 会对每个仓库判「无法判断」。blobless 克隆保留完整的 refs，
# 体积和浅克隆相当。

set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: $0 <org-or-user> [dest]" >&2
  exit 2
fi

ORG="$1"
DEST="${2:-target/orgs/$ORG}"
mkdir -p "$DEST"

if command -v gh >/dev/null 2>&1; then
  # gh 认账号，私有仓库也拿得到
  names=$(gh repo list "$ORG" --limit 200 --no-archived --json name -q '.[].name')
else
  echo "note: gh not found, falling back to the public API (60 requests/hour, public repos only)" >&2
  # 认 full_name，不认 name。
  #
  # `"name"` 在一个仓库对象里出现不止一次：嵌套的 license 对象也有一个
  # `"name": "Apache License 2.0"`。照着它去 clone，会得到一个叫 Apache 的
  # 不存在的仓库——这个脚本第一次跑就是这么挂的。
  # `"full_name"` 每个仓库只出现一次，形如 "owner/repo"，取斜杠后那半。
  names=$(curl -fsSL "https://api.github.com/users/${ORG}/repos?per_page=100" \
    | grep -oE '"full_name"[[:space:]]*:[[:space:]]*"[^"]+"' \
    | sed -E 's|.*/([^"]+)"$|\1|')
fi

if [ -z "$names" ]; then
  echo "error: no repositories found for $ORG" >&2
  exit 1
fi

# 一个仓库拉不动不该毁掉整趟：记下来，最后一起报
failed=""
for name in $names; do
  if [ -d "$DEST/$name" ]; then
    echo "  skip   $name"
    continue
  fi
  echo "  clone  $name"
  if git clone --filter=blob:none --quiet "https://github.com/${ORG}/${name}.git" "$DEST/$name"; then
    :
  else
    # 半个克隆比没有更糟：scan 会把它当成一个真仓库去评分
    rm -rf "${DEST:?}/${name}"
    failed="$failed $name"
  fi
done

echo
if [ -n "$failed" ]; then
  echo "could not clone:$failed" >&2
fi
echo "cloned into $DEST"
echo "next:  repolish scan $DEST --remote"

[ -z "$failed" ]
