# 01 · 技术架构

[English](01-architecture.md) · [中文](01-architecture.zh-CN.md)

## 为什么 Rust

- **单静态二进制**：GitHub Action 里下载即用，冷启动 <1s，无运行时依赖。scorecard 这类工具能铺开，核心原因就是这个。
- **快**：大仓库的文件遍历 + git 历史分析在毫秒级完成，可以当 pre-commit 钩子用。
- **将来可编 WASM**：同一套检查逻辑能跑在浏览器里，不必写两份。

**代价（需承认）：** LLM 生态比 Python 薄，往那个方向走要多写胶水代码。这一条可控——LLM 调用本质是 HTTP + JSON，不需要框架。真正难的是「改 Markdown 而不弄坏它」，而最后的答案**不是**本文最初设想的那个，见[核心设计三](#核心设计三markdown-改写发生在文本层)。

---

## Workspace 结构

```
repolish/
├── crates/
│   ├── repolish-core/      # Check trait、Evidence、评分聚合、Report 模型
│   ├── repolish-ingest/    # 仓库摄取：文件树、依赖清单、git 历史、语言构成、Profile 探测
│   ├── repolish-checks/    # 各检查项的具体实现
│   ├── repolish-md/        # README 解析、区块识别、行级编辑
│   ├── repolish-render/    # 终端报告 / REPOLISH.md / badge.json / SVG 卡片与录屏
│   └── repolish-cli/       # 唯一二进制
├── action/                 # composite action 定义与用法示例
├── assets/                 # 品牌文件，由 repolish-render 的 logo example 生成
├── skills/                 # 智能体技能，由 `repolish skill` 生成
├── demo/                   # 录屏所针对的样例仓库
└── docs/
```

**硬性边界：评分路径上不许出现模型。** 评分必须是纯确定性的。模型可以生成建议文本，但绝不能影响任何一个数字——分数不可复现，徽章就没有公信力。目前没有 LLM crate，将来加也不能动这条边界。

---

## crate 选型

| 模块 | 选型 | 理由 |
|---|---|---|
| CLI 参数 | `clap`（derive） | 无争议 |
| 文件遍历 | **`ignore`** | ripgrep 同款，自动尊重 `.gitignore`，`.repolishignore` 顺带就有了 |
| Git 历史 | **`gix`** | 纯 Rust，无 libgit2 / C 依赖，静态编译不破功。活跃度与 release 节奏由此取得 |
| GitHub API | ~~`octocrab`~~ → **`ureq`** | 远程需求只有一个 `GET /repos/{owner}/{repo}`。octocrab 会带进 tokio + hyper，把这个全同步的 CLI 拖成异步，也拖慢多平台静态构建。改用阻塞式 `ureq`（ring 后端，Windows 上无需 cmake/nasm） |
| Markdown | **`comrak`** | GFM 兼容，且 `sourcepos` 提供编辑层需要的行号 |
| 依赖清单 | `toml` + `serde_json` | Cargo.toml / package.json / pyproject.toml / go.mod 直接解析 |
| 错误 | `anyhow` + `thiserror` | 够用。CLI 面向用户的报错是一条条手写的 |
| 路径 | `dunce` | Windows 的 UNC 路径，否则会把 `\\?\C:\…` 原样打给用户看 |

**依赖列表刻意很短，而且比本文最初计划的更短。** 早先的草案选过 `tokei` 做语言统计、`minijinja` 做 SVG 模板、`rayon` 并行跑检查项、`miette` 做诊断、`insta` 做快照测试。它们**一个都没有进来**：

- **语言统计**就是按扩展名数文件（`repolish-ingest/lang.rs`）。卡片上写的是「by file」，数文件不需要库。
- **SVG** 在 `repolish-render/draw.rs` 里用字符串拼。模板引擎会把版式放在一种语言里、几何放在另一种里，而这些图是生成的，从来不用手写。
- **并行**始终没有需要——整次运行本来就在毫秒级。
- **诊断**走我们自己的渲染器：终端报告有固定形状（分数、类别条、点阵、发现），通用诊断库产不出这个。
- **快照测试**会把「有正当理由变化」的输出也冻住；测试改为断言不变量（自包含、确定性、不画到框外）。

以上每一条都是**没有引入**的依赖。列出来正是为了这个。

明确不引入：`axum`、`sqlx`、`moka`——没有服务端。

---

## 核心设计一：Check trait

整个引擎的地基。

```rust
pub trait Check: Send + Sync {
    fn id(&self) -> &'static str;              // "readme-quickstart"
    fn risk(&self) -> Risk;                    // Critical/High/Medium/Low → 权重 10/7.5/5/2.5
    fn requires_remote(&self) -> bool;         // 是否需要 GitHub API
    fn applies_to(&self, p: Profile) -> bool;  // 项目类型适用性
    fn run(&self, ctx: &RepoContext) -> Outcome;
}

pub enum Outcome {
    /// 已判定
    Scored { score: u8, evidence: Vec<Evidence>, fixes: Vec<Fix> },
    /// 该项目类型不需要此项（由 Profile 决定）
    NotApplicable { profile: Profile },
    /// 想查但客观查不了
    Inconclusive { reason: String },
    /// 配置或运行模式导致未执行（缺 --remote、被 --skip）
    Skipped { reason: String },
}
```

四种终态的差别在于**报告与徽章行为**，不只是语义：

| 状态 | 计入分母 | 进「未验证」 | 影响徽章 |
|---|---|---|---|
| `Scored` | ✅ | | |
| `NotApplicable` | ❌ | ❌ | ❌ |
| `Inconclusive` | ❌ | ✅ | ❌ |
| `Skipped` | ❌ | ✅ | ✅ 标注 |

把「我没检查」「这个项目不需要」「我检查了不合格」**在数据层**分开，报告才能诚实。

### 聚合算法

沿用 scorecard 的风险加权，分母只含 `Scored` 项：

```
总分 = Σ(score_i × weight_i) / Σ(weight_i) × 10
```

**不设类级权重**——双层权重会让调参的影响无法推理。类别得分仅作展示。

**分母保护：** 若 `Scored` 项的权重和不足本次运行注册权重和的 50%，不输出总分，只输出分项报告并说明原因。否则「查了三项、三项都过」会读成 100/100。

详见 [03-scoring](03-scoring.zh-CN.md)。

---

## 核心设计二：离线优先

```
repolish check .           → 纯本地，无网络无 key，秒出分数
repolish check . --remote  → 补 GitHub API 元数据
```

**默认路径零配置能跑**，这是 CLI 采用率的生死线。

**本地分与远程分基准不同**——三个远程检查项会变成 `Skipped` 并被剔出分母——二者不可横向比较。所以 `badge.json` 带 `mode` 字段，本地模式的徽章标签会写出 `(local)`。

---

## 核心设计三：Markdown 改写发生在文本层

最难的一块，而且计划在撞上现实之后改了。

最初的设计是：`comrak` 解析 → 改 AST → `format_commonmark` 序列化回去。**这条路被实测之后放弃了。** 往返是有损的——引用式链接被展平、setext 标题变 ATX、`*` 列表标记变 `-`、制表符变空格。12 个真实 README 上无损的有**零**个。那次实验现在还留在仓库里（`repolish-md/examples/roundtrip.rs`），因为它就是现在这个设计的理由。

所以 `polish` 走文本层：

1. `comrak` 解析 README，AST 只回答一个问题：**插在第几行**（`sourcepos`）。
2. 按那些行号切开原文，再拼回去。
3. 其余每一个字节都不碰——制表符、CRLF、列表标记、引用式链接定义全部原样保留。

`repolish-md` 因此是一个**只读** crate，从不产出文本；编辑在 `edit.rs` 里，形式是行插入。

**由此得到一条硬不变量：`polish` 只增量插入。** 它不能重排、不能重排序、不能删除。这不是谨慎过头——一个教人把仓库弄体面的工具，不该顺手把别人的散文重排一遍。

---

## Profile 探测

`repolish-ingest` 在摄取阶段判定项目类型（`library` / `app` / `cli` / `docs` / `collection` / `meta`），驱动检查项的 `applies_to`。判据与不适用规则见 [03-scoring](03-scoring.zh-CN.md)。

探测可被 `.repolish.toml` 的 `profile` 或 `--profile` 覆盖，且**必须在报告中显示探测结果**——否则作者会困惑于「为什么少了几项」。
