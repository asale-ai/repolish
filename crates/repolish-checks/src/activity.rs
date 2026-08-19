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
            return Outcome::inconclusive("不是 git 仓库，或没有任何提交");
        };

        let days = git.days_since_head();
        let note = format!("最近一次提交在 {days} 天前（{}）", git.short_id());

        let (score, severity, advice) = match days {
            0..=30 => return Outcome::perfect(vec![Evidence::new(".", note)]),
            31..=90 => (
                8,
                Severity::P3,
                "保持提交节奏；长期沉寂会让使用者判断项目已弃坑",
            ),
            91..=180 => (
                5,
                Severity::P2,
                "已超过 3 个月没有提交。若项目仍在维护，在 README 说明维护状态",
            ),
            181..=365 => (
                3,
                Severity::P2,
                "近一年几乎没有更新。建议在 README 顶部标明维护状态（活跃 / 维护模式 / 已归档）",
            ),
            _ => (
                0,
                Severity::P1,
                "超过一年没有提交。要么恢复维护，要么在 README 明确标注已归档并指向替代方案",
            ),
        };

        Outcome::scored(
            score,
            vec![Evidence::new(".", note)],
            vec![Fix::new(severity, advice)],
        )
    }
}
