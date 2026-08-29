# 03 · 评分维度

[English](03-scoring.md) · [中文](03-scoring.zh-CN.md)

> 检查项清单已定稿于 v1 口径。具体判定阈值会在实现中微调，但**检查项集合与权重不再增删**——见文末「决策记录」。

## 权重定义

沿用 scorecard 的风险加权：

| 风险等级 | 权重 |
|---|---|
| Critical | 10 |
| High | 7.5 |
| Medium | 5 |
| Low | 2.5 |

**不设类级权重。** 三大类的得分只在报告中展示，不参与总分计算——双层权重会让「调一个数」的影响无法推理。类别间的实际占比由检查项权重自然形成（可发现性 ≈ 23%，可理解性 ≈ 34%，可信度 ≈ 43%）。

---

## 聚合规则

```
总分 = Σ(score_i × weight_i) / Σ(weight_i) × 10
```

每个检查项有四种终态，只有 `Scored` 计入分母：

| 状态 | 含义 | 计入分母 | 进「覆盖限制」 | 影响徽章 |
|---|---|---|---|---|
| `Scored` | 已判定，0–10 分 | ✅ | | |
| `NotApplicable` | 该项目类型不需要此项 | ❌ | ❌ | ❌ |
| `Inconclusive` | 想查但客观查不了 | ❌ | ✅ | ❌ |
| `Skipped` | 用户配置或运行模式导致未执行 | ❌ | ✅ | ✅ 标注 |

**分母保护：** 若 `Scored` 项的权重和不足**本次运行注册的**检查项权重和的 **50%**，不输出总分，只输出分项报告并说明原因。避免「大部分没查 → 剩下几项满分 → 100/100」这种虚高。

### 本地分与远程分不可比 —— 必须标注

`--remote` 缺失时，`repo-description` / `repo-topics` / `repo-homepage` 会变成 `Skipped`，被剔出分母。这三项占可发现性权重的一大半，**导致本地分与远程分是两个不同基准，数值不可横向比较**。

处理办法：

- `badge.json` 增加 `mode` 字段（`"remote"` / `"local"`）
- `mode = "local"` 时，徽章 label 降级为 `repolish (local)`，使读者一眼可辨
- `ci` 阶段生成的 workflow 默认带 `--remote`（Action 里 `GITHUB_TOKEN` 免费可得），因此正常路径产出的都是完整分

---

## 检查项清单

`本地` = 无需网络；`远程` = 需 `--remote`。22 项全部已实现。

### 一、可发现性 Discoverability

| id | 说明 | 风险 | 来源 |
|---|---|---|---|
| `repo-description` | 仓库 description 非空且具备信息量（非仅重复项目名） | High | 远程 |
| `repo-topics` | topics 数量合理，且与本地信号交叉验证通过 | High | 远程 |
| `repo-homepage` | 设置了 homepage 字段 | Low | 远程 |
| `readme-title-tagline` | README 首屏有项目名 + 一句话说明「这是什么」 | Critical | 本地 |
| `readme-badges` | 存在基础徽章（构建 / 版本 / 许可证） | Low | 本地 |

### 二、可理解性 Comprehensibility

| id | 说明 | 风险 | 来源 |
|---|---|---|---|
| `readme-quickstart` | 存在安装 / 快速开始区块 | Critical | 本地 |
| `readme-usage-example` | 存在可复制的代码示例 | High | 本地 |
| `readme-install-consistency` | README 中的安装命令与实际包管理器清单一致 | High | 本地 |
| `readme-link-health` | README 中的相对链接与图片指向的文件真实存在 | Medium | 本地 |
| `readme-length` | 长度适中（过短信息不足，过长应拆分到 docs/） | Medium | 本地 |
| `readme-toc` | 较长的 README 提供目录 | Low | 本地 |
| `docs-presence` | 存在 `docs/` 目录或指向文档站的链接 | Medium | 本地 |
| `readme-i18n` | 提供多语言 README | Low | 本地 | |

