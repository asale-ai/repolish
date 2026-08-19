use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};

/// README 长度是否适中。
///
/// 两头都是问题：太短说明不了「这是什么、怎么用」；太长说明该拆到 `docs/` 了——
/// 没有人会读完三千词才决定要不要用你的库。
///
/// 分档（词）：<80 = 3；80–149 = 6；150–2500 = 10；2501–6000 = 8；>6000 = 6
pub struct ReadmeLength;

const TOO_SHORT: usize = 80;
const MIN_WORDS: usize = 150;
const COMFORTABLE_MAX: usize = 2500;
const TOO_LONG: usize = 6000;

impl Check for ReadmeLength {
    fn id(&self) -> &'static str {
        "readme-length"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::Medium
    }

    /// 资源集合的 README 就是内容本体，长是它的形态而不是缺陷。
    /// 这条不在 docs/05 的例外表里，是 M2 实现时补的——见路线图。
    fn applies_to(&self, profile: Profile) -> bool {
        profile != Profile::Collection
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "没有 README")],
                vec![Fix::new(Severity::P1, "添加 README")],
            );
        };
        let name = crate::util::readme_name(readme);
        let words = readme.word_count();
        let note = format!("约 {words} 词");

        if words < TOO_SHORT {
            return Outcome::scored(
                3,
                vec![Evidence::new(&name, format!("{note}，信息量不足"))],
                vec![Fix::new(
                    Severity::P1,
                    "至少写清四件事：这是什么、解决什么问题、怎么安装、最小可运行示例",
                )],
            );
        }
        if words < MIN_WORDS {
            return Outcome::scored(
                6,
                vec![Evidence::new(&name, format!("{note}，偏薄"))],
                vec![Fix::new(
                    Severity::P2,
                    "补上使用示例与适用场景。读者判断要不要用，靠的是示例而不是描述",
                )],
            );
        }
        if words <= COMFORTABLE_MAX {
            return Outcome::perfect(vec![Evidence::new(&name, note)]);
        }
        if words <= TOO_LONG {
            return Outcome::scored(
                8,
                vec![Evidence::new(&name, format!("{note}，偏长"))],
                vec![Fix::new(
                    Severity::P3,
                    "把 API 详情、配置项、进阶用法挪进 `docs/`，README 只留「这是什么、怎么开始」",
                )],
            );
        }
        Outcome::scored(
            6,
            vec![Evidence::new(&name, format!("{note}，过长"))],
            vec![Fix::new(
                Severity::P2,
                "README 已经长到没人会读完。拆到 `docs/` 并在 README 留一份索引",
            )],
        )
    }
}
