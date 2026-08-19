# repolish

**Score and improve what an open-source repository looks like to a first-time visitor — from the command line.**

[![CI](https://github.com/asale-ai/repolish/actions/workflows/ci.yml/badge.svg)](https://github.com/asale-ai/repolish/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Docs](https://img.shields.io/badge/docs-design%20notes-blue.svg)](docs/README.md)

[English](README.md) · [中文](README.zh-CN.md)

## Contents

- [Why](#why)
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

## Quick start

Requires Rust 1.88 or newer. Pre-built binaries and `cargo install` arrive with the first
release — see [Status](#status).

```bash
git clone https://github.com/asale-ai/repolish
cd repolish
cargo build --release
./target/release/repolish check .
```

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

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0        # release-hygiene needs tags; the default depth of 1 has none
- run: repolish check . --remote --min-score 70
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

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
[docs/05-评分维度.md](docs/05-评分维度.md).

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

`repolish check` is complete and dogfooded on 12 real repositories. Everything else is
not built yet, and this section will say so until it is:

| | |
|---|---|
| ✅ | `check` — 22 checks, `--remote`, JSON output, `--min-score` |
| ⏳ | `badge`, `report`, `init`, GitHub Action, pre-built binaries |
| ⏳ | README rewriting (`polish --apply`), LLM-assisted suggestions |

See [docs/06-路线图.md](docs/06-路线图.md) for the roadmap and a written record of every
defect found during acceptance.

## Development

```bash
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
