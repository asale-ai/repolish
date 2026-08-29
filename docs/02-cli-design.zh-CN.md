# 02 · CLI 设计

[English](02-cli-design.md) · [中文](02-cli-design.zh-CN.md)

二进制名：`repolish`

## 命令面

**没有子命令。** 一条命令、一条流水线，而且默认干跑。

```
repolish                         # 打分，然后列出它会动的每一个文件。一个字节都不写。
repolish --apply                 # 落盘
```

两条性质撑起整个设计。**不加 `--apply` 就什么都不写**——第一次对着别人的仓库跑必须是
免费的，否则没人会跑第二次。以及**一次分析喂给所有阶段**：四个阶段共用同一份
`RepoContext` 和同一份 `Report`。分开跑意味着多打几次 GitHub API，更要命的是几份产物
可能来自不同次的评分结果。

### 阶段

`--stages` 决定跑哪几段，默认是 `check,polish,artifacts,ci,demo`。顺序即执行顺序，而顺序是
有意义的：`polish` 可能刚插入了一张卡片的引用，`artifacts` 紧接着才画得出那张图。

| 阶段 | 写什么 | 在默认流程里 |
|---|---|---|
| `check` | 什么都不写 | 是 |
| `polish` | README 插入、`CONTRIBUTING.md`、issue/PR 模板 | 是 |
| `artifacts` | `.repolish/badge.json`、横幅、两张卡片，以及 README 已经引用的每一张 SVG | 是 |
| `ci` | `.github/workflows/repolish.yml` | 是 |
| `skill` | `SKILL.md`；带 `--target` 时写进智能体自己的目录 | 否 |
| `demo` | `.repolish/demo.svg`——会**执行**它录下的命令 | 是，但不加 `--apply` 就只打印清单 |

`skill` 不在默认里是有意的：它写的文件只有用智能体的人需要。`demo` **在**默认里，但
「执行那些命令」不在——不加 `--apply` 时它只打印将要执行的清单就停下。那份清单就是
执行前的知情同意；一次默认运行不能背着作者去跑 README 里的任意命令。

```
repolish --stages check --format json      # 供 CI 消费
repolish --stages check --min-score 70     # 不达标退出码非 0 → 可当 CI 门禁
repolish --stages check --base origin/main # 也评一遍那个 ref，报出差值
repolish --stages check --sarif out.sarif  # 每条发现一条注解，落在它自己那一行
repolish --stages check --comment out.md   # 短报告，给 PR 评论用

repolish --stages artifacts --apply --artifact overview   # 只画 .repolish/overview.svg
repolish --stages artifacts --apply --artifact score      # 只画 .repolish/card.svg
repolish --stages artifacts --apply --artifact tables     # 重画每一张已包过的表
repolish --stages artifacts --apply --remote --stars      # 加上 star 增长曲线

repolish --stages skill --list             # 这台机器上装了哪几家智能体
repolish --stages skill --target detect --apply   # 装进探测到的那几家
repolish --stages demo                     # 只列出它会跑哪几条命令，什么都不执行
repolish --stages demo --apply --tape      # 录制，并写一份 VHS tape 给想要 GIF 的人

repolish --apply --visuals                 # 概览卡片、末尾分数卡片、SVG 表格
repolish --suggest                         # 请模型写它写不了的那三段；从不落盘
```

**`artifacts` 覆盖，`polish` 不覆盖。** 这个分工是有意的，也是这两段之间唯一需要记住的
事：`polish` 负责第一次把 `<img>` 引用插进 README，此后一次都不再动那个文件；
`artifacts` 负责每一次重画。反过来的话，要么 `polish` 破了「从不覆盖」这条不变量，
要么 README 上永远挂着第一次生成的那张图。

这也是为什么 `artifacts` 的判据是**「是不是已经被引用」**而不是一个开关：给一张没人引用
的表生成 SVG，落下的是一个孤儿文件——它会被提交、被一直带着，而没有任何东西指向它。
真想单独画一张时用 `--artifact` 覆盖这个判据。

### `--output` 和 `--stdout` 什么时候合法

每张产物的默认路径都不同，所以这两个参数只在**恰好产出一件东西**时才说得清：一个
`--stages`，并且——对 `artifacts` 而言——一个 `--artifact`。其余情况一律报错，不去猜。

**优先级说明：** `--min-score` 与 `ci` 阶段的优先级高于任何生成类功能——CLI-only 产品的留存靠「进了 CI 就不会被删」。

## 全局参数

| 参数 | 说明 |
|---|---|
| `--format <text\|json\|markdown\|sarif\|comment>` | 默认 `text` |
| `--config <path>` | 默认读 `.repolish.toml` |
| `--profile <auto\|library\|app\|cli\|docs\|collection\|meta>` | 默认 `auto`，覆盖类型探测结果 |
| `--only <ids>` / `--skip <ids>` | 按 check id 过滤（被过滤项状态为 `Skipped`） |
| `--stars` / `--no-stars` | star 增长曲线。`--remote` 下默认就拉，前提是有 token **且**这次会画概览卡——曲线没有别的去处，而匿名时它会把 60/小时配额的五分之一花在一段装饰上 |
| `--no-color` | CI 环境 |
| `-v` | 展开全部检查项、通过清单，以及每个新文件的完整内容 |
| `--apply` | 落盘。不加它，除 `--sarif` / `--comment` 外什么都不写 |
| `--force` | 覆盖已存在的文件，并允许在非 git 目录下写 |
| `--stages <list>` | 默认 `check,polish,artifacts,ci,demo` |
| `--artifact <list>` | 把 `artifacts` 阶段限定到 `badge`、`report`、`hero`、`overview`、`score`、`tables` |
| `--no-visuals` | 不动 README 的视觉产物。卡片与 SVG 表格默认是开的 |

