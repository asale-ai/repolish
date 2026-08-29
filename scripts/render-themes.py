#!/usr/bin/env python3
"""Regenerate docs/themes: one page per palette, with real cards.

The hex values are read out of crates/repolish-render/src/theme.rs rather than
repeated here — the palette has exactly one source of truth, and a docs page
that disagrees with it is worse than no docs page. The cards are rendered by
the built binary, not mocked up: what the page shows is what `--theme <name>`
actually writes.

    cargo build && python3 scripts/render-themes.py

Set GITHUB_TOKEN (or run with --no-remote) to include the star history curve.
"""
import os, re, shutil, subprocess, sys, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
THEME_RS = os.path.join(ROOT, "crates/repolish-render/src/theme.rs")
OUT = os.path.join(ROOT, "docs/themes")
BIN = os.path.join(ROOT, "target/debug/repolish")

# ── prose. The only thing here that is not derived from the code ───────────
# (key, aliases, kind, one-line EN, one-line ZH, why EN, why ZH)
THEMES = [
 ("dark", ["neon"], "dark",
  "Neon on near-black — the default, and the same palette as the terminal report",
  "近黑底上的霓虹——默认色板，与终端报告同源",
  "The card and the terminal say the same thing in the same colours. Pick another "
  "palette when the card has to sit inside someone else's page; pick this one when "
  "the card is the page.",
  "卡片和终端里的颜色是同一组常量，说的是同一件事。当卡片要贴进别人的版面时该换一套；"
  "当卡片本身就是主角时，用这一套。"),
 ("porcelain", ["light", "cream"], "light",
  "Warm paper, dark ink",
  "暖白纸底，深墨字",
  "It exists for readability, not taste: a dark card dropped into a light README is "
  "a hole in the page. The series colours are a lightness ramp rather than five "
  "hues — neon smears together on paper.",
  "存在的理由是可读性而不是口味：一张深色卡片贴进浅色 README，在页面上就是一块挖空。"
  "序列色是一条明度阶而不是五个色相——霓虹色在浅底上会糊成一片。"),
 ("slate", ["github"], "dark",
  "GitHub's own dark blue-grey",
  "GitHub 深色主题同源的蓝灰",
  "The safest choice on GitHub. The background is the one the page around it "
  "already uses, so the card reads as part of the page rather than as an image "
  "someone pasted in. It has no opinion — which is what most repositories want.",
  "GitHub 上最保险的一张。底色与它周围的页面同源，卡片看上去是页面的一部分，"
  "而不是谁贴进来的一张图。它没有主张——大多数仓库要的其实正是这个。"),
 ("nord", [], "dark",
  "Nordic and desaturated",
  "北欧低饱和，冷静不刺眼",
  "Every hue is pulled back toward grey. Documentation sites, infrastructure, "
  "libraries — projects whose card should be a supporting actor, not the lead.",
  "所有色相都被压回中灰。文档站、基础设施、库——这些项目的卡片该是配角，"
  "`nord` 是给它们的。"),
 ("ember", ["gruvbox"], "dark",
  "Gruvbox: warm brown, amber and olive",
  "Gruvbox 暖棕底，橙黄与橄榄绿",
  "The one warm dark palette. Amber and olive on brown-black is what an old "
  "terminal looked like, and what the Rust / C / systems crowd has been staring "
  "at for years. People who recognise it read the card as made by one of their own.",
  "唯一一套暖底深色。橙与橄榄绿落在棕黑上是老式终端的颜色，也是 Rust / C / 系统"
  "工具这一圈人看惯的颜色——认得出的人会觉得这张卡片是自己人做的。"),
 ("solar", ["solarized"], "dark",
  "Solarized dark, unmodified",
  "Solarized 深青底，原样照抄",
  "Six low-saturation hues against a fixed neutral ramp, frozen since 2011 and "
  "still the default skin in a great many editors. The values are copied exactly: "
  "adjust them and it is no longer Solarized, just a green that resembles it.",
  "六个低饱和色相配一组固定明度的中性色，2011 年就定死，至今还是很多人编辑器里的"
  "默认皮肤。色值原样照抄——改了就不是 Solarized，只是一套长得像的绿。"),
 ("phosphor", ["crt", "mono"], "dark",
  "One green, five brightnesses",
  "单色 CRT 绿，只靠明度分层",
  "There is no second hue. The series colours are a brightness ramp, so the card "
  "still separates when it is printed in black and white — a chart that separates "
  "by hue alone turns into one grey smear. The error colour is the deliberate "
  "exception: a failure has to be red.",
  "没有第二个色相。序列色是一条绿色明暗阶，所以卡片打印成黑白仍然分得开——"
  "靠色相区分的图表一旦去色就全糊在一起。判定色是唯一的例外：错误必须是红的。"),
 ("blueprint", [], "dark",
  "Drafting blue with cold white rules",
  "工程制图蓝，冷白线打底",
  "Deep blue, cold white, pale cyan — a blueprint. Hardware, protocol and "
  "architecture READMEs usually already carry a diagram or two; the card lines up "
  "with those better than it lines up with our brand colours.",
  "深蓝底、冷白线、淡青强调，像一张蓝图。硬件、协议、架构类项目的 README 里通常"
  "已经有几张示意图，卡片跟着它们走比跟着我们的品牌色走更整齐。"),
 ("okabe", ["okabe-ito", "colorblind"], "dark",
  "Okabe–Ito on pure black",
  "Okabe–Ito 色盲友好方案 + 纯黑底",
  "The one palette that solves a problem before it has a taste. The five series "
  "colours come from the Okabe–Ito eight-colour set, which stays distinguishable "
  "with red-green colour blindness — where the default's pink and cyan converge. "
  "Pure black puts body text at 21:1.",
  "唯一一套先解决问题、再谈好看的色板。五个序列色取自 Okabe–Ito 的八色方案，"
  "红绿色觉异常的人也能区分——默认那套的粉与青在二型色觉下会靠得很近。"
  "纯黑底把正文对比度顶到 21:1。"),
 ("newsprint", ["swiss"], "light",
  "Greyscale with a single red",
  "瑞士排版：灰阶加一点红",
  "Light. The series colours are a grey ramp and only the emphasis is red, like a "
  "page of newsprint or an annual report. For projects whose card should read as a "
  "document rather than as an interface.",
  "浅色。序列色是一条灰阶，只有需要强调的地方是红的——像一页报纸或一份年报。"
  "给那些希望卡片读起来是「文件」而不是「界面」的项目。"),
 ("sakura", ["pastel"], "light",
  "Soft rose paper",
  "柔和粉彩纸底",
  "The gentlest of the light palettes: rose, lavender and sage, all held below "
  "full brightness so nothing glares on pink-white paper. Design tools, content "
  "projects, repositories whose readers are not all engineers — their READMEs "
  "usually run at this temperature already.",
  "浅色里最温和的一套。玫瑰、薰衣草、鼠尾草都压过明度，在粉白纸上不刺眼。设计工具、"
  "内容项目、面向非工程读者的仓库——它们的 README 通常也是这个温度，卡片不该是那一页"
  "上唯一硬的东西。"),
 ("glacier", ["ice"], "light",
  "Light and cold, where porcelain is light and warm",
  "冷调近白，与暖调的 porcelain 互补",
  "`porcelain` goes with cream, wood and serifs; this one goes with blue. With only "
  "one light palette, half of all READMEs have to make do — which is why there are two.",
  "`porcelain` 配米色、木色、衬线字的版面，这一套配蓝色系的版面。浅色只有一套时，"
  "一半的 README 只能将就。"),
 ("carbon", ["mono", "bw"], "dark",
  "White on black. No hue anywhere, and no gradient",
  "白字黑底，没有色相，也没有渐变",
  "`phosphor` is already single-hue, but it is single-hue *green* — it still has a "
  "taste. This one has none: five score bands are five brightnesses, and the three "
  "stops of the brand gradient hold the same value, so the wordmark is a flat block "
  "rather than a sweep. For repositories whose card should have no opinion, and for "
  "any card that ends up on paper.",
  "`phosphor` 已经是单色，但那是**绿色的**单色——它仍然在表达一种趣味。这一套连趣味"
  "都不表达：五档分数是五级明度，品牌渐变的三段取值相同，所以 wordmark 上是一块平色"
  "而不是一条渐变。给那些「卡片不该有观点」的仓库，以及任何最后会被印在纸上的卡片。"),
 ("paper", ["print"], "light",
  "Black on white. Carbon inverted",
  "黑字白纸，carbon 翻过来的那一面",
  "Fax, photocopier, black-and-white print, greyscale e-ink — every colour palette "
  "collapses into a heap of indistinguishable greys there. This one was drawn in "
  "greys to begin with: the score bands run from pure black down to 3:1, and the "
  "card looks the same before and after the colour is taken out of it.",
  "传真、影印、黑白打印、灰度电子墨水——在这些地方任何一套彩色色板都会退化成一堆分"
  "不开的灰。这一套本来就是按灰设计的：五档分数从纯黑到 3:1，卡片去色前后长得一模"
  "一样。"),
]

