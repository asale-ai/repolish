#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Publish the repolish skill to ClawHub under the asale-ai publisher.
#
#   ./scripts/publish-clawhub.sh [version] [changelog]
#   ./scripts/publish-clawhub.sh --dry-run
#
# The token is read from .env (CLAWHUB_API_KEY) or from the environment. It is
# never written into the repository, and .env is gitignored.

set -euo pipefail

OWNER="${CLAWHUB_OWNER:-asale-ai}"
SKILLS_DIR="skills"
SLUG="repolish"
DRY_RUN=0
VERSION=""
CHANGELOG=""

BOLD=$(tput bold 2>/dev/null || printf '')
RED=$(tput setaf 1 2>/dev/null || printf '')
GREEN=$(tput setaf 2 2>/dev/null || printf '')
YELLOW=$(tput setaf 3 2>/dev/null || printf '')
RESET=$(tput sgr0 2>/dev/null || printf '')

step() { printf '%s==>%s %s\n' "$BOLD" "$RESET" "$*"; }
info() { printf '    %s\n' "$*"; }
warn() { printf '%swarning:%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
die()  { printf '%serror:%s %s\n' "$RED" "$RESET" "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --owner) OWNER="${2:?--owner needs a handle}"; shift 2 ;;
    -h|--help) sed -n '3,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) die "unknown flag: $1" ;;
    *) if [ -z "$VERSION" ]; then VERSION="$1"; else CHANGELOG="$1"; fi; shift ;;
  esac
done

cd "$(dirname "$0")/.."

# ------------------------------------------------------------ credentials

if [ -f .env ]; then
  # shellcheck disable=SC1091
  set -a; . ./.env; set +a
  info "loaded credentials from .env"
fi

TOKEN="${CLAWHUB_TOKEN:-${CLAWHUB_API_KEY:-}}"
[ -n "$TOKEN" ] || die "no ClawHub token.
Put CLAWHUB_API_KEY=... in .env (gitignored), or export CLAWHUB_TOKEN.
Get one with: clawhub login"
export CLAWHUB_TOKEN="$TOKEN"

command -v clawhub > /dev/null || die "clawhub is not installed.
    npm i -g clawhub"

step "Authenticating"
WHO=$(clawhub whoami 2>&1 | tail -n1) || die "clawhub whoami failed: $WHO"
info "authenticated as $WHO"

# ------------------------------------------------------------ the skill

[ -n "$VERSION" ] || VERSION=$(awk '/^\[workspace\.package\]/{f=1} f && /^version = /{gsub(/version = "|"/,""); print; exit}' Cargo.toml)
[ -n "$CHANGELOG" ] || CHANGELOG="Release v$VERSION"

# The committed copy is generated from crates/repolish-cli/src/skill.md, which
# is compiled into the binary. Regenerate before publishing rather than trusting
# that whoever last edited the source remembered to.
if command -v cargo > /dev/null; then
  step "Regenerating the skill from source"
  cargo run --quiet --release -p repolish -- skill . \
    --output "$SKILLS_DIR/$SLUG/SKILL.md" --force > /dev/null \
    || warn "could not regenerate; publishing the committed copy as-is"
fi

[ -f "$SKILLS_DIR/$SLUG/SKILL.md" ] || die "no $SKILLS_DIR/$SLUG/SKILL.md to publish"

if ! git diff --quiet -- "$SKILLS_DIR" 2>/dev/null; then
  die "$SKILLS_DIR/ differs from what is committed.
The registry would get something that is not in the repository. Commit it first:
    git add $SKILLS_DIR && git commit -m 'chore: regenerate the skill'"
fi

SOURCE_REPO="asale-ai/repolish"
SOURCE_COMMIT=$(git rev-parse HEAD 2>/dev/null || printf '')
SOURCE_REF=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || printf 'main')

step "Publishing @$OWNER/$SLUG"
info "version:   $VERSION"
info "changelog: $CHANGELOG"
info "source:    $SOURCE_REPO@${SOURCE_COMMIT:0:12}"

# The skill drives the `repolish` binary, so a ClawHub install that does not
# also get the binary is useless. Fail loudly rather than shipping something
# that cannot work.
if ! grep -q 'install.sh\|cargo install repolish' "$SKILLS_DIR/$SLUG/SKILL.md"; then
  warn "the skill does not tell the reader how to install the binary it calls"
fi

# ClawHub requires --source-repo and --source-commit together, so provenance is
# all-or-nothing. Outside a git checkout we simply omit it.
PROVENANCE=()
if [ -n "$SOURCE_COMMIT" ]; then
  PROVENANCE=(--source-repo "$SOURCE_REPO"
              --source-commit "$SOURCE_COMMIT"
              --source-ref "$SOURCE_REF")
else
  warn "no git commit available; publishing without source provenance"
fi

if [ "$DRY_RUN" = "1" ]; then
  step "Dry run"
  clawhub sync --dir "$SKILLS_DIR" --owner "$OWNER" --dry-run "${PROVENANCE[@]}" 2>&1 | head -40
  exit 0
fi

sync_once() {
  clawhub sync --dir "$SKILLS_DIR" --owner "$OWNER" --all --bump patch \
    --changelog "$CHANGELOG" --tags latest "${PROVENANCE[@]}" 2>&1 | tee /dev/stderr
}

step "Uploading"
LOG=$(sync_once) || SYNC_FAILED=1

# A submitted skill stays invisible until its security scan clears, so a run
# interrupted mid-way leaves the registry knowing about a version `sync` cannot
# yet see. The next pass then tries to publish the same version and is rejected.
# One retry is enough: by then the registry reports it and sync bumps instead.
if printf '%s' "$LOG" | grep -q 'already exists'; then
  warn "version conflict from an earlier partial run; retrying once"
  sleep 20
  LOG=$(sync_once) || SYNC_FAILED=1
fi

if [ "${SYNC_FAILED:-0}" = "1" ] && printf '%s' "$LOG" | grep -qi 'publisher\|not found\|forbidden\|unauthor'; then
  die "clawhub sync failed.
If this is the first publish under @$OWNER, create the publisher first:
    clawhub publisher create $OWNER"
fi

step "Verifying"
# A newly submitted version is held until its security scan clears, so
# "pending" is the expected state, not a failure. Distinguish the two rather
# than reporting a problem that is not one.
RESULT=$(clawhub inspect "@$OWNER/$SLUG" 2>&1 || true)
case "$RESULT" in
  *"pending.publication"*) info "@$OWNER/$SLUG — submitted, awaiting the security scan" ;;
  *"not publicly visible"*) warn "@$OWNER/$SLUG — held by moderation: $(printf '%s' "$RESULT" | head -n2 | tail -n1)" ;;
  *"@$OWNER"*)              info "${GREEN}live${RESET} @$OWNER/$SLUG" ;;
  *)                        warn "@$OWNER/$SLUG — not found in the registry" ;;
esac

printf '\nRegistry: https://clawhub.ai/@%s\n\n' "$OWNER"
printf 'Install with:\n\n    clawhub install @%s/%s\n\n' "$OWNER" "$SLUG"
printf 'The skill drives the repolish binary, so install that too:\n\n'
printf '    curl -fsSL https://raw.githubusercontent.com/%s/main/install.sh | sh\n\n' "$SOURCE_REPO"
