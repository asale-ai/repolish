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
    /// 这条不在 docs/03 的例外表里，是 M2 实现时补的：
    /// 验收时 awesome 类仓库全部撞上「过长」，但那种仓库本来就该长。
    /// 资料仓库（`Profile::Meta`）**仍然适用**：那张名片是不是短到什么都没说，
    /// 正是它唯一值得判的事情之一。
    fn applies_to(&self, profile: Profile) -> bool {
        profile != Profile::Collection
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "no README")],
                vec![Fix::new(Severity::P1, "Add a README")],
            );
        };
        let name = crate::util::readme_name(readme);
        let words = readme.word_count();
        let note = format!("~{words} words");

        if words < TOO_SHORT {
            return Outcome::scored(
                3,
                vec![Evidence::new(&name, format!("{note} — too little to go on"))],
                vec![Fix::new(
                    Severity::P1,
                    "Cover four things at minimum: what this is, what problem it solves, how to install it, and one example that runs",
                )],
            );
        }
        if words < MIN_WORDS {
            return Outcome::scored(
                6,
                vec![Evidence::new(&name, format!("{note} — on the thin side"))],
                vec![Fix::new(
                    Severity::P2,
                    "Add usage examples and the cases this is meant for. Readers decide from examples, not from descriptions",
                )],
            );
        }
        if words <= COMFORTABLE_MAX {
            return Outcome::perfect(vec![Evidence::new(&name, note)]);
        }
        if words <= TOO_LONG {
            return Outcome::scored(
                8,
                vec![Evidence::new(&name, format!("{note} — on the long side"))],
                vec![Fix::new(
                    Severity::P3,
                    "Move the API details, configuration reference, and advanced usage into `docs/`, and leave the README with \"what is this\" and \"how do I start\"",
                )],
            );
        }
        Outcome::scored(
            6,
            vec![Evidence::new(&name, format!("{note} — far too long"))],
            vec![Fix::new(
                Severity::P2,
                "The README is long past the point where anyone finishes it. Split it into `docs/` and leave an index behind",
            )],
        )
    }
}
