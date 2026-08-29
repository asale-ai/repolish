# repolish 设计文档

面向开源作者的仓库诊断 / 优化 CLI 工具。

[English](README.md) · [中文](README.zh-CN.md)

| 文档 | 内容 |
|---|---|
| [01-architecture](01-architecture.zh-CN.md) | Rust workspace 结构、crate 选型、三个核心设计 |
| [02-cli-design](02-cli-design.zh-CN.md) | 命令面、退出码、输出契约（含终端配色与 SVG 卡片）、Action 模板 |
| [03-scoring](03-scoring.zh-CN.md) | 检查项清单、权重、聚合规则与 Profile 适用性 |
| [04-usage](04-usage.zh-CN.md) | 用法参考：polish 的排版开关、`--suggest` 的边界、PR 上的那几个开关 |
| [themes/](themes/README.zh-CN.md) | `--theme` 的每一套色板，都用本仓库自己的卡片渲染过 |

`01`–`03` 是**对外的契约**：`03` 定义分数怎么算，`02` 定义输出结构与退出码，
`01` 解释为什么是这些取舍。改动它们等于改动使用者能依赖的东西。`04` 是 README 链出去的
用法参考，跟着 CLI 走，不冻结。

## 当前状态

v0.3.0 已发布到 GitHub Releases。22 个检查项冻结，JSON schema 冻结在 `schemaVersion: 1`。

- 语言：**Rust**（MSRV 1.88）
- 形态：**仅 CLI** + GitHub Action，不做托管服务
- 评分：纯确定性、离线优先，同一 commit 重复运行逐字节一致
- 徽章与卡片：shields.io 读取你自己仓库里的 `.repolish/badge.json`，README 里的 `<img>` 指向同目录的 `overview.svg`（顶部，讲这个项目）与 `card.svg`（末尾，讲我们打的分），我们不托管任何东西

产出文案一律英文。本目录的设计文档中英各一份，**以英文版为准**——改动先改英文，再同步中文；
代码注释仍是中文。见 [CONTRIBUTING.md](../CONTRIBUTING.md) 的第三条规则。
