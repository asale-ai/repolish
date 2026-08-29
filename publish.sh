#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Unattended release. Bumps the workspace version on a release branch, opens a
# pull request against main, lets it merge once the required checks pass, tags
# the commit that actually landed. That tag triggers release.yml, which builds
# the binaries, creates the GitHub release, and publishes the crates to
# crates.io — so no crates.io token is ever needed on a developer machine.
#
#   ./publish.sh "fix the table column widths"
#   ./publish.sh --minor "add the overview card"
#   ./publish.sh --version 1.0.0 "first stable release"
#   ./publish.sh --clawhub "publish the skill to ClawHub too"
#   ./publish.sh --local-npm "publish the npm package from here, interactively"
#   ./publish.sh --dry-run "see what would happen"
#
# npm goes out from the release workflow, over trusted publishing: GitHub mints
# a short-lived OIDC token and npm accepts it, so **no npm credential exists
# anywhere** — not in the repository, not on this machine, not in this script.
# --local-npm is the escape hatch and publishes interactively, letting npm ask
# for the second factor. It is the only way to publish a package npm has never
# seen, because the trusted-publisher setting lives under a package page, which
# a package that does not exist does not have.
#
# There is no interactive confirmation anywhere. Everything that could need a
# decision is a flag with a documented default.

set -euo pipefail

REPO_SLUG="asale-ai/repolish"
BASE_BRANCH="main"
BUMP="patch"
EXPLICIT_VERSION=""
DRY_RUN=0
WITH_CLAWHUB=0
SKIP_TESTS=0
LOCAL_CRATES=0
LOCAL_NPM=0
MESSAGE=""

# The npm package is scoped; the binary it installs is still called `repolish`.
NPM_PKG="@asale/repolish"
NPM_DIR="npm"
NPM_REGISTRY="https://registry.npmjs.org/"

# Publish order is the dependency order. cargo will not accept a crate whose
# path dependencies are not on crates.io yet, so this list is not cosmetic.
CRATES=(repolish-md repolish-ingest repolish-core repolish-checks repolish-render repolish)

BOLD=$(tput bold 2>/dev/null || printf '')
RED=$(tput setaf 1 2>/dev/null || printf '')
GREEN=$(tput setaf 2 2>/dev/null || printf '')
YELLOW=$(tput setaf 3 2>/dev/null || printf '')
RESET=$(tput sgr0 2>/dev/null || printf '')

step() { printf '%s==>%s %s\n' "$BOLD" "$RESET" "$*"; }
info() { printf '    %s\n' "$*"; }
warn() { printf '%swarning:%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
die()  { printf '%serror:%s %s\n' "$RED" "$RESET" "$*" >&2; exit 1; }

usage() {
  # The header block is the help text. Taking it up to the first blank line
  # rather than a hard-coded range: a line number in a sed script is a claim
  # about the file above it, and adding a paragraph silently truncated the help
  # once already.
  sed -n '3,/^$/p' "$0" | sed 's/^# \{0,1\}//'
  cat <<'EOF'

Flags:
  --patch | --minor | --major   Which component to bump (default: --patch)
  --version X.Y.Z               Set an exact version instead of bumping
  --clawhub                     Also publish the skill to ClawHub
  --skip-tests                  Skip the local cargo test (CI still gates the PR)
  --local-crates                Publish to crates.io from here instead of letting
                                the release workflow do it. Needs a local token
  --local-npm                   Publish the npm package from this terminal
                                instead of letting the release workflow do it
                                over trusted publishing. Interactive: npm asks
                                for the second factor. Needed only for a package
                                npm has never seen
  --dry-run                     Print what would happen; change nothing
  -h, --help                    This text
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --patch) BUMP="patch"; shift ;;
    --minor) BUMP="minor"; shift ;;
    --major) BUMP="major"; shift ;;
    --version) EXPLICIT_VERSION="${2:?--version needs X.Y.Z}"; shift 2 ;;
    --clawhub) WITH_CLAWHUB=1; shift ;;
    --skip-tests) SKIP_TESTS=1; shift ;;
    --local-crates) LOCAL_CRATES=1; shift ;;
    --local-npm) LOCAL_NPM=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) die "unknown flag: $1 (try --help)" ;;
    *) MESSAGE="$1"; shift ;;
  esac
done

[ -n "$MESSAGE" ] || { usage; die "a commit message is required"; }

