use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// 是否有行为准则。
///
/// 二元判定的少数例外：行为准则要么有要么没有，不存在「写得不够好」的中间档
/// ——绝大多数项目直接采用 Contributor Covenant 原文。
/// 正因为它没有改进梯度，权重定为 Low。
pub struct CodeOfConduct;

const DIRS: &[&str] = &["", ".github/", "docs/"];
const NAMES: &[&str] = &[
    "code_of_conduct.md",
    "code-of-conduct.md",
    "codeofconduct.md",
    "code_of_conduct.rst",
    "code_of_conduct",
];

impl Check for CodeOfConduct {
    fn id(&self) -> &'static str {
        "code-of-conduct"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::Low
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let found = DIRS.iter().flat_map(|d| NAMES.iter().map(move |n| format!("{d}{n}")))
            .find_map(|c| {
                ctx.files
                    .iter()
                    .find(|p| p.to_lowercase() == c)
                    .map(str::to_string)
            });

        match found {
            Some(p) => Outcome::perfect(vec![Evidence::new(p, "存在行为准则")]),
            None => Outcome::scored(
                0,
                vec![Evidence::new(".", "没有 CODE_OF_CONDUCT")],
                vec![Fix::new(
                    Severity::P3,
                    "采用 Contributor Covenant 原文即可（contributor-covenant.org）。\
                     GitHub 会在仓库首页展示它，也是社区评估项目是否可参与的常规信号",
                )],
            ),
        }
    }
}