### 三、可信度 Credibility

| id | 说明 | 风险 | 来源 |
|---|---|---|---|
| `license` | LICENSE 文件存在且可识别为标准许可证 | Critical | 本地 |
| `claim-consistency` | README 承诺的命令 / 脚本 / API 在代码中真实存在 | High | 本地 |
| `ci-present` | 存在 CI 配置 | High | 本地 |
| `tests-present` | 存在测试目录或测试文件 | High | 本地 |
| `activity` | 近 90 天内有提交 | High | 本地 |
| `contributing` | 存在 CONTRIBUTING | Medium | 本地 |
| `issue-pr-template` | `.github/` 下有 issue / PR 模板 | Medium | 本地 |
| `release-hygiene` | 有 tag / release，且 release 带说明 | Medium | 本地 |
| `code-of-conduct` | 存在行为准则 | Low | 本地 |

**合计 22 项**，全部已实现。其中 19 项无需网络。

---

## `repo-topics` 的相关性判定

**不引入 LLM 参与打分。** 相关性拆成两个确定性信号：

**1. 数量分档**

| topics 数 | 分 |
|---|---|
| 0 | 0 |
| 1–2 | 4 |
| 3–5 | 8 |
| 6–12 | 10 |
| 13–20 | 8（堆砌，GitHub 上限 20） |

**2. 交叉验证（上述得分的上限约束）**

用三组本地信号构成期望 topic 词表：

- 主语言与次语言（按扩展名统计文件数）
- 依赖清单中的框架 / 生态名（`package.json`、`Cargo.toml`、`pyproject.toml` 等）
- README 一级标题与 tagline 中的名词

若现有 topics 与该词表**交集为空**，得分封顶 5 分，并在 `Fix` 中给出建议补充的 topic 列表（直接来自词表，无需 LLM）。

**语义相关性不做判定。** LLM 模式下可以生成更好的 topic 建议文本，但**不影响分数**——守住「评分确定性」这条边界（见 [01-architecture](01-architecture.zh-CN.md)）。

---

## 项目类型 Profile

**不按类型调整分数线**——那会让分数无法横向比较，也难以向作者解释。类型只影响**某些检查项是否适用**（`NotApplicable`，不计分母）。

探测信号：

| Profile | 判据 |
|---|---|
| `cli` | 存在可执行入口定义（`[[bin]]`、`bin` 字段、`console_scripts`） |
| `library` | 有包清单与发布配置，无可执行入口 |
| `app` | 有 Dockerfile / 部署配置，无包发布元数据 |
| `docs` | 内容以 Markdown 为主，代码占比极低 |
| `collection` | README 极长 + 大量外链 + 几乎无代码（awesome-list 类） |
| `meta` | 仓库名为 `.github`，或有 `profile/README.md` 且无任何代码 |

不适用规则（仅列例外）：

| 检查项 | 在这些 Profile 下 `NotApplicable` |
|---|---|
| `tests-present` | `docs`、`collection` |
| `ci-present` | `collection` |
| `readme-quickstart` | `collection` |
| `readme-usage-example` | `docs`、`collection` |
| `readme-install-consistency` | `docs`、`collection`，以及未探测到任何包管理器清单时 |
| `readme-length` | `collection`（M2 补充：资源集合的 README 就是内容本体，长是形态不是缺陷） |
| `docs-presence` | `docs` |
| **其余 19 项** | `meta`（见下） |

### `meta`：组织资料仓库

`OWNER/.github` 是 GitHub 用来放组织名片的地方，内容就是一张给陌生人看的
`profile/README.md`。它不是项目：要求它有 license、CI、测试、CONTRIBUTING
只会产出满屏假警报，而一屏假警报会让人开始怀疑整张表。

因此 `meta` 下**只保留三项**——恰好是「这张名片读得懂吗」的三个问题：

