<p align="center">
  <img src="assets/hero.svg" alt="" width="100%">
</p>

# repolish

**Score and improve what an open-source repository looks like to a first-time visitor — from the command line.**

[![crates.io](https://img.shields.io/crates/v/repolish.svg)](https://crates.io/crates/repolish)
[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/asale-ai/repolish/main/.repolish/badge.json)](https://github.com/asale-ai/repolish)
[![CI](https://github.com/asale-ai/repolish/actions/workflows/ci.yml/badge.svg)](https://github.com/asale-ai/repolish/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Docs](https://img.shields.io/badge/docs-design%20notes-blue.svg)](docs/README.md)

[English](README.md) · [中文](README.zh-CN.md)

<img src=".repolish/overview.svg" alt="repolish at a glance" width="880">

<sup>The overview card above is written by `repolish card .` and committed by CI on every
push — a plain file in this repository, no fonts, no scripts, nothing hosted by us. Our
score for this repository is at the [bottom of the page](#polished-with-repolish), where
it belongs.</sup>

## Contents

- [Why](#why)
- [Install](#install)
- [Quick start](#quick-start)
- [Usage](#usage)
- [The cards, the tables and the recording](#the-cards-the-tables-and-the-recording)
- [For coding agents](#for-coding-agents)
- [What it checks](#what-it-checks)
- [How scoring works](#how-scoring-works)
- [Status](#status)
- [Contributing](#contributing)
- [License](#license)

## Why

Two kinds of tools already exist: generators that write your README with an LLM, and
linters that check whether a section is present. Neither answers the question an author
actually has — *what is wrong with my repository right now, and what do I change first?*

repolish reads a repository the way a stranger does, scores 22 concrete signals, and for
every point it deducts it names the file and line and tells you what to write instead.

Two rules keep the number worth trusting. **No model in the scoring path** — the same
commit always produces the same score; an LLM can suggest wording, but never moves a
number. And **it says when it does not know** — a check that cannot be decided returns
*inconclusive* and is excluded rather than guessed at, and every excluded check is named.

## Install

### One line

```bash
curl -fsSL https://raw.githubusercontent.com/asale-ai/repolish/main/install.sh | sh
```

Downloads the release binary for your platform, verifies its `.sha256`, installs it into
`~/.local/bin`, and drops the [agent skill](#for-coding-agents) into whichever agents it
finds. It is POSIX `sh`, about 200 lines, and does exactly those four things — read it
first if you would rather not pipe a script into a shell. `REPOLISH_VERSION`,
`REPOLISH_BIN_DIR` and `REPOLISH_TARGET` override the defaults.

The Linux builds are glibc-only. On musl the installer says so and stops rather than
installing a binary that cannot run; use `cargo install repolish` there.

### With cargo

Requires Rust 1.88 or newer.

```bash
cargo install repolish
```

To build the unreleased `main` instead:

```bash
cargo install --git https://github.com/asale-ai/repolish repolish
```

Release archives for five targets, each with a `.sha256`, are on the
[releases page](https://github.com/asale-ai/repolish/releases). For the GitHub Action —
which `repolish init` will write a workflow for — see [action/README.md](action/README.md).

## Quick start

```bash
repolish check .
```

That is the whole thing. Here it is against `demo/sample`, a repository written badly on
purpose — check it, fix what can be fixed, check it again:

<img src=".repolish/demo.svg" alt="repolish scoring a rough repository, fixing it, and scoring it again" width="910">

<sup>Recorded by repolish itself — see [Recording a CLI](#recording-a-cli). The two
scores in it are whatever that run actually produced; a tool whose job is checking that a
README's promises are true has no business faking its own demo. It is re-recorded by hand
rather than on every push, for a reason worth reading:
[demo/README.md](demo/README.md).</sup>

<details>
<summary>The first of those three commands, as text</summary>

```text
  acme/taskvault  · cli (detected) · local · 52d9d0e4

  SCORE   23 / 100    poor        ▄▄▄▄▄▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁

  DISCOVERABILITY    ▄▄▄▄▄▄▁▁▁▁▁▁   56
  COMPREHENSIBILITY  ▄▄▄▁▁▁▁▁▁▁▁▁   28
  CREDIBILITY        ▄▁▁▁▁▁▁▁▁▁▁▁   13

  CHECKS  ●○○○●●●○○●●●●●●●●●●●●●   17 scored · 5 not verified

  ── TO FIX ──────────────────────────────────────────────────────────────

   P1  claim-consistency
       1 of the 1 verifiable command claims in the README no longer work.
       Typing the first command from a README and getting an error is the
       fastest way to lose a user
       └ README.md:8  `scripts/setup.sh` — does not exist in the repository

   P1  license
       Add a LICENSE file. No license means all rights reserved — legally,
       nobody may use your code
       └ .  no LICENSE file in the repository root
```

</details>

Three things there are the point of the tool: **`README.md:8`** — every deduction names a
file and, where there is one, a line; **`5 not verified`** — checks it could not decide are
counted separately and never folded into the score as if they had passed; and **`local`** —
the report always says which baseline produced it, because a local score and a `--remote`
one are not comparable. Run `repolish check . -v` for the full finding list.

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

### Styling what it inserts

```bash
repolish polish . --badge-style for-the-badge --toc-style fold --align center
repolish polish . --logo assets/hero.svg --logo-width full --tree-depth 2
repolish polish . --visuals         # --overview --footer-card --tables svg
```

Also settable under `[readme]` in `.repolish.toml`; the full list is in
[docs/02-cli-design.md](docs/02-cli-design.md). **None of it moves a score** — the check
list and weights are frozen at v1, so a repository cannot look better by picking a
different badge style. The logo, the tree and the cards are not driven by any check; they
stay off unless you ask, and the dry run says "requested by configuration" rather than
dressing them up as fixes.

### Fixing what can be fixed

```bash
repolish polish .                   # print the changes it would make
repolish polish . --apply           # write them
```

`polish` only makes changes that follow mechanically from the findings: the repolish
badge, a table of contents built from your own headings, GitHub issue and pull request
templates, and a `CONTRIBUTING.md` whose build and test commands come from your **detected
package manifest**.

**Where it cannot know, it does not write.** No manifest means no `CONTRIBUTING.md`,
because the alternative is `<your build command here>` — a file that turns the check green
while the problem stays exactly where it was.

**It only inserts.** The diff is new lines and nothing else: your tabs, list markers,
reference-style link definitions and line endings survive byte for byte. `--apply` refuses
to run outside a git repository unless you pass `--force`, because `git checkout` is the
undo button.

### As a CI gate

On GitHub, the action takes the threshold directly:

```yaml
- uses: asale-ai/repolish@v0.3.0
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

<img src=".repolish/tables/exit-codes.svg" alt="Exit codes" width="880">

<details>
<summary>Exit codes as a table</summary>

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Score below `--min-score` |
| 2 | Bad arguments |
| 3 | Not a valid repository |
| 4 | `--remote` failed (API error, rate limit, private repo) |
| 5 | Fewer than half the checks could run — no total score is reported |

</details>

## The cards, the tables and the recording

Everything repolish draws is a **self-contained, deterministic SVG**: no external fonts,
no scripts, nothing hosted by us, and the same commit renders a byte-identical file. All
of it is a plain file in **your** repository.

```bash
repolish card .                 # .repolish/overview.svg — what this project is
repolish card . --kind score    # .repolish/card.svg     — what repolish scored it
repolish card . --kind tables   # redraw the README's tables
repolish demo .                 # run the CLI and record it as an animated SVG
repolish polish . --apply --visuals   # insert all of the above into the README

repolish card . --theme porcelain   # light palette, for a light-leaning README
repolish card . --lang ja           # en / zh-CN / ja; by default it follows your README
```

**Where each one goes is the point.** The overview card belongs at the top, under the
badges: a stranger's first question is what this is and whether it is still alive. The
report card belongs at the [end](#polished-with-repolish) — at the top it would mean the
first thing a visitor sees is our tool grading your project instead of your project.

`--tables svg` draws each README table as a picture and folds the original into
`<details>` beneath it, in every language the repository has a README for. The original
stays: an image has no text layer, so screen readers, `grep` and the next person to edit
that table all read the folded copy.

`repolish demo` **runs** the commands and records the result — the scores in the recording
at the top of this page are what that run actually produced. Use `--dry-run` to see the
commands first.

The reasoning behind all of it — why no `prefers-color-scheme`, why frame zero of the
recording is the finished state, why tables are named by slug rather than index, why kana
decides Japanese before the CJK ratio decides Chinese — is in
[docs/02-cli-design.md](docs/02-cli-design.md).

## For coding agents

Ask an agent to "improve this README" and its first move is to rewrite the whole file.
That replaces the author's voice, layout and examples with something that reads like every
other README, and it is exactly the failure this tool exists to prevent.

```bash
repolish skill --list             # which agents are installed here
repolish skill --target detect    # install into the ones that are
repolish skill .                  # or write SKILL.md into a repository
```

[skills/repolish/SKILL.md](skills/repolish/SKILL.md) is the file to hand to Claude Code,
Codex, Gemini, OpenCode or anything reading `AGENTS.md`. Beyond the command surface it
carries the order — **measure, apply what is mechanical, hand back what needs judgement,
measure again** — where repolish's own confidence runs out, and what a good fix looks like
per finding. A `claim-consistency` failure is fixed by making the claim true, **never** by
deleting the line, which turns the check green and leaves the reader with nothing.

## What it checks

22 checks in three categories. Full definitions, weights and thresholds are in
[docs/03-scoring.md](docs/03-scoring.md).

<img src=".repolish/tables/what-it-checks.svg" alt="What it checks" width="880">

<details>
<summary>What it checks as a table</summary>

| Category | Checks |
|---|---|
| **Discoverability** | README title and tagline, repository description, topics, homepage, badges |
| **Comprehensibility** | quickstart, usage example, install-command consistency, link health, length, docs presence, table of contents, translations |
| **Credibility** | license, **claim consistency**, CI, tests, activity, contributing guide, issue and PR templates, release hygiene, code of conduct |

</details>

**Claim consistency** is the one no other tool does: it verifies that the commands your
README promises actually exist. `npm run build` must be in `package.json`, `make test`
must be a real target, `./scripts/setup.sh` must be a real file. A README that fails on
its first command is where readers leave.

## How scoring works

Each check returns 0–10 and carries a risk weight; the total is the weighted average.
Checks that end up *not applicable*, *inconclusive* or *skipped* are excluded from the
denominator rather than counted as passes — and **if fewer than half the weights were
scored, no total is reported at all**, because "we checked three things and they passed"
must not read as 100/100.

Weights, thresholds and the aggregation rules are in
[docs/03-scoring.md](docs/03-scoring.md).

## Status

Everything described above is shipped. Still to come: LLM-assisted wording suggestions,
with no model in the scoring path.

The check set and the JSON schema are frozen for v1. Adding, removing or reweighting a
check changes what a score means everywhere, so it is a versioned decision rather than
ordinary work.

## Contributing

Issues and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) covers building
the project, the three rules that are not up for debate, how to add a check, and the
release runbook. Design notes are in [docs/](docs/README.md). By participating you agree
to the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.

## Polished with repolish

<img src=".repolish/card.svg" alt="repolish report card" width="880">

This card is generated by [repolish](https://github.com/asale-ai/repolish) and is a plain file in this repository — no external fonts, no scripts, nothing hosted by a third party. To score your own: `cargo install repolish && repolish check .`.

