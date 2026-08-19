# repolish GitHub Action

The action definition itself lives at [`action.yml`](../action.yml) in the repository
root — `uses: owner/repo@ref` only resolves a root-level `action.yml`. This directory
holds usage examples.

Generate a ready-to-commit workflow with `repolish init`, or copy one of these.

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
      - uses: asale-ai/repolish@v0.1.0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## As a gate

Fails the job below the threshold. Exit code 1 means "score too low"; 4 means the GitHub
API call failed — deliberately different, so a rate limit does not read as a quality
regression.

```yaml
      - uses: asale-ai/repolish@v0.1.0
        with:
          min-score: 70
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## Commit the badge back

The badge is served from your own repository — shields.io reads
`.repolish/badge.json` over `raw.githubusercontent.com`. Nothing is hosted by us, which
also means the file has to be committed for the badge to update.

```yaml
      - uses: asale-ai/repolish@v0.1.0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Commit badge
        run: |
          git config user.name  github-actions
          git config user.email github-actions@github.com
          git add .repolish/badge.json
          git diff --staged --quiet || git commit -m "chore: update repolish score"
          git push
```

## Use the score in later steps

```yaml
      - uses: asale-ai/repolish@v0.1.0
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
| `report` | `false` | write `REPOLISH.md` |
| `summary` | `true` | append the report to the job summary |
| `args` | *(none)* | raw arguments; a full escape hatch — see below |

## `args` takes over completely

When `args` is set it replaces the entire command line, and the action stops managing
artifacts: no badge, no report, nothing appended to the job summary. It cannot know what
your arguments produced, and guessing would mean appending a stale `REPOLISH.md` that was
committed months ago. `outputs.score` still works, because it is read from the JSON the
run actually printed.

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
