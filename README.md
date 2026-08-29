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

<sup>The overview card above is written by `repolish card .` and committed by CI — a
plain file in this repository, no fonts, no scripts, nothing hosted by us. Our score for
it is at the [bottom of the page](#polished-with-repolish), where it belongs.</sup>

## Contents

- [Why](#why)
- [Install](#install)
- [Quick start](#quick-start)
- [Usage](#usage)
- [Proving the README is true](#proving-the-readme-is-true)
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
every point it deducts names the file and line and what to write instead. Then it
[runs the commands](#proving-the-readme-is-true) your README promises, to prove they
still work.

Two rules keep the number worth trusting. **No model in the scoring path** — the same
commit always produces the same score; a model can suggest wording, but never moves a
number. And **it says when it does not know** — a check that cannot be decided returns
*inconclusive* and is excluded rather than guessed at, and every excluded check is named.

## Install

### One line

```bash
curl -fsSL https://raw.githubusercontent.com/asale-ai/repolish/main/install.sh | sh
```

Downloads the release binary for your platform, verifies its `.sha256`, installs it into
`~/.local/bin`, and drops the [agent skill](#for-coding-agents) into whichever agents it
finds. It is POSIX `sh`, about 200 lines, and does exactly those four things — read it first if
you would rather not pipe a script into a shell. `REPOLISH_VERSION`, `REPOLISH_BIN_DIR`
and `REPOLISH_TARGET` override the defaults.

### With npx

```bash
npx repolish check .
```

A launcher, not a reimplementation: it downloads the same release binary, verifies its
`.sha256`, and runs it. It exists because most repositories worth checking are not Rust
projects, and `cargo install` asks those authors to set up a toolchain before they can
see a single number.

Linux builds are glibc-only. On musl the installers say so and stop rather than leaving a
binary that cannot run; use `cargo install repolish` there.

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
[releases page](https://github.com/asale-ai/repolish/releases). For the GitHub Action,
see [action/README.md](action/README.md).

## Quick start

```bash
repolish check .
```

That is the whole thing. Here it is against `demo/sample`, written badly on purpose —
check, fix, check again:

<img src=".repolish/demo.svg" alt="repolish scoring a rough repository, fixing it, and scoring it again" width="910">

<sup>Recorded by repolish itself. The two scores in it are whatever that run actually
produced; a tool whose job is checking that a README's promises are true has no business
faking its own demo. Why it is re-recorded by hand rather than on every push:
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

`--remote` reads `GITHUB_TOKEN` or `GH_TOKEN`; without one, the anonymous quota is 60
requests per hour.

### Styling what it inserts

```bash
repolish polish . --badge-style for-the-badge --toc-style fold --align center
repolish polish . --logo assets/hero.svg --logo-width full --tree-depth 2
repolish polish . --visuals         # --overview --footer-card --tables svg
```

Also settable under `[readme]` in `.repolish.toml`
([the full list](docs/02-cli-design.md)). **None of it moves a score** — the check list
and weights are frozen at v1, so a repository cannot look better by picking a different
badge style.

### Fixing what can be fixed

```bash
repolish polish .                   # print the changes it would make
repolish polish . --apply           # write them
```

`polish` only makes changes that follow mechanically from the findings: the repolish
badge, a table of contents built from your own headings, GitHub issue and PR templates,
and a `CONTRIBUTING.md` whose commands come from your **detected package manifest**.

**Where it cannot know, it does not write.** No manifest means no `CONTRIBUTING.md`,
because the alternative is `<your build command here>` — a file that turns the check green
while the problem stays exactly where it was.

**It only inserts.** The diff is new lines and nothing else: your tabs, list markers,
reference-style link definitions and line endings survive byte for byte. `--apply` refuses
to run outside a git repository unless you pass `--force`, because `git checkout` is the
undo button.

### The three things it cannot write for you

```bash
repolish polish . --suggest         # needs REPOLISH_LLM_API_KEY
```

The heaviest checks are the ones no mechanical rule can satisfy: the tagline (Critical),
the quick start (Critical), the usage example (High). `--suggest` drafts those three, and
only those three.

**No model in the scoring path** is a rule about *scoring*, and it has not moved — run
`check` before and after, the number is identical. Extending it to *fixing* was the
mistake: it left `polish` inserting badges while the author was stuck on the sentence
under the title.

The boundary sits elsewhere, and is stricter. It **never writes**, not even with
`--apply`. It **only fills gaps**, never rewriting what is there. And it **cannot
invent**: given the repository's real manifest, it is told to leave a suggestion empty
rather than make one up — a fabricated install command being exactly what
`claim-consistency` was built to catch. [Why each](docs/02-cli-design.md).

### As a CI gate

On GitHub, the action takes the threshold directly:

```yaml
- uses: asale-ai/repolish@v0.3.0
  with:
    min-score: 70
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

Anywhere else the exit code is the gate: `repolish check . --remote --min-score 70`.
Exit 1 means the score was too low; exit 4 means the GitHub call failed — deliberately
different, so a rate limit never reads as a regression.

#### On a pull request, the change is the story

An absolute score tells a reviewer nothing. *This pull request dropped it four points,
because the link on line 42 stopped resolving* tells them what to do.

```bash
repolish check . --base origin/main
repolish check . --sarif repolish.sarif    # one annotation per finding, on its line
repolish check . --comment comment.md      # the short form, for a PR comment
```

`--base` checks the baseline out into a **temporary git worktree** and scores it with the
identical options — your working tree is never touched, a local score is never compared
against a remote one, and the report lists only the checks that moved.

Every deduction has carried a file and a line since the first release; SARIF is what puts
them **in the diff** instead of in a log nobody expands. `repolish init` writes a workflow
wiring all three up on every pull request — see
[action/README.md](action/README.md).

### Exit codes

Tool failure and "checks did not pass" are deliberately different codes.

<img src=".repolish/tables/exit-codes.svg" alt="Exit codes" width="880">

<details>
<summary>Exit codes as a table</summary>

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Score below `--min-score`, or a README command failed under `verify` |
| 2 | Bad arguments |
| 3 | Not a valid repository |
| 4 | `--remote` failed (API error, rate limit, private repo) |
| 5 | Fewer than half the checks could run — no total score is reported |
| 6 | `verify --run` could not run: no container engine, or the container died |
| 7 | `--base` could not be resolved: shallow clone, unknown ref, no git |

</details>

## Proving the README is true

`claim-consistency` checks that the commands your README promises **exist**, which catches
renames and deletions. It does not catch what a new user actually hits: the command is
still there, but it needs a system package nobody wrote down, and step one fails on a
machine that is not yours.

`verify` closes that gap by **running them**.

```bash
repolish verify .                   # print the plan; run nothing
repolish verify . --run             # execute them in a clean container
repolish verify . --run --image python:3.12 --section Install
```

Without `--run` it prints what it would execute and why it would skip the rest. With
`--run` it executes them **in a container**, in one shell session — so a `cd` in step two
still applies to step three, the way a reader following along experiences it.

Three rules it does not bend. **Never on your machine**: with no container engine it stops
at exit code 6 rather than falling back to your host. **Your repository is mounted
read-only** and copied in. And **every skipped command is listed with its reason** — a
report saying "12 passed" while quietly skipping nine is worse than no report.
[What it skips, and why](docs/02-cli-design.md).

In CI the exit code is the point: **1 when a command failed**, the same class of event as
a score below the threshold; **6 when it could not run at all**, which is not a quality
regression and must not read as one.


## The cards, the tables and the recording

```bash
repolish card .                 # .repolish/overview.svg — what this project is
repolish card . --kind score    # .repolish/card.svg     — what repolish scored it
repolish card . --kind tables   # redraw the README's tables
repolish demo .                 # run the CLI and record it as an animated SVG
repolish polish . --apply --visuals   # insert all of the above into the README
```

Everything repolish draws is a **self-contained, deterministic SVG**: no external fonts,
no scripts, nothing hosted by us, and the same commit renders a byte-identical file. It is
a plain file in **your** repository, so it cannot go 404 on you, rate-limit you, or log
who read your README.

**Where each one goes is the point.** The overview card belongs at the top, under the
badges. The report card belongs at the [end](#polished-with-repolish) — at the top, the
first thing a visitor sees would be our tool grading your project instead of your project.

The rest — `--theme`, `--lang`, why `--stars` only works on repositories you administer,
why `demo` really runs the commands it records — is in
[docs/02-cli-design.md](docs/02-cli-design.md).

## For coding agents

Ask an agent to "improve this README" and its first move is to rewrite the whole file —
replacing the author's voice, layout and examples with something that reads like every
other README. That is exactly the failure this tool exists to prevent.

```bash
repolish skill --list             # which agents are installed here
repolish skill --target detect    # install into the ones that are
repolish skill .                  # or write SKILL.md into a repository
```

[skills/repolish/SKILL.md](skills/repolish/SKILL.md) is the file to hand to Claude Code,
Codex, Gemini, OpenCode or anything reading `AGENTS.md`. Beyond the command surface it
carries the order — **measure, apply what is mechanical, hand back what needs judgement,
measure again** — and what a good fix looks like per finding.

## What it checks

22 checks in three categories. Full definitions and weights:
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
must be a real target. A README that fails on its first command is where readers leave —
and `repolish verify` takes those same commands and
[actually runs them](#proving-the-readme-is-true).

## How scoring works

Each check returns 0–10 and carries a risk weight; the total is the weighted average.
Checks that end up *not applicable*, *inconclusive* or *skipped* are excluded from the
denominator rather than counted as passes — and **if fewer than half the weights were
scored, no total is reported at all**, because "we checked three things and they passed"
must not read as 100/100. Weights, thresholds and the aggregation rules are in
[docs/03-scoring.md](docs/03-scoring.md).

## Status

Everything described above is shipped, `verify` and the wording suggestions included —
with no model in the scoring path, the rule that has not moved.

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

