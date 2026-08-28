# 03 · Scoring

[English](03-scoring.md) · [中文](03-scoring.zh-CN.md)

> The check list is final for v1. Individual thresholds may be tuned in implementation, but
> **checks are not added, removed or reweighted** — see the decision record at the end.

## Weights

Risk-weighted, following scorecard:

| Risk | Weight |
|---|---|
| Critical | 10 |
| High | 7.5 |
| Medium | 5 |
| Low | 2.5 |

**No category-level weights.** The three category scores are shown in the report but take
no part in the total — two layers of weighting make the effect of changing one number
impossible to reason about. The categories' real share falls out of the check weights
(discoverability ≈ 23%, comprehensibility ≈ 34%, credibility ≈ 43%).

---

## Aggregation

```
total = Σ(score_i × weight_i) / Σ(weight_i) × 10
```

Each check has four terminal states; only `Scored` counts toward the denominator:

| State | Meaning | In denominator | Listed as "not verified" | Affects badge |
|---|---|---|---|---|
| `Scored` | Decided, 0–10 | ✅ | | |
| `NotApplicable` | This project type does not need it | ❌ | ❌ | ❌ |
| `Inconclusive` | Wanted to check, objectively could not | ❌ | ✅ | ❌ |
| `Skipped` | Not run because of configuration or mode | ❌ | ✅ | ✅ labelled |

**Denominator protection:** if the weight of `Scored` checks is under **50%** of the
weight registered **for this run**, no total is reported — only the breakdown and the
reason. This prevents "most of it went unchecked, the remainder passed, therefore
100/100".

### Local and remote scores are not comparable — and must say so

Without `--remote`, `repo-description`, `repo-topics` and `repo-homepage` become `Skipped`
and drop out of the denominator. Those three are most of the discoverability weight, so a
local score and a remote score are **two different baselines** and cannot be compared.

How that is handled:

- `badge.json` carries a `mode` field (`"remote"` / `"local"`)
- when `mode = "local"` the badge label degrades to `repolish (local)`, so a reader can
  tell at a glance
- the workflow `repolish init` generates passes `--remote` by default, since `GITHUB_TOKEN`
  is free inside an Action — so the normal path produces a full score

---

## The checks

`local` = no network; `remote` = needs `--remote`. All 22 are implemented.

### Discoverability

| id | What it looks for | Risk | Source |
|---|---|---|---|
| `repo-description` | Description is non-empty and informative, not just the project name | High | remote |
| `repo-topics` | A sensible number of topics, cross-checked against local signals | High | remote |
| `repo-homepage` | The homepage field is set | Low | remote |
| `readme-title-tagline` | The first screen has a name and one line saying what this is | Critical | local |
| `readme-badges` | Basic badges are present (build / version / licence) | Low | local |

### Comprehensibility

| id | What it looks for | Risk | Source |
|---|---|---|---|
| `readme-quickstart` | An install or quick-start section exists | Critical | local |
| `readme-usage-example` | A copyable code example exists | High | local |
| `readme-install-consistency` | The install command matches the actual package manifest | High | local |
| `readme-link-health` | Relative links and images point at files that exist | Medium | local |
| `readme-length` | Neither too thin nor long enough to belong in `docs/` | Medium | local |
| `readme-toc` | A long README offers a table of contents | Low | local |
| `docs-presence` | A `docs/` directory or a link to a documentation site | Medium | local |
| `readme-i18n` | A translated README is offered | Low | local |

### Credibility

| id | What it looks for | Risk | Source |
|---|---|---|---|
| `license` | A LICENSE file exists and is a recognisable standard licence | Critical | local |
| `claim-consistency` | Commands, scripts and APIs the README promises actually exist | High | local |
| `ci-present` | A CI configuration exists | High | local |
| `tests-present` | A test directory or test files exist | High | local |
| `activity` | A commit within the last 90 days | High | local |
| `contributing` | A CONTRIBUTING file exists | Medium | local |
| `issue-pr-template` | Issue or PR templates under `.github/` | Medium | local |
| `release-hygiene` | Tags or releases exist, with notes | Medium | local |
| `code-of-conduct` | A code of conduct exists | Low | local |

---

## How `repo-topics` judges relevance

**No model takes part in scoring.** Relevance is split into two deterministic signals.

**1. Count bands**

