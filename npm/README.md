# repolish

Score and improve what an open-source repository looks like to a first-time
visitor — from the command line.

```bash
npx repolish check .
```

This package is a thin launcher. On install it downloads the release binary for
your platform from [the GitHub releases][releases], verifies its `.sha256`, and
runs it. There is no JavaScript implementation of the checks — the tool itself
is a single static Rust binary.

If you already have a Rust toolchain, `cargo install repolish` gets you the same
binary without this wrapper.

Linux builds are glibc-only. On musl (Alpine and similar) the installer says so
and stops rather than leaving behind a binary that cannot run; use
`cargo install repolish` there.

Everything else — what it checks, how scoring works, the GitHub Action — is in
[the main README](https://github.com/asale-ai/repolish#readme).

[releases]: https://github.com/asale-ai/repolish/releases