cd "$(dirname "$0")"

run() {
  if [ "$DRY_RUN" = "1" ]; then
    printf '    %s[dry-run]%s %s\n' "$YELLOW" "$RESET" "$*"
  else
    "$@"
  fi
}

# ------------------------------------------------------------ preflight

step "Preflight"
command -v cargo > /dev/null || die "cargo is not installed"
command -v git   > /dev/null || die "git is not installed"
command -v gh    > /dev/null || die "gh is not installed (brew install gh)"
gh auth status > /dev/null 2>&1 || die "gh is not authenticated; run: gh auth login"
git rev-parse --git-dir > /dev/null 2>&1 || die "not a git repository"
git remote get-url origin > /dev/null 2>&1 || die "no 'origin' remote configured"

# No npm credential is read anywhere in this script. The default path uses the
# OIDC token GitHub mints for one workflow run; --local-npm publishes
# interactively and lets npm ask for the second factor.
if [ "$LOCAL_NPM" = "1" ] && [ "$DRY_RUN" = "0" ]; then
  # Finding out node is missing after the tag is pushed is the worst possible
  # moment — the tag is immutable, so the release cannot be redone.
  command -v node > /dev/null || die "node is not installed, and the npm package needs it."
  command -v npm  > /dev/null || die "npm is not installed."
  # npm prompts for the second factor on the terminal, at the very end of a long
  # run. Better to find out now that there is no terminal to prompt on.
  [ -t 0 ] || die "--local-npm publishes interactively and npm asks for the second factor
on the terminal, but stdin is not one. Run this from a terminal, or drop
--local-npm and let the release workflow publish over trusted publishing."
fi

if [ "$LOCAL_CRATES" = "1" ] && [ "$DRY_RUN" = "0" ]; then
  # Only matters on the --local-crates path. Finding out the token is missing
  # after the tag is pushed is the worst possible moment: the tag is immutable,
  # so the release cannot simply be redone.
  if [ -z "${CARGO_REGISTRY_TOKEN:-}" ] && [ ! -f "${CARGO_HOME:-$HOME/.cargo}/credentials.toml" ]; then
    die "no crates.io credentials, and --local-crates was requested.
Run 'cargo login', export CARGO_REGISTRY_TOKEN, or drop --local-crates and let
the release workflow publish (it holds the token as a repository secret)."
  fi
fi

START_BRANCH=$(git rev-parse --abbrev-ref HEAD)
info "starting from: $START_BRANCH"

git fetch --quiet origin "$BASE_BRANCH" || die "could not fetch origin/$BASE_BRANCH"

BEHIND=$(git rev-list --count "HEAD..origin/$BASE_BRANCH")
if [ "$BEHIND" != "0" ]; then
  die "HEAD is $BEHIND commit(s) behind origin/$BASE_BRANCH. Pull, then re-run."
fi

if [ -n "$(git status --porcelain)" ] && [ "$DRY_RUN" = "0" ]; then
  die "the working tree is dirty. Commit or stash first — a release should not
carry changes nobody reviewed."
fi

# ------------------------------------------------------------ version

# The version lives once, in [workspace.package]; every crate inherits it.
CURRENT=$(awk '/^\[workspace\.package\]/{f=1} f && /^version = /{gsub(/version = "|"/,""); print; exit}' Cargo.toml)
[ -n "$CURRENT" ] || die "could not read the version from [workspace.package] in Cargo.toml"

if [ -n "$EXPLICIT_VERSION" ]; then
  echo "$EXPLICIT_VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' \
    || die "--version must be X.Y.Z, got: $EXPLICIT_VERSION"
  NEW="$EXPLICIT_VERSION"
else
  IFS=. read -r MAJOR MINOR PATCH <<< "$CURRENT"
  case "$BUMP" in
    major) NEW="$((MAJOR + 1)).0.0" ;;
    minor) NEW="$MAJOR.$((MINOR + 1)).0" ;;
    patch) NEW="$MAJOR.$MINOR.$((PATCH + 1))" ;;
  esac
fi

RELEASE_BRANCH="release/v$NEW"
step "Version $CURRENT -> $NEW"

# A tag pushed at the wrong commit cannot be repaired if the ruleset forbids
# deleting it — only abandoned for a higher version. Hence these checks, and
# hence the tag is created at the very end, against what actually landed.
if git rev-parse --verify --quiet "refs/tags/v$NEW" > /dev/null; then
  die "tag v$NEW already exists locally. Pass --version with a higher number."
