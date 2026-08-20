# repolish

**Score and improve what an open-source repository looks like to a first-time visitor — from the command line.**

[![crates.io](https://img.shields.io/crates/v/repolish.svg)](https://crates.io/crates/repolish)
[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/asale-ai/repolish/main/.repolish/badge.json)](https://github.com/asale-ai/repolish)
[![CI](https://github.com/asale-ai/repolish/actions/workflows/ci.yml/badge.svg)](https://github.com/asale-ai/repolish/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Docs](https://img.shields.io/badge/docs-design%20notes-blue.svg)](docs/README.md)

[English](README.md) · [中文](README.zh-CN.md)

## Contents

- [Why](#why)
- [Install](#install)
- [Quick start](#quick-start)
- [Usage](#usage)
- [What it checks](#what-it-checks)
- [How scoring works](#how-scoring-works)
- [Status](#status)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Why

Two kinds of tools already exist: generators that write your README with an LLM, and
linters that check whether a section is present. Neither answers the question an author
actually has — *what is wrong with my repository right now, and what do I change first?*

repolish reads a repository the way a stranger does, scores 22 concrete signals, and for
every point it deducts it names the file and line and tells you what to write instead.

Two rules keep the number worth trusting:

- **No model in the scoring path.** The same commit always produces the same score. An
  LLM can suggest wording later, but it never moves a number.
- **It says when it does not know.** A check that cannot be decided returns
  *inconclusive* and is excluded from the score rather than guessed at. Every excluded
  check is listed in the report.

## Install

### Pre-built binary

Every release ships binaries for five targets, each with a `.sha256` beside it:
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.

```bash
VERSION=0.1.0
TARGET=x86_64-unknown-linux-gnu
curl -fsSL "https://github.com/asale-ai/repolish/releases/download/v${VERSION}/repolish-v${VERSION}-${TARGET}.tar.gz" | tar -xz
sudo install "repolish-v${VERSION}-${TARGET}/repolish" /usr/local/bin/
```

Windows archives are `.zip` and contain `repolish.exe`. Browse every asset on the
[releases page](https://github.com/asale-ai/repolish/releases).

### With cargo

Requires Rust 1.88 or newer.

```bash
cargo install repolish
```

To build the unreleased `main` instead:

```bash
cargo install --git https://github.com/asale-ai/repolish repolish
```

### In GitHub Actions

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0
- uses: asale-ai/repolish@v0.1.0
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

`repolish init` writes a complete workflow for you, pinned to the version that generated
it. More examples in [action/README.md](action/README.md).

## Quick start

```bash
repolish check .
```

That is the whole thing: it prints a score, the findings behind it, and — under
`--remote` — what GitHub itself knows about the repository.

## Usage

```bash
repolish check .                    # local checks only, no network
repolish check . --remote           # also read description / topics / homepage from GitHub
repolish check . --format json      # machine-readable, schema frozen at version 1
repolish check . --min-score 70     # exit 1 when below the threshold
repolish check . --only license,ci-present
```

`--remote` reads `GITHUB_TOKEN` or `GH_TOKEN` from the environment. Without a token it
falls back to the anonymous quota of 60 requests per hour.

### As a CI gate

On GitHub, the action takes the threshold directly:

```yaml
- uses: asale-ai/repolish@v0.1.0
  with:
    min-score: 70
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

Anywhere else, the exit code is the gate:

```bash
repolish check . --remote --min-score 70
```

Exit code 1 means the score was too low. Exit code 4 means the GitHub call failed —
deliberately different, so a rate limit never reads as a quality regression.

### Exit codes

Tool failure and "checks did not pass" are deliberately different codes, so CI can tell
them apart.

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Score below `--min-score` |
| 2 | Bad arguments |
| 3 | Not a valid repository |
| 4 | `--remote` failed (API error, rate limit, private repo) |
| 5 | Fewer than half the checks could run — no total score is reported |

## What it checks

22 checks in three categories. Full definitions, weights and thresholds are in
[docs/03-评分维度.md](docs/03-评分维度.md).

| Category | Checks |
|---|---|
| **Discoverability** | README title and tagline, repository description, topics, homepage, badges |
| **Comprehensibility** | quickstart, usage example, install-command consistency, link health, length, docs presence, table of contents, translations |
| **Credibility** | license, **claim consistency**, CI, tests, activity, contributing guide, issue and PR templates, release hygiene, code of conduct |

**Claim consistency** is the one no other tool does: it verifies that the commands your
README promises actually exist. `npm run build` must be in `package.json`, `make test`
must be a real target, `./scripts/setup.sh` must be a real file. A README that fails on
its first command is where readers leave.

## How scoring works

Each check returns 0–10 and carries a risk weight (critical 10, high 7.5, medium 5,
low 2.5). The total is the weighted average, scaled to 100.

A check can also end up *not applicable* (a documentation site needs no test suite),
*inconclusive* (a shallow clone has no tags to judge releases by), or *skipped*. Only
scored checks count toward the denominator, and **if fewer than half the registered
weights were actually scored, no total is reported at all** — otherwise "we checked
three things and they passed" would read as 100/100.

Local and remote scores are not comparable: without `--remote`, three discoverability
checks drop out of the denominator. Reports label which one you are looking at.

## Status

Everything below is either shipped or explicitly not built yet. This section will keep
saying so until it changes:

| | |
|---|---|
| ✅ | `check` — 22 checks, `--remote`, JSON output, `--min-score` |
| ✅ | `badge`, `report`, `init`, GitHub Action, pre-built binaries for 5 targets, published on crates.io |
| ⏳ | README rewriting (`polish --apply`), LLM-assisted suggestions |

The check set and the JSON schema are frozen for v1: adding, removing, or reweighting a
check changes what a score means everywhere, so it is a versioned decision rather than
ordinary work.

## Development

```bash
git clone https://github.com/asale-ai/repolish
cd repolish
cargo test
cargo clippy --all-targets
./scripts/fetch-fixtures.sh
```

`fetch-fixtures.sh` clones the 12 real repositories used for manual acceptance. Each entry
is annotated with the defect that repository originally exposed.

Design documents live in [docs/](docs/README.md) and are written in Chinese.

## Contributing

Issues and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). By
participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.
