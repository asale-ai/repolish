use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// 项目是否还活着。M1 只看 HEAD 提交时间；M2 会补贡献者与 release 节奏。
///
/// 分档：≤30 天 = 10；≤90 = 8；≤180 = 5；≤365 = 3；更久 = 0
pub struct Activity;

impl Check for Activity {
    fn id(&self) -> &'static str {
        "activity"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::High
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(git) = &ctx.git else {
            return Outcome::inconclusive("not a git repository, or it has no commits");
        };

        let days = git.days_since_head();
        let note = format!("last commit {days} days ago ({})", git.short_id());

        let (score, severity, advice) = match days {
            0..=30 => return Outcome::perfect(vec![Evidence::new(".", note)]),
            31..=90 => (
                8,
                Severity::P3,
                "Keep the commits coming. A long silence reads as abandonment no matter how good the code is",
            ),
            91..=180 => (
                5,
                Severity::P2,
                "No commits for over three months. If the project is still maintained, say so in the README — nobody can tell from the outside",
            ),
            181..=365 => (
                3,
                Severity::P2,
                "Barely touched in a year. State the maintenance status at the top of the README: active, maintenance mode, or archived",
            ),
            _ => (
                0,
                Severity::P1,
                "No commits in over a year. Either resume maintenance, or mark the project archived in the README and point people at an alternative",
            ),
        };

        Outcome::scored(
            score,
            vec![Evidence::new(".", note)],
            vec![Fix::new(severity, advice)],
        )
    }
}
