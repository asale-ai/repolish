# sakura

**Soft rose paper** · Light

[English](README.md) · [中文](README.zh-CN.md) · [All palettes](../README.md)

<img src="overview.svg" alt="The repolish overview card in the sakura palette" width="880">

```bash
repolish --apply --theme sakura
```

```toml
# .repolish.toml
[readme]
theme = "sakura"
```

`--theme` and `.repolish.toml` also accept `pastel`.

## Why this one

The gentlest of the light palettes: rose, lavender and sage, all held below full brightness so nothing glares on pink-white paper. Design tools, content projects, repositories whose readers are not all engineers — their READMEs usually run at this temperature already.

Body text sits at **13.7:1** against the background and secondary text at
**5.9:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the sakura palette" width="880">

## Every colour

| Role | Hex | Contrast on the background |
|---|---|---|
| Background | `#fff6f8` | — |
| Panel | `#ffe9ee` | 1.1:1 |
| Body text | `#3a2130` | 13.7:1 |
| Secondary text | `#7d5566` | 5.9:1 |
| Rules | `#f3d3dc` | 1.3:1 |
| Bar track | `#f7dde4` | 1.2:1 |
| Warning | `#a86a12` | 4.2:1 |
| Failure | `#c03952` | 5.0:1 |
| Brand 1 | `#b3436e` | 5.0:1 |
| Brand 2 | `#8a6bb0` | 4.1:1 |
| Brand 3 | `#4f8a72` | 3.8:1 |
| Series 1 | `#b3436e` | 5.0:1 |
| Series 2 | `#8a6bb0` | 4.1:1 |
| Series 3 | `#4f8a72` | 3.8:1 |
| Series 4 | `#b5793a` | 3.4:1 |
| Series 5 | `#6b7fa8` | 3.8:1 |
| Band 1 (excellent) | `#2f7f6c` | 4.5:1 |
| Band 2 (good) | `#4f8a3a` | 3.9:1 |
| Band 3 (fair) | `#a86a12` | 4.2:1 |
| Band 4 (weak) | `#bd5c22` | 4.2:1 |
| Band 5 (poor) | `#c03952` | 5.0:1 |

---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