| 保留 | 为什么 |
|---|---|
| `readme-title-tagline` | 开头有没有说清「这是谁、在做什么」，正是名片存在的全部意义 |
| `readme-link-health` | 名片上的链接全是给陌生人点的，断了比在项目 README 里断了更难堪 |
| `readme-length` | 那张名片是不是短到什么都没说 |

实现上，`Check::applies_to` 的**默认值**是「对 `meta` 不适用」，上面三项显式覆盖回来。
默认取「不适用」而非「适用」，是因为这个方向的错是安全的：新加的检查项不会在
没人过问的情况下突然对组织名片开火。

`meta` 仓库的 README 按 GitHub 的约定在 `profile/` 下而不是根目录，摄取时会认这条约定。

**不能**用「owner 与 name 相同」判定资料仓库：`chalk/chalk`、`eslint/eslint`、
`prettier/prettier` 都是真项目。那条规则只对**用户**的同名仓库成立，而 owner
是不是用户，不打一次 API 分不出来。

**`readme-toc` 对短 README 判满分，而不是 `NotApplicable`。** 「要求已被满足」与
「不适用」是两回事：后者会把这一项从分母里剔出去，从而抬高其余项的权重占比。
短 README 是真的达标了。

探测结果可被 `.repolish.toml` 的 `profile = "library"` 或 `--profile` 覆盖。**报告中必须显示探测到的 profile**，否则作者会困惑于「为什么少了几项」。

---

## 设计原则

**1. 分档而非二元。** 每项返回 0–10 分。例：`readme-quickstart` 缺失 = 0；有标题无命令 = 4；有命令无前置条件说明 = 7；完整 = 10。二元判定会让分数跳变，作者看不到改进路径。

**2. 每条扣分必须给出可执行的 `Fix`。** 只说「你缺 X」而不说「怎么补」的检查项不做。这也是控制检查项总数的闸门。

**3. `claim-consistency` 是差异化重点。** 来自 repo-audit：README 里写的 `npm run build`、`cargo xtask`、示例中 import 的模块，代码里真的存在吗。其他工具都没做这一项。

**4. 宁可 `Inconclusive` 也不猜。** 无法可靠判定时返回 `Inconclusive` 并写明原因，进入报告的「覆盖限制」章节。误判会直接摧毁工具的可信度。

**5. 不重复 scorecard。** 安全维度（SAST、Fuzzing、签名、依赖固定等）一律不做，报告中链接过去即可。

**6. 产出一律英文。** `Evidence` / `Fix` / `Inconclusive` 的文案、终端渲染、CLI 帮助与 `init` 生成的 workflow 注释都用英文；代码注释保持中文；设计文档中英各一份，以英文版为准。`REPOLISH.md` 会被提交进陌生人的仓库，混合语言的报告没人愿意留着。反过来，识别中文 README 的能力（`section.rs` 的标题别名、中文停用词）不受影响——那是输入，不是输出。`tests/checks.rs::all_messages_are_english` 与 `repolish-cli/tests/cli_is_english.rs` 守住这条。

---

## 决策记录

| 原待确认项 | 结论 | 理由 |
|---|---|---|
| 是否加类级权重 | **不加** | 双层权重使调参影响不可推理；类别得分仅作展示 |
| `repo-topics` 相关性如何判定 | **数量分档 + 本地信号交叉验证封顶**，LLM 只出建议不参与打分 | 守住评分确定性；交叉验证已能识别绝大多数「完全没填对」的情况 |
| 是否按项目类型调整期望值 | **不调分数线，只调适用性**（`NotApplicable`） | 保持分数横向可比与可解释 |
| 检查项是否收敛到 20 项 | **定为 22 项，v1 冻结** | 项数本身不是目标；凑整会砍掉有价值的项。新增须走 minor 版本并变更 `repolishVersion`，因为它改变分数口径 |

**衍生决策：** 本地分与远程分基准不同 → `badge.json` 增加 `mode` 字段，local 模式徽章 label 标注；`Outcome` 从 2 个状态扩为 4 个；新增分母保护阈值 50%。
