# repolish design notes

A repository diagnosis and improvement CLI for open-source authors.

[English](README.md) · [中文](README.zh-CN.md)

| Document | Contents |
|---|---|
| [01-architecture.md](01-architecture.md) | Rust workspace layout, crate choices, the three core designs |
| [02-cli-design.md](02-cli-design.md) | Command surface, exit codes, output contracts (terminal colours and SVG cards), Action template |
| [03-scoring.md](03-scoring.md) | The check list, weights, aggregation rules, and profile applicability |
| [04-usage.md](04-usage.md) | Usage reference: polish styling, what `--suggest` may do, the pull-request flags |
| [themes/](themes/README.md) | Every `--theme` palette, rendered on this repository's own card |

`01`–`03` are the **external contract**: `03` defines how the score is computed, `02`
defines the output shape and exit codes, and `01` explains why the trade-offs are what
they are. Changing them changes what users are allowed to depend on. `04` is the usage
reference the README links out to, and grows with the CLI rather than being frozen.

## Status

v0.3.0 is published to GitHub Releases. 22 checks are frozen, and the JSON schema is
frozen at `schemaVersion: 1`.

- Language: **Rust** (MSRV 1.88)
- Shape: **CLI only** plus a GitHub Action. No hosted service.
- Scoring: purely deterministic and offline-first. The same commit run twice produces
  byte-identical output.
- Badge and cards: shields.io reads `.repolish/badge.json` out of **your** repository, and
  the `<img>` tags in your README point at `overview.svg` (top of the page, about your
  project) and `card.svg` (end of the page, about our score). We host nothing.

Everything the tool emits is in English. These design notes exist in both languages, the
English file being the one to edit first — see rule 3 in
[CONTRIBUTING.md](../CONTRIBUTING.md).
