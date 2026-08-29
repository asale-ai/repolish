# nord

**Nordic and desaturated** · Dark

[English](README.md) · [中文](README.zh-CN.md) · [All palettes](../README.md)

<img src="overview.svg" alt="The repolish overview card in the nord palette" width="880">

```bash
repolish --apply --theme nord
```

```toml
# .repolish.toml
[readme]
theme = "nord"
```

## Why this one

Every hue is pulled back toward grey. Documentation sites, infrastructure, libraries — projects whose card should be a supporting actor, not the lead.

Body text sits at **10.8:1** against the background and secondary text at
**6.3:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the nord palette" width="880">

## Every colour

| Role | Hex | Contrast on the background |
|---|---|---|
| Background | `#2e3440` | — |
| Panel | `#3b4252` | 1.2:1 |
| Body text | `#eceff4` | 10.8:1 |
| Secondary text | `#b0b8c6` | 6.3:1 |
| Rules | `#434c5e` | 1.4:1 |
| Bar track | `#4c566a` | 1.7:1 |
| Warning | `#ebcb8b` | 8.0:1 |
| Failure | `#bf616a` | 3.1:1 |
| Brand 1 | `#5e81ac` | 3.1:1 |
| Brand 2 | `#88c0d0` | 6.2:1 |
| Brand 3 | `#a3be8c` | 6.1:1 |
| Series 1 | `#88c0d0` | 6.2:1 |
| Series 2 | `#81a1c1` | 4.6:1 |
| Series 3 | `#a3be8c` | 6.1:1 |
| Series 4 | `#ebcb8b` | 8.0:1 |
| Series 5 | `#b48ead` | 4.4:1 |
| Band 1 (excellent) | `#8fbcbb` | 6.0:1 |
| Band 2 (good) | `#a3be8c` | 6.1:1 |
| Band 3 (fair) | `#ebcb8b` | 8.0:1 |
| Band 4 (weak) | `#d08770` | 4.4:1 |
| Band 5 (poor) | `#bf616a` | 3.1:1 |

---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
