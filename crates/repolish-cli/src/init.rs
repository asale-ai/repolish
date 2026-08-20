//! `repolish init`：生成 GitHub Actions workflow。
//!
//! 这是留存的关键一步——CLI-only 的产品，用户装完跑一次就忘了；
//! 进了 CI 才会每周替他跑一次，也才会有人持续看到那个徽章。
//!
//! 模板里有两个默认值是踩出来的，不要改：
//!
//! - `fetch-depth: 0`：`actions/checkout` 默认只拉一个 commit，一个 tag 都没有，
//!   `release-hygiene` 会因此对每个项目判「无法判断」
//! - `--remote`：Action 里 `GITHUB_TOKEN` 免费可得，没有理由产出基准更窄的本地分

pub const WORKFLOW_PATH: &str = ".github/workflows/repolish.yml";

pub fn workflow(branch: &str, min_score: Option<u8>) -> String {
    // 不设门禁时整个 `with:` 块都不能出现。只留一条注释会让它解析成
    // `with: null`，GitHub 拒绝这种步骤（Unexpected value ''）。
    let (hint, gate) = match min_score {
        Some(n) => (
            String::new(),
            format!(
                "\n        with:\n          # Below {n} this step fails with exit code 1.\n          # Delete these two lines to record the score without enforcing it.\n          min-score: {n}"
            ),
        ),
        None => (
            "\n      # The score is recorded, not enforced. To gate on it, add a `with:` block\n      # holding `min-score: 60`."
                .to_string(),
            String::new(),
        ),
    };

    format!(
        r#"name: repolish

on:
  push:
    branches: [{branch}]
  schedule:
    # Weekly: activity and link rot get worse on their own, with no commits involved
    - cron: '0 0 * * 1'
  workflow_dispatch:

permissions:
  contents: write

jobs:
  score:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          # Must be 0: the default fetch-depth of 1 brings down no tags at all,
          # which leaves release-hygiene unable to judge the release cadence
          fetch-depth: 0

      # remote and badge are on by default, so they need no declaration here{hint}
      - uses: {action}{gate}
        env:
          GITHUB_TOKEN: ${{{{ secrets.GITHUB_TOKEN }}}}

      - name: Commit badge
        run: |
          git config user.name  github-actions
          git config user.email github-actions@github.com
          git add {badge}
          git diff --staged --quiet || git commit -m "chore: update repolish score"
          git push
"#,
        action = action_ref(),
        badge = repolish_render::BADGE_PATH,
    )
}

/// 固定到生成这份 workflow 的那个版本。浮动 tag 更省心，但也意味着
/// 上游一改判定口径，用户的分数就会在没有任何改动的情况下变动。
fn action_ref() -> String {
    format!("asale-ai/repolish@v{}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_keeps_the_two_defaults_that_were_learned_the_hard_way() {
        let w = workflow("main", Some(60));
        assert!(
            w.contains("fetch-depth: 0"),
            "浅克隆会让 release-hygiene 失效"
        );
        assert!(w.contains("min-score: 60"));
        assert!(w.contains("branches: [main]"));
    }

    #[test]
    fn omitting_the_gate_produces_a_report_only_workflow() {
        let w = workflow("master", None);
        // 注释里会提到 `min-score: 60` 作为开启方式，所以只能看生效的那一行
        let active = w.lines().any(|l| l.trim_start().starts_with("min-score:"));
        assert!(!active, "未设门禁时不应有生效的 min-score 行");
        assert!(w.contains("branches: [master]"));
    }

    /// 返回第一个「底下没有任何键」的 `with:` 行号。
    ///
    /// 光看 `with:` 是不够的——`actions/checkout` 那个带着 `fetch-depth: 0`
    /// 的也长这样。判据是它后面有没有缩进更深的内容。
    fn first_empty_with(w: &str) -> Option<usize> {
        let lines: Vec<&str> = w.lines().collect();
        lines.iter().enumerate().find_map(|(i, l)| {
            if l.trim() != "with:" {
                return None;
            }
            let indent = l.len() - l.trim_start().len();
            let has_key = lines[i + 1..]
                .iter()
                .filter(|n| !n.trim().is_empty())
                .take_while(|n| n.len() - n.trim_start().len() > indent)
                .any(|n| !n.trim_start().starts_with('#'));
            (!has_key).then_some(i + 1)
        })
    }

    /// 不设门禁时不能留下一个空的 `with:`。
    ///
    /// 第一次在真实仓库上试 action 就撞到了：只留注释的 `with:` 解析成
    /// `with: null`，GitHub 拒绝这种步骤。生成器的产物是直接进别人仓库的，
    /// 语法错误比判错分更糟。
    #[test]
    fn no_empty_with_block_when_there_is_no_gate() {
        let w = workflow("main", None);
        if let Some(line) = first_empty_with(&w) {
            panic!("第 {line} 行留下了空的 with:\n{w}");
        }
        // 有门禁时 with: 必须带着 min-score 一起出现
        let g = workflow("main", Some(70));
        let with_at = g
            .lines()
            .position(|l| l.trim() == "with:")
            .expect("有门禁时应当有 with:");
        assert!(
            g.lines()
                .skip(with_at)
                .any(|l| l.trim_start().starts_with("min-score:")),
            "with: 底下必须有 min-score"
        );
    }

    #[test]
    fn action_is_pinned_to_this_binarys_version() {
        assert!(workflow("main", None).contains(&format!("@v{}", env!("CARGO_PKG_VERSION"))));
    }
}
