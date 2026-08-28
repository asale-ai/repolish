<p align="center">
  <img src="assets/hero.svg" alt="" width="100%">
</p>

# repolish

**在命令行上诊断并改进「一个开源仓库在陌生人眼里是什么样」。**

[![crates.io](https://img.shields.io/crates/v/repolish.svg)](https://crates.io/crates/repolish)
[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/asale-ai/repolish/main/.repolish/badge.json)](https://github.com/asale-ai/repolish)
[![CI](https://github.com/asale-ai/repolish/actions/workflows/ci.yml/badge.svg)](https://github.com/asale-ai/repolish/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#许可证)
[![Docs](https://img.shields.io/badge/docs-design%20notes-blue.svg)](docs/README.md)

[English](README.md) · [中文](README.zh-CN.md)

<img src=".repolish/overview.zh-CN.svg" alt="repolish 项目概览" width="880">

<sup>上面这张概览卡片由 `repolish card . --lang zh-CN` 生成，每次 push 由 CI 提交。它就是
本仓库里的一个普通文件——没有字体、没有脚本，我们不托管任何东西。我们给这个仓库打的**分数**
在[页面末尾](#用-repolish-打磨)，那才是它该待的地方。</sup>

## 目录

- [为什么做这个](#为什么做这个)
- [安装](#安装)
- [快速开始](#快速开始)
- [用法](#用法)
- [几张卡片](#几张卡片)
- [给编码智能体用](#给编码智能体用)
- [检查什么](#检查什么)
- [分数怎么来的](#分数怎么来的)
- [当前状态](#当前状态)
- [参与开发](#参与开发)
- [贡献](#贡献)
- [许可证](#许可证)

## 为什么做这个

现有的工具分两类：用 LLM 替你写 README 的生成器，和检查「某个区块在不在」的检查器。
两类都没有回答作者真正的问题——**我的仓库现在到底哪里不行，该先改哪一条？**

repolish 按陌生人的读法过一遍仓库，对 22 个具体信号打分，每扣一分都指出文件和行号，
并告诉你该改成什么。

有两条规则让这个分数值得信：

- **评分路径上没有模型。** 同一个 commit 永远得到同一个分数。LLM 可以在之后润色措辞，
  但它不会改动任何一个数字。
- **判不了就说判不了。** 无法确定的检查项返回「不确定」并被剔出分母，而不是猜一个。
  被剔出去的每一项都会列在报告里。

## 安装

### 一行装好

```bash
curl -fsSL https://raw.githubusercontent.com/asale-ai/repolish/main/install.sh | sh
```

它做四件事：下载对应平台的发布二进制、核对 `.sha256`、装进 `~/.local/bin`、把[智能体技能](#给编码智能体用)放进这台机器上装了的那几家智能体里。不想把脚本管道进 shell 的话，先读一遍——POSIX `sh`，两百行左右，就是这四件事。

| 变量 | 缺省 | 作用 |
|---|---|---|
| `REPOLISH_VERSION` | 最新发布 | 装指定 tag，例如 `v0.2.0` |
| `REPOLISH_BIN_DIR` | `~/.local/bin` | 二进制装到哪 |
| `REPOLISH_TARGET` | `detect` | 技能装给谁：`detect`、`all`、`none`，或某一个 id |
| `REPOLISH_NO_SKILL` | 未设 | 设成 `1` 则只装二进制 |

Linux 版只有 glibc 构建。在 musl（Alpine）上安装脚本会直说并停下，而不是装一个根本跑不起来的二进制——那种情况请用 `cargo install repolish`。

### 预编译二进制

每个 release 提供五个目标的二进制，各自带一份 `.sha256`：
`x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`、`x86_64-apple-darwin`、
`aarch64-apple-darwin`、`x86_64-pc-windows-msvc`。

```bash
VERSION=0.3.0
TARGET=x86_64-unknown-linux-gnu
curl -fsSL "https://github.com/asale-ai/repolish/releases/download/v${VERSION}/repolish-v${VERSION}-${TARGET}.tar.gz" | tar -xz
sudo install "repolish-v${VERSION}-${TARGET}/repolish" /usr/local/bin/
```

Windows 的归档是 `.zip`，里面是 `repolish.exe`。全部产物见
[releases 页面](https://github.com/asale-ai/repolish/releases)。

### 用 cargo 安装

需要 Rust 1.88 或更新版本。

```bash
cargo install repolish
```

想装未发布的 `main`：

```bash
cargo install --git https://github.com/asale-ai/repolish repolish
```

### 在 GitHub Actions 里

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0
- uses: asale-ai/repolish@v0.3.0
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

`repolish init` 会替你生成一份完整的 workflow，并固定到生成它的那个版本。
更多示例见 [action/README.md](action/README.md)。

## 快速开始

```bash
repolish check .
```

就这一条。下面是它跑在 `demo/sample` 上的真实录屏——那是一个故意写得很糙的仓库：先 check，再把能改的改掉，然后重新 check。

<img src=".repolish/demo.svg" alt="repolish 给一个很糙的仓库打分、修复、再打分" width="910">

<sup>由 repolish 自己录的，见[录一段 CLI](#录一段-cli)。里面那两个分数就是那一次真的跑出来的分数——一个专门检查「README 承诺的命令是否真的存在」的工具，自己的演示不能是编的。它是手动重录而不是每次 push 重录，理由值得一读：[demo/README.md](demo/README.md)。</sup>

<details>
<summary>上面三条命令里的第一条，文字版</summary>

```text
  acme/taskvault  · cli (detected) · local · 52d9d0e4

  SCORE   23 / 100    poor        ▄▄▄▄▄▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁

  DISCOVERABILITY    ▄▄▄▄▄▄▁▁▁▁▁▁   56
  COMPREHENSIBILITY  ▄▄▄▁▁▁▁▁▁▁▁▁   28
  CREDIBILITY        ▄▁▁▁▁▁▁▁▁▁▁▁   13

  CHECKS  ●○○○●●●○○●●●●●●●●●●●●●   17 scored · 5 not verified

  ── TO FIX ──────────────────────────────────────────────────────────────

   P1  claim-consistency
       1 of the 1 verifiable command claims in the README no longer work.
       Typing the first command from a README and getting an error is the
       fastest way to lose a user
       └ README.md:8  `scripts/setup.sh` — does not exist in the repository

   P1  license
       Add a LICENSE file. No license means all rights reserved — legally,
       nobody may use your code
       └ .  no LICENSE file in the repository root
```

</details>

这段输出里有三处才是这个工具的意义所在：

- **`README.md:8`** —— 每一处扣分都指出文件，有行号的给行号。
- **`5 not verified`** —— 判不了的检查项单独计数并逐条列名，绝不混进分数里当成通过。
- **`local`** —— 报告永远标明这个分数出自哪一种基准，见[分数怎么来的](#分数怎么来的)。

想看横幅、配色和完整的发现列表，跑 `repolish check . -v`。

## 用法

```bash
repolish check .                    # 只做本地检查，不联网
repolish check . --remote           # 额外读取 GitHub 的 description / topics / homepage
repolish check . --format json      # 机器可读，schema 已冻结在版本 1
repolish check . --min-score 70     # 低于阈值时以退出码 1 结束
repolish check . --only license,ci-present
```

`--remote` 从环境变量读取 `GITHUB_TOKEN` 或 `GH_TOKEN`。没有 token 时走匿名配额，
每小时 60 次。

### 扫描一整个组织

`check` 回答「这个仓库哪里不行」；`scan` 回答的是另一个问题——**我这一堆仓库，先动哪一个，以及哪一条修一次能覆盖最多仓库**。

```bash
./scripts/clone-org.sh asale-ai        # 并排 clone 到本地
repolish scan target/orgs/asale-ai --remote
```

```text
  SCORE  REPOSITORY         DISC COMP CRED  FIRST THING TO FIX
  ────────────────────────────────────────────────────────────────────────
    65   agent-firewall       92   55   57   P1 readme-quickstart
    75   token-meter          73   81   71   P2 repo-topics
    85   anything-to-skill    78   90   85   P2 repo-topics
    86   llm-verify           73   96   85   P2 issue-pr-template
    91   asale                78   96   96   P2 readme-title-tagline
    92   seo-geo-skill        87   86  100   P2 repo-topics
    98   repolish             95   99  100   P2 repo-topics

  7 repositories · median 86 · 2 below 80 · 2 P1 in total

  ── FIX ONCE, LIFTS SEVERAL ─────────────────────────────────────────────

     P2 repo-topics                   5 of 7 repositories
     P2 issue-pr-template             4 of 7 repositories
     P2 ci-present                    2 of 7 repositories
```

按分数升序，因为看这张表的人是来找活干的，不是来领奖的。最后那段是它区别于「跑 N 次 `check`」的全部理由：`issue-pr-template` 在 7 个仓库里缺 4 个，那就是写一次文件、收益乘以四的一刀。

那一段按 **(检查项, 严重度)** 分组，而不是只按检查项。同一项在 0 分的仓库出 P1、在 7 分的仓库出 P2，混成一行再贴上更严重的那个标签，就会写出三条 P1 而实际只有一条。

**`scan` 不负责 clone。** 那要求这个二进制会联网、带 git，而评分是离线优先的。把仓库弄到本地是 `git` 的事。

`--remote` 下**一个仓库拉不到就整次扫描失败**（退出码 4），不会默不作声地退回本地分。把两种基准混在同一张表里排序，是这个工具最不该犯的错。

### 插入物的排版

`polish` 插进去的东西长什么样是可配的，命令行或 `.repolish.toml` 的 `[readme]` 段都行：

```bash
repolish polish . --badge-style for-the-badge --toc-style fold --align center
repolish polish . --logo assets/hero.svg --logo-width full --tree-depth 2
repolish polish . --visuals         # 等于 --overview --footer-card --tables svg
```

`--badge-style` 取 shields.io 自己的值，`--toc-style` 有 `bullet` / `number` / `roman` / `fold`（最后一个把目录折进 `<details>`，长 README 很受用），`--tree-depth` 会在末尾追加一棵项目结构树。

`--logo-width` 收像素数，也收 **`full`**——后者输出 `width="100%"`。横幅要的就是 `full`：钉死在一个像素宽度上，它在宽屏里缩在左上角，在窄屏里又撑破版心。本文顶上那张横幅就是 `--logo assets/hero.svg --logo-width full --align center`。

`--visuals` 是[几张卡片](#几张卡片)那一节里三件事的简写：徽章下面的概览卡片、末尾的分数卡片、以及把每张表格画成 SVG 并把原表格折在图下面。三者也可以单独开：`--overview`、`--footer-card`、`--tables svg`。

**这些一个分数都不动。** 检查项清单与权重在 v1 冻结；一个仓库不能靠换徽章样式让自己好看一点，否则分数在仓库之间就不可比了——而那正是这个工具存在的理由。

`--badge-style` 不指定时会**跟着 README 里已有的徽章走**。一排徽章里混进一个样式不同的，比样式统一但不是我们的默认值更难看。

其中三样——logo、目录树、卡片——**不由任何一条检查驱动**。没有哪一项检查要求 README 里有横幅或图表。它们默认关闭，只有你开了才生成，而且 `polish` 的干跑输出会照实写「由配置要求」，不会把它们打扮成一条修复。

### 把能改的直接改掉

```bash
repolish polish .                   # 打印它会做哪些改动
repolish polish . --apply           # 落盘
```

`polish` 只做能从检查结果里机械推出来的改动：repolish 徽章（以及它指向的那份
`.repolish/badge.json`）、用你自己的标题生成的目录、GitHub 的 issue 与 PR 模板，
以及一份 `CONTRIBUTING.md`——里面的构建与测试命令来自**探测到的包清单**：
Cargo 项目写 `cargo test`，npm 项目只有 `package.json` 里真有 `test` 脚本时才写。

推不出来的就不写。探测不出包生态就不生成 `CONTRIBUTING.md`，因为另一条路是写一句
`<your build command here>`——那种文件会让检查项变绿而问题还在原地。行为准则则完全
不生成：Contributor Covenant 里唯一项目特有的是举报邮箱，而一份留着占位符的行为准则，
承诺了一条并不存在的举报通道。

它**只插入**。产出的 diff 全是新增行：制表符、列表标记、引用式链接定义、
行尾都逐字节保留。这不是为谨慎而谨慎——把 README 过一遍 Markdown 格式化器
再写回去，在 12 个真实 README 上 12 个都有损；一个教别人把仓库弄体面的工具，
没有资格顺手重排别人的排版。

不在 git 仓库里时 `--apply` 会拒绝执行，除非加 `--force`——`git checkout`
就是那个撤销键。

### 用作 CI 门禁

在 GitHub 上，action 直接收阈值：

```yaml
- uses: asale-ai/repolish@v0.3.0
  with:
    min-score: 70
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

其他地方，退出码就是门禁：

```bash
repolish check . --remote --min-score 70
```

退出码 1 表示分数不达标，4 表示 GitHub 调用失败——两者刻意区分开，
免得一次限流被读成质量退步。

### 退出码

工具自身失败与「检查不通过」用的是不同的码，否则 CI 里分辨不出来。

| 码 | 含义                            |
| - | ----------------------------- |
| 0 | 成功                            |
| 1 | 分数低于 `--min-score`            |
| 2 | 参数错误                          |
| 3 | 目标不是有效的仓库                     |
| 4 | `--remote` 失败（API 错误、限流、私有仓库） |
| 5 | 能跑的检查项不到一半，不输出总分              |

## 几张卡片

两张 SVG，而**哪张放哪儿才是重点**：

```bash
repolish card .                     # .repolish/overview.svg —— 这个项目是什么
repolish card . --kind score        # .repolish/card.svg     —— repolish 给它打了几分
repolish card . --kind all          # 两张都要，外加下面所有表格
```

**概览卡片**放在顶上、徽章下面。它回答的是一个陌生人真正带着的问题——这是什么、用什么写的、还活着吗——内容是按文件数的语言构成、代码与文档与配置的比例、一年的提交活跃度、许可证和最新的 tag。

**分数卡片**放在[页面末尾](#用-repolish-打磨)，单独一节。这个位置不是装饰。分数卡片放在顶上，意味着访客第一眼看到的是我们的工具在给你的项目评级，而不是你的项目；放在末尾，读者已经决定了要不要用它，此时「这份 README 是用 repolish 打磨的」才是一条有用的信息，而不是一块广告。这个 README 早先版本正好是反过来的，那是错的。

两张都是徽章再往前走一步：分发方式完全一样——文件在**你自己的**仓库里，由你自己的 raw URL 提供，我们不托管任何东西——区别只是徽章上写得下一个数字。

### 「自包含」在这里是什么意思

不引外部字体、不引脚本、不引远程图片，渲染时不碰网络。wordmark 是从一张点阵表转成矩形画出来的，因为读者机器上装没装 JetBrains Mono 不由我们决定。渲染是确定性的，同一个 commit 产出逐字节一致的文件，CI 不会提交一堆只有噪声的 diff。

有两样东西可调，而且都不动分数：

```bash
repolish card . --theme porcelain   # 浅色板，给以浅色为主的 README 用
repolish card . --lang zh-CN        # 默认跟着你的 README 的语言走
```

`--theme dark` 是默认值。`porcelain` 的存在理由是**可读性**而不是口味：一张深色卡片贴进一份浅色 README，在页面上就是一块挖空。这里刻意不做 `prefers-color-scheme` 切换——GitHub 把 SVG 当图片经代理渲染，媒体查询在那条链路上并不可靠，所以文件本身要么是深色的，要么是浅色的。

`--lang` 默认是 **auto**，读你的 README 然后跟着它走。一张写着 `LANGUAGES · BY FILE` 的卡片贴在中文 README 顶上，是我们把自己的语言塞进了别人的门面。它跟的是 README，不是你终端的 locale——否则 CI 里一次 `LANG=C` 就会把它悄悄翻成英文。本页顶上和末尾那两张卡片就是 `--lang zh-CN` 生成的。

### 把表格画成图

GitHub 会渲染 Markdown 表格。crates.io、npm 和大多数 README 聚合站不会——它们把管道符原样吐出来。`--tables svg` 把每张表画一次，画成一张在哪儿都一样的图：

```bash
repolish polish . --apply --tables svg
repolish card . --kind tables       # 改完 README 之后重画
```

**原表格会留着，折进紧挨在图下面的 `<details>` 里。** 这不是客气，是硬要求：图片没有文本层，读屏软件、`grep`、翻译工具，以及下一个想改这张表的人，读的都是折起来的那一份。

包起来这件事仍然是纯插入。表格自己的字节一个都没动，只是在它上下各加了几行。

少于两行的表不画（画成图没有增益），超过十六行的也不画，并且会说一声——一张那么高的图在手机上根本看不清，而真正的表格本来就会自己滚动。

### 录一段 CLI

如果这个项目有可执行文件，它的 README 里最有用的东西是几秒钟的真实运行画面：

```bash
repolish demo .                     # 真的跑一遍，写出 .repolish/demo.svg
repolish demo . --cmd "tool build" --cmd "tool run"
repolish demo . --dry-run           # 只列出它会跑哪几条命令，什么都不执行
repolish demo . --tape              # 顺带写一份 VHS tape，给想要 GIF 的人
```

**它真的会跑那些命令**，输出也是真的——本页顶上那段录屏里的两个分数，就是那一次跑出来的。这也意味着：只对你愿意执行其命令的仓库用它，拿不准就先 `--dry-run`。

产出是一张**会动的 SVG**，动画由 CSS 关键帧驱动。为什么不直接调 [VHS](https://github.com/charmbracelet/vhs)：VHS 很好，但它要 ttyd 和 ffmpeg，产出的是 GIF，而 GIF 与这个仓库对自己每一个产物的三条约束全不相容——

- **二进制。** 一个几百 KB 的 GIF 每次重录都整个换掉，git 历史会被撑肥——本仓库原先那个 GIF workflow 只肯手动触发就是这个原因。文本 SVG diff 得动，内容没变就没有 diff。（这解决的是格式，不是频率：录屏里仍然带着命令打印出来的东西，包括一个 commit 哈希。本仓库因此仍然手动重录，完整理由——连同一次把事情弄得更糟的修复——写在 [demo/README.md](demo/README.md)。）
- **没有文本层。** 录屏里那行命令，读者复制不走，`grep` 也找不到。SVG 里那是**真的文字**。
- **要先装一条视频工具链。** 一个「让你的仓库体面起来」的工具，不该开口就让人装两个外部程序。

两处得说清楚的限制：

- **不做完整终端模拟。** 认 SGR 颜色、`\n` 和 `\r`，认到此为止。会重绘屏幕的程序——进度条、spinner、全屏 TUI——录出来是不对的。
- **不带伪终端。** 输出接的是管道，所以用 `CLICOLOR_FORCE` 与 `FORCE_COLOR` 强制开色；仍然坚持关色的程序录出来就是黑白的。

默认只录 `--help`，因为那是对**任何** CLI 都成立的唯一一条命令。哪几条命令值得给人看是作者的判断，不是我们的，所以其余的一律走 `--cmd`。

### 让它们保持最新

```yaml
- uses: asale-ai/repolish@v0.3.0
  with:
    card: true
    overview: true
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

注意 `card` 会覆盖重写，`polish` 不会。`polish` 从不覆盖任何东西——它只负责第一次把引用插进 README，到此为止。此后每一次重画是 `card` 的活儿，本仓库的 CI 每次 push 跑的就是它。

## 给编码智能体用

让一个智能体「把这个 README 改好」，它的第一个动作是把整个文件重写一遍。那会用一份读起来和其他所有 README 一模一样的东西，换掉作者的语气、排版和例子——而那正是这个工具存在的理由所要防的事。

```bash
repolish skill --list               # 这台机器上装了哪几家智能体
repolish skill --target detect      # 装进探测到的那几家
repolish skill --target claude,codex
repolish skill .                    # 或者把 SKILL.md 写进一个仓库
```

`--target` 装进智能体自己的目录（`~/.claude/skills/repolish/` 之类），装一次所有项目都用得上；不带 `--target` 则写进一个仓库，跟着代码走。Gemini 会额外拿到 `gemini-extension.json` 和它点名的 `GEMINI.md`——只写清单会让 Gemini CLI 每次启动都指向一个不存在的文件。

[skills/repolish/SKILL.md](skills/repolish/SKILL.md) 是一份可以直接交给 Claude Code、Codex 或任何读 skill 定义的东西的文件。它写着命令清单、JSON 结构和退出码，但真正有用的那一半是关于**判断**的：

- 顺序——**先量，再落实能机械落实的，把需要判断的交回给人，然后再量一次**；
- repolish 自己的把握到哪里为止。它有三种判法——事实、交叉核对、以及分档的关键词启发式，而第三种是弱的那一种。分数量的是「读者需要的那套东西在不在、README 承诺的是不是真的」，**它不量文字写得好不好**；
- 每一条发现的「好的修法长什么样」，以及各自的翻车方式。`license` 是作者要做的法律决定，不是丢一个文件进去就完事；`claim-consistency` 的修法是把那条声明变成真的、或者改成真的写法，**绝不是把那一行删掉**——删掉只会让检查变绿，读者手里什么都不剩。

它也明确写着：不许重写 README，不许编一个工具没给出的数字，工具说 `not scored` 就得说 `not scored`。

这个分工是有意的，也正是「为什么不给它接一个 LLM」的答案：智能体拥有 repolish 结构上不可能有的上下文——代码库、你的意图、这段对话；而 repolish 拥有智能体不可能有的确定性。一个会因为模型今早心情不同而变的分数，一文不值。

对智能体来说，`--format json` 才是接口。schema 在 v1 冻结，每一条发现都带着文件、行号和严重度：

```bash
repolish check . --format json
```

## 检查什么

22 个检查项，分三类。完整定义、权重与阈值见
[docs/03-评分维度.md](docs/03-评分维度.md)。

| 类别       | 检查项                                                  |
| -------- | ---------------------------------------------------- |
| **可发现性** | README 标题与一句话说明、仓库 description、topics、homepage、徽章    |
| **可理解性** | 快速开始、用法示例、安装命令一致性、链接有效性、长度、文档、目录、多语言                 |
| **可信度**  | 许可证、**声明一致性**、CI、测试、活跃度、贡献指南、issue 与 PR 模板、发布规范、行为准则 |

**声明一致性**是别的工具都没做的一项：它核对 README 里承诺的命令是否真的存在。
`npm run build` 必须在 `package.json` 里，`make test` 必须是真的目标，
`./scripts/setup.sh` 必须是真的文件。README 的第一条命令就跑不通，读者就是在那里离开的。

## 分数怎么来的

每个检查项返回 0–10 分，并带一个风险权重（Critical 10、High 7.5、Medium 5、Low 2.5）。
总分是加权平均，再换算到 100 分制。

检查项还可能落在「不适用」（文档站不需要测试套件）、「不确定」（浅克隆没有 tag，
无从判断发布节奏）或「已跳过」。只有已打分的项计入分母，而且**当实际打分的权重
不足注册总权重的一半时，干脆不输出总分**——否则「只查了三项、三项都过」会显示成
100/100。

本地分与远程分不可比较：没有 `--remote` 时，三个可发现性检查项会被剔出分母。
报告里会标明你看到的是哪一种。

## 当前状态

下面每一项要么已经发了，要么明确还没做。在情况变化之前这一节会一直如实写着：

| <br /> | <br />                                              |
| ------ | --------------------------------------------------- |
| ✅      | `check` —— 22 个检查项、`--remote`、JSON 输出、`--min-score` |
| ✅      | `badge`、`report`、`init`、GitHub Action、5 个平台的预编译二进制、已发布到 crates.io  |
| ✅      | `polish --apply` —— 徽章、目录、issue / PR 模板、CONTRIBUTING；只增量插入，不重写            |
| ✅      | `card` —— 自包含的 SVG 报告卡片，可直接贴进 README            |
| ✅      | `scan` —— 给一个目录下所有仓库排名，并找出它们的共性缺项            |
| ✅      | `.repolish.toml`，以及 `polish` 插入物的全部排版选项            |
| ⏳      | LLM 辅助润色措辞，评分路径上依然没有模型                              |

检查项清单与 JSON schema 在 v1 冻结：增删检查项或改权重会改变分数在所有仓库上的
含义，因此那是一个带版本号的决定，而不是日常改动。

## 参与开发

```bash
git clone https://github.com/asale-ai/repolish
cd repolish
cargo test
cargo clippy --all-targets
cargo fmt --all -- --check
./scripts/fetch-fixtures.sh
```

`fetch-fixtures.sh` 会克隆用于人工验收的 12 个真实仓库，每一条都注明了这个仓库
当初暴露出的缺陷。

设计文档在 [docs/](docs/README.md)。

### 发布

```bash
./publish.sh "改了什么"                # patch 位 +1
./publish.sh --minor "加了概览卡片"
./publish.sh --version 1.0.0 "第一个稳定版"
./publish.sh --clawhub "…"            # 顺带把技能发到 ClawHub
./publish.sh --dry-run "…"            # 每一步都打出来，什么都不改
```

一条命令走完整个发布：跑测试、改工作区版本号、把文档里每一处 `repolish@vX.Y.Z`
一并改掉、开 PR、等必需检查通过、给**真正落地的那个 commit** 打 tag、盯着
`release.yml` 构建五个平台的二进制，最后**按依赖顺序**把六个 crate 发到
crates.io——`repolish-md`、`repolish-ingest`、`repolish-core`、`repolish-checks`、
`repolish-render`、`repolish`——并在每一个之间等索引更新，因为 cargo 不接受一个
path 依赖尚未发布的 crate。

它可以重跑：已经发到新版本的 crate 会被跳过，所以中途失败只要带
`--version X.Y.Z --skip-tests` 再跑一次。工作区不干净、分支落后于 `main`、
tag 已存在、或者没有 crates.io 凭据，它都会在**打 tag 之前**就停下——
这几件事在打 tag 之后才发现，代价大得多。

## 贡献

欢迎提 issue 与 PR，见 [CONTRIBUTING.md](CONTRIBUTING.md)。参与即表示你同意遵守
[行为准则](CODE_OF_CONDUCT.md)。

## 许可证

在 [Apache License 2.0](LICENSE-APACHE) 与 [MIT License](LICENSE-MIT) 之间任选其一。

## 用 repolish 打磨

<img src=".repolish/card.zh-CN.svg" alt="repolish 报告卡片" width="880">

这张卡片由 [repolish](https://github.com/asale-ai/repolish) 生成，是仓库里的一个普通文件——没有外部字体、没有脚本、不由任何第三方托管。想给自己的仓库打一次分：`cargo install repolish && repolish check .`。
