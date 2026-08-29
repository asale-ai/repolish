# okabe

**Okabe–Ito on pure black** · Dark

[English](README.md) · [中文](README.zh-CN.md) · [All palettes](../README.md)

<img src="overview.svg" alt="The repolish overview card in the okabe palette" width="880">

```bash
repolish --apply --theme okabe
```

```toml
# .repolish.toml
[readme]
theme = "okabe"
```

`--theme` and `.repolish.toml` also accept `okabe-ito`, `colorblind`.

## Why this one

The one palette that solves a problem before it has a taste. The five series colours come from the Okabe–Ito eight-colour set, which stays distinguishable with red-green colour blindness — where the default's pink and cyan converge. Pure black puts body text at 21:1.

Body text sits at **21.0:1** against the background and secondary text at
**8.6:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the okabe palette" width="880">

## Every colour

| Role | Hex | Contrast on the background |
|---|---|---|
| Background | `#000000` | — |
| Panel | `#101010` | 1.1:1 |
| Body text | `#ffffff` | 21.0:1 |
| Secondary text | `#a6a6a6` | 8.6:1 |
| Rules | `#2e2e2e` | 1.5:1 |
| Bar track | `#3d3d3d` | 1.9:1 |
| Warning | `#e69f00` | 9.3:1 |
| Failure | `#d55e00` | 5.4:1 |
| Brand 1 | `#56b4e9` | 9.1:1 |
| Brand 2 | `#cc79a7` | 6.9:1 |
| Brand 3 | `#009e73` | 6.1:1 |
| Series 1 | `#56b4e9` | 9.1:1 |
| Series 2 | `#e69f00` | 9.3:1 |
| Series 3 | `#009e73` | 6.1:1 |
| Series 4 | `#cc79a7` | 6.9:1 |
| Series 5 | `#f0e442` | 15.9:1 |
| Band 1 (excellent) | `#56b4e9` | 9.1:1 |
| Band 2 (good) | `#009e73` | 6.1:1 |
| Band 3 (fair) | `#f0e442` | 15.9:1 |
| Band 4 (weak) | `#e69f00` | 9.3:1 |
| Band 5 (poor) | `#d55e00` | 5.4:1 |

---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