`--sarif` 和 `--comment` 是 `--apply` 的两个例外：点名一个输出路径本身就是请求，
而且它们默认都不往仓库自己的树里写。

除 `text` 外的每一种格式下，**stdout 只有那份报告**，所有过程性输出走 stderr——所以
`repolish --format json | jq` 在完整流水线下也是通的，不只是 `--stages check`。

## 退出码

| 码 | 含义 |
|---|---|
| 0 | 成功；若指定 `--min-score` 则表示达标 |
| 1 | 分数低于 `--min-score` |
| 2 | 参数错误 / 配置错误 |
| 3 | 目标不是有效的 git 仓库 |
| 4 | 远程调用失败（`--remote` 时 API 错误或配额耗尽） |
| 5 | 有效检查项覆盖不足 50%，无法给出总分 |
| 7 | `--base` 的基线取不到：浅克隆、ref 不存在、PATH 上没有 git |

工具自身的运行失败与「检查不通过」必须用不同退出码区分，否则 CI 里无法判断。

1 是「检查没通过」那一档。7 与 4 一起站在这条线的另一侧——一个从未 fetch 过基线的
浅克隆不是质量退步，也绝不能被报成质量退步。

6 已废弃。它原本表示 `verify --run` 起不来容器；`verify` 已移除，这个值**刻意留空
不再复用**，因为老版本写的脚本里可能还留着对 6 的判断。

---


## `--base`

基线检出到一个**临时 `git worktree`** 里再评一次。不用 `git stash` 或 `git checkout`：
可能正有人在改那份 README，为了算一个数字去动他的文件是不可接受的。

也不去读基线上那份提交进仓库的 `.repolish/badge.json`。那个文件只有一个总分，
答不出「哪一项动了」——而那是评审的人唯一需要的信息。它还是个提交进仓库的产物：
一份忘了更新的 badge.json 会让差值凭空出现。

基线用**与 head 相同的 `RunOptions`** 跑，并把 head 已经取到的 `RemoteFacts` 复制过去。
描述、topics、homepage 是**仓库**的属性而不是 commit 的属性，重新拉一次是花配额买同一个答案；
更要紧的是不复制过去的话，基线会悄悄退化成 local 模式——两个不同的分母相减。

目标是子目录时（`repolish demo/sample --base …`），子路径要在检出里接回去。
不接的话评的就是仓库根：一份看起来完全正常、内容全错的差值。

`delta` 以 `skip_serializing_if` 加进 JSON，所以不给 `--base` 时这个键根本不出现，
v1 的消费方看到的是逐字节相同的文档。**加字段是允许的**，改含义和删字段才需要
递增 `schemaVersion`。

---

## `polish --suggest`

repolish 里唯一会跟模型说话的地方，而它不在评分路径上。

「评分路径无模型」是一条关于**评分**的规矩（见 [01-architecture](01-architecture.zh-CN.md)）：
同一个 commit 必须永远得到同一个数字。把它延伸到**修复**上是个错误，
那等于把这个工具最有价值的一半让了出去——权重最高的三项
（`readme-title-tagline` Critical、`readme-quickstart` Critical、`readme-usage-example` High）
恰好全是机械规则满足不了的。

所以边界画在别处，而且更死：

- **从不落盘。** 连 `--apply` 都不写。它打印，作者自己贴。
- **只有那三项。** 把其余 19 项交给模型产出的是噪声：它们要么是机械的（`polish` 已经在做），
  要么是「去做一件事」而不是「写一句话」。
- **只问没拿满分的。** 给一个已经满分的 tagline 再生成一个「更好的」，
  是这条功能最先会走偏的地方。
- **编不出东西。** 提示词里带着真实的包清单、真实的可执行文件名、真实的脚本名，
  并要求缺事实就留空并说明，绝不硬编。一条编造的安装命令正是 `claim-consistency` 要抓的。

提示词的构造与回答的解析都是纯函数，各自带单元测试——那段文字决定了产出有没有用，
它值得像代码一样被盯着。

模型默认 `claude-opus-5`，可用 `.repolish.toml` 的 `[suggest] model` 改。
密钥取自 `REPOLISH_LLM_API_KEY` 或 `ANTHROPIC_API_KEY`——**绝不**从配置文件读，
那是一个会被提交进仓库的文件。

---

## SARIF

`--format sarif` / `--sarif <path>`，SARIF 2.1.0。

每一条扣分本来就带着文件和行号。SARIF 只是让 GitHub 把它们渲染进 PR 的 diff 里，
而不是留在一段没人展开的日志中。

- **不产生时间戳。** 同一个 commit 逐字节相同，与其余所有产物一个规矩。
  SARIF 允许 `invocation.startTimeUtc`，我们不写。
