# glacier

**Light and cold, where porcelain is light and warm** · Light

[English](README.md) · [中文](README.zh-CN.md) · [All palettes](../README.md)

<img src="overview.svg" alt="The repolish overview card in the glacier palette" width="880">

```bash
repolish --apply --theme glacier
```

```toml
# .repolish.toml
[readme]
theme = "glacier"
```

`--theme` and `.repolish.toml` also accept `ice`.

## Why this one

`porcelain` goes with cream, wood and serifs; this one goes with blue. With only one light palette, half of all READMEs have to make do — which is why there are two.

Body text sits at **15.8:1** against the background and secondary text at
**5.3:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the glacier palette" width="880">

## Every colour

| Role | Hex | Contrast on the background |
|---|---|---|
| Background | `#f6f9fc` | — |
| Panel | `#e9eff6` | 1.1:1 |
| Body text | `#0f1f2e` | 15.8:1 |
| Secondary text | `#546a80` | 5.3:1 |
| Rules | `#d3dee9` | 1.3:1 |
| Bar track | `#dde6ef` | 1.2:1 |
| Warning | `#8a6a1f` | 4.8:1 |
| Failure | `#b03a34` | 5.7:1 |
| Brand 1 | `#0f6fbd` | 4.9:1 |
| Brand 2 | `#1b9aaa` | 3.2:1 |
| Brand 3 | `#3f9f7a` | 3.1:1 |
| Series 1 | `#0f6fbd` | 4.9:1 |
| Series 2 | `#0d8a8a` | 4.0:1 |
| Series 3 | `#5c5fbd` | 5.2:1 |
| Series 4 | `#8a6a1f` | 4.8:1 |
| Series 5 | `#a8443f` | 5.6:1 |
| Band 1 (excellent) | `#0d7d7d` | 4.7:1 |
| Band 2 (good) | `#2f7a3a` | 5.0:1 |
| Band 3 (fair) | `#8a6a1f` | 4.8:1 |
| Band 4 (weak) | `#b0561c` | 4.7:1 |
| Band 5 (poor) | `#b03a34` | 5.7:1 |

---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
