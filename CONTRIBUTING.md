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
```

To reproduce the manual acceptance run against real repositories:

```bash
./scripts/fetch-fixtures.sh
./target/debug/repolish check target/fixtures/ripgrep
```

The fixtures are shallow clones, matching what `actions/checkout` does by default. They
are not committed.

## The three rules that are not up for debate

**1. Scoring is deterministic.** `repolish-core` must not depend on `repolish-llm`. The
same commit must always produce the same score, byte for byte. A model may generate
suggestion text; it must never influence a number. If the score is not reproducible, the
badge is worthless and so is the tool.

**2. A false accusation costs more than a missed finding.** If a check cannot decide,
it returns `Inconclusive` with a reason and drops out of the denominator. It does not
guess. Concretely: `ruff check path/to/code/file.py` in a usage example is about the
*reader's* files, not this repository's, so it is not a broken claim.

**3. Everything the tool emits is in English.** Check messages, terminal output, CLI
help, and the comments in the workflow `repolish init` generates. `REPOLISH.md` gets
committed into other people's repositories, and a report in two languages is one nobody
keeps. Code comments and the design docs under `docs/` are in Chinese; that is
deliberate and separate. Recognising *Chinese* READMEs is input, not output, so the
heading aliases in `section.rs` and the stop-word lists stay as they are.
`tests/checks.rs::all_messages_are_english` fails the build if a message slips through.

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

- Run `cargo test` and `cargo clippy --all-targets` before pushing. CI runs both.
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
