# repolish GitHub Action

The action definition itself lives at [`action.yml`](../action.yml) in the repository
root — `uses: owner/repo@ref` only resolves a root-level `action.yml`. This directory
holds usage examples.

Generate a ready-to-commit workflow with `repolish --stages ci --apply`, or copy one of these.

## Minimal

Scores the repository on every push and writes `.repolish/badge.json`.

```yaml
name: repolish
on:
  push:
    branches: [main]

permissions:
  contents: write

jobs:
  score:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0        # required: the default depth of 1 fetches no tags
      - uses: asale-ai/repolish@v0.4.2
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## As a gate

Fails the job below the threshold. Exit code 1 means "score too low"; 4 means the GitHub
API call failed — deliberately different, so a rate limit does not read as a quality
regression.

```yaml
      - uses: asale-ai/repolish@v0.4.2
        with:
          min-score: 70
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## On a pull request

The three inputs that make a review readable. An absolute score tells a reviewer nothing;
*this pull request dropped it four points, because the link on line 42 stopped resolving*
tells them what to do.

```yaml
name: repolish
on: pull_request

permissions:
  contents: read
  pull-requests: write      # for `comment: true`
  security-events: write    # for uploading the SARIF

jobs:
  score:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0    # required: the baseline commit has to exist locally

      - uses: asale-ai/repolish@v0.4.2
        with:
          base: ${{ github.event.pull_request.base.sha }}
          sarif: repolish.sarif
          comment: true
          min-score: 70
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      # Puts every finding on its own line in the diff. `if: always()` so the
      # annotations still appear on the run that failed the gate — that is the
      # run where they matter most.
      - uses: github/codeql-action/upload-sarif@v3
        if: always()
        with:
          sarif_file: repolish.sarif
```

Each of the three needs something from you:

- **`base`** needs `fetch-depth: 0`. The baseline is checked out into a temporary git
  worktree and scored with the identical options; on a shallow clone that commit was never
  fetched, and repolish says so rather than reporting a mystery. Your working tree is never
  touched.
- **`sarif`** needs `security-events: write` on the job, and the upload step above.
- **`comment`** needs `pull-requests: write`. It edits the previous repolish comment in
  place instead of adding another — a bot that posts a fresh comment on every push gets
  collapsed by the third one, taking the run that actually went red down with it. On
  anything other than a `pull_request` event it does nothing and says so.

`outputs.points` carries the difference, e.g. `-4`, for a step that wants to act on the
direction rather than the absolute number. It is empty without `base`, and empty when
either side produced no total score.

## Commit the badge back

The badge is served from your own repository — shields.io reads
`.repolish/badge.json` over `raw.githubusercontent.com`. Nothing is hosted by us, which
also means the file has to be committed for the badge to update. The same applies to
`.repolish/card.svg` when `card: true` is set, so the step below stages the whole
directory rather than naming files one by one.

```yaml
      - uses: asale-ai/repolish@v0.4.2
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Commit badge
        run: |
          git config user.name  github-actions
          git config user.email github-actions@github.com
          git add .repolish
          git diff --staged --quiet || git commit -m "chore: update repolish score"
          git push
```

## Use the score in later steps

```yaml
      - uses: asale-ai/repolish@v0.4.2
        id: repolish
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - run: echo "scored ${{ steps.repolish.outputs.score }}"
```

`outputs.score` is empty when coverage was too low to produce a total — treat empty as
"unknown", not as zero.

## Inputs

| Input | Default | Notes |
|---|---|---|
| `version` | pinned | repolish release to download, without the `v` |
| `path` | `.` | repository to score |
| `remote` | `true` | read description / topics / homepage from the GitHub API |
| `min-score` | *(none)* | fail below this score |
| `badge` | `true` | write `.repolish/badge.json` |
| `card` | `false` | write `.repolish/card.svg`, a self-contained report card to embed in the README |
| `report` | `false` | write `REPOLISH.md` |
| `overview` | `false` | write `.repolish/overview.svg`, the project overview card |
| `theme` / `lang` | `dark` / `auto` | palette and language for the SVG cards |
| `summary` | `true` | append the report to the job summary |
| `base` | *(none)* | also score this ref and report the difference; needs `fetch-depth: 0` |
| `sarif` | *(none)* | write SARIF to this path; needs `security-events: write` to upload |
| `comment` | `false` | post/update the short report as a PR comment; needs `pull-requests: write` |
| `args` | *(none)* | raw arguments; a full escape hatch — see below |

## Outputs

| Output | Notes |
|---|---|
| `score` | 0–100. **Empty** when coverage was too low to produce a total — treat empty as "unknown", never as zero |
| `points` | Change against `base`, e.g. `-4`. Empty without `base`, or when either side produced no total |

## `args` takes over completely

When `args` is set it replaces the entire command line, and the action stops managing
artifacts: no badge, no report, nothing appended to the job summary. It cannot know what
your arguments produced, and guessing would mean appending a stale `REPOLISH.md` that was
committed months ago. `outputs.score` still works, because it is read from the JSON the
run actually printed.

**There are no subcommands**, so `args` starts with the path — `args: ". --stages check
--format json"`, not `args: "check . --format json"`. And repolish writes nothing without
`--apply`, so include it if your arguments are meant to produce files.

## `REPOLISH.md` is left as it was

With `report: false` (the default) and `summary: true`, the action still has to generate
the report to put it in the job summary. If your repository already has a committed
`REPOLISH.md`, it is backed up first and restored afterwards, so the working tree is
exactly as it started. Without that, the next `git add` in your workflow would commit a
deletion nobody asked for.

## Why `fetch-depth: 0`

`actions/checkout` fetches a single commit by default, which brings down no tags. Without
tags the `release-hygiene` check cannot tell "this project has never tagged a release"
from "the tags were simply not fetched", so it reports *inconclusive* rather than
guessing — and you lose the check. Fetching the full history costs a few seconds and
gets it back.

`base` needs it for a second reason: the baseline commit itself is not in a shallow clone,
so there is nothing to check out and compare against.

## Pinning

The examples pin an exact release. From the next one onwards a floating `@v0` will also
exist, moved by the release workflow on every non-prerelease tag — the usual Actions
convention, and the only way a patch release ever reaches the people who copied a snippet
a year ago. Keep the exact pin if you would rather review every upgrade yourself.
