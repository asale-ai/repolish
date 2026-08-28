# 02 · CLI design

[English](02-cli-design.md) · [中文](02-cli-design.zh-CN.md)

Binary name: `repolish`

## Command surface

```
repolish check .                 # local only, terminal report + exit code
repolish check . --remote        # adds GitHub metadata (topics / description / homepage)
repolish check . --format json   # for CI to consume
repolish check . --min-score 70  # non-zero exit below the threshold → a CI gate
repolish check . --badge         # also write .repolish/badge.json
repolish check . --card          # also write .repolish/card.svg (the score card)
repolish check . --overview      # also write .repolish/overview.svg (the overview card)

repolish badge .                 # write .repolish/badge.json + print a markdown snippet
repolish card .                  # write .repolish/overview.svg (the default)
repolish card . --kind score     # write .repolish/card.svg
repolish card . --kind tables    # redraw every table SVG in the README
repolish card . --kind all       # all of the above
repolish card . --remote --stars # add the star history curve
repolish report .                # write REPOLISH.md
repolish demo .                  # really run the commands, write .repolish/demo.svg
repolish demo . --dry-run        # list the commands it would run, run nothing
repolish demo . --tape           # also write a VHS tape, for people who want a GIF
repolish skill .                 # write SKILL.md into a repository
repolish skill --list            # which agents are installed on this machine
repolish skill --target detect   # install the skill into the ones that are
repolish init                    # generate .github/workflows/repolish.yml

repolish polish .                # print the mechanical changes, write nothing
repolish polish . --apply        # write them; the user commits
repolish polish . --apply --visuals   # plus overview card, footer score card, SVG tables
```

**`card` overwrites; `polish` does not.** That division is deliberate and is the only
thing to remember about these two commands: `polish` inserts the `<img>` reference into
the README the first time and never touches that file again; `card` handles every redraw
afterwards. The other way round, either `polish` breaks its never-overwrite invariant, or
the README keeps showing the image generated the first time forever. CI runs `card`.

## Global flags

| Flag | Meaning |
|---|---|
| `--format <text\|json\|markdown>` | Default `text` |
| `--config <path>` | Defaults to `.repolish.toml` |
| `--profile <auto\|library\|app\|cli\|docs\|collection\|meta>` | Default `auto`, overrides detection |
| `--only <ids>` / `--skip <ids>` | Filter by check id (filtered checks become `Skipped`) |
| `--theme <dark\|porcelain>` | Palette for the SVG output |
| `--lang <auto\|en\|zh-CN\|ja>` | Language of the text inside the SVG output |
| `--stars` | Also fetch the star history curve. Needs `--remote`; costs ~12 API calls |
| `--no-color` | For CI |
| `-v` | Expand every check and the passing list |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success; with `--min-score`, the threshold was met |
| 1 | Score below `--min-score` |
| 2 | Bad arguments or bad configuration |
| 3 | The target is not a valid git repository |
| 4 | Remote call failed (API error or rate limit under `--remote`) |
| 5 | Under 50% coverage, so no total is reported |

The tool failing and the checks failing must be different exit codes, or CI cannot tell
them apart.

---

### The star history curve

`--stars` adds a star-growth curve to the overview card. Off by default, because it is the
only part of repolish that costs more than one API call.

**GitHub has no "stars over time" endpoint.** What it does have is `/stargazers`, which
with `Accept: application/vnd.github.star+json` returns stargazers **in the order they
starred**, each with `starred_at`. So page *k*'s first entry is the exact moment the
repository reached star *(k-1)×100+1*. Sampling a dozen pages therefore yields a dozen
**exact** points; the straight lines between them are the only approximation.

Three consequences worth stating:

- **The curve starts at the repository's creation, at zero stars.** That is not an
  invented point — the repository genuinely had no stars then. It makes the left edge the
  real beginning rather than "the first star", and it means a repository with a single
  star still has a curve. Which is the owner most likely to want one.