fi
if git ls-remote --exit-code --tags origin "refs/tags/v$NEW" > /dev/null 2>&1; then
  die "tag v$NEW already exists on origin. Pass --version with a higher number."
fi
if git rev-parse --verify --quiet "refs/heads/$RELEASE_BRANCH" > /dev/null; then
  die "branch $RELEASE_BRANCH already exists locally. Delete it, or use --version."
fi

# ------------------------------------------------------------ tests

if [ "$SKIP_TESTS" = "0" ]; then
  step "Testing"
  if [ "$DRY_RUN" = "1" ]; then
    info "[dry-run] cargo test --workspace --locked"
  else
    cargo test --workspace --locked || die "tests failed — nothing was committed or pushed"
    cargo clippy --workspace --all-targets -- -D warnings \
      || die "clippy failed — nothing was committed or pushed"
    cargo fmt --all -- --check || die "cargo fmt --all would change files"
  fi
else
  warn "skipping the local test run; CI still gates the pull request"
fi

# ------------------------------------------------------------ bump

step "Release branch $RELEASE_BRANCH"
run git switch -c "$RELEASE_BRANCH"

step "Writing the new version"
if [ "$DRY_RUN" = "0" ]; then
  # Rewrite the version in [workspace.package] and every `version = "X"` that
  # appears in a workspace dependency line. awk rather than sed: BSD and GNU
  # sed disagree about the "first match only" idiom, and this has to work on
  # both macOS and Linux.
  awk -v new="$NEW" -v old="$CURRENT" '
    /^\[workspace\.package\]/ { inpkg = 1 }
    /^\[/ && !/^\[workspace\.package\]/ { inpkg = 0 }
    inpkg && /^version = "/ { sub(/"[^"]*"/, "\"" new "\"") }
    # repolish-core = { path = "...", version = "0.2.0" }
    /^repolish-[a-z]+ *= *\{/ { gsub("version = \"" old "\"", "version = \"" new "\"") }
    { print }
  ' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

  WROTE=$(awk '/^\[workspace\.package\]/{f=1} f && /^version = /{gsub(/version = "|"/,""); print; exit}' Cargo.toml)
  [ "$WROTE" = "$NEW" ] || die "Cargo.toml still reads $WROTE after the bump"
  STALE=$(grep -c "version = \"$CURRENT\"" Cargo.toml || true)
  [ "$STALE" = "0" ] || die "$STALE workspace dependency line(s) still pin $CURRENT.
Publishing would then resolve to the previous release."

  # Keep Cargo.lock's record of the workspace versions in step, or
  # `cargo build --locked` in CI fails on the very PR this script opens.
  cargo metadata --format-version 1 > /dev/null 2>&1 || true

  # SKILL.md states the current version in its install section, and it is
  # generated from a source file compiled into the binary. Bumping without
  # regenerating leaves the published skill telling agents to install the
  # previous release — caught by publish-clawhub.sh, but far too late.
  cargo run --quiet --release -p repolish -- . --stages skill \
    --output skills/repolish/SKILL.md --force --apply > /dev/null \
    || warn "could not regenerate skills/repolish/SKILL.md"

  # The npm package version is part of the URL it downloads the binary from, so
  # a stale one installs a release whose number says A and whose binary is B.
  # npm/test.js asserts the two match; this is what keeps that test passing.
  if [ -f "$NPM_DIR/package.json" ]; then
    perl -pi -e "s/^(\s*\"version\": \")\Q$CURRENT\E(\",)\$/\${1}$NEW\${2}/" "$NPM_DIR/package.json"
    NPM_WROTE=$(node -p "require('./$NPM_DIR/package.json').version")
    [ "$NPM_WROTE" = "$NEW" ] || die "$NPM_DIR/package.json still reads $NPM_WROTE after the bump"
  fi

  # The action pins a version in the workflow template it generates, and the
  # README documents an install command with the version in it. Both go stale
  # silently, so they are rewritten here rather than remembered.
  # 名单**必须**和下面的校验用同一份，否则漏掉的文件既不会被重写、
  # 也不会被发现。docs/ 曾经就不在名单里：从 0.3.0 起，那里的三个 action
  # 钉子一直指着两个 minor 之前的版本，而每次发布都「校验通过」。
  PINNED=$(git ls-files '*.md' '*.yml')
  for f in $PINNED; do
    [ -f "$f" ] || continue
    # 替换的是**任何**版本，不只是 $CURRENT。只替换 $CURRENT 的话，一个已经
    # 落后两个版本的钉子永远追不回来——它每一轮都不匹配，于是每一轮都留下。
    perl -pi -e "s/repolish\@v[0-9]+\.[0-9]+\.[0-9]+/repolish\@v$NEW/g; s/^(\s*default: )\Q$CURRENT\E\$/\${1}$NEW/; s/^VERSION=\Q$CURRENT\E\$/VERSION=$NEW/" "$f"
  done

  # Trust nothing: a regex that silently matched nothing would ship a release
  # whose docs still tell people to install the previous one.
  # 查的同样是**任何**旧钉子，并且报出文件和行号。
  LEFT=$(grep -n -o "repolish@v[0-9][0-9.]*" $PINNED 2>/dev/null \
    | grep -v "repolish@v$NEW\$" || true)
  [ -z "$LEFT" ] || die "these pins do not point at $NEW after the rewrite:
$LEFT"
fi
info "Cargo.toml, Cargo.lock and the pinned versions updated"

# ------------------------------------------------------------ commit + push

step "Committing"
if [ "$DRY_RUN" = "0" ]; then
  git add -A
  git diff --cached --quiet && die "nothing to commit — a pull request needs a commit"
  git commit -q -m "$MESSAGE" -m "Release v$NEW"
  info "$(git log -1 --oneline)"
else
  info "[dry-run] git commit -m \"$MESSAGE\""
fi

step "Pushing $RELEASE_BRANCH"
run git push -u origin "$RELEASE_BRANCH"

# ------------------------------------------------------------ pull request

step "Opening the pull request"
PR_NUM=""
if [ "$DRY_RUN" = "0" ]; then
  gh pr create \
    --base "$BASE_BRANCH" --head "$RELEASE_BRANCH" \
    --title "$MESSAGE" \
    --body "Release v$NEW.

$MESSAGE

Opened by publish.sh; merges itself once the required checks pass." > /dev/null
  PR_NUM=$(gh pr view "$RELEASE_BRANCH" --json number --jq .number)
  info "#$PR_NUM $(gh pr view "$PR_NUM" --json url --jq .url)"
else
  info "[dry-run] gh pr create --base $BASE_BRANCH --head $RELEASE_BRANCH"
fi

step "Merging"
if [ "$DRY_RUN" = "1" ]; then
  info "[dry-run] gh pr merge --squash --delete-branch"
elif gh pr merge "$PR_NUM" --squash --auto --delete-branch 2>/dev/null; then
  info "auto-merge armed; waiting for the required checks"
else
  warn "auto-merge unavailable; watching the required checks instead"
  gh pr checks "$PR_NUM" --watch --fail-fast \
    || die "required checks failed on #$PR_NUM. The release branch is still open;
fix it, push to $RELEASE_BRANCH, and merge the pull request yourself."
  gh pr merge "$PR_NUM" --squash --delete-branch || die "could not merge #$PR_NUM"
fi

# ------------------------------------------------------------ wait for main

step "Waiting for #${PR_NUM:-?} to land"
MERGE_SHA=""
if [ "$DRY_RUN" = "0" ]; then
  STATE=""
  for _ in $(seq 1 180); do
    STATE=$(gh pr view "$PR_NUM" --json state --jq .state 2>/dev/null || echo "")
    case "$STATE" in
      MERGED) break ;;
      CLOSED) die "#$PR_NUM was closed without merging" ;;
    esac
    sleep 10
  done
  [ "$STATE" = "MERGED" ] || die "timed out waiting for #$PR_NUM to merge.
