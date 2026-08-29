# carbon

**White on black. No hue anywhere, and no gradient** · Dark

[English](README.md) · [中文](README.zh-CN.md) · [All palettes](../README.md)

<img src="overview.svg" alt="The repolish overview card in the carbon palette" width="880">

```bash
repolish --apply --theme carbon
```

```toml
# .repolish.toml
[readme]
theme = "carbon"
```

`--theme` and `.repolish.toml` also accept `mono`, `bw`.

## Why this one

`phosphor` is already single-hue, but it is single-hue *green* — it still has a taste. This one has none: five score bands are five brightnesses, and the three stops of the brand gradient hold the same value, so the wordmark is a flat block rather than a sweep. For repositories whose card should have no opinion, and for any card that ends up on paper.

Body text sits at **17.1:1** against the background and secondary text at
**7.8:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the carbon palette" width="880">

## Every colour

| Role | Hex | Contrast on the background |
|---|---|---|
| Background | `#000000` | — |
| Panel | `#121212` | 1.1:1 |
| Body text | `#e8e8e8` | 17.1:1 |
| Secondary text | `#9e9e9e` | 7.8:1 |
| Rules | `#2e2e2e` | 1.5:1 |
| Bar track | `#3d3d3d` | 1.9:1 |
| Warning | `#bdbdbd` | 11.2:1 |
| Failure | `#ffffff` | 21.0:1 |
| Brand 1 | `#e8e8e8` | 17.1:1 |
| Brand 2 | `#e8e8e8` | 17.1:1 |
| Brand 3 | `#e8e8e8` | 17.1:1 |
| Series 1 | `#ffffff` | 21.0:1 |
| Series 2 | `#d4d4d4` | 14.2:1 |
| Series 3 | `#a8a8a8` | 8.8:1 |
| Series 4 | `#808080` | 5.3:1 |
| Series 5 | `#5c5c5c` | 3.1:1 |
| Band 1 (excellent) | `#ffffff` | 21.0:1 |
| Band 2 (good) | `#d4d4d4` | 14.2:1 |
| Band 3 (fair) | `#a8a8a8` | 8.8:1 |
| Band 4 (weak) | `#808080` | 5.3:1 |
| Band 5 (poor) | `#5c5c5c` | 3.1:1 |

---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
