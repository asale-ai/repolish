---
name: repolish
description: >-
  Diagnose and improve what an open-source repository looks like to a first-time
  visitor. Use when asked to review, score, or improve a README, a repository's
  documentation, its "first impression", its discoverability, or its
  open-source hygiene (license, CI, contributing guide, issue templates) — and
  before rewriting any README by hand. Also use to generate the SVG overview
  card, the report card, SVG tables, or a VHS terminal recording for a CLI.
---

# repolish

A command-line tool that scores 22 concrete signals about a repository and, for
every point it deducts, names the file and line and says what to write instead.

**No model is in the scoring path.** The same commit always produces the same
score. You may suggest wording; you must never claim a number the tool did not
produce.

## The rule that matters most

**Do not rewrite the README.** Your instinct will be to replace it with a
well-structured one. That destroys the author's voice, their layout, their
examples, and usually their accuracy. Instead:

1. **Measure.** `repolish check .` — get the actual findings.
2. **Apply what is mechanical.** `repolish polish . --apply` — badge, table of
   contents, issue/PR templates, `CONTRIBUTING.md`. It only ever *inserts*; the
   diff is new lines and nothing else.
3. **Hand back what needs judgement.** A missing quickstart, a vague tagline, a
   README claim whose command no longer exists — those need the author's
   knowledge, or a targeted edit you can justify from evidence.
4. **Measure again** and report the delta.

Only edit prose directly when a finding names a specific line and the fix is
unambiguous, and say which finding you were acting on.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/asale-ai/repolish/main/install.sh | sh
```

That installs the binary into `~/.local/bin` and drops this skill into whichever
agents it finds on the machine. `cargo install repolish` works too, as does a
release binary from `https://github.com/asale-ai/repolish/releases` (five
targets, each with a `.sha256`). The current version is 0.3.0.

**Check the binary is reachable before you rely on it.** This skill calls
`repolish` by name; if `~/.local/bin` is not on PATH, every command below fails
with "command not found" rather than anything informative:

```bash
repolish --version
```

## Commands

### Score it

```bash
repolish check .                    # local checks only, no network
repolish check . --remote           # also read description / topics / homepage from GitHub
repolish check . --format json      # machine-readable, schema frozen at version 1
repolish check . --min-score 70     # exit 1 below the threshold, for CI
```

`--remote` reads `GITHUB_TOKEN` or `GH_TOKEN`. Without a token it falls back to
60 anonymous requests per hour.

**Prefer `--format json` when you are going to act on the result.** The text
output is laid out for a human reading a terminal; the JSON is stable and tells
you, per check, the score, the evidence (file and line) and the fixes.

Shape of the JSON, abridged:

```json
{
  "score": 82,
  "coverage": 0.86,
  "mode": "remote",
  "categories": [{ "category": "credibility", "score": 90 }],
  "checks": [
    {
      "id": "license",
      "category": "credibility",
      "risk": "critical",
      "outcome": {
        "kind": "scored",
        "score": 0,
        "evidence": [{ "file": ".", "line": null, "note": "no LICENSE file" }],
        "fixes": [{ "severity": "P1", "message": "Add a LICENSE file" }]
      }
    }
  ],
  "coverageLimits": ["repo-topics: requires --remote"]
}
```

Read `score: null` as **"no score"**, not zero: it means fewer than half the
registered checks could run. Report that honestly rather than picking a number.

An outcome can also be `notApplicable`, `inconclusive` or `skipped`. Those are
excluded from the score on purpose. Never present them as passes.

### Fix what can be fixed

```bash
repolish polish .                   # dry run: print the changes it would make
repolish polish . --apply           # write them
repolish polish . --apply -v        # also print every new file in full
```

What it will do: the repolish badge (plus the `.repolish/badge.json` it points
at), a table of contents built from the author's own headings, GitHub issue and
PR templates, and a `CONTRIBUTING.md` whose build and test commands come from the
**detected package manifest**.

What it will **not** do: rewrite a single existing line, invent a build command
when there is no manifest, or write a code of conduct. Where it cannot know, it
does not write.

`--apply` refuses to run outside a git repository unless you pass `--force`,
because `git checkout` is the undo button. Never pass `--force` on the user's
behalf without saying so.

### The visuals

```bash
repolish card .                     # .repolish/overview.svg — what this project is
repolish card . --kind score        # .repolish/card.svg — what repolish scored it
repolish card . --kind both
repolish card . --theme porcelain   # light palette, for a light-leaning README
repolish card . --lang zh-CN        # by default the card follows the README's language
```

The **overview card** goes at the top of the README: languages, file
composition, commit activity, licence. The **score card** goes at the bottom,
under a "Polished with repolish" heading. Do not swap them — the top of a README
belongs to the project, not to our tool.

To have `polish` insert them, and render README tables as SVG with the original
folded into `<details>`:

```bash
repolish polish . --apply --visuals
# same as: --overview --footer-card --tables svg
repolish polish . --apply --logo assets/hero.svg --logo-width full --align center
```

Every SVG is self-contained and deterministic: no external fonts, no scripts,
nothing hosted by a third party, and the same commit renders a byte-identical
file.