Nothing was tagged. Check: gh pr view $PR_NUM --web"
  MERGE_SHA=$(gh pr view "$PR_NUM" --json mergeCommit --jq .mergeCommit.oid)
  info "merged as ${MERGE_SHA:0:12}"
fi

step "Syncing $BASE_BRANCH"
run git switch "$BASE_BRANCH"
run git fetch origin "$BASE_BRANCH"
# The squash rewrote history: the commit built locally is not the commit on
# main. Discarding the local branch is the point, not a side effect.
run git reset --hard "origin/$BASE_BRANCH"
# `gh pr merge --delete-branch` may already have taken the local branch with it,
# and `git branch -D` on a missing branch is a hard error under `set -e`. Losing
# a release between the merge and the tag over a cleanup step is not acceptable:
# the tag is the only thing that triggers the build and the crates.io publish.
if [ "$DRY_RUN" = "0" ]; then
  git branch -D "$RELEASE_BRANCH" 2>/dev/null || true
else
  info "[dry-run] git branch -D $RELEASE_BRANCH"
fi

# ------------------------------------------------------------ tag

if [ "$DRY_RUN" = "0" ]; then
  HEAD_SHA=$(git rev-parse HEAD)
  [ "$HEAD_SHA" = "$MERGE_SHA" ] \
    || die "origin/$BASE_BRANCH is at ${HEAD_SHA:0:12}, not the merge commit