# ── read the palettes out of the Rust source ───────────────────────────────
def rgb(m):
    return "#%02x%02x%02x" % (int(m[0], 16), int(m[1], 16), int(m[2], 16))

def parse_palettes():
    """Pull every `pub const … : Palette` out of theme.rs.

    Fields are written either as a literal or as one of the named constants at
    the top of the file (`bg: INK`), so the named ones are resolved first.
    """
    src = open(THEME_RS, encoding="utf-8").read()
    named = {
        m.group(1): rgb(m.group(2, 3, 4))
        for m in re.finditer(
            r"pub const ([A-Z_]+): Rgb = Rgb\(0x(..), 0x(..), 0x(..)\);", src
        )
    }

    def colour(token):
        token = token.strip()
        m = re.fullmatch(r"Rgb\(0x(..), 0x(..), 0x(..)\)", token)
        if m:
            return rgb(m.groups())
        if token in named:
            return named[token]
        sys.exit(f"theme.rs: cannot resolve the colour {token!r}")

    out = {}
    for block in re.finditer(
        r"pub const [A-Z_]+: Palette = Palette \{(.*?)\n\};", src, re.S
    ):
        body = block.group(1)
        name = re.search(r'name: "([a-z-]+)"', body).group(1)
        def one(field):
            m = re.search(rf"\n    {field}: (.+),\n", body)
            return colour(m.group(1))
        def many(field):
            seg = re.search(rf"\n    {field}: \[(.*?)\],\n", body, re.S).group(1)
            parts = re.findall(r"Rgb\([^)]*\)|[A-Z][A-Z_]*", seg)
            return [colour(t) for t in parts]
        out[name] = dict(
            bg=one("bg"), panel=one("panel"), text=one("text"), muted=one("muted"),
            line=one("line"), track=one("track"), warn=one("warn"), bad=one("bad"),
            series=many("series"), bands=many("bands"), brand=many("brand"),
        )
    return out

