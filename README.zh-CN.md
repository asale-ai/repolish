<p align="center">
  <img src="assets/hero.zh-CN.svg" alt="" width="100%">
</p>

# repolish

**在命令行上诊断并改进「一个开源仓库在陌生人眼里是什么样」。**

[![crates.io](https://img.shields.io/crates/v/repolish.svg)](https://crates.io/crates/repolish)
[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/asale-ai/repolish/main/.repolish/badge.json)](https://github.com/asale-ai/repolish)
[![CI](https://github.com/asale-ai/repolish/actions/workflows/ci.yml/badge.svg)](https://github.com/asale-ai/repolish/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#许可证)
[![Docs](https://img.shields.io/badge/docs-design%20notes-blue.svg)](docs/README.zh-CN.md)

[English](README.md) · [中文](README.zh-CN.md)

<img src=".repolish/overview.zh-CN.svg" alt="repolish 项目概览" width="880">

repolish 按陌生人的读法过一遍仓库，对 22 个具体信号打分，每扣一分都指出文件和行号，
并告诉你该改成什么。然后把其中能机械落实的那部分直接改掉。

有两条规则让这个分数值得信：**评分路径上没有模型**，同一个 commit 永远得到同一个分数；
**判不了就说判不了**，无法确定的检查项报成「未验证」并被剔出分母，绝不猜一个。

## 目录

- [安装](#安装)
- [一条命令](#一条命令)
- [它做了什么](#它做了什么) —— 四个阶段
- [怎么控制它](#怎么控制它)
- [配置](#配置)
- [在 CI 里](#在-ci-里)
- [卡片与录屏](#卡片与录屏)
- [给编码智能体用](#给编码智能体用)
- [检查什么](#检查什么)
- [分数怎么来的](#分数怎么来的)
- [退出码](#退出码)
- [当前状态](#当前状态)
- [贡献](#贡献)
- [许可证](#许可证)

## 安装

```bash
npx @asale/repolish
```

不用装任何东西，有 Node 的地方就能跑。这个包是一层启动器，不是另一份实现：
它下载对应平台的发布二进制、校验 `.sha256`、然后执行——不管走哪条路，
真正做检查的都是同一个静态 Rust 二进制。

<details>
<summary>另外四种装法</summary>

**用 npm 全局装**，让 `repolish` 进 PATH：

```bash
npm install -g @asale/repolish
```

**一行装完**，顺带把[智能体技能](#给编码智能体用)装进这台机器上找得到的那些智能体：

```bash
curl -fsSL https://raw.githubusercontent.com/asale-ai/repolish/main/install.sh | sh
```

同样的下载与 `.sha256` 校验，装到 `~/.local/bin`。POSIX `sh`，约 200 行——
不想把脚本直接管道进 shell 的话，先读一遍。`REPOLISH_VERSION`、`REPOLISH_BIN_DIR`
和 `REPOLISH_TARGET` 可以覆盖默认值。

**用 cargo 装**，需要 Rust 1.88 以上：

```bash
cargo install repolish
cargo install --git https://github.com/asale-ai/repolish repolish  # 未发布的 main
```

**直接下压缩包**，五个目标平台，每个都带 `.sha256`，在[发布页](https://github.com/asale-ai/repolish/releases)。

</details>

Linux 构建只支持 glibc。在 musl 上安装脚本会明说并停下，而不是留一个跑不起来的二进制；
那种环境请用 `cargo install repolish`。

**下文的命令一律用 `npx @asale/repolish`**，不需要预先装任何东西。注意 npx 不会把
`repolish` 放进你的 PATH——它下载到缓存里直接执行。如果你确实全局装过（npm、cargo，
或上面那个脚本），把前缀去掉、直接敲 `repolish` 即可，参数完全一样。

## 一条命令

```bash
npx @asale/repolish
```

**没有子命令，而且一个字节都不写。** 它给仓库打分，然后列出它会创建或改动的每一个文件：

<img src=".repolish/demo.svg" alt="repolish 给一个很糙的仓库打分、修复、再打分" width="910">

<sup>由 repolish 自己录制，对象是[demo/sample](demo/sample)——一个故意写得很糙的仓库；
里面那两个分数就是那一次真的跑出来的。它为什么是手动重录：[demo/README.md](demo/README.md)。</sup>

<details>
<summary>一次运行长什么样（文字版）</summary>

```text
  acme/taskvault  · cli (detected) · local · 52d9d0e4

  SCORE   23 / 100    poor        ▄▄▄▄▄▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁

  DISCOVERABILITY    ▄▄▄▄▄▄▁▁▁▁▁▁   56
  COMPREHENSIBILITY  ▄▄▄▁▁▁▁▁▁▁▁▁   28
  CREDIBILITY        ▄▁▁▁▁▁▁▁▁▁▁▁   13

  CHECKS  ●○○○●●●○○●●●●●●●●●●●●●   17 scored · 5 not verified

  ── TO FIX ──────────────────────────────────────────────────────────────

   P1  license
       Add a LICENSE file. No license means all rights reserved — legally,
       nobody may use your code
       └ .  no LICENSE file in the repository root

  WOULD WRITE (6 files)
    .github/ISSUE_TEMPLATE/bug_report.yml       new file
    .github/ISSUE_TEMPLATE/feature_request.yml  new file
    .github/pull_request_template.md            new file
    CONTRIBUTING.md                             new file
    .repolish/badge.json                        score badge
    .github/workflows/repolish.yml              CI workflow

  Nothing was written. Apply with: npx @asale/repolish --apply
```

</details>

这段输出里有三处才是这个工具的意义所在：**`README.md:8`**——每一处扣分都指出文件，
有行号的给行号；**`5 not verified`**——判不了的检查项绝不混进分数里当成通过；
**`local`**——报告永远标明这个分数出自哪一种基准，本地分和 `--remote` 分不可横向比较。

计划看着没问题，就落盘：

```bash
npx @asale/repolish --apply
```

整个流程就这样。`--apply` **只增量插入**：产出的 diff 全是新增行，你的制表符、列表标记、
引用式链接定义和行尾逐字节保留。不在 git 仓库里时它会拒绝执行，除非加 `--force`——
没有 `git checkout` 就没有撤销键。

## 它做了什么

四个阶段，按顺序跑。顺序是有意义的：`polish` 可能刚往 README 里插入了一张卡片的引用，
`artifacts` 紧接着才画得出那张图。

| 阶段 | 做什么 |
|---|---|
| `check` | 给仓库打分并打印报告 |
| `polish` | 能机械落实的那些改动：徽章、用你自己的标题生成的目录、GitHub 的 issue 与 PR 模板，以及一份构建和测试命令来自探测到的包清单的 `CONTRIBUTING.md` |
| `artifacts` | 写 `.repolish/badge.json`，画出横幅与两张卡片，并重画 README 已经引用的每一张 SVG |
| `ci` | 写 `.github/workflows/repolish.yml` |

**推不出来就不写。** 没有包清单就没有 `CONTRIBUTING.md`——另一条路是写一份
`<your build command here>`，那种文件让检查变绿，问题却原地不动。已经存在的文件一律
不动，`--force` 才重新生成。

还有两个阶段，**刻意不在默认流程里**：

| 阶段 | 为什么要显式点名 |
|---|---|
| `skill` | 写 `SKILL.md`，只有用编码智能体的人才需要 |
| `demo` | 会**真的执行**它录下的命令，那不是一个默认动作该做的事。跳过它的那次运行会在末尾提醒你 |

## 怎么控制它

```bash
npx @asale/repolish --stages check                 # 只打分，什么都不写
npx @asale/repolish --stages check,polish --apply  # 只修复，不写徽章 JSON、不写 CI workflow
npx @asale/repolish --stages demo --apply          # 录制动画
npx @asale/repolish -v                             # P3 建议、通过项、以及每个新文件的完整内容
npx @asale/repolish --remote                       # 额外从 GitHub 读 description / topics / homepage
```

`--remote` 从环境变量读取 `GITHUB_TOKEN` 或 `GH_TOKEN`。没有 token 时走匿名配额，
每小时 60 次。

```bash
npx @asale/repolish --format json              # schema 已冻结在版本 1
npx @asale/repolish --only license,ci-present  # 只跑这几项
npx @asale/repolish --skip repo-topics         # 除了这项其余都跑
```

`--format` 接受 `text`（默认）、`json`、`markdown`、`sarif` 和 `comment`。除 `text` 外的
每一种格式下，**stdout 只有那份报告**，所有过程性输出走 stderr——所以
`npx @asale/repolish --format json | jq` 在完整流水线下也是通的。

有三项发现是刻意留给你的：一句话简介、快速开始、用法示例。任何机械规则都满足不了它们。
模型可以起草这三段，也只有这三段：

```bash
npx @asale/repolish --suggest  # 需要 REPOLISH_LLM_API_KEY，或 ANTHROPIC_API_KEY
```

`--suggest` **从不落盘**，`--apply` 也不写；它**只补缺的那一段**；它还**编不出东西**
——缺一个事实就把建议留空。它不影响任何一个分数。[为什么是这三段](docs/04-usage.zh-CN.md)。

## 配置

那些你本来要在命令行上反复敲的东西，写进仓库根目录的 `.repolish.toml`。命令行永远优先，
写错一个键名会直接报错，而不是静默忽略。

```toml
profile   = "cli"      # 覆盖类型探测
min_score = 70         # 等价于 --min-score

[checks]
skip = ["repo-topics"]

[readme]               # 插入物的排版。这一段不影响任何一个分数。
toc-style = "fold"
theme     = "porcelain"

[suggest]              # 哪个模型来起草建议。密钥不放这里：这个文件是要提交的。
model = "claude-sonnet-4-5"
```

逐检查项的阈值刻意不开放：让每个仓库自己调阈值，等于让分数在仓库之间不可比，
而那正是这个工具存在的理由。[完整键表](docs/04-usage.zh-CN.md)。

## 在 CI 里

`ci` 阶段写出的 workflow 里是两个 job：push 上那个记录分数并把徽章提交回来；PR 上那个
报告**这次改动**让分数变成了什么样，上传 SARIF 让每条发现落在 diff 里它自己那一行，
并发一条评论。

```bash
npx @asale/repolish --stages ci --min-score 70 --apply
```

想手工接的话，action 直接收阈值：

```yaml
- uses: asale-ai/repolish@v0.4.1
  with:
    min-score: 70
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

其他地方，退出码就是门禁：`npx @asale/repolish --stages check --remote --min-score 70`。退出码 1
表示分数不达标，4 表示 GitHub 调用失败——两者刻意区分开，免得一次限流被读成质量退步。
在 PR 上，重点是变化量：

```bash
npx @asale/repolish --stages check --base origin/main      # 报告相对某个 ref 的变化量
npx @asale/repolish --stages check --sarif repolish.sarif  # 每条发现一条注解，落在它自己那一行
npx @asale/repolish --stages check --comment comment.md    # 短报告，给 PR 评论用
```

`--sarif` 和 `--comment` 不受 `--apply` 约束：你明确点了输出路径，那本身就是请求，
不是对仓库的改动。

[每一个的行为](docs/04-usage.zh-CN.md) · [action 的输入项](action/README.md)

## 卡片与录屏

```bash
npx @asale/repolish --apply                     # 已含卡片与 SVG 表格
npx @asale/repolish --apply --no-visuals        # 不动 README 的视觉产物
npx @asale/repolish --stages artifacts --apply  # 重画所有已经被引用的图
npx @asale/repolish --stages demo --apply       # 把 CLI 录成动画 SVG
```

repolish 画出来的一切都是**自包含、确定性的 SVG**，而且是**你自己**仓库里的普通文件——
它不会对你 404、不会限流、也不会记录谁读了你的 README。概览卡片该在顶部、徽章下面；
分数卡片该在[末尾](#用-repolish-打磨)，因为放在顶部时访客第一眼看到的会是我们的工具在
给你的项目打分，而不是你的项目。

`polish` 负责第一次把引用插进 README，此后每一次重画由 `artifacts` 负责，图就不会过期。
要单独指定一张，用 `--artifact badge,report,hero,overview,score,tables`。横幅上印的是
**你的**项目名——点阵字体画不出的名字（非拉丁文，或者单纯太长）会退回普通文字，
而不是渲染成一片空白。其余的——`--theme`、
`--lang`、`--stars` 为什么只对你有管理权的仓库有用、`demo` 为什么真的会执行它录下的
命令——都在 [docs/02-cli-design.zh-CN.md](docs/02-cli-design.zh-CN.md)。

## 给编码智能体用

叫智能体「改进一下这个 README」，它第一步就是把整个文件重写一遍，把作者的语气和例子换成
一份读起来跟别人一模一样的 README。那恰好是这个工具要防的事。

```bash
npx @asale/repolish --stages skill --list                   # 这台机器上装了哪些智能体
npx @asale/repolish --stages skill --target detect --apply  # 装进那些真的装了的
npx @asale/repolish --stages skill --apply                  # 或者把 SKILL.md 写进某个仓库
```

[skills/repolish/SKILL.md](skills/repolish/SKILL.md) 就是交给 Claude Code、Codex、
Gemini、OpenCode 或任何读 `AGENTS.md` 的工具的那个文件。除了命令表面，它还带着顺序——
**先量，机械的先落实，需要判断的交回给人，再量一次**。

## 检查什么

22 个检查项，分三类，其中 3 项需要 `--remote`。完整定义与权重见
[docs/03-scoring.zh-CN.md](docs/03-scoring.zh-CN.md)。

<img src=".repolish/tables/zh-cn/t-0a861c.svg" alt="检查什么" width="880">

<details>
<summary>检查什么（表格原文）</summary>

| 类别 | 检查项 |
|---|---|
| **可发现性** | README 标题与一句话简介、仓库描述、topics、homepage、徽章 |
| **可理解性** | 快速开始、用法示例、安装命令一致性、链接健康、篇幅、docs 目录、目录、多语言 |
| **可信度** | 许可证、**声明一致性**、CI、测试、活跃度、贡献指南、issue 与 PR 模板、发布卫生、行为准则 |

</details>

**声明一致性**是别的工具都不做的那一项：`npm run build` 必须在 `package.json` 里，
`make test` 必须是一个真的 target。README 第一条命令就跑不通的地方，就是读者离开的地方。
它抓的是改名和删除——那条悄悄不存在了的命令。

## 分数怎么来的

每个检查项返回 0–10，并带一个风险权重，总分是加权平均。判为**不适用**、**不确定**或
**跳过**的项从分母里剔出去，而不是当成通过——并且**当能打分的权重不到总权重的一半时，
根本不输出总分**，因为「我们检查了三项，都过了」不能读成 100/100。权重、阈值和聚合规则
都在 [docs/03-scoring.zh-CN.md](docs/03-scoring.zh-CN.md)。

## 退出码

工具自身失败与「检查不通过」用的是不同的码，否则 CI 里分辨不出来。

<img src=".repolish/tables/zh-cn/t-402e74.svg" alt="退出码" width="880">

<details>
<summary>退出码（表格原文）</summary>

| 码 | 含义                            |
| - | ----------------------------- |
| 0 | 成功                            |
| 1 | 分数低于 `--min-score`             |
| 2 | 参数错误                          |
| 3 | 目标不是有效的仓库                     |
| 4 | `--remote` 失败（API 错误、限流、私有仓库） |
| 5 | 能打分的权重不到一半，不输出总分              |
| 7 | `--base` 的基线取不到：浅克隆、ref 不存在、没有 git |

</details>

## 当前状态

上面写到的都已经发布，措辞建议也在内。

检查项清单与 JSON schema 在 v1 冻结。增删或调整权重会改变分数在所有地方的含义，
因此那是一个需要走版本的决定，不是日常改动。

## 贡献

欢迎 issue 和 PR。[CONTRIBUTING.md](CONTRIBUTING.md) 讲了怎么构建、三条不讨论的规则、
怎么加一个检查项，以及发布流程。设计笔记在 [docs/](docs/README.zh-CN.md)。参与即表示你同意
[行为准则](CODE_OF_CONDUCT.md)。

## 许可证

在 [Apache License 2.0](LICENSE-APACHE) 与 [MIT License](LICENSE-MIT) 之间任选其一。

## 用 repolish 打磨

<img src=".repolish/card.svg" alt="repolish 报告卡片" width="880">

这张卡片由 [repolish](https://github.com/asale-ai/repolish) 生成，是本仓库里的一个普通
文件——没有外部字体、没有脚本，我们不托管任何东西。给你自己的仓库打分：
`npx @asale/repolish`。
