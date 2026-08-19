use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// `.github/` 下是否有 issue / PR 模板。
///
/// 分档：都没有 = 0；只有 PR 模板 = 6；只有 issue 模板 = 7；两者都有 = 10
///
/// issue 模板权重更高：它决定了收到的 bug 报告里有没有版本号与复现步骤，
/// 直接决定维护者要花多少轮对话才能开始排查。
pub struct IssuePrTemplate;

impl Check for IssuePrTemplate {
    fn id(&self) -> &'static str {
        "issue-pr-template"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::Medium
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let issue = find_issue(ctx);
        let pr = find_pr(ctx);

        match (issue, pr) {
            (Some(i), Some(p)) => Outcome::perfect(vec![
                Evidence::new(i, "issue 模板"),
                Evidence::new(p, "PR 模板"),
            ]),
            (Some(i), None) => Outcome::scored(
                7,
                vec![Evidence::new(i, "有 issue 模板，但没有 PR 模板")],
                vec![Fix::new(
                    Severity::P3,
                    "加 `.github/pull_request_template.md`，列出提交前的自检项（测试、文档、变更说明）",
                )],
            ),
            (None, Some(p)) => Outcome::scored(
                6,
                vec![Evidence::new(p, "有 PR 模板，但没有 issue 模板")],
                vec![Fix::new(
                    Severity::P2,
                    "加 `.github/ISSUE_TEMPLATE/`。没有模板的 bug 报告通常缺版本号和复现步骤，\
                     每一条都要额外几轮对话才能开始排查",
                )],
            ),
            (None, None) => Outcome::scored(
                0,
                vec![Evidence::new(".", "`.github/` 下没有 issue 或 PR 模板")],
                vec![Fix::new(
                    Severity::P2,
                    "加 issue 模板（bug 报告 / 功能请求）与 PR 模板。\
                     这是投入产出比最高的一项：写一次，之后每个报告都省一轮来回",
                )],
            ),
        }
    }
}

/// `.github/ISSUE_TEMPLATE/config.yml` 只是入口配置，本身不是模板——
/// 只有它而没有真模板时，等于没有模板。
fn find_issue(ctx: &RepoContext) -> Option<String> {
    ctx.files
        .iter()
        .find(|p| {
            let l = p.to_lowercase();
            if !l.starts_with(".github/issue_template") {
                return false;
            }
            if l.ends_with("/config.yml") || l.ends_with("/config.yaml") {
                return false;
            }
            l.ends_with(".md") || l.ends_with(".yml") || l.ends_with(".yaml")
        })
        .map(str::to_string)
}

fn find_pr(ctx: &RepoContext) -> Option<String> {
    ctx.files
        .iter()
        .find(|p| {
            let l = p.to_lowercase();
            (l.starts_with(".github/pull_request_template")
                || l.starts_with("pull_request_template")
                || l.starts_with("docs/pull_request_template"))
                && (l.ends_with(".md") || l.ends_with(".txt"))
        })
        .map(str::to_string)
}