def luminance(hexcolour):
    def channel(v):
        s = v / 255
        return s / 12.92 if s <= 0.03928 else ((s + 0.055) / 1.055) ** 2.4
    c = hexcolour.lstrip("#")
    r, g, b = (int(c[i:i+2], 16) for i in (0, 2, 4))
    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)

def contrast(a, b):
    x, y = luminance(a) + 0.05, luminance(b) + 0.05
    return x / y if x > y else y / x

# ── render the cards ───────────────────────────────────────────────────────
def render(theme, artifact, path, extra):
    cmd = [BIN, "--stages", "artifacts", "--artifact", artifact,
           "--theme", theme, "--stdout"] + extra
    svg = subprocess.run(cmd, cwd=ROOT, capture_output=True).stdout
    if not svg.startswith(b"<svg"):
        sys.exit(f"{theme}/{artifact}: the binary produced no SVG")
    # --stdout promises the artifact and nothing else; check rather than trust,
    # because a file with a paragraph of prose after </svg> still opens fine in
    # a browser and fails in everything that parses XML for real.
    if not svg.rstrip().endswith(b"</svg>"):
        sys.exit(f"{theme}/{artifact}: something followed the SVG on stdout")
    open(path, "wb").write(svg)

def swatch_table(p, lang):
    head = ("| Role | Hex | Contrast on the background |\n|---|---|---|\n"
            if lang == "en" else
            "| 用途 | 色值 | 与底色的对比度 |\n|---|---|---|\n")
    label = {
        "en": dict(bg="Background", panel="Panel", text="Body text", muted="Secondary text",
                   line="Rules", track="Bar track", warn="Warning", bad="Failure",
                   brand="Brand %d", series="Series %d", band="Band %d (%s)"),
        "zh": dict(bg="卡片底色", panel="内嵌面板", text="正文", muted="弱色文字",
                   line="分隔线", track="条形轨道", warn="警告", bad="失败",
                   brand="品牌渐变 %d", series="序列色 %d", band="第 %d 档（%s）"),
    }[lang]
    words_en = ["excellent", "good", "fair", "weak", "poor"]
    words_zh = ["优秀", "良好", "及格", "偏弱", "差"]
    rows = []
    for k in ("bg", "panel", "text", "muted", "line", "track", "warn", "bad"):
        rows.append((label[k], p[k]))
    for i, c in enumerate(p["brand"]):
        rows.append((label["brand"] % (i + 1), c))
    for i, c in enumerate(p["series"]):
        rows.append((label["series"] % (i + 1), c))
    for i, c in enumerate(p["bands"]):
        word = (words_en if lang == "en" else words_zh)[i]
        rows.append((label["band"] % (i + 1, word), c))
    body = ""
    for name, colour in rows:
        ratio = "—" if colour == p["bg"] else f"{contrast(colour, p['bg']):.1f}:1"
        body += f"| {name} | `{colour}` | {ratio} |\n"
    return head + body

