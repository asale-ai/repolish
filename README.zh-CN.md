<p align="center">
  <img src="assets/hero.zh-CN.svg" alt="" width="100%">
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
- [卡片、表格与录屏](#卡片表格与录屏)
- [给编码智能体用](#给编码智能体用)
- [检查什么](#检查什么)
- [分数怎么来的](#分数怎么来的)
- [当前状态](#当前状态)
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

<img src=".repolish/tables/zh-cn/t-e72559.svg" alt="一行装好" width="880">

<details>
<summary>一行装好（表格原文）</summary>

| 变量 | 缺省 | 作用 |
|---|---|---|
| `REPOLISH_VERSION` | 最新发布 | 装指定 tag，例如 `v0.2.0` |
| `REPOLISH_BIN_DIR` | `~/.local/bin` | 二进制装到哪 |
| `REPOLISH_TARGET` | `detect` | 技能装给谁：`detect`、`all`、`none`，或某一个 id |
| `REPOLISH_NO_SKILL` | 未设 | 设成 `1` 则只装二进制 |

</details>

Linux 版只有 glibc 构建。在 musl（Alpine）上安装脚本会直说并停下，而不是装一个根本跑不起来的二进制——那种情况请用 `cargo install repolish`。

### 用 cargo 安装

需要 Rust 1.88 或更新版本。

```bash
cargo install repolish
```

想装未发布的 `main`：

```bash
cargo install --git https://github.com/asale-ai/repolish repolish
```

五个平台的发布归档（每个都带 `.sha256`）在[发布页](https://github.com/asale-ai/repolish/releases)。GitHub Action 的用法——`repolish init` 会替你生成 workflow——见 [action/README.md](action/README.md)。

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

`--logo-width` 收像素数，也收 **`full`**——后者输出 `width="100%"`。横幅要的就是 `full`：钉死在一个像素宽度上，它在宽屏里缩在左上角，在窄屏里又撑破版心。本文顶上那张横幅就是 `--logo assets/hero.zh-CN.svg --logo-width full --align center`。

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

<img src=".repolish/tables/zh-cn/t-402e74.svg" alt="退出码" width="880">

<details>
<summary>退出码（表格原文）</summary>

| 码 | 含义                            |
| - | ----------------------------- |
| 0 | 成功                            |
| 1 | 分数低于 `--min-score`            |
| 2 | 参数错误                          |
| 3 | 目标不是有效的仓库                     |
| 4 | `--remote` 失败（API 错误、限流、私有仓库） |
| 5 | 能跑的检查项不到一半，不输出总分              |

</details>

## 卡片、表格与录屏

repolish 画出来的每一张图都是**自包含、确定性的 SVG**：不引外部字体、不引脚本、不由我们托管，同一个 commit 逐字节一致。全部是**你自己仓库里**的普通文件。

```bash
repolish card .                 # .repolish/overview.svg —— 这个项目是什么
repolish card . --kind score    # .repolish/card.svg     —— repolish 给它打了几分
repolish card . --kind tables   # 重画 README 里的表格
repolish demo .                 # 真的跑一遍 CLI，录成会动的 SVG
repolish polish . --apply --visuals   # 把以上全部插进 README
```

**哪张放哪儿才是重点。** 概览卡片在顶上、徽章下面：陌生人第一个问题是「这是什么、用什么写的、还活着吗」。分数卡片在[末尾](#用-repolish-打磨)——放顶上意味着访客第一眼看到的是我们的工具在给你的项目评级，而不是你的项目。这个 README 早先版本正好反过来，那是错的。

**表格变成图，原文留着。** GitHub 渲染 Markdown 表格，crates.io、npm 和多数聚合站不渲染，只会把管道符原样吐出来。`--tables svg` 把每张表画一次，并把原表格折进紧挨着的 `<details>`——图片没有文本层，读屏软件、`grep` 和下一个想改这张表的人，读的都是折起来的那份。包起来仍是纯插入：表格自己的字节一个都没动。

**录屏是真的跑命令。** `repolish demo` 执行它们，把结果渲染成一张由 CSS 关键帧驱动的动画 SVG——是文本，diff 得动；不要 `ttyd`、不要 `ffmpeg`、不往历史里塞 GIF。本页顶上那段录屏里的两个分数，就是那一次真的跑出来的。`--dry-run` 先看它会跑什么，`--tape` 则给不渲染 SVG 的平台留一份 VHS 脚本。

两处可调，都不动分数：

```bash
repolish card . --theme porcelain   # 浅色板，给以浅色为主的 README
repolish card . --lang zh-CN        # 默认跟着你的 README 的语言走
```

`--lang` 缺省是 **auto**，读的是你的 README，不是系统 locale——一张写着 `LANGUAGES · BY FILE` 的卡片贴在中文 README 顶上，是我们把自己的语言塞进了别人的门面。

背后的取舍——为什么不做 `prefers-color-scheme`、为什么录屏第 0 帧是终态、为什么表格文件按 slug 而不是序号命名——写在 [docs/02-cli-design.zh-CN.md](docs/02-cli-design.zh-CN.md)。

## 给编码智能体用

让一个智能体「把这个 README 改好」，它的第一个动作是把整个文件重写一遍。那会用一份读起来和其他所有 README 一模一样的东西，换掉作者的语气、排版和例子——而那正是这个工具存在的理由所要防的事。

```bash
repolish skill --list             # 这台机器上装了哪几家智能体
repolish skill --target detect    # 装进探测到的那几家
repolish skill .                  # 或者把 SKILL.md 写进一个仓库
```

[skills/repolish/SKILL.md](skills/repolish/SKILL.md) 可以直接交给 Claude Code、Codex、Gemini、OpenCode 或任何读 `AGENTS.md` 的东西。除了命令清单，它真正要紧的是：那个顺序——**先量，再落实能机械落实的，把需要判断的交回给人，然后再量一次**；repolish 自己的把握到哪里为止；以及每一条发现「好的修法长什么样」。`claim-consistency` 的修法是把那条声明变成真的，**绝不是删掉那一行**——删掉只会让检查变绿，读者手里什么都不剩。

这个分工就是「为什么不给它接一个 LLM」的答案：智能体拥有 repolish 结构上不可能有的上下文，而 repolish 拥有智能体不可能有的确定性。一个会因为模型今早心情不同而变的分数，一文不值。

## 检查什么

22 个检查项，分三类。完整定义、权重与阈值见
[docs/03-scoring.zh-CN.md](docs/03-scoring.zh-CN.md)。

<img src=".repolish/tables/zh-cn/t-0a861c.svg" alt="检查什么" width="880">

<details>
<summary>检查什么（表格原文）</summary>

| 类别       | 检查项                                                  |
| -------- | ---------------------------------------------------- |
| **可发现性** | README 标题与一句话说明、仓库 description、topics、homepage、徽章    |
| **可理解性** | 快速开始、用法示例、安装命令一致性、链接有效性、长度、文档、目录、多语言                 |
| **可信度**  | 许可证、**声明一致性**、CI、测试、活跃度、贡献指南、issue 与 PR 模板、发布规范、行为准则 |

</details>

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

<img src=".repolish/tables/zh-cn/t-d0f407.svg" alt="当前状态" width="880">

<details>
<summary>当前状态（表格原文）</summary>

| <br /> | <br />                                              |
| ------ | --------------------------------------------------- |
| ✅      | `check` —— 22 个检查项、`--remote`、JSON 输出、`--min-score` |
| ✅      | `badge`、`report`、`init`、GitHub Action、5 个平台的预编译二进制、已发布到 crates.io  |
| ✅      | `polish --apply` —— 徽章、目录、issue / PR 模板、CONTRIBUTING；只增量插入，不重写            |
| ✅      | `card` —— 自包含的 SVG 报告卡片，可直接贴进 README            |
| ✅      | `scan` —— 给一个目录下所有仓库排名，并找出它们的共性缺项            |
| ✅      | `.repolish.toml`，以及 `polish` 插入物的全部排版选项            |
| ⏳      | LLM 辅助润色措辞，评分路径上依然没有模型                              |

</details>

检查项清单与 JSON schema 在 v1 冻结：增删检查项或改权重会改变分数在所有仓库上的
含义，因此那是一个带版本号的决定，而不是日常改动。

## 贡献

欢迎提 issue 与 PR，见 [CONTRIBUTING.md](CONTRIBUTING.md)。参与即表示你同意遵守
[行为准则](CODE_OF_CONDUCT.md)。

## 许可证

在 [Apache License 2.0](LICENSE-APACHE) 与 [MIT License](LICENSE-MIT) 之间任选其一。

## 用 repolish 打磨

<img src=".repolish/card.zh-CN.svg" alt="repolish 报告卡片" width="880">

这张卡片由 [repolish](https://github.com/asale-ai/repolish) 生成，是仓库里的一个普通文件——没有外部字体、没有脚本、不由任何第三方托管。想给自己的仓库打一次分：`cargo install repolish && repolish check .`。
