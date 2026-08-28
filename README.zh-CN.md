<h1><img src="assets/wordmark.svg" alt="repolish" height="52"></h1>

**在命令行上诊断并改进「一个开源仓库在陌生人眼里是什么样」。**

[![crates.io](https://img.shields.io/crates/v/repolish.svg)](https://crates.io/crates/repolish)
[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/asale-ai/repolish/main/.repolish/badge.json)](https://github.com/asale-ai/repolish)
[![CI](https://github.com/asale-ai/repolish/actions/workflows/ci.yml/badge.svg)](https://github.com/asale-ai/repolish/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#许可证)
[![Docs](https://img.shields.io/badge/docs-design%20notes-blue.svg)](docs/README.md)

[English](README.md) · [中文](README.zh-CN.md)

<img src=".repolish/card.svg" alt="repolish 给自己这个仓库打的分" width="880">

<sup>这张卡片由 `repolish card .` 生成，每次 push 由 CI 提交。它就是本仓库里的一个
普通文件——没有字体、没有脚本，我们不托管任何东西。</sup>

## 目录

- [为什么做这个](#为什么做这个)
- [安装](#安装)
- [快速开始](#快速开始)
- [用法](#用法)
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

### 预编译二进制

每个 release 提供五个目标的二进制，各自带一份 `.sha256`：
`x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`、`x86_64-apple-darwin`、
`aarch64-apple-darwin`、`x86_64-pc-windows-msvc`。

```bash
VERSION=0.2.0
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
- uses: asale-ai/repolish@v0.2.0
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

`repolish init` 会替你生成一份完整的 workflow，并固定到生成它的那个版本。
更多示例见 [action/README.md](action/README.md)。

## 快速开始

```bash
repolish check .
```

就这一条。下面是它跑在 `demo/sample` 上的真实输出——那是一个故意写得很糙的仓库，你可以原样复现：

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

### 报告卡片

```bash
repolish card .                     # 写出 .repolish/card.svg
repolish card . --stdout            # 打印，不落盘
```

本文件顶上那张卡片就是它。这是徽章再往前走一步：分发方式完全一样——文件在**你自己的**
仓库里，由你自己的 raw URL 提供，我们不托管任何东西——区别只是徽章上写得下一个数字，
卡片写得下扣分在哪。

它是一张自包含的 SVG。不引外部字体、不引脚本、不引远程图片：分数和 wordmark 是从一张
点阵表转成矩形画出来的，因为读者机器上装没装 JetBrains Mono 不由我们决定。渲染是确定性的，
同一个 commit 产出逐字节一致的文件，CI 不会提交一堆只有噪声的 diff。

想让它一直是最新的，让 CI 和徽章一起写：

```yaml
- uses: asale-ai/repolish@v0.2.0
  with:
    card: true
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### 插入物的排版

`polish` 插进去的东西长什么样是可配的，命令行或 `.repolish.toml` 的 `[readme]` 段都行：

```bash
repolish polish . --badge-style for-the-badge --toc-style fold --align center
repolish polish . --logo assets/hero.png --logo-width 420 --tree-depth 2
```

`--badge-style` 取 shields.io 自己的值，`--toc-style` 有 `bullet` / `number` / `roman` / `fold`（最后一个把目录折进 `<details>`，长 README 很受用），`--tree-depth` 会在末尾追加一棵项目结构树。

**这些一个分数都不动。** 检查项清单与权重在 v1 冻结；一个仓库不能靠换徽章样式让自己好看一点，否则分数在仓库之间就不可比了——而那正是这个工具存在的理由。

`--badge-style` 不指定时会**跟着 README 里已有的徽章走**。一排徽章里混进一个样式不同的，比样式统一但不是我们的默认值更难看。

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
- uses: asale-ai/repolish@v0.2.0
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

## 贡献

欢迎提 issue 与 PR，见 [CONTRIBUTING.md](CONTRIBUTING.md)。参与即表示你同意遵守
[行为准则](CODE_OF_CONDUCT.md)。

## 许可证

在 [Apache License 2.0](LICENSE-APACHE) 与 [MIT License](LICENSE-MIT) 之间任选其一。
