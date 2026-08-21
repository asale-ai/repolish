# repolish

**在命令行上诊断并改进「一个开源仓库在陌生人眼里是什么样」。**

[![crates.io](https://img.shields.io/crates/v/repolish.svg)](https://crates.io/crates/repolish)
[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/asale-ai/repolish/main/.repolish/badge.json)](https://github.com/asale-ai/repolish)
[![CI](https://github.com/asale-ai/repolish/actions/workflows/ci.yml/badge.svg)](https://github.com/asale-ai/repolish/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#许可证)
[![Docs](https://img.shields.io/badge/docs-design%20notes-blue.svg)](docs/README.md)

[English](README.md) · [中文](README.zh-CN.md)

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

就这一条：它会打印分数、支撑分数的发现，以及在 `--remote` 下 GitHub 自己
知道的那部分信息。

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

### 把能改的直接改掉

```bash
repolish polish .                   # 打印它会做哪些改动
repolish polish . --apply           # 落盘
```

`polish` 只做能从检查结果里机械推出来的改动：插入 repolish 徽章（以及它指向的
那份 `.repolish/badge.json`），以及用你自己的标题生成一份目录。

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
| ✅      | `polish --apply` —— 插入徽章与目录；只增量插入，不重写            |
| ⏳      | polish 的更多改动、LLM 辅助建议                              |

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