${MERGE_SHA:0:12}. Something else landed; nothing was tagged."
  LANDED=$(awk '/^\[workspace\.package\]/{f=1} f && /^version = /{gsub(/version = "|"/,""); print; exit}' Cargo.toml)
  [ "$LANDED" = "$NEW" ] || die "main reads version $LANDED, not $NEW. Nothing was tagged."
fi

step "Tagging v$NEW"
if [ "$DRY_RUN" = "0" ]; then
  git tag -a "v$NEW" -m "v$NEW: $MESSAGE" "$MERGE_SHA"
else
  info "[dry-run] git tag -a v$NEW <merge commit>"
fi
run git push origin "v$NEW"

# ------------------------------------------------------------ binaries

step "Release workflow"
if [ "$DRY_RUN" = "0" ]; then
  RUN_ID=""
  # Filter by the tag: CI also runs on pull requests, so `--limit 1` alone can
  # hand back somebody else's run.
  for _ in $(seq 1 24); do
    RUN_ID=$(gh run list --workflow release.yml --branch "v$NEW" --limit 1 \
               --json databaseId --jq '.[0].databaseId' 2>/dev/null || true)
    [ -n "$RUN_ID" ] && break
    sleep 5
  done
  if [ -n "$RUN_ID" ]; then
    info "watching run $RUN_ID"
    gh run watch "$RUN_ID" --exit-status || die "the release workflow failed.
Inspect it with: gh run view $RUN_ID --log-failed
The tag v$NEW is already pushed; fix and release a higher version."
    info "${GREEN}binaries published${RESET}"
  else
    warn "could not find the workflow run; check https://github.com/$REPO_SLUG/actions"
  fi
else
  info "[dry-run] watch release.yml for tag v$NEW"
fi

# ------------------------------------------------------------ crates.io

if [ "$LOCAL_CRATES" = "1" ]; then
  # Escape hatch for when the workflow cannot run. Same order, same skip rule.
  step "Publishing ${#CRATES[@]} crates from here"
  for crate in "${CRATES[@]}"; do
    if [ "$DRY_RUN" = "1" ]; then
      info "[dry-run] cargo publish -p $crate"
      continue
    fi
    if cargo search "$crate" --limit 1 2>/dev/null | grep -q "^$crate = \"$NEW\""; then
      info "$crate $NEW is already on crates.io — skipping"
      continue
    fi
    info "publishing $crate"
    cargo publish -p "$crate" --locked --no-verify || die "cargo publish failed for $crate.
Crates already published are skipped, so re-running is safe:
    ./publish.sh --version $NEW --skip-tests --local-crates \"$MESSAGE\""
    if [ "$crate" != "${CRATES[-1]}" ]; then
      printf '    waiting for the index'
      for _ in $(seq 1 60); do
        if cargo search "$crate" --limit 1 2>/dev/null | grep -q "^$crate = \"$NEW\""; then
          printf ' ok\n'; break
        fi
        printf '.'; sleep 5
      done
      printf '\n'
    fi
  done
else
  # The `crates` job in release.yml publishes on the tag, using the repository's
  # CARGO_REGISTRY_TOKEN secret. Watching release.yml above already covered it.
  info "crates.io: published by the release workflow (--local-crates to do it here)"
fi

# ------------------------------------------------------------ npm