- **只有扣分才成为 result。** 把通过项也放进去，PR 上就会出现 22 条注解，其中 19 条说一切正常。
- **所有检查项都声明为 rule**，这样即使通过的项，Security 标签页里也有元数据可看。
- 仓库级证据（`file: "."`）锚在 `README.md`，并带 `region.properties.wholeFile`——
  指向仓库根的 location，GitHub 不会渲染。
- `partialFingerprints` 让一条发现在文件被编辑后仍被认作同一条，而不是「关掉一条、新开一条」。

---

### star 增长曲线

`--stars` 在概览卡片上加一条 star 增长曲线。它是 repolish 里唯一一处代价超过一次 API 请求的功能，所以只在**看得见、而且划得来**的时候才拉：`--remote` 下、有 token、并且这次运行真的会画概览卡。匿名时它会把 60/小时配额的五分之一花在一段装饰上；而 Action 默认根本不画概览卡——两种情况下那十几次请求都什么也买不到。`--stars` 可以强行覆盖这两个条件，`--no-stars` 则是退出。

**GitHub 没有「历年 star 数」这样的接口。** 但它有 `/stargazers`——带上 `Accept: application/vnd.github.star+json` 之后，它按**加星时间升序**返回，每条带 `starred_at`。于是第 *k* 页的第一个人，就是这个仓库第 *(k-1)×100+1* 颗星落下的那一刻。抽十几页就得到十几个**精确**的点，唯一近似的是点与点之间那几段直线。

三条推论值得写下来：

- **曲线的原点是仓库创建的那一刻，星数为 0。** 这不是补出来的假点——那一刻它确实是零星的。加上它，左边缘才是真正的起点，而不是「第一颗星」这个已经不为零的位置；而且只有一颗星的新仓库也画得出曲线，而那恰好是最想看这条曲线的人。
- **最后一个点取最新那位 stargazer 的 `starred_at`，不取「现在」。** 这样曲线完全由远端状态决定，同一份状态渲染出同一个文件。用时钟的话，每次跑都会画出一条略微不同的尾巴。
- **横坐标是时间，不是样本序号。** 抽样在页上是均匀的，而 star 不是均匀涨的；按序号画会把沉寂的一年和爆火的一周画成同样宽。
- **GitHub 把名单限制给了 admin 与 collaborator。** 2026 年 7 月起，starring 相关接口只对有仓库权限的人开放，其余一律 404，未登录则 401。这绕不过去，所以取不到时会把原因报出来，而不是在卡片上留一块空白。实际代价不大：repolish 本来就是给**你自己的**仓库打分的，而你自己的仓库你是 admin。
- **分页上限是 400 页。** 超过四万颗星的部分取不到，曲线就从数据开始的地方开始，而不是假装取到了。

取曲线失败返回空而不是报错：它是卡片上的一段装饰，不该把「配额用完了」变成「评分失败」。点数不足两个就整节不画——一个空的图表框读起来像「这个项目一颗星都没有」。

---

## 输出契约

### `.repolish/badge.json`

