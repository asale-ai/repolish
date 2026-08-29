# paper

**Black on white. Carbon inverted** · Light

[English](README.md) · [中文](README.zh-CN.md) · [All palettes](../README.md)

<img src="overview.svg" alt="The repolish overview card in the paper palette" width="880">

```bash
repolish --apply --theme paper
```

```toml
# .repolish.toml
[readme]
theme = "paper"
```

`--theme` and `.repolish.toml` also accept `print`.

## Why this one

Fax, photocopier, black-and-white print, greyscale e-ink — every colour palette collapses into a heap of indistinguishable greys there. This one was drawn in greys to begin with: the score bands run from pure black down to 3:1, and the card looks the same before and after the colour is taken out of it.

Body text sits at **21.0:1** against the background and secondary text at
**7.0:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the paper palette" width="880">

## Every colour

| Role | Hex | Contrast on the background |
|---|---|---|
| Background | `#ffffff` | — |
| Panel | `#f2f2f2` | 1.1:1 |
| Body text | `#000000` | 21.0:1 |
| Secondary text | `#595959` | 7.0:1 |
| Rules | `#d4d4d4` | 1.5:1 |
| Bar track | `#e6e6e6` | 1.2:1 |
| Warning | `#5c5c5c` | 6.7:1 |
| Failure | `#000000` | 21.0:1 |
| Brand 1 | `#000000` | 21.0:1 |
| Brand 2 | `#000000` | 21.0:1 |
| Brand 3 | `#000000` | 21.0:1 |
| Series 1 | `#000000` | 21.0:1 |
| Series 2 | `#2e2e2e` | 13.6:1 |
| Series 3 | `#545454` | 7.6:1 |
| Series 4 | `#757575` | 4.6:1 |
| Series 5 | `#8f8f8f` | 3.2:1 |
| Band 1 (excellent) | `#000000` | 21.0:1 |
| Band 2 (good) | `#2e2e2e` | 13.6:1 |
| Band 3 (fair) | `#545454` | 7.6:1 |
| Band 4 (weak) | `#757575` | 4.6:1 |
| Band 5 (poor) | `#949494` | 3.0:1 |

---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
