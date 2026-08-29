# 04 · Usage reference

[English](04-usage.md) · [中文](04-usage.zh-CN.md)

The README covers what repolish is and how to start. This is the rest: every flag that
changes what the `polish` stage inserts, what `--suggest` is and is not allowed to do,
and how the pull-request flags behave.

## Styling what the polish stage inserts

These change how the insertions look. Like every run, they report and write nothing until
you add `--apply`:

```bash
npx @asale/repolish --badge-style for-the-badge --toc-style fold --align center
npx @asale/repolish --logo assets/hero.svg --logo-width full --tree-depth 2 --apply
npx @asale/repolish --visuals --apply  # --overview --footer-card --tables svg
```

Also settable under `[readme]` in `.repolish.toml`
([every key](02-cli-design.md)). **None of it moves a score** — the check list and weights
are frozen at v1, so a repository cannot look better by picking a different badge style.

The logo, the file tree and the cards are driven by no check at all: they never appear
unless you ask for them, and a dry run says so — *requested by configuration*, rather
than dressing them up as a fix.

## What `--suggest` may and may not do

```bash
npx @asale/repolish --suggest  # needs REPOLISH_LLM_API_KEY
```

**No model in the scoring path** is a rule about *scoring*, and it has not moved — score
before and after, the number is identical. Extending it to *fixing* was the mistake: it
left the `polish` stage inserting badges while the author was stuck on the sentence under
the title.

The boundary sits elsewhere, and is stricter. It **never writes**, not even with
`--apply`. It **only fills gaps**, never rewriting what is there. And it **cannot
invent**: given the repository's real manifest, it is told to leave a suggestion empty
rather than make one up — a fabricated install command being exactly what
`claim-consistency` was built to catch. [Why each](02-cli-design.md).

The key comes from `REPOLISH_LLM_API_KEY` or `ANTHROPIC_API_KEY`. Nothing else in
repolish talks to a model.

## On a pull request, the change is the story

An absolute score tells a reviewer nothing. *This pull request dropped it four points,
because the link on line 42 stopped resolving* tells them what to do.

```bash
npx @asale/repolish --stages check --base origin/main
npx @asale/repolish --stages check --sarif repolish.sarif  # one annotation per finding, on its line
npx @asale/repolish --stages check --comment comment.md    # the short form, for a PR comment
```

`--base` checks the baseline out into a **temporary git worktree** and scores it with the
identical options — your working tree is never touched, a local score is never compared
against a remote one, and the report lists only the checks that moved.

Every deduction has carried a file and a line since the first release; SARIF is what puts
them **in the diff** instead of in a log nobody expands. The action wires all three
together:

```yaml
- uses: asale-ai/repolish@v0.4.3
  with:
    min-score: 70
    base: ${{ github.event.pull_request.base.sha }}
    sarif: repolish.sarif
    comment: true
```

The comment is **rewritten in place on every push**, not appended — a bot that posts a
fresh comment each time gets collapsed by everyone after the third, burying the one run
that actually went red along with it. `npx @asale/repolish --stages ci --apply` writes this workflow
for you. The permissions it needs, the SARIF upload step, and the `fetch-depth: 0` the
baseline requires are in [action/README.md](../action/README.md).