遵循 [shields.io endpoint 协议](https://shields.io/badges/endpoint-badge)：

```json
{
  "schemaVersion": 1,
  "label": "repolish",
  "message": "88/100",
  "color": "brightgreen",
  "repolishVersion": "0.3.0",
  "mode": "remote"
}
```

配色阈值：`>=90` brightgreen，`>=75` green，`>=60` yellow，`>=40` orange，`<40` red。这些数字来自 `repolish_core::band_index`，全仓库只有那一处。

`repolishVersion` 与 `mode` 为非标准字段，shields.io 会忽略。

**`mode` 的作用：** 本地分与远程分基准不同（三个远程检查项会被剔出分母），数值不可横向比较。因此：

- `mode = "local"` 时，`label` 必须降级为 `repolish (local)`，使读者一眼可辨
- `ci` 阶段生成的 workflow 默认带 `--remote`（Action 里 `GITHUB_TOKEN` 免费可得），正常路径产出的都是完整分

覆盖不足 50% 时不生成 `badge.json`，并以退出码 5 提示。

### 徽章 snippet

只选了 `artifacts` 阶段时，它还会打印：

```markdown
[![Repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/OWNER/REPO/BRANCH/.repolish/badge.json)](https://github.com/OWNER_OF_REPOLISH/repolish)
```

OWNER / REPO / BRANCH 从 git remote 与当前分支推断，推断失败则提示用户手填。

### `--format json`

**已冻结（`schemaVersion: 1`）。** 字段只增不改；删字段或改含义必须递增 `schemaVersion`：

```json
{
  "repolishVersion": "0.3.0",
  "schemaVersion": 1,
  "repository": { "owner": "...", "name": "...", "commit": "..." },
  "profile": { "detected": "cli", "overridden": false },
  "mode": "remote",
  "score": 88,
  "coverage": 0.774,
  "categories": [{ "category": "discoverability", "score": 96 }],
  "checks": [
    {
      "id": "readme-quickstart",
      "risk": "critical",
      "status": "scored",
      "score": 10,
      "evidence": [{ "file": "README.md", "line": 42, "note": "..." }],
      "fixes": [{ "severity": "P2", "message": "...", "autofixable": true }]
    },
    { "id": "tests-present",  "status": "not_applicable", "profile": "docs" },
    { "id": "repo-topics", "status": "skipped", "reason": "requires --remote" },
    {
      "id": "readme-install-consistency",
      "status": "inconclusive",
      "reason": "no install commands in the README to compare"
    }
  ],
  "coverageLimits": [
    "repo-topics: requires --remote",
    "readme-install-consistency: no install commands in the README to compare"
  ]
}
```

`status` 取值：`scored` / `not_applicable` / `skipped` / `inconclusive`（对应 [01-architecture](01-architecture.zh-CN.md) 的 `Outcome`）。

`coverageLimits` 是顶层字段，收纳 `skipped` 与 `inconclusive` 两类，强制报告消费方看到「哪些没验证」。`not_applicable` 不进此列表。

### `REPOLISH.md`

固定结构：

1. 总分 + 探测到的 profile + 三大类得分（可发现性 / 可理解性 / 可信度）
2. P1 / P2 / P3 分级发现，每条带**文件 + 行号**证据
3. 已验证通过的清单
4. **覆盖限制**：未能验证的项及原因
5. 页脚：`Generated by [repolish](https://github.com/OWNER_OF_REPOLISH/repolish) vX.Y.Z on <commit>`

### `.repolish/card.svg`

可以直接 `<img>` 进 README 的报告卡片。和 `badge.json` 是同一套分发模型——文件在用户自己的仓库里，由他自己的 raw URL 提供，我们不托管任何东西——区别只是徽章写得下一个数字，卡片写得下扣分在哪。

内容自上而下：品牌标记 + wordmark / profile · mode｜大号总分 + 判词 + 三条类别分｜22 项检查的点阵｜最多 3 条发现｜仓库 slug · commit · 版本号。

三条硬约束：

| 约束 | 为什么 |
|---|---|
| **自包含** | 不引外部字体、脚本、远程图片。分数与 wordmark 走点阵转矩形——读者机器上有没有某个字体不由我们决定 |
| **确定性** | 无时间戳、无随机数。同一个 commit 逐字节一致，否则 CI 每次都提交一堆只有噪声的 diff |
| **色板恒定** | 不做 `prefers-color-scheme`：GitHub 把 SVG 当图片经 camo 代理渲染，媒体查询在那条链路上不可靠 |

`assets/` 下的 logo 与 wordmark 由 `cargo run -p repolish-render --example logo` 从**同一段几何**生成。手写两份，改了一处忘另一处，两个月后就是两个 logo。

### `.repolish/overview.svg`

**概览卡片，说的是被检查的那个项目，不是我们的分数。** 内容：项目名与一句话、按文件数的语言构成、代码/文档/配置/其他的堆叠比例、一年的每周提交活跃度、许可证、最新 tag，以及 `--remote` 下才有的星标与主题数。

位置在 README 的**顶部**，徽章下面。分数卡片（`.repolish/card.svg`）在**末尾**。这一条早期弄反过：一个陌生人点进你的仓库，第一眼该看到的是这个项目做什么，不是我们给它打了几分。分数卡片的读者其实是作者自己，以及顺着它找过来的下一个作者——那个位置在页面末尾更合适，此时读者已经决定了要不要用这个项目。

活跃度图的窗口**终点是 HEAD 的提交时间，不是「现在」**。按当前时间开窗的话，一个停更两年的仓库会画出一条整齐的零线——那看着像没数据，而不像停更；同时它也会破坏「同一个 commit 逐字节可复现」这条约束。

### `.repolish/tables/*.svg`

README 里每一张表格画成的图。`--stages polish --tables svg` 第一次生成并插入引用，`--stages artifacts --artifact tables` 此后重画。

**原表格必须留着**，折进紧挨在图下面的 `<details>` 里。图片没有文本层：读屏软件、`grep`、翻译工具，以及下一个想改这张表的人，读的都是折起来的那一份。这不是可选项。

包起来这件事仍然是**纯插入**——表格自己的字节一个都没动，只是在它上下各加了几行，`polish` 的「只增量插入」不变量因此完好。

选表规则：少于 2 行不画（画成图没有增益），超过 16 行不画并且会说一声（那么高的图在手机上看不清，而真表格本来就会滚动）。文件名里的序号取自**全部**表格的下标而不是入选表格的下标——跳过一张之后如果后面跟着重编号，README 里已经写好的引用就全断了。

### `.repolish/demo.svg`

`demo` 阶段的产物：一段**会动的终端录屏**，动画由 CSS 关键帧驱动。仅当探测到可执行文件、或用 `--cmd` 显式指定了命令时才有意义。

**录制与渲染都在这个仓库里。** 早先的做法是生成一份 [VHS](https://github.com/charmbracelet/vhs) tape 交给用户自己渲染，但 VHS 要 ttyd 和 ffmpeg，产出的是 GIF，而 GIF 与本仓库对自己每一个产物的三条约束全不相容：二进制大块会撑肥 git 历史；没有文本层意味着录屏里那行命令复制不走、`grep` 也找不到；而要求使用者先装一条视频工具链，对一个「让你的仓库体面起来」的工具来说是本末倒置。`--tape` 仍然保留，因为不是每个包平台都渲染 SVG（crates.io 渲染，npm 与 PyPI 对 README 的 HTML 消毒更狠）。

**它真的会跑那些命令。** 这是刻意的：一个专门检查「README 承诺的命令是否真的存在」的工具，自己的演示不能是编的。代价是它会在使用者的机器上执行程序，所以不加 `--apply` 就什么都不跑：这一阶段先把将要执行的命令清单打印出来就停下。那份清单就是执行前的知情同意，`--help` 里也这么写。

两处硬限制，写在 `repolish-render/src/cast.rs` 上：

- **不是终端模拟器。** 只认 SGR 颜色、`\n` 和 `\r`。会重绘屏幕的程序（进度条、spinner、全屏 TUI）录出来是不对的。认全就是在写一个 vt100，而这个 crate 的工作是画卡片。
- **不带伪终端。** 输出接的是管道，所以用 `CLICOLOR_FORCE` 与 `FORCE_COLOR` 强制开色；仍然坚持关色的程序录出来就是黑白的。拉一个 PTY 依赖进来能救最后那一小撮程序，代价是一个平台相关的依赖和 Windows 上另一条实现路径——为一个演示功能不值当。

**第 0 帧是「跑完之后的样子」。** 时间轴开头先定格在最后一步的终态，然后才从第一步开始敲。这是给那些把 SVG 冻在第 0 帧的渲染器兜底的——真存在这样的环境，而如果第 0 帧是一个空终端，那些读者拿到的就是一张白图。`prefers-reduced-motion` 是另一条兜底，两条针对两种情况，都需要。

**它不进每次 push 的 CI。** 录屏里带着样例仓库的 commit 哈希；把提交时间写死好让哈希稳定，那个仓库就变成一年多没人动的仓库，`activity` 判 P1 盖过演示本身，而且报告里会出现每天都在变的「last commit N days ago」。做不到既确定又与时间无关，所以它跟着内容变、由 `demo` workflow 手动重录。理由完整写在 `demo/README.md`。

### `SKILL.md`

`skill` 阶段写出的智能体说明。内容写死在 `crates/repolish-cli/src/skill.md`（编译进二进制），仓库里提交的那一份在 `skills/repolish/SKILL.md`，由脚本重新生成——要改内容，改前者。

两种落点，语义不同：

- `repolish --stages skill --apply` 写进**一个仓库**（`SKILL.md`），跟着代码走，谁 clone 谁就有。
- `repolish --stages skill --target claude --apply` 写进**这台机器上的智能体**（`~/.claude/skills/repolish/SKILL.md`），装一次所有项目都用得上。`--target detect` 只装进真的存在的那几家——往一个没装 Codex 的机器上写 `~/.codex/skills` 会凭空造出一个目录，看着像那工具装了。

它的重点不是「有哪些命令」（`--help` 就写着），而是**顺序和边界**：先量，再落实能机械落实的，把需要判断的交回给人。一个智能体拿到「把这个仓库的 README 弄好」时的默认动作是直接重写整个文件——那正是这个工具花了全部力气去反对的做法。

文件里还有一节讲**判断**：repolish 自己的三种判法（事实、交叉核对、分档关键词启发式）哪一种弱，以及每一条发现「好的修法长什么样」与各自的翻车方式。分数量的是「读者需要的那套东西在不在、承诺是不是真的」，不量文字写得好不好——那个缺口正是智能体要补的。

### SVG 的语言

终端报告与 `REPOLISH.md` 一律英文：那是给作者自己看的诊断输出，读它的人正在用一个英文 CLI。

**SVG 不一样。** 卡片会被贴进**别人的 README**，被那个项目的读者看到。一张中文 README 顶上写着 `LANGUAGES · BY FILE` 的卡片，是我们把自己的语言塞进了别人的门面。所以卡片上的每一个字走 `repolish-render/src/i18n.rs` 的文案表，`--lang` 默认 `auto`——**读 README 判断，不读系统 locale**：CI 里一次 `LANG=C` 就把中文 README 顶上的卡片换成英文，是很荒唐的。

判据分两步，顺序不能反。**先看假名**：平假名与片假名只出现在日文里，而一份真的日文 README 不可能一个假名都没有，所以它是区分日文与中文的那条线——汉字回答不了这个问题，因为中日共用汉字。假名不够，才轮到 CJK 占比去分中文与英文。

那条占比线是三分之一。不用「有没有 CJK」判：一份中文 README 里夹着英文命令名是常态，那样判会把几乎所有 README 都判成中文；也不用「过半」判：一个汉字承载的信息量远大于一个拉丁字母，等量对比会永远判成英文。假名的门槛低得多（5%），因为英文 README 里引用一个片假名的名字不该翻盘，而成段的日文一出现就立刻过线。

文案表是一个**结构体**而不是查表函数：少一个字段编译就过不去，翻译漏一条不可能溜进发布版。

### 色板

SVG 有十四套完整色板，终端只有一套。区别在于**谁画底**：终端的底色不由我们决定，只能挑一组在深浅底上都立得住的前景色；SVG 自己画底，所以能按真实对比度来定——正文按 WCAG AAA（7:1），弱色按 AA（4.5:1），两条都有测试守着。每一套都是**完整**色板而不是「默认色板加几个覆盖」——半套色板迟早会漏掉一个常量，在卡片某处留下一行看不见的字。

`porcelain` 的存在理由是可读性而不是口味：一张深色卡片贴进一份浅色 README，在页面上就是一块挖空。另外十套的理由是：卡片是贴进**别人的版面**的，而那一页本来就有自己的温度——GitHub 自己的蓝灰（`slate`）、暖色终端（`ember`）、工程制图蓝（`blueprint`）、报纸（`newsprint`）。其中四套回答的不是口味问题：`okabe` 在红绿色觉异常下仍能区分；`phosphor` 的序列色只靠明度分层；`carbon` 与 `paper` 则完全没有色相、也没有渐变，卡片去色前后是同一张图——影印机上、电子墨水上、纸上都一样。每一套都用本仓库自己的卡片渲染在 [docs/themes](themes/README.zh-CN.md) 里。

**色板不改变分数。** 它只是给同一组数字挑颜色。

**不做 `prefers-color-scheme` 切换。** GitHub 把 SVG 当图片经 camo 代理渲染，媒体查询在那条链路上并不可靠——要浅色版就显式选，让文件本身就是浅色的。

### 产出语言

`reason` / `note` / `message` 一律英文，见 [03-scoring](03-scoring.zh-CN.md) 设计原则 6。
上面示例里的字符串是实际产出的原文。

### 终端输出

默认只展示总分、三条类别分、22 项检查的点阵、P1 / P2 与覆盖限制，`-v` 展开全部检查项与通过清单。

配色与 SVG 卡片同源（`repolish-render` 的 `theme` 与 `glyph` 两个模块），终端里长什么样，README 里就长什么样。分数的分档颜色与徽章颜色在同一组阈值上翻面（90 / 75 / 60 / 40）——同一个仓库不能有两种说法。

色彩按终端能力降级，转义序列全部经由一个 `Pen`，避免某处漏了降级在 16 色终端上吐出裸的 truecolor 序列：

| 级别 | 判据 |
|---|---|
| 关色 | `--no-color`、`NO_COLOR`、`TERM=dumb`，或 stdout 不是 tty 且未设 `CLICOLOR_FORCE` / `FORCE_COLOR` |
| truecolor | `COLORTERM` 含 `truecolor` / `24bit`，或 `WT_SESSION` / iTerm / VS Code |
| 256 色 | `TERM` 含 `256` |
| 16 色 | 其余有 `TERM` 的情况 |

不是 tty 就默认关色：否则重定向到文件的输出会带一串转义序列。Windows 的控制台默认不解释 ANSI，`main` 在第一次输出前会打开 VT 模式。

类别条在终端和卡片上都切成 12 段，且**向下取整**——连续条在高分区没有信息，99 和 100 差不到 4 个像素，谁也看不出来；切段之后那一格空缺就是给人看的。

---

## 配置文件 `.repolish.toml`

仓库根目录，或用 `--config <path>` 指定。全部字段：

```toml
profile   = "library"      # 覆盖类型探测；等价于 --profile
min_score = 70             # 等价于 --min-score

[checks]
only = []                  # 非空时只跑这些 id
skip = ["code-of-conduct"]

# polish 插入物的排版。全部有同名命令行开关，命令行优先。
[readme]
badge-style = "flat"       # flat | flat-square | plastic | for-the-badge | social
align       = "left"       # left | center
toc-style   = "bullet"     # bullet | number | roman | fold
logo        = "assets/hero.svg"
logo-width  = "full"       # 像素数，或 "full" → width="100%"
tree-depth  = 2            # 缺省 = 不生成目录树
theme       = "dark"       # SVG 产物的色板，共 14 套，见 docs/themes/
lang        = "auto"       # auto | en | zh-CN | ja，SVG 里那些字的语言
overview    = true         # 徽章下面插一张概览卡片
footer-card = true         # 末尾插分数卡片与「用 repolish 打磨」一节
tables      = "svg"        # keep | svg
```

`[readme]` 这一段**不影响任何一个分数**。检查项清单与权重在 v1 冻结，一个仓库不能靠换徽章样式让自己好看一点——那样分数就不可横向比较了。

三条实现上的硬约束，都是在真实 README 上验出来的：

| 约束 | 为什么 |
|---|---|
| logo 的 `alt` 必须为空 | 非空 alt 会让这张图成为标题候选，图片标题把 `readme-title-tagline` 从 10 打到 5。空 alt 同时是正确的无障碍语义：旁边已有文字标题，这张图是装饰性的 |
| logo 块结尾必须空一行 | 图片块是 HTML，紧跟其后的 Markdown 会被并进那个块。少这一行，下面的 `# Name` 就不再是标题，实测 10 分掉到 6 分，标题被认成正文第一个小节 |
| 追加到已有徽章行时只能用 Markdown | 那一排是作者用 Markdown 写的，混一行 HTML 进去会在渲染上留下接缝。只有另起一块时才谈得上 `align` |
| 通栏横幅要 `logo-width = "full"` | 钉死在一个像素宽度上的横幅，在宽屏里缩在左上角，在窄屏里撑破版心。`full` 输出 `width="100%"`，配的图 viewBox 也得是通栏比例——一张 450×56 的 wordmark 按 100% 拉开会变成一条横穿页面的巨型字 |

`badge-style` 不指定时**跟着 README 里已有的徽章走**（取出现最多的那种）。一排徽章里混进一个样式不同的，比样式统一但不是默认样式更难看。

`logo`、`tree-depth`、`overview`、`footer-card`、`tables` 这五项**不由任何一条检查驱动**——没有哪一项检查要求 README 里有横幅、目录树或图表。它们默认关闭，`polish` 的干跑输出里也照实写「由配置要求」（requested by configuration），不打扮成一条修复。命名这个例外，比假装它们也是修复要诚实。命令行上 `--visuals` 是后三项的简写。

**优先级：命令行 > 配置文件 > 默认值。** 命令行永远赢——CI 里能临时改的只有那一行。

两条刻意的限制：

- **未知键直接报错**，不静默忽略。打错一个键名却什么都没发生，比报错更糟：使用者会以为配置生效了。报错会指出是哪个键，并列出合法值。
- **不开放逐检查项的阈值。** 检查项清单与权重在 v1 冻结（见 [03-scoring](03-scoring.zh-CN.md)）；允许每个仓库自己调阈值，等于让分数在仓库之间不可比，而那正是这个工具存在的理由。`[checks.readme-length] min_words = 150` 这样的写法会被当作未知键拒绝。
- `--config` 指向的文件必须存在，找不到是错误。使用者以为自己指定了一份配置，静默回退到默认值会让他拿到一个解释不了的分数。

---

## install.sh

一行安装脚本。它解析最新发布、下载对应平台的归档、核对旁边那个 `.sha256`、原子地装进 `~/.local/bin`（先写临时名再改名——就地覆盖一个正在运行的二进制会把它截断），然后让二进制自己去装智能体技能，这样脚本永远不必知道哪家智能体把技能放在哪。

| 变量 | 缺省 | 作用 |
|---|---|---|
| `REPOLISH_VERSION` | 最新发布 | 装指定 tag，例如 `v0.5.0` |
| `REPOLISH_BIN_DIR` | `~/.local/bin` | 二进制装到哪 |
| `REPOLISH_TARGET` | `detect` | 技能装给谁：`detect`、`all`、`none`，或某一个 id |
| `REPOLISH_NO_SKILL` | 未设 | 设成 `1` 则只装二进制 |

用 POSIX `sh`：它要能在 Alpine、精简 CI 镜像和 macOS 那个上古 bash 上跑——不用数组、不用 `[[ ]]`、不用进程替换。

Linux 版只有 glibc 构建，所以脚本探测到 musl 就直说并停下，而不是装一个第一次运行就报链接错误的二进制。归档名是与 `release.yml` 的契约：`repolish-{tag}-{target}.tar.gz`，tag 保留 `v` 前缀。

---

## GitHub Action

composite action 定义在**仓库根目录的 `action.yml`**（`uses: owner/repo@ref` 只认根目录），
`action/` 下放用法示例。它下载对应平台的二进制并执行，比 Docker action 快一个数量级。

为省掉重复的 API 调用，action 只发起**一次**调用——需要写东西时是 `--stages
check,artifacts --apply`——让徽章 JSON、两张卡片和报告都出自同一次评分；分开跑意味着
几份产物可能来自不同次的评分。徽章是默认写的，action 的 `badge` 输入关掉时它加的是
`--no-badge`。

`ci` 阶段生成的 workflow 模板：

```yaml
name: repolish
on:
  push:
    branches: [main]
  schedule:
    - cron: '0 0 * * 1'

jobs:
  score:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
        with:
          # 默认的 fetch-depth 1 一个 tag 都拉不到，release-hygiene 会因此失效
          fetch-depth: 0

      # remote 与 badge 默认开启；args 作为逃生舱可覆盖全部开关
      - uses: asale-ai/repolish@v0.5.0
        with:
          min-score: 60
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      - name: Commit badge
        run: |
          git config user.name  github-actions
          git config user.email github-actions@github.com
          git add .repolish/badge.json
          git diff --staged --quiet || git commit -m "chore: update repolish score"
          git push
```

模板有两个默认值不能改：`fetch-depth: 0`（否则 `release-hygiene` 在 CI 里永远
判不了）与 action 默认开启的 `--remote`（Action 里 `GITHUB_TOKEN` 免费可得，
没有理由产出基准更窄的本地分）。

Action 内部还需把分数写入 `$GITHUB_STEP_SUMMARY`，使每次运行页顶部展示健康度卡片。

想要自动开 PR 的用户，在模板中改用 `peter-evans/create-pull-request` 承接 `--apply` 那一次运行的产物。

## `polish`

把**能机械落实**的建议直接落地。默认只打印，`--apply` 才落盘。

两条硬边界：

1. **对 README 只增量插入，不重写任何已有内容。** 产出的 diff 必须全是新增行，
   别的行改动一个字节都算 bug。
2. **新文件只新建，从不覆盖。** 目标路径已存在就跳过——哪怕里面只有一行，
   那也是作者写的。

这不是保守，是被验出来的。`comrak` 的 `parse_document` → `format_commonmark`
往返在 12 个真实 README 上 **0/12 无损**：引用式链接被展平（serde 底部那张
徽章 URL 表整个消失）、setext 标题变 ATX、`*` 列表标记变 `-`、制表符变空格。
ripgrep 541→466 行，axios 2851→2839 行。见 `crates/repolish-md/examples/roundtrip.rs`。

所以实现走文本层：AST 只回答「插在第几行」（`sourcepos`），原文按行切开、
拼入、接回，行尾跟着锚点行走。见 `crates/repolish-md/examples/locate.rs`，
15 份真实 README 上 15/15 只多出插入的那几行。

### 目前能落实的

| 改动 | 触发条件 |
|---|---|
| 插入 repolish 徽章 | README 里还没有，且能算出仓库 slug，且覆盖率够得上出徽章 |
| 插入目录 | `readme-toc` 扣了分，且正文最浅一层有 4 个以上标题 |
| 写 issue 表单（bug / feature） | `issue-pr-template` 扣了分，且 `.github/` 下一个 issue 模板都没有 |
| 写 `pull_request_template.md` | 同上，且该文件不存在 |
| 写 `CONTRIBUTING.md` | `contributing` 扣了分，根 / `.github/` / `docs/` 三处都没有，**且探测得出包生态** |

所有触发条件都**读检查结果**，不另写阈值。「多长算长」是 `readme-toc`
定义的，polish 这边再写一遍迟早会漂。

### 什么可以生成，什么不可以

收进来的标准只有一条：**内容能从仓库里已有的事实推出来，不需要猜。**

- issue / PR 模板是纯脚手架。GitHub 自己的表单 schema，问的是版本号、复现步骤、
  改了什么——没有一处是项目特有的，因此没有可猜的余地。
- `CONTRIBUTING.md` 里的构建与测试命令来自**探测到的包清单**：Cargo 就写
  `cargo build` / `cargo test`，npm 则只有 `package.json` 里真有 `test` 脚本时
  才写 `npm test`。**探测不出生态就不生成**——与其写一份
  `<your build command here>`，不如让这一项继续扣分：一份糊弄过检查项的
  贡献指南会让分数变绿而问题还在原地。
- **不生成行为准则。** Contributor Covenant 是标准文本，唯一项目特有的是举报
  邮箱，而那个推不出来。一份留着占位符的行为准则，承诺了一条并不存在的举报
  通道，比没有更糟。

干跑时每个新文件都会连同**它对应的那条检查结果**一起打出来。没有理由的新文件
不该出现在别人的仓库里。

覆盖率不足时**连徽章文件都不写**——往别人 README 里插一个指向不存在文件的
链接，比不插更糟。同理，`.repolish/badge.json` 不存在时会一并写出。

### 徽章插在哪

按可信度依次：

1. 开头 40 行内**已有的**那排徽章之后（徽章最多的那一段；并列取靠前的）
2. 那排徽章若是 HTML 块，插入前必须空一行——紧跟 HTML 块的 Markdown 会被
   并进那个块，徽章根本不会被解析成图片（flask、fzf 都栽在这里）
3. 没有徽章行时，插在标题之后（空一行）
4. 连标题都认不出来就**什么都不做**

「一排徽章」的判据是：段落里只有图片，且**至少有一张被 `<a>` 或 `[]()` 包着的
徽章图**。裸图是 logo 或截图——ripgrep 正文里那张截图、flask 开头那张 logo，
都是「只含图片的段落」，挂上去就跑到正文中间或标题前面了。判据和 `title.rs`
用的是同一条：真 logo 不会是超链接。

### 已知落点不理想

axios 与 awesome 的 README 开头是几百行 HTML（赞助商表格、居中 hero），
标题节点本身就横跨到 421 / 77 行，徽章会落在那一大块之后。位置合法、
文档结构不受损，但离首屏很远。这两个是 `fetch-fixtures.sh` 里特意留着的样本。

### 安全边界

- 默认不落盘。`--apply` 才写。
- `-v` 打印每个将被创建的文件的**完整内容**。README 的每一行插入本来就看得见，
  整个新文件却只报一个路径，是说不过去的：落进别人仓库的东西，落盘前该能看全。
- 写过新文件后的提示给的是 `git add -A && git diff --staged`，不是 `git diff`——
  未跟踪的新文件不出现在后者里，照着那句话去检查，会以为 polish 只改了 README，
  而它刚往仓库里放了四个文件。
- 不在 git 仓库里时拒绝 `--apply`，除非 `--force`——没有 `git checkout` 就没有撤销键。
- 幂等：已经有徽章就什么都不做，判据是 URL 里的 `.repolish/badge.json` 路径，
  不是整段 snippet（分支名不同不该被当成两个徽章）。

### 目录插什么

条目取**正文里最浅一层**的标题，不是写死 h2：ripgrep 的标题是 setext h2、
正文小节全是 `###`，按 h2 取会得到一个空目录。目录自身的标题层级也跟着走，
否则凭空多出一层，把原有的层级切断。

锚点按 GitHub 的 github-slugger 算，四步顺序不能调：去空白 → 转小写 →
删掉所有非字母数字非 `-` `_` 非空格的字符 → 空格换 `-`。
`## 🚀 Install` 的锚点是 `-install` 而不是 `install`，开头那个连字符是真的。
重名按**全文顺序**编号（`-1`、`-2`），只算目录里那几条会错位。
见 `crates/repolish-md/src/toc.rs`。

目录里的每一条都是作者自己写的标题，一个字都不是编的。这是它能进
`polish` 的原因——`## Install` 底下该写 `cargo install <name>` 这类改动
就进不来：清单里有 `name` 不等于那个包真的发布了，那是在替别人对外部世界
下断言，撞设计原则 4。

标题语言跟着 README 走（`## Contents` / `## 目录`）。这一段是写进**别人的**
文档的，和 repolish 自己的报告一律英文是两回事。
