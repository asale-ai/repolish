# dark

**Neon on near-black — the default, and the same palette as the terminal report** · Dark

[English](README.md) · [中文](README.zh-CN.md) · [All palettes](../README.md)

<img src="overview.svg" alt="The repolish overview card in the dark palette" width="880">

```bash
repolish --apply --theme dark
```

```toml
# .repolish.toml
[readme]
theme = "dark"
```

`--theme` and `.repolish.toml` also accept `neon`.

## Why this one

The card and the terminal say the same thing in the same colours. Pick another palette when the card has to sit inside someone else's page; pick this one when the card is the page.

Body text sits at **14.7:1** against the background and secondary text at
**5.0:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the dark palette" width="880">

## Every colour

| Role | Hex | Contrast on the background |
|---|---|---|
| Background | `#1a1a24` | — |
| Panel | `#222130` | 1.1:1 |
| Body text | `#edebfa` | 14.7:1 |
| Secondary text | `#8b87a3` | 5.0:1 |
| Rules | `#35334a` | 1.4:1 |
| Bar track | `#3e3b58` | 1.6:1 |
| Warning | `#ffc53d` | 10.9:1 |
| Failure | `#ff4f6e` | 5.4:1 |
| Brand 1 | `#7d56f4` | 3.7:1 |
| Brand 2 | `#ff5fd1` | 6.4:1 |
| Brand 3 | `#43e5d0` | 11.0:1 |
| Series 1 | `#7d56f4` | 3.7:1 |
| Series 2 | `#ff5fd1` | 6.4:1 |
| Series 3 | `#43e5d0` | 11.0:1 |
| Series 4 | `#a9f05f` | 12.6:1 |
| Series 5 | `#ffc53d` | 10.9:1 |
| Band 1 (excellent) | `#43e5d0` | 11.0:1 |
| Band 2 (good) | `#a9f05f` | 12.6:1 |
| Band 3 (fair) | `#ffc53d` | 10.9:1 |
| Band 4 (weak) | `#ff8f5f` | 7.7:1 |
| Band 5 (poor) | `#ff4f6e` | 5.4:1 |

---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