def page(key, aliases, kind, tag, why, p, lang):
    en = lang == "en"
    other = "README.zh-CN.md" if en else "README.md"
    other_label = "中文" if en else "English"
    kind_word = ({"dark": "Dark", "light": "Light"} if en else
                 {"dark": "深色", "light": "浅色"})[kind]
    alias_line = ""
    if aliases:
        names = ", ".join(f"`{a}`" for a in aliases)
        alias_line = (f"`--theme` and `.repolish.toml` also accept {names}.\n\n" if en
                      else f"`--theme` 与 `.repolish.toml` 同样接受 {names}。\n\n")
    body_c = contrast(p["text"], p["bg"])
    muted_c = contrast(p["muted"], p["bg"])
    nav = (f"[English](README.md) · [中文](README.zh-CN.md) · "
           + (f"[All palettes](../README.md)" if en else f"[全部色板](../README.zh-CN.md)"))
    if en:
        return f"""# {key}

**{tag}** · {kind_word}

{nav}

<img src="overview.svg" alt="The repolish overview card in the {key} palette" width="880">

```bash
repolish --apply --theme {key}
```

```toml
# .repolish.toml
[readme]
theme = "{key}"
```

{alias_line}## Why this one

{why}

Body text sits at **{body_c:.1f}:1** against the background and secondary text at
**{muted_c:.1f}:1** — the thresholds the palette tests enforce are 7:1 and 4.5:1, and
every band colour clears 3:1. A card nobody can read is not a style choice.

## The report card

<img src="card.svg" alt="The repolish report card in the {key} palette" width="880">

## Every colour

{swatch_table(p, 'en')}
---

Rendered by `scripts/render-themes.py` from [`theme.rs`](../../../crates/repolish-render/src/theme.rs),
which is the only place these values exist.
"""
    return f"""# {key}

**{tag}** · {kind_word}

{nav}

<img src="overview.svg" alt="{key} 色板下的 repolish 总览卡片" width="880">

```bash
repolish --apply --theme {key}
```

```toml
# .repolish.toml
[readme]
theme = "{key}"
```

{alias_line}## 为什么有这一套

{why}

正文与底色的对比度是 **{body_c:.1f}:1**，弱色文字是 **{muted_c:.1f}:1**——色板测试卡住的线是
7:1 和 4.5:1，每一档分数色都过 3:1。一张读不清的卡片不叫风格。

## 报告卡片

<img src="card.svg" alt="{key} 色板下的 repolish 报告卡片" width="880">

## 全部用色

{swatch_table(p, 'zh')}
---

由 `scripts/render-themes.py` 从 [`theme.rs`](../../../crates/repolish-render/src/theme.rs)
生成——那里是这些色值唯一存在的地方。
"""