# After the release workflow, never before it. The package is a launcher: its
# postinstall downloads repolish-v$NEW-<target> from the GitHub release and
# verifies the .sha256 next to it. Published first, every `npx` would 404 until
# the binaries caught up.
if [ "$LOCAL_NPM" = "0" ]; then
  # The workflow published it over trusted publishing while we watched the run
  # above. Confirm rather than assume: that job is allowed to skip itself, and a
  # release that quietly did not reach npm is the failure this whole script
  # exists to prevent.
  step "Confirming $NPM_PKG@$NEW reached npm"
  if [ "$DRY_RUN" = "1" ]; then
    info "[dry-run] npm view $NPM_PKG@$NEW"
  else
    ON_NPM=0
    for _ in $(seq 1 20); do
      if npm view "$NPM_PKG@$NEW" version --registry "$NPM_REGISTRY" > /dev/null 2>&1; then
        ON_NPM=1; break
      fi
      sleep 5
    done
    if [ "$ON_NPM" = "1" ]; then
      info "${GREEN}https://www.npmjs.com/package/$NPM_PKG/v/$NEW${RESET}"
    else
      warn "$NPM_PKG@$NEW is not on npm.
If the npm job failed on authentication, trusted publishing is not configured:
    https://www.npmjs.com/package/$NPM_PKG/access
      -> add $REPO_SLUG, workflow release.yml
npm only offers that setting once the package exists, so a package it has never
seen has to be published once with --local-npm first.
The tag and the binaries are already out; publishing npm afterwards is safe."
    fi
  fi
else
  step "Publishing $NPM_PKG to npm from here"
  if [ "$DRY_RUN" = "1" ]; then
    info "[dry-run] npm publish $NPM_DIR ($NPM_PKG@$NEW)"
  elif npm view "$NPM_PKG@$NEW" version --registry "$NPM_REGISTRY" > /dev/null 2>&1; then
    info "$NPM_PKG@$NEW is already on npm — skipping"
  else
    # **Interactive, with no token anywhere.** npm will ask for the second
    # factor on this terminal, so this needs one — and it is the last step of a
    # long run, which means somebody has to still be sitting here.
    #
    # There used to be a token path, reading NPM_TOKEN out of .env into a
    # scoped .npmrc. It is gone because it cannot succeed: the package requires
    # 2FA and disallows bypass-2fa tokens, so every token publish comes back
    # EOTP. Forty lines that can only ever fail are worse than no lines.
    [ -t 0 ] || die "--local-npm publishes interactively, and npm asks for the second
factor on the terminal — but stdin is not one. Run this from a terminal, or let
the release workflow publish over trusted publishing (drop --local-npm)."

    # The shim parses tar and zip itself rather than take a dependency, and the
    # exit code it forwards is what --min-score gates ride on. Both are covered
    # by these tests, and this is the last moment they are free to run.
    node "$NPM_DIR/test.js" || die "npm shim tests failed; nothing was published"

    # --registry is pinned rather than inherited. A mirror
    # (registry.npmmirror.com and friends) is a read cache: publishing at it
    # either fails for want of auth — which is how this was found — or, worse,
    # succeeds somewhere nobody installs from.
    #
    # No --provenance: it needs a CI OIDC token, which a laptop does not have.
    # The workflow passes it when the workflow is the one publishing.
    (cd "$NPM_DIR" && npm publish --registry "$NPM_REGISTRY") \
      || die "npm publish failed for $NPM_PKG@$NEW.
If it asked you to log in:
    npm login --registry $NPM_REGISTRY
The tag and the binaries are already out, so re-running just this part is safe:
    cd $NPM_DIR && npm publish --registry $NPM_REGISTRY"

    info "${GREEN}https://www.npmjs.com/package/$NPM_PKG/v/$NEW${RESET}"
    info "If this was the package's first release, configure trusted publishing"
    info "now and --local-npm is never needed again:"
    info "  https://www.npmjs.com/package/$NPM_PKG/access"
  fi
fi

# ------------------------------------------------------------ clawhub

if [ "$WITH_CLAWHUB" = "1" ]; then
  step "Publishing the skill to ClawHub"
  if [ "$DRY_RUN" = "1" ]; then
    info "[dry-run] ./scripts/publish-clawhub.sh $NEW"
  else
    ./scripts/publish-clawhub.sh "$NEW" "$MESSAGE"
  fi
fi

printf '\n%sv%s%s\n' "$GREEN" "$NEW" "$RESET"
info "cargo install repolish"
info "npx $NPM_PKG"
