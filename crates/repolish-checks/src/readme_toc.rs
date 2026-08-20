use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// 较长的 README 是否提供目录。
///
/// 短 README 不需要目录，此时判满分而非 `NotApplicable`——
/// 「要求已被满足」和「不适用」是两回事，后者会把这一项从分母里剔出去。
///
/// 权重定为 Low：GitHub 现在会自动渲染标题大纲，手写目录的价值已经打了折，
/// 但在 GitHub 之外（npm、crates.io、PyPI 的项目页）仍然只有手写目录管用。
pub struct ReadmeToc;

/// 触发门槛：标题数与行数同时达到才认为「长到需要目录」
const MIN_HEADINGS: usize = 8;
const MIN_LINES: usize = 120;
/// 目录区的判定：靠前位置出现的一串锚点链接
const MIN_ANCHORS: usize = 5;
const ANCHOR_ZONE_RATIO: usize = 3;

const TOC_TITLES: &[&str] = &[
    "table of contents",
    "contents",
    "toc",
    "目录",
    "索引",
    "導覽",
];

impl Check for ReadmeToc {
    fn id(&self) -> &'static str {
        "readme-toc"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::Low
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::inconclusive("no README");
        };
        let name = crate::util::readme_name(readme);
        let lines = readme.raw.lines().count();
        let headings = readme.sections.len();

        if headings < MIN_HEADINGS || lines < MIN_LINES {
            return Outcome::perfect(vec![Evidence::new(
                &name,
                format!("{headings} headings over {lines} lines — short enough not to need a table of contents"),
            )]);
        }

        if let Some(s) = readme.sections.iter().find(|s| {
            let t = s.title.to_lowercase();
            TOC_TITLES.iter().any(|k| t.contains(k))
        }) {
            return Outcome::perfect(vec![Evidence::at(
                &name,
                s.line,
                "table of contents present",
            )]);
        }

        // 有些 README 不给目录加标题，直接在开头列一串锚点链接
        let zone = (lines / ANCHOR_ZONE_RATIO).max(40);
        let anchors = readme
            .links
            .iter()
            .filter(|l| !l.is_image && l.url.starts_with('#') && l.line <= zone)
            .count();
        if anchors >= MIN_ANCHORS {
            return Outcome::perfect(vec![Evidence::new(
                &name,
                format!("{anchors} anchor links near the top, which serve as a table of contents"),
            )]);
        }

        Outcome::scored(
            4,
            vec![Evidence::new(
                &name,
                format!("{headings} headings over {lines} lines, with no table of contents"),
            )],
            vec![Fix::new(
                Severity::P3,
                "Add a table of contents. GitHub's automatic outline covers part of this, \
                 but on the npm, crates.io, and PyPI project pages a hand-written one is \
                 the only thing that works",
            )],
        )
    }
}