def index(pals, lang):
    en = lang == "en"
    rows = []
    for key, aliases, kind, tag, tag_zh, why, why_zh in THEMES:
        kind_word = ({"dark": "dark", "light": "light"} if en else
                     {"dark": "深色", "light": "浅色"})[kind]
        t = tag if en else tag_zh
        rows.append(f"| [`{key}`]({key}/{'README.md' if en else 'README.zh-CN.md'}) "
                    f"| {kind_word} | {t} |")
    table = ("| Palette | | What it is |\n|---|---|---|\n" if en
             else "| 色板 | | 是什么 |\n|---|---|---|\n") + "\n".join(rows)
    gallery = "\n".join(
        f'<a href="{k}/{"README.md" if en else "README.zh-CN.md"}">'
        f'<img src="{k}/overview.svg" alt="{k}" width="420"></a>'
        for k, *_ in THEMES)
    if en:
        return f"""# Palettes

Every palette `--theme` accepts, each rendered on this repository's own overview card.
Nothing here is a mock-up: each image is what `repolish --theme <name>` writes.

[English](README.md) · [中文](README.zh-CN.md) · [Design notes](../README.md)

{table}

```bash
repolish --apply --theme nord
```

All of them pass the same contrast tests: body text at 7:1 against the background,
secondary text at 4.5:1, and every score band at 3:1. Picking a palette changes how the
card looks and nothing else — **no palette can change a score**.

{gallery}
"""
    return f"""# 色板

`--theme` 接受的每一套色板，都用本仓库自己的总览卡片渲染了一遍。这里没有示意图：
每一张都是 `repolish --theme <名字>` 真正写出来的文件。

[English](README.md) · [中文](README.zh-CN.md) · [设计笔记](../README.zh-CN.md)

{table}

```bash
repolish --apply --theme nord
```

它们全部通过同一组对比度测试：正文与底色 7:1，弱色文字 4.5:1，每一档分数色 3:1。
换色板只改变卡片长什么样——**没有任何一套色板能改变分数**。

{gallery}
"""

def main():
    if not os.path.exists(BIN):
        sys.exit("build it first: cargo build")
    pals = parse_palettes()
    known = {k for k, *_ in THEMES}
    missing = known - pals.keys()
    if missing:
        sys.exit(f"theme.rs has no palette named {sorted(missing)}")
    argv = sys.argv[1:]
    # Rewriting the prose should not cost another few hundred API calls: the
    # cards on disk are already the ones this palette produces.
    docs_only = "--docs-only" in argv
    argv = [a for a in argv if a != "--docs-only"]
    extra = argv or ["--remote", "--stars"]
    os.makedirs(OUT, exist_ok=True)

    # Every card is drawn before a single one is written. The overview card
    # counts the files in the repository, so writing card 1 into the tree would
    # make card 2 report a different number — a gallery of twelve cards that
    # disagree about the project they all describe.
    staging = tempfile.mkdtemp(prefix="repolish-themes-")
    try:
        for key, *_ in THEMES:
            os.makedirs(os.path.join(staging, key))
            if docs_only:
                for f in ("overview.svg", "card.svg"):
                    have = os.path.join(OUT, key, f)
                    if not os.path.exists(have):
                        sys.exit(f"--docs-only, but {key}/{f} has never been drawn")
                    shutil.copyfile(have, os.path.join(staging, key, f))
                continue
            render(key, "overview", os.path.join(staging, key, "overview.svg"), extra)
            # The star curve only ever lands on the overview card, and fetching
            # it costs about a dozen API calls — asking for it once per palette
            # on a card that cannot show it is a slow way to spend the quota.
            render(key, "score", os.path.join(staging, key, "card.svg"),
                   [a for a in extra if a != "--stars"] + ["--no-stars"])
            print("reused" if docs_only else "drew", key)
        for key, aliases, kind, tag, tag_zh, why, why_zh in THEMES:
            d = os.path.join(OUT, key)
            os.makedirs(d, exist_ok=True)
            for f in ("overview.svg", "card.svg"):
                shutil.copyfile(os.path.join(staging, key, f), os.path.join(d, f))
            p = pals[key]
            open(os.path.join(d, "README.md"), "w", encoding="utf-8").write(
                page(key, aliases, kind, tag, why, p, "en"))
            open(os.path.join(d, "README.zh-CN.md"), "w", encoding="utf-8").write(
                page(key, aliases, kind, tag_zh, why_zh, p, "zh"))
            print("wrote", key)
    finally:
        shutil.rmtree(staging, ignore_errors=True)

    # A palette that was removed leaves a page behind, and a stale page is worse
    # than a missing one: it documents a --theme value that no longer exists.
    for entry in sorted(os.listdir(OUT)):
        path = os.path.join(OUT, entry)
        if os.path.isdir(path) and entry not in known:
            shutil.rmtree(path)
            print("removed the page for", entry, "— theme.rs no longer has it")
    open(os.path.join(OUT, "README.md"), "w", encoding="utf-8").write(index(pals, "en"))
    open(os.path.join(OUT, "README.zh-CN.md"), "w", encoding="utf-8").write(index(pals, "zh"))
    print("wrote the index")

main()