- **The last point is the newest stargazer's `starred_at`, not "now".** The curve is then
  entirely a function of remote state, so the same state renders the same file. Using the
  clock would give a slightly different tail on every run.
- **The x axis is time, not sample index.** Sampling is uniform in pages, but stars do not
  arrive uniformly; plotting by index would draw a quiet year and a viral week the same
  width.
- **GitHub restricts the list to admins and collaborators.** Since July 2026 the
  stargazer endpoints are limited to people with access to the repository; anyone else
  gets 404, and unauthenticated requests get 401. There is no way around it, so a failure
  reports the reason rather than leaving a blank space on the card. It costs little in
  practice: repolish scores *your* repository, and on your own repository you are an
  admin.
- **Pagination is capped at 400 pages.** Beyond 40,000 stars the early history is not
  reachable, and the curve starts where the data starts rather than pretending otherwise.

A failure fetching the curve returns no curve rather than an error: it is decoration on a
card, and it must not turn "rate limit reached" into "scoring failed". Fewer than two
points means the section is not drawn at all — an empty chart frame reads as "this project
has no stars".

---

## Output contracts

### `.repolish/badge.json`

Follows the [shields.io endpoint protocol](https://shields.io/badges/endpoint-badge):

```json
{
  "schemaVersion": 1,
  "label": "repolish",
  "message": "88/100",
  "color": "brightgreen",
  "repolishVersion": "0.3.0",
  "mode": "remote"
}
```

Colour thresholds: `>=90` brightgreen, `>=75` green, `>=60` yellow, `>=40` orange, `<40`
red. They come from `repolish_core::band_index`, the single place those numbers exist.

`repolishVersion` and `mode` are non-standard fields; shields.io ignores them.

**What `mode` is for:** local and remote scores use different denominators (three remote
checks drop out), so the numbers are not comparable. Therefore:

- when `mode = "local"`, `label` degrades to `repolish (local)` so a reader can tell
- the workflow `repolish init` writes passes `--remote` by default, since `GITHUB_TOKEN`
  is free inside an Action

Under 50% coverage no `badge.json` is written, and exit code 5 says why.

### Badge snippet

`repolish badge` prints:

```markdown
[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/OWNER/REPO/BRANCH/.repolish/badge.json)](https://github.com/asale-ai/repolish)
```

OWNER / REPO / BRANCH are inferred from the git remote and the current branch; if
inference fails the user is told to fill them in.

### `--format json`

**Frozen at `schemaVersion: 1`.** Fields may be added but not changed; removing a field or
changing its meaning requires bumping `schemaVersion`:

```json
{
  "repolishVersion": "0.3.0",
  "schemaVersion": 1,
  "repository": { "owner": "...", "name": "...", "commit": "..." },
  "profile": { "detected": "cli", "overridden": false },
  "mode": "remote",
  "score": 88,
  "coverage": 0.774,
  "categories": [{ "category": "discoverability", "score": 96 }],
  "checks": [
    {
      "id": "readme-quickstart",
      "risk": "critical",
      "status": "scored",
      "score": 10,
      "evidence": [{ "file": "README.md", "line": 42, "note": "..." }],
      "fixes": [{ "severity": "P2", "message": "...", "autofixable": true }]
    },
    { "id": "tests-present", "status": "not_applicable", "profile": "docs" },
    { "id": "repo-topics", "status": "skipped", "reason": "requires --remote" }
  ],
  "coverageLimits": ["repo-topics: requires --remote"]
}
```

`status` is one of `scored` / `not_applicable` / `skipped` / `inconclusive`, matching
`Outcome` in [01-architecture](01-architecture.md).

`coverageLimits` is a top-level field holding both `skipped` and `inconclusive`, so a
consumer is forced to see what went unverified. `not_applicable` is not in that list.

### `REPOLISH.md`

Fixed structure:

1. Total score, detected profile, three category scores
2. P1 / P2 / P3 findings, each with **file and line** evidence
3. The list of checks that passed
4. **Coverage limits**: what could not be verified, and why
5. Footer: `Generated by repolish vX.Y.Z on <commit>`

### `.repolish/overview.svg`

**The overview card describes the project being checked, not our score.** Contents: the
project name and one line about it, languages by file count, the stacked split between
code / docs / config / other, a year of weekly commit activity, the licence, the latest
tag, and — only under `--remote` — stars and topic count.

It belongs at the **top** of the README, under the badges. The score card belongs at the
**end**. This was the wrong way round early on: when a stranger opens your repository, the
first thing they should see is what the project does, not what we scored it. The score
card's real audience is the author, and the next author who follows it here.

The activity window **ends at HEAD's commit time, not "now"**. Opening the window at the
current time would draw a dormant repository as a flat line of zeros — which reads as "no
data" rather than "abandoned" — and would break byte-for-byte reproducibility.

### `.repolish/card.svg`

The score card, embeddable with `<img>`. Same distribution model as `badge.json`: the file
lives in the user's own repository, served from their own raw URL, and we host nothing.
A badge has room for one number; a card has room for where the points went.

Top to bottom: brand mark and wordmark / profile · mode | large total, verdict word, three
category bars | a dot grid of the 22 checks | at most 3 findings | repository slug ·
commit · version.

### `.repolish/tables/*.svg`

One image per README table. `polish --tables svg` generates them and inserts the
references the first time; `card --kind tables` redraws them afterwards, and only for
tables `polish` has already wrapped — drawing one for an unwrapped table would leave an
orphan file that nothing points at.

**The original table is kept**, folded into a `<details>` immediately below the image. An
image has no text layer: screen readers, `grep`, translation tools and the next person to
edit that table all read the folded copy. This is not optional.

The wrapping is still **pure insertion** — the table's own bytes are untouched, with lines
added above and below — so `polish`'s insert-only invariant holds.

Selection: fewer than 2 rows is not drawn (a picture buys nothing), more than 16 is not
drawn and says so (an image that tall is unreadable on a phone, and a real table scrolls).
Filenames come from the **section slug**, never the document index: adding a table earlier
in the file must not rename the ones after it, or the references already written into the
README all break.

### `.repolish/demo.svg`

An **animated terminal recording**, driven by CSS keyframes. Only meaningful when an
executable is detected, or commands are given explicitly with `--cmd`.

**Recording and rendering both live in this repository.** The earlier approach generated a
[VHS](https://github.com/charmbracelet/vhs) tape for the user to render, but VHS needs
ttyd and ffmpeg and produces a GIF — and a GIF fails all three constraints this repository
holds its output to: a binary blob bloats the history, there is no text layer so the
command cannot be copied or grepped, and requiring a video toolchain to use a
"make your repository presentable" tool is backwards. `--tape` remains, because not every
package registry renders SVG (crates.io does; npm and PyPI sanitise more aggressively).

**It really runs the commands.** That is deliberate: a tool whose job is checking that a
README's promises are true has no business faking its own demo. The cost is that it
executes programs on the user's machine, which is why `--dry-run` exists and why the help
text says so.

Two hard limits, documented in `repolish-render/src/cast.rs`:

- **Not a terminal emulator.** SGR colours, `\n` and `\r`, and nothing else. Programs that
  redraw the screen — progress bars, spinners, full-screen TUIs — do not record correctly.
  Handling everything would mean writing a vt100, and this crate's job is drawing cards.
- **No pseudo-terminal.** Output goes through a pipe, so colour is forced with
  `CLICOLOR_FORCE` and `FORCE_COLOR`; a program that still insists on monochrome records
  in monochrome. A PTY dependency would rescue the last few, at the cost of a
  platform-specific dependency and a second code path on Windows — not worth it for a
  demo feature.

**Frame zero is the finished state.** The timeline opens holding the last step's completed
output before typing begins. That is the fallback for renderers that freeze an SVG at
frame zero — they exist, and if frame zero were an empty terminal those readers would get
a blank image. `prefers-reduced-motion` is a separate fallback for a separate audience;
both are needed.

**It does not run in per-push CI.** The recording embeds the sample repository's commit
hash. Pinning the sample's commit date to stabilise that hash turns it into a repository
nobody has touched in over a year, so `activity` fires a P1 unrelated to the demo, and the
report then prints `last commit N days ago` — which changes every day. It cannot be both
reproducible and time-independent, so it tracks content and is re-recorded from the `demo`
workflow. Full reasoning in `demo/README.md`.

### `SKILL.md`

The agent instructions written by `repolish skill`. The content is compiled into the
binary from `crates/repolish-cli/src/skill.md`; the copy committed here is
`skills/repolish/SKILL.md`, regenerated by script — edit the former.

Two destinations with different meanings:

- `repolish skill .` writes into **a repository** (`SKILL.md`), so it travels with the
  code and everyone who clones gets it.
- `repolish skill --target claude` writes into **an agent on this machine**
  (`~/.claude/skills/repolish/SKILL.md`), installed once and available in every project.
  `--target detect` only installs into agents that actually exist — writing
  `~/.codex/skills` on a machine without Codex would fabricate its presence.

The point of the file is not the command list (`--help` has that) but the **order and the
boundaries**: measure, apply what is mechanical, hand back what needs judgement. An agent
asked to "fix this repository's README" will otherwise rewrite the whole file, which is
precisely what this tool spends its effort opposing.

It also carries a section on **judgement**: which of repolish's three decision modes
(facts, cross-references, graded keyword heuristics) is the weak one, and what a good fix
looks like per finding along with the failure mode for each. The score measures whether
the machinery a reader needs is present and whether the promises are true — not whether
the writing is good. That gap is what the agent is there to close.

### The language of the SVG output

The terminal report and `REPOLISH.md` are always English: they are diagnostics for the
author, who is running an English CLI.

**The SVGs are different.** A card gets pasted into **someone else's README** and is read
by that project's readers. A card saying `LANGUAGES · BY FILE` on top of a Chinese README
is our language pushed into someone else's front door. Every string on a card therefore
goes through the table in `repolish-render/src/i18n.rs`, and `--lang` defaults to `auto` —
**judged from the README, not the system locale**: one CI run with `LANG=C` flipping a
Chinese README's card to English would be absurd.

Detection runs in two steps, and the order matters. **Kana first**: hiragana and katakana
appear only in Japanese, and no real Japanese README is without them, so their presence
settles Japanese versus Chinese — a question kanji alone cannot answer, since the two
scripts share them. Only then does the CJK share decide Chinese versus English.

That share is a third of the letters. Not "contains any CJK": a Chinese README with
English command names in it is the norm, and that test would call almost every README
Chinese. Not "more than half" either: a CJK character carries far more than a Latin
letter, so an equal comparison always answers English. The kana threshold is much lower
(5%), because a stray katakana name quoted in an English README should not flip it while
any real paragraph of Japanese clears it immediately.

The table is a **struct** rather than a lookup function: a missing field is a compile
error, so a missing translation cannot reach a release.

### Palettes

The SVGs have two complete palettes (`--theme dark` / `porcelain`); the terminal has one.
The difference is **who paints the background**: a terminal's background is not ours to
choose, so we pick foreground colours that hold up on both light and dark; an SVG paints
its own, so it can be held to real contrast — body text at WCAG AAA (7:1), muted text at
AA (4.5:1), both enforced by test.

`porcelain` exists for readability rather than taste: a dark card dropped into a
light-leaning README is a hole in the page.

**No `prefers-color-scheme` switching.** GitHub serves SVGs through the camo image proxy,
where media queries are not reliable — so choose the light palette explicitly and the file
itself is light.

### Terminal output

By default: the total, three category bars, a dot grid of the 22 checks, P1/P2 findings,
and coverage limits. `-v` expands every check and the passing list.

Colours share their source with the SVG cards (`theme` and `glyph` in `repolish-render`),
so what it looks like in the terminal is what it looks like in the README. The score band
colours and the badge colour flip at the same thresholds — one repository must not get two
answers.

Colour degrades with terminal capability, and every escape sequence goes through one `Pen`
so no site can forget to degrade and emit raw truecolor on a 16-colour terminal:

| Level | Detected by |
|---|---|
| Off | `--no-color`, `NO_COLOR`, `TERM=dumb`, or stdout is not a tty and neither `CLICOLOR_FORCE` nor `FORCE_COLOR` is set |
| truecolor | `COLORTERM` contains `truecolor` / `24bit`, or `WT_SESSION` / iTerm / VS Code |
| 256 | `TERM` contains `256` |
| 16 | Anything else with a `TERM` |

Not a tty means colour off by default, or output redirected to a file carries escape
sequences. Windows consoles do not interpret ANSI by default, so `main` enables VT mode
before the first write.

Category bars are cut into 12 segments in both the terminal and the cards, and **round
down**: a continuous bar carries no information at the top end, where 99 and 100 differ by
under four pixels. Segmented, that missing block is the point.

---

## Configuration: `.repolish.toml`

In the repository root, or given with `--config <path>`:

```toml
profile   = "library"      # overrides detection; same as --profile
min_score = 70             # same as --min-score

[checks]
only = []                  # when non-empty, only these ids run
skip = ["code-of-conduct"]

# Presentation of what polish inserts. Every key has a matching CLI flag; the flag wins.
[readme]
badge-style = "flat"       # flat | flat-square | plastic | for-the-badge | social
align       = "left"       # left | center
toc-style   = "bullet"     # bullet | number | roman | fold
logo        = "assets/hero.svg"
logo-width  = "full"       # a pixel count, or "full" → width="100%"
tree-depth  = 2            # omitted = no project tree
theme       = "dark"       # dark | porcelain
lang        = "auto"       # auto | en | zh-CN | ja
overview    = true         # insert the overview card under the badges
footer-card = true         # insert the score card at the end, under its own heading
tables      = "svg"        # keep | svg
```

The `[readme]` section **moves no score**. The check list and weights are frozen for v1; a
repository must not be able to look better by choosing a different badge style, or scores
stop being comparable.

Three implementation constraints, all found on real READMEs:

| Constraint | Why |
|---|---|
| The logo's `alt` must be empty | A non-empty alt makes the image a title candidate, and an image title drops `readme-title-tagline` from 10 to 5. Empty alt is also the correct accessibility semantics: there is a text title beside it, so the image is decorative |
| The logo block must end with a blank line | An image block is HTML, and Markdown immediately after it is absorbed into that block. Without the blank line the following `# Name` stops being a heading — measured at 10 → 6, with the first body section read as the project name |
| Appending to an existing badge row must use Markdown | The author wrote that row in Markdown; mixing in one line of HTML leaves a visible seam. Alignment only applies when starting a new block |
| A full-width banner needs `logo-width = "full"` | Pinned to a pixel width it huddles in the corner of a wide window and overflows a narrow one. `full` emits `width="100%"`, and the image needs a wide viewBox to match — a 450×56 wordmark stretched to 100% becomes one enormous line of type |

Unset, `badge-style` **follows the badges already in the README** (whichever style is most
common). One badge in a different style from the rest of the row looks worse than a row
that is uniformly not our default.

`logo`, `tree-depth`, `overview`, `footer-card` and `tables` are **not driven by any
check** — nothing asks for a banner or a diagram. They stay off unless requested, and
`polish`'s dry run says "requested by configuration" rather than dressing them up as
fixes. `--visuals` is the CLI shorthand for the last three.

Two deliberate restrictions:

- **Unknown keys are an error**, not silently ignored. Mistyping a key and having nothing
  happen is worse than an error: the user believes the configuration took effect.
- **Per-check thresholds are not exposed.** The check list and weights are frozen for v1
  (see [03-scoring](03-scoring.md)); letting each repository tune its own thresholds makes
  scores incomparable, which is the entire reason this tool exists.
- `--config` must point at a file that exists. Silently falling back to defaults would
  hand the user a score they cannot explain.

**Precedence: CLI > config file > default.** The CLI always wins, because in CI the
command line is the only thing that can be changed.

---

## install.sh

The one-line installer. It resolves the latest release, downloads the archive for the
detected platform, verifies the `.sha256` beside it, installs atomically into `~/.local/bin`
(write to a temp name and rename — replacing a running binary in place truncates it), and
then asks the binary itself to install the agent skill, so the script never needs to know
where any agent keeps its skills.

| Variable | Default | Effect |
|---|---|---|
| `REPOLISH_VERSION` | latest release | Install a specific tag, e.g. `v0.3.0` |
| `REPOLISH_BIN_DIR` | `~/.local/bin` | Where the binary goes |
| `REPOLISH_TARGET` | `detect` | Which agents get the skill: `detect`, `all`, `none`, or one id |
| `REPOLISH_NO_SKILL` | unset | Set to `1` for the binary only |

POSIX `sh`, because it has to run on Alpine, on minimal CI images, and on macOS's ancient
bash: no arrays, no `[[ ]]`, no process substitution.

The Linux builds are glibc-only, so the script detects musl and stops with a pointer to
`cargo install` rather than installing a binary that dies with a linker error on first
run. The archive name is a contract with `release.yml`: `repolish-{tag}-{target}.tar.gz`,
with the tag keeping its `v` prefix.

---

## GitHub Action

The composite action is defined in **`action.yml` at the repository root** (`uses:
owner/repo@ref` only looks there); `action/` holds usage examples. It downloads the
platform binary and runs it, an order of magnitude faster than a Docker action.

To avoid duplicate API calls the action runs `check` **once**, using `--badge --report` so
one run produces every artifact. Separate runs could produce artifacts from different
scores.

Two defaults in the generated workflow must not be changed: `fetch-depth: 0` (otherwise
`release-hygiene` can never decide in CI, since the default depth fetches no tags) and the
action's `--remote` (a `GITHUB_TOKEN` is free inside an Action, so there is no reason to
produce a narrower local score).

The action also writes the score into `$GITHUB_STEP_SUMMARY`, so every run page shows the
health card at the top.

Users who want an automatic pull request swap in `peter-evans/create-pull-request` to
carry the output of `polish --apply`.

---

## `polish`

Applies the suggestions that follow **mechanically**. Prints by default; `--apply` writes.

Two hard boundaries:

1. **Into a README, only insert; never rewrite anything already there.** The diff must be
   new lines and nothing else. Any other byte changing is a bug.
2. **New files are only created, never overwritten.** If the target path exists, skip it —
   even a one-line file is the author's.

This is not conservatism; it was measured. `comrak`'s `parse_document` →
`format_commonmark` round trip was **lossless on 0 of 12** real READMEs: reference-style
links flattened (serde's whole table of badge URLs disappeared), setext headings became
ATX, `*` list markers became `-`, tabs became spaces. ripgrep went 541 → 466 lines, axios
2851 → 2839. See `crates/repolish-md/examples/roundtrip.rs`.

So the implementation works at the text layer: the AST answers only "which line"
(`sourcepos`), and the original is split, spliced and rejoined, with line endings following
the anchor line. See `crates/repolish-md/examples/locate.rs` — on 15 real READMEs, 15/15
gained only the inserted lines.

### What it can apply today

| Change | Trigger |
|---|---|
| Insert the repolish badge | Not already there, the repository slug is known, and coverage is high enough for a badge |
| Insert a table of contents | `readme-toc` lost points and the shallowest heading level has 4 or more entries |
| Write issue forms (bug / feature) | `issue-pr-template` lost points and there is no issue template under `.github/` |
| Write `pull_request_template.md` | Same, and the file does not exist |
| Write `CONTRIBUTING.md` | `contributing` lost points, none exists in the root, `.github/` or `docs/`, **and a package ecosystem was detected** |

Every trigger **reads the check result**; no thresholds are written twice. "How long is
long" is defined by `readme-toc`, and restating it here would drift.

### What may be generated, and what may not

One rule: **the content must follow from facts already in the repository, with no
guessing.**

- Issue and PR templates are pure scaffolding. GitHub's own form schema asks for a version,
  reproduction steps and what changed — nothing project-specific, so there is nothing to
  guess.
- The build and test commands in `CONTRIBUTING.md` come from the **detected manifest**:
  Cargo gets `cargo build` / `cargo test`; npm gets `npm test` only if `package.json`
  really has that script. **No ecosystem detected means no file** — better to leave the
  check failing than to write `<your build command here>`, a file that turns the check
  green while the problem stays exactly where it was.
- **No code of conduct is generated.** The Contributor Covenant is standard text whose only
  project-specific part is the reporting address, and that cannot be derived. A code of
  conduct with a placeholder there promises a reporting channel that does not exist.

The dry run prints each new file **alongside the check result that asked for it**. A new
file with no stated reason has no business appearing in someone else's repository.

Under low coverage **the badge file is not written either** — inserting a link to a
nonexistent file is worse than inserting nothing.

### Where the badge goes

In order of confidence:

1. After the **existing** badge row within the first 40 lines (the row with the most
   badges; ties go to the earlier one)
2. If that row is an HTML block, a blank line must precede the insertion — Markdown
   immediately after an HTML block is absorbed into it and the badge is never parsed as an
   image (flask and fzf both hit this)
3. With no badge row, after the title, separated by a blank line
4. If even the title cannot be identified, **do nothing**

A "badge row" is a paragraph containing only images, **at least one of which is wrapped in
`<a>` or `[]()`**. Bare images are logos or screenshots — ripgrep's inline screenshot and
flask's opening logo are both image-only paragraphs, and attaching to them puts the badge
mid-prose or above the title. The test is the same one `title.rs` uses: a real logo is not
a hyperlink.

### Known imperfect placements

axios and awesome open with hundreds of lines of HTML (sponsor tables, centred heroes), so
the title node itself spans to line 421 / 77 and the badge lands after that block. The
position is legal and the document structure is intact, but it is far from the first
screen. Both are kept in `fetch-fixtures.sh` on purpose.

### Safety boundaries

- Nothing is written by default; `--apply` writes.
- `-v` prints the **full contents** of every file that would be created. Each inserted
  README line is visible already; reporting a whole new file as just a path is
  indefensible — anything landing in someone else's repository should be readable before
  it lands.
- After writing new files the hint is `git add -A && git diff --staged`, not `git diff` —
  untracked files do not appear in the latter, and following that advice would suggest
  `polish` only touched the README when it had just added four files.
- `--apply` is refused outside a git repository unless `--force` is given: without
  `git checkout` there is no undo button.
- Idempotent: an existing badge means do nothing, judged by the `.repolish/badge.json`
  path inside the URL rather than the whole snippet, since a different branch name should
  not read as a different badge.

### What goes in the table of contents

Entries come from the **shallowest heading level present in the body**, not a hardcoded
h2: ripgrep's title is a setext h2 and its body sections are all `###`, so taking h2 would
yield an empty contents list. The contents heading itself follows that level, or an extra
layer appears from nowhere and cuts the existing hierarchy.

Anchors follow GitHub's github-slugger, and the four steps cannot be reordered: trim
whitespace → lowercase → remove everything that is not alphanumeric, `-`, `_` or a space →
spaces become `-`. The anchor for `## 🚀 Install` is `-install`, not `install`; that leading
hyphen is real. Duplicates are numbered in **document order** (`-1`, `-2`), and counting
only the listed entries would misnumber them. See `crates/repolish-md/src/toc.rs`.

Every entry is a heading the author wrote — not a word of it is invented. That is why it
qualifies for `polish`, and why "write `cargo install <name>` under `## Install`" does not:
a manifest containing `name` does not mean that package was ever published, and asserting
it would be making a claim about the outside world on the author's behalf, which collides
with design principle 4.

The heading language follows the README (`## Contents` / `## 目录`). That text is written
into **someone else's** document, which is a different matter from repolish's own reports
always being English.
