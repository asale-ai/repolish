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
                Evidence::new(i, "issue template"),
                Evidence::new(p, "PR template"),
            ]),
            (Some(i), None) => Outcome::scored(
                7,
                vec![Evidence::new(i, "issue template present, PR template missing")],
                vec![Fix::new(
                    Severity::P3,
                    "Add `.github/pull_request_template.md` with the pre-submit checklist: tests, docs, a description of the change",
                )],
            ),
            (None, Some(p)) => Outcome::scored(
                6,
                vec![Evidence::new(p, "PR template present, issue template missing")],
                vec![Fix::new(
                    Severity::P2,
                    "Add `.github/ISSUE_TEMPLATE/`. Bug reports filed without a template \
                     usually arrive with no version and no reproduction, and each one costs \
                     a few round trips before triage can even start",
                )],
            ),
            (None, None) => Outcome::scored(
                0,
                vec![Evidence::new(".", "no issue or PR templates under `.github/`")],
                vec![Fix::new(
                    Severity::P2,
                    "Add issue templates (bug report, feature request) and a PR template. \
                     This is the cheapest fix on the list: write it once, and every report \
                     afterwards saves you a round trip",
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
