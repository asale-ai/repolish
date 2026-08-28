# Demo

`demo.gif` is rendered from `demo.tape` with [VHS](https://github.com/charmbracelet/vhs).

## Rendering it

```bash
vhs demo/demo.tape
```

VHS needs `ttyd` and `ffmpeg`, which rules out Windows. If you are not on Linux or
macOS, run the **demo** workflow from the Actions tab instead — it renders on an Ubuntu
runner and commits the result. It is manual-only on purpose: a GIF is a binary blob, and
re-rendering it on every push would do nothing but inflate the history.

## What it shows

A real before-and-after, not a staged one:

1. `repolish check .` against `sample/`, a deliberately rough repository — 21/100
2. `repolish polish . --apply`
3. `repolish check .` again

`setup.sh` copies `sample/` to a temporary directory and gives it its own git remote
before anything runs. That copy matters: scored in place, git discovery would find this
repository's remote and the report would be titled `asale-ai/repolish`. A demo showing
the wrong repository name is worse than no demo.

`sample/` is excluded from the Cargo workspace — it is a fixture, not a crate.
