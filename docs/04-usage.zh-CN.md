# 04 · 用法参考

[English](04-usage.md) · [中文](04-usage.zh-CN.md)

README 讲的是「这是什么」和「怎么开始」。剩下的在这里：所有改变 `polish` 插入内容的
开关、`--suggest` 能做和不能做的事，以及那几个 pull request 开关各自的行为。

## polish 插入的东西怎么调样式

```bash
npx @asale/repolish --badge-style for-the-badge --toc-style fold --align center
npx @asale/repolish --logo assets/hero.svg --logo-width full --tree-depth 2 --apply
npx @asale/repolish --visuals --apply  # --overview --footer-card --tables svg
```

也可以写在 `.repolish.toml` 的 `[readme]` 下（[全部键](02-cli-design.zh-CN.md)）。
**这些都不影响分数**——检查项与权重在 v1 冻结，换一个徽章样式不会让仓库显得更好。

logo、目录树和卡片不由任何检查驱动：不显式开就不会有，干跑时也照实写「由配置要求」，
而不是把它们打扮成修复。

## `--suggest` 能做什么、不能做什么

```bash
npx @asale/repolish --suggest  # 需要 REPOLISH_LLM_API_KEY
```

**评分路径里没有模型**是一条关于*评分*的规则，它没有变——`check` 前后跑，数字一模一样。
把它顺手扩张到*修复*上才是那个错误：结果是 `polish` 忙着插徽章，而作者卡在标题下面
那一句话上。

边界在别处，而且更严。它**从不写文件**，`--apply` 也不行；它**只填空缺**，不改写已有内容；
它**不能凭空造**——交给它的是仓库里真实的包清单、可执行文件名和脚本名，并被要求
「缺一个事实就把建议留空并说明，绝不硬编一个」：一条编造的安装命令，正是
`claim-consistency` 生来要抓的东西。[每一条的理由](02-cli-design.zh-CN.md)。

密钥取自 `REPOLISH_LLM_API_KEY` 或 `ANTHROPIC_API_KEY`。repolish 里没有别的地方
会跟模型说话。

## 在 pull request 上，变化才是重点

一个绝对分数对 reviewer 没有意义。*这个 PR 掉了四分，因为第 42 行的链接不通了*
才告诉他该做什么。

```bash
npx @asale/repolish --stages check --base origin/main
npx @asale/repolish --stages check --sarif repolish.sarif  # 每条发现一个标注，落在它自己那一行
npx @asale/repolish --stages check --comment comment.md    # 短版本，用来发 PR 评论
```

`--base` 把基线检出到一个**临时 git worktree** 里，用完全相同的选项打分——你的工作区
不会被碰，本地分数绝不会拿去和远程分数比，报告只列出真正变动的检查项。

从第一个版本起，每一处扣分都带着文件和行号；SARIF 做的事是把它们放进 **diff 里**，
紧挨着代码，而不是留在一段没人展开的 CI 日志里。action 把三者一起接好了：

```yaml
- uses: asale-ai/repolish@v0.4.1
  with:
    min-score: 70
    base: ${{ github.event.pull_request.base.sha }}
    sarif: repolish.sarif
    comment: true
```

评论是**每次推送改写同一条**，不是往下追加——一个每次都新发一条评论的机器人，
第三条之后就会被所有人折叠，连带把真正变红的那一次一起埋掉。`npx @asale/repolish --stages ci --apply` 会替你
写好这个 workflow。所需权限、SARIF 上传步骤，以及基线所需的 `fetch-depth: 0`，见
[action/README.md](../action/README.md)。
