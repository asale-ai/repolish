# Contributing to repolish

Thanks for taking the time. This document tells you how to build the project, what the
non-negotiable rules are, and what a good pull request looks like here.

## Getting it running

Requires Rust 1.88 or newer.

```bash
git clone https://github.com/asale-ai/repolish
cd repolish
cargo build
cargo test
cargo clippy --all-targets
cargo fmt --all -- --check
```

To reproduce the manual acceptance run against real repositories:

```bash
./scripts/fetch-fixtures.sh
./target/debug/repolish check target/fixtures/ripgrep
```

The fixtures are shallow clones, matching what `actions/checkout` does by default. They
are not committed.

## Brand assets

Everything under `assets/` is generated. Do not hand-edit those files — the logo in the
README and the mark on the report card are drawn from one piece of geometry, and two
hand-maintained copies become two different logos the first time someone edits one of
them. Regenerate with:

```bash
cargo run -p repolish-render --example logo
```

The colours and the letterforms live in `repolish-render`'s `theme` and `glyph` modules,
which the terminal renderer uses as well. Change them there and the terminal, the cards
and the logo all move together. `assets/hero.svg` is the full-width banner at the top of
the README: same geometry as the wordmark, but a wide viewBox, because that file is
referenced at `width="100%"` and a 450×56 viewBox stretched to full width becomes one
enormous line of type.

Every SVG the tool writes shares one set of primitives in `repolish-render/src/draw.rs`,
and every one of them must satisfy the same three constraints: **self-contained** (no
external fonts, scripts or images — one `http` in the output, and it is the SVG
namespace), **deterministic** (no timestamps, no randomness; the same commit renders a
byte-identical file, or CI commits a diff made of nothing but noise), and **one fixed
palette per file** (no `prefers-color-scheme`; GitHub serves SVGs through an image proxy
where media queries are not reliable). There are tests for all three.

## Releasing

```bash
./publish.sh "what changed"          # patch bump
./publish.sh --minor "add the card"
./publish.sh --version 1.0.0 "first stable release"
./publish.sh --clawhub "…"           # publish the skill to ClawHub as well
./publish.sh --dry-run "…"           # print every step, change nothing
```

One command does the whole release: run the tests, bump the workspace version,
rewrite every pinned `repolish@vX.Y.Z` in the docs, open a pull request, wait
for the required checks, tag the commit that actually landed, watch
`release.yml` build the five binaries, then publish the six crates to crates.io
**in dependency order** — `repolish-md`, `repolish-ingest`, `repolish-core`,
`repolish-checks`, `repolish-render`, `repolish` — waiting for the index between
each, because cargo rejects a crate whose path dependencies are not published
yet.

It is designed to be re-runnable. Crates already at the new version are skipped,
so a partial failure is fixed by running it again with `--version X.Y.Z
--skip-tests`. It refuses to start if the tree is dirty, if the branch is behind
`main`, if the tag already exists, or if there are no crates.io credentials —
each of those is far cheaper to hit before the tag is pushed than after.

## The three rules that are not up for debate

**1. Scoring is deterministic.** `repolish-core` must not depend on `repolish-llm`. The
same commit must always produce the same score, byte for byte. A model may generate
suggestion text; it must never influence a number. If the score is not reproducible, the
badge is worthless and so is the tool.

**2. A false accusation costs more than a missed finding.** If a check cannot decide,
it returns `Inconclusive` with a reason and drops out of the denominator. It does not
guess. Concretely: `ruff check path/to/code/file.py` in a usage example is about the
*reader's* files, not this repository's, so it is not a broken claim.

**3. Everything the tool emits is in English — except what gets embedded in someone
else's README.** Check messages, terminal output, CLI help, `REPOLISH.md`, and the
comments in the workflow `repolish init` generates are all English: their reader is an
author running an English CLI, and a report in two languages is one nobody keeps.
`tests/checks.rs::all_messages_are_english` fails the build if a message slips through.

The **SVG cards are the deliberate exception**, and the line is worth stating precisely:
a card is not read by the person who ran the command, it is read by the visitors of the
repository it is pasted into. A card saying `LANGUAGES · BY FILE` on top of a Chinese
README is our language pushed into someone else's front door. Every string on a card
therefore goes through `repolish-render/src/i18n.rs`, which is a *struct* rather than a
lookup table so a missing translation is a compile error rather than a silent fallback.
The language follows the README (`--lang auto`), never the shell's locale.

Code comments and the design docs under `docs/` are in Chinese; that is deliberate and
separate. Recognising *Chinese* READMEs is input, not output, so the heading aliases in
`section.rs` and the stop-word lists stay as they are.

## Adding or changing a check

The check set is frozen at 22 for v1. Adding, removing, or reweighting a check changes
what a score means across every repository that has one, so it is a versioned decision —
open an issue first. Threshold tuning inside an existing check is ordinary work.

Every check must satisfy:

- **Every deduction ships an actionable `Fix`.** "You are missing X" without "here is
  what to write" is not a check we accept. A `debug_assert` enforces this.
- **Evidence carries a file, and a line number when one exists.** Reports are read by
  people who are about to edit that file.
- **New judgement calls get a regression test named after the real repository that
  motivated them.** Most of the rules in this codebase exist because a real README broke
  an earlier assumption; the tests record which one.

## Pull requests

- Run `cargo test`, `cargo clippy --all-targets` and `cargo fmt --all` before pushing.
  CI runs all three, and `cargo fmt` is checked, not applied.
- Keep commit messages descriptive; explain *why*, not just what.
- If you changed check behaviour, re-run the fixtures and say in the PR which scores
  moved and why. A silent score change is the thing reviewers most need to see.
- Documentation lives in `docs/` and is written in Chinese. Code comments explain the
  reasoning behind non-obvious rules — please keep that habit rather than restating what
  the code does.

## Reporting bugs

The most valuable bug report is "repolish said X about repository Y, and it is wrong."
Include the repository URL, the check id, and what you expected. Nearly every judgement
rule in this codebase started as exactly that — a real repository that broke an earlier
assumption.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).