### Record a CLI

```bash
repolish demo .                     # runs the commands, writes .repolish/demo.svg
repolish demo . --dry-run           # list what it would run, run nothing
repolish demo . --cmd "tool build" --cmd "tool run"
repolish demo . --tape              # also write a VHS tape, for a GIF instead
```

**This executes the commands.** That is the point — the output in the recording is real —
but it means you must not run it against a repository whose commands you have not looked
at. Run `--dry-run` first and show the user the list. Never pass `--cmd` with anything
destructive.

The output is an animated SVG with a real text layer, not a GIF, and it needs no external
tools. Only meaningful when the project actually has a binary; the tape is a plain text
file the author is expected to edit.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Score below `--min-score` |
| 2 | Bad arguments |
| 3 | Not a valid repository |
| 4 | `--remote` failed (API error, rate limit, private repo) |
| 5 | Fewer than half the checks could run — no total score is reported |

Code 4 is deliberately distinct from code 1: a rate limit is not a quality
regression, and must never be reported as one.

## Making the calls repolish cannot

This is the part of the job that is actually yours, so it is worth being precise about
where the tool stops.

repolish decides three different ways, and only two of them are strong:

1. **Facts.** Does a LICENSE file exist, is there a workflow, how many headings are
   there. Filesystem and Markdown AST. Not arguable.
2. **Cross-references.** `claim-consistency` takes the commands out of the README and
   checks them against the manifest and the filesystem. `readme-install-consistency`
   checks that the install command installs *this* package. These are joins between two
   sources of truth, not opinions — and they are the checks worth acting on first.
3. **Graded heuristics.** `readme-quickstart` scores 0/4/6/8/10 from hand-curated
   substring lists. Most of the README checks have a list like that somewhere.

So the score is honestly a measure of **whether the machinery a reader needs is present
and whether the promises are true.** It is not a measure of whether the writing is any
good. repolish will happily give 10/10 to a quickstart made of the right keywords in the
wrong order.

That is the gap you are here to close. **Do not try to close it by rewriting.** Work
finding by finding:

| Finding | What a good fix looks like | The failure mode to avoid |
|---|---|---|
| `claim-consistency` | Make the claim true (restore the script, add the npm script) or correct the text to what actually works | **Deleting the line.** That turns the check green and leaves the reader with no instructions at all |
| `readme-title-tagline` | One line saying what it does and who it is for, in the author's register | Replacing a specific tagline with "A blazingly fast, modern toolkit for…" |
| `readme-quickstart` | The shortest path from zero to one working result, with the prerequisite named | Turning it into a feature tour, or inventing a command you have not run |
| `readme-usage-example` | A real example lifted from tests or `examples/` | Inventing an API that does not exist. Check it compiles or runs |
| `readme-length` | Move reference detail into `docs/` and link it | Deleting the detail |
| `license` | Tell the author their options and let them choose | Picking one for them. It is a legal decision, not a formatting one |
| `code-of-conduct` | Ask for a real reporting address | A Contributor Covenant with a placeholder email promises a channel that does not exist |
| `contributing` | Take build and test commands from the detected manifest | `<your build command here>` |
| `repo-description` / `repo-topics` / `repo-homepage` | Draft the text and hand it to the author | Changing repository settings yourself |

Three rules that hold across all of them:

- **Cite the finding.** Every edit you make should be traceable to an id and a
  file:line that repolish reported. If you cannot name one, you are redecorating.
- **Leave the voice alone.** Match the surrounding register, list markers, heading
  depth and line width. A README that suddenly reads like documentation-as-a-service is
  a worse README even when it scores higher.
- **A higher score is not the goal.** The goal is a repository a stranger can use. If a
  change would raise the number without helping that stranger, do not make it, and say
  why.

### Why there is no model inside repolish

If you are wondering whether to suggest wiring an LLM into it: the scoring path is
deliberately model-free, and that is not conservatism. A badge whose number moves because
a model answered differently this morning is worth nothing, and the same commit has to
produce the same score for the number to be comparable between repositories at all.

The intended arrangement is the one you are already in: **repolish supplies evidence,
you supply judgment.** You have context it structurally cannot have — the codebase, the
user's intent, this conversation. It has determinism you cannot have. Neither half is
improved by moving it into the other.

## Things to get right

- **Local and remote scores are not comparable.** Without `--remote`, three
  discoverability checks drop out of the denominator. Say which one you ran.
- **Do not tune thresholds.** `.repolish.toml` deliberately does not expose
  per-check thresholds; the check set and weights are frozen for v1 so scores
  stay comparable between repositories.
- **`claim-consistency` is the check to take seriously.** It verifies that the
  commands the README promises actually exist — `npm run build` in
  `package.json`, `make test` as a real target, `./scripts/setup.sh` as a real
  file. A README that fails on its first command is where readers leave. Fix
  those first, and fix them by making the claim true or by correcting it — never
  by deleting the line to make the check pass.
- **Report the numbers the tool gave you.** If it says `not scored`, say
  `not scored`.
