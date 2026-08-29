# slate

**GitHub's own dark blue-grey** · Dark

[English](README.md) · [中文](README.zh-CN.md) · [All palettes](../README.md)

<img src="overview.svg" alt="The repolish overview card in the slate palette" width="880">

```bash
repolish --apply --theme slate
```

```toml
# .repolish.toml
[readme]
theme = "slate"
```

`--theme` and `.repolish.toml` also accept `github`.

## Why this one

The safest choice on GitHub. The background is the one the page around it already uses, so the card reads as part of the page rather than as an image someone pasted in. It has no opinion — which is what most repositories want.

Body text sits at **16.0:1** against the background and secondary text at
**6.5:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the slate palette" width="880">

## Every colour

| Role | Hex | Contrast on the background |
|---|---|---|
| Background | `#0d1117` | — |
| Panel | `#161b22` | 1.1:1 |
| Body text | `#e6edf3` | 16.0:1 |
| Secondary text | `#9198a1` | 6.5:1 |
| Rules | `#30363d` | 1.6:1 |
| Bar track | `#21262d` | 1.2:1 |
| Warning | `#d29922` | 7.5:1 |
| Failure | `#f85149` | 5.6:1 |
| Brand 1 | `#58a6ff` | 7.5:1 |
| Brand 2 | `#bc8cff` | 7.5:1 |
| Brand 3 | `#3fb950` | 7.4:1 |
| Series 1 | `#58a6ff` | 7.5:1 |
| Series 2 | `#bc8cff` | 7.5:1 |
| Series 3 | `#3fb950` | 7.4:1 |
| Series 4 | `#d29922` | 7.5:1 |
| Series 5 | `#ff7b72` | 7.5:1 |
| Band 1 (excellent) | `#3fb950` | 7.4:1 |
| Band 2 (good) | `#56d364` | 9.8:1 |
| Band 3 (fair) | `#d29922` | 7.5:1 |
| Band 4 (weak) | `#db6d28` | 5.6:1 |
| Band 5 (poor) | `#f85149` | 5.6:1 |

---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