| Topics | Score |
|---|---|
| 0 | 0 |
| 1–2 | 4 |
| 3–5 | 8 |
| 6–12 | 10 |
| 13–20 | 8 (padding; GitHub's limit is 20) |

**2. Cross-validation, which caps the score above**

An expected-topic vocabulary is built from three local signals:

- the primary and secondary languages, from the file-extension tally
- framework and ecosystem names in the manifests (`package.json`, `Cargo.toml`,
  `pyproject.toml`, …)
- nouns in the README's H1 and tagline

If the existing topics have an **empty intersection** with that vocabulary, the score is
capped at 5 and the `Fix` lists topics worth adding — taken straight from the vocabulary,
with no model involved.

**Semantic relevance is not judged.** A model could write better topic suggestions, but it
**does not move the number** — that is the determinism boundary from
[01-architecture](01-architecture.md).

---

## Project profiles

**Profiles never change the score line.** That would make scores incomparable and hard to
explain. A profile only decides whether certain checks **apply at all**
(`NotApplicable`, excluded from the denominator).

| Profile | Detected by |
|---|---|
| `cli` | An executable entry point (`[[bin]]`, a `bin` field, `console_scripts`) |
| `library` | A package manifest and publish config, with no executable entry point |
| `app` | A Dockerfile or deployment config, with no package publishing metadata |
| `docs` | Mostly Markdown, with very little code |
| `collection` | A very long README, many outbound links, almost no code (awesome-list shaped) |
| `meta` | The repository is named `.github`, or has `profile/README.md` and no code |

Non-applicability, exceptions only:

| Check | `NotApplicable` under |
|---|---|
| `tests-present` | `docs`, `collection` |
| `ci-present` | `collection` |
| `readme-quickstart` | `collection` |
| `readme-usage-example` | `docs`, `collection` |
| `readme-install-consistency` | `docs`, `collection`, and when no package manifest was detected |
| `readme-length` | `collection` — for a resource list the README *is* the content; length is its shape, not a defect |
| `docs-presence` | `docs` |
| **the other 19** | `meta`, see below |

### `meta`: organisation profile repositories

`OWNER/.github` is where GitHub keeps an organisation's calling card, and its content is a
single `profile/README.md` written for strangers. It is not a project: demanding a licence,
CI, tests and a CONTRIBUTING file from it produces a screen of false alarms — and a screen
of false alarms makes people distrust the whole table.

So `meta` keeps **three checks**, which happen to be the three questions "is this card
readable at all":

| Kept | Why |
|---|---|
| `readme-title-tagline` | Whether the opening says who this is and what they do — the entire point of a calling card |
| `readme-link-health` | Every link on a card is there for a stranger to click; a dead one is more embarrassing here than in a project README |
| `readme-length` | Whether the card is so short it says nothing |

In code, `Check::applies_to` **defaults** to not applying under `meta`, and those three
override it back. Defaulting to "not applicable" rather than "applicable" is the safe
direction: a newly added check cannot start firing at organisation profiles without anyone
noticing.

A `meta` repository's README lives under `profile/` by GitHub's convention rather than at
the root, and ingestion honours that.

Owner and name being identical **cannot** be used to detect a profile repository:
`chalk/chalk`, `eslint/eslint` and `prettier/prettier` are all real projects. That rule
only holds for a **user's** self-named repository, and telling a user from an organisation
takes an API call.

**`readme-toc` gives a short README full marks rather than `NotApplicable`.** "The
requirement is already met" and "this does not apply" are different: the latter removes the
check from the denominator and inflates everything else's share. A short README genuinely
passes.

Detection can be overridden with `profile = "library"` in `.repolish.toml` or `--profile`.
**The report must show the detected profile**, or an author is left wondering why some
checks vanished.

---

## Design principles

**1. Graded, not binary.** Each check returns 0–10. `readme-quickstart`: missing = 0;
heading but no command = 4; command but no prerequisites = 7; complete = 10. Binary
judgements make the score jump and leave the author no path to improve.

**2. Every deduction must carry an actionable `Fix`.** A check that says "you are missing
X" without saying how to add it does not get written. This is also the gate that keeps the
check count down.

**3. `claim-consistency` is the differentiator.** Does the `npm run build` in the README
exist? The `cargo xtask`? The module the example imports? No other tool checks this.

**4. `Inconclusive` beats guessing.** When something cannot be decided reliably, return
`Inconclusive` with a reason and list it under coverage limits. A false accusation destroys
trust in the whole report.

**5. Do not duplicate scorecard.** Security dimensions (SAST, fuzzing, signing, pinned
dependencies) are out of scope; link to it instead.

**6. Everything the tool emits is in English.** `Evidence`, `Fix` and `Inconclusive` text,
the terminal renderer, CLI help, and the comments in the workflow `init` generates.
`REPOLISH.md` gets committed into strangers' repositories, and nobody keeps a report in two
languages. The SVG cards are the deliberate exception — they are read by *other people's*
readers, so they follow that README's language. Recognising Chinese READMEs is input rather
than output and is unaffected. `tests/checks.rs::all_messages_are_english` and
`repolish-cli/tests/cli_is_english.rs` hold the line.

---

## Decision record

| Question | Conclusion | Why |
|---|---|---|
| Add category weights? | **No** | Two layers make tuning impossible to reason about; category scores are display only |
| How should `repo-topics` judge relevance? | **Count bands, capped by cross-validation against local signals**; a model may suggest but never scores | Keeps scoring deterministic, and cross-validation already catches almost every "nobody filled these in" case |
| Adjust expectations by project type? | **Adjust applicability, never the score line** | Keeps scores comparable and explainable |
| Round the check list to 20? | **22, frozen for v1** | The count is not the goal; rounding down would cut checks that earn their place. Additions require a minor version and a `repolishVersion` change, because they change what a score means |

**Consequences:** local and remote baselines differ → `badge.json` gained a `mode` field
and the local badge is labelled; `Outcome` grew from 2 states to 4; the 50% denominator
floor was added.
