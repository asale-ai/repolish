# Demo

`../.repolish/demo.svg` is the recording at the top of the README. It is produced by
repolish itself:

```bash
sample="$(bash demo/setup.sh)"
repolish "$sample" --stages demo --apply \
  --cmd "repolish" \
  --cmd "repolish --apply" \
  --cmd "repolish" \
  --output .repolish/demo.svg
```

## What it shows

A real before-and-after, not a staged one:

1. `repolish` against `sample/`, a deliberately rough repository — 23/100
2. `repolish --apply`
3. `repolish` again — 34/100

The scores are whatever that run actually produced. A tool whose whole job is checking
that a README's promises are true has no business faking its own demo.

## Why it is re-recorded by hand, not on every push

The **format** is no longer the reason. The old GIF pipeline was manual-only because a
GIF is a binary blob that gets replaced wholesale on every render; the SVG is text, it
diffs, and an unchanged recording produces no diff at all.

The **content** is the reason, and it does not go away. The recording contains the
sample repository's commit hash, which `setup.sh` regenerates every time it runs.

Pinning the commit date to make that hash stable was tried, and it is worse:

- A fixed date makes `sample/` a repository nobody has touched in over a year, so
  `activity` fires a P1 that has nothing to do with what the demo is showing. The score
  drops from 23 to 16 and credibility goes to zero.
- The report then prints `last commit N days ago` — and that number changes **every
  day**, which churns harder than the hash did.

So the recording cannot be both reproducible and time-independent. It tracks content
rather than pushes: re-record it from the **demo** workflow in the Actions tab, or
locally with the command above, when the output has actually changed.

`setup.sh` copies `sample/` to a temporary directory and gives it its own git remote
before anything runs. That copy matters: scored in place, git discovery would find this
repository's remote and the report would be titled `asale-ai/repolish`. A demo showing
the wrong repository name is worse than no demo.

`sample/` is excluded from the Cargo workspace — it is a fixture, not a crate.

## If you would rather have a GIF

`repolish --stages demo --apply --tape` writes a [VHS](https://github.com/charmbracelet/vhs) tape for
any project. Render it with `vhs`, which needs `ttyd` and `ffmpeg`.

That path exists for a real reason rather than nostalgia: **not every package registry
renders SVG.** crates.io does — it rewrites relative image paths to the repository and
serves SVG fine — but npm and PyPI sanitise README HTML more aggressively. If your
package page is the front door, check it before committing to an SVG.

This repository no longer keeps a checked-in tape of its own. It used to, and nothing
rendered it: a script nobody runs, listing commands that drift out of date, is precisely
the rot `claim-consistency` exists to catch. Generate a fresh one when you want it.
