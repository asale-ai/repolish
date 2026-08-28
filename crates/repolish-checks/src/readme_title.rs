use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};
use repolish_md::TitleSource;

/// README 首屏是否说清「这是什么」。
///
/// 分档：
/// - 0  没有 README
/// - 3  找不到任何形式的标题
/// - 5  标题只是图片 alt（无文本标题）
/// - 6  有标题，但没有说明段落
/// - 7  说明过短
/// - 10 标题 + 有效说明
pub struct ReadmeTitleTagline;

const MIN_TAGLINE: usize = 20;

impl Check for ReadmeTitleTagline {
    fn id(&self) -> &'static str {
        "readme-title-tagline"
    }
    fn category(&self) -> Category {
        Category::Discoverability
    }
    fn risk(&self) -> Risk {
        Risk::Critical
    }

    /// 资料仓库（`Profile::Meta`）**仍然适用**：那张名片开头有没有说清楚
    /// 「这是谁、在做什么」，正是它存在的全部意义。
    fn applies_to(&self, _profile: Profile) -> bool {
        true
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "no README in the repository root")],
                vec![Fix::new(
                    Severity::P1,
                    "Add README.md. Put the project name on the first line, and one sentence under it saying what this is and what problem it solves",
                )],
            );
        };

        let name = readme
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let Some(title) = &readme.title else {
            return Outcome::scored(
                3,
                vec![Evidence::at(&name, 1, "no title of any kind")],
                vec![Fix::new(
                    Severity::P1,
                    "Put a heading with the project name at the top of the README",
                )],
            );
        };

        let title_line = readme.title_line.unwrap_or(1);

        // 标题只存在于图片 alt：人能看见，但搜索引擎、读屏软件与本工具都难以提取
        if readme.title_source == Some(TitleSource::ImageAlt) {
            return Outcome::scored(
                5,
                vec![Evidence::at(
                    &name,
                    title_line,
                    format!("the title is an image; only its alt text is readable: \"{title}\""),
                )],
                vec![Fix::new(
                    Severity::P2,
                    "Add a text heading under the banner image. An image-only title is invisible to search engines and to screen readers",
                )],
            );
        }

        match &readme.tagline {
            None => Outcome::scored(
                6,
                vec![Evidence::at(
                    &name,
                    title_line,
                    format!("title \"{title}\" is followed by no descriptive paragraph"),
                )],
                vec![Fix::new(
                    Severity::P1,
                    "Add one sentence under the title: what this is, who it is for, what it solves",
                )],
            ),
            Some(t) if t.chars().count() < MIN_TAGLINE => Outcome::scored(
                7,
                vec![Evidence::at(
                    &name,
                    title_line,
                    format!("the description is only {} characters: \"{t}\"", t.chars().count()),
                )],
                vec![Fix::new(
                    Severity::P3,
                    format!("Expand the opening description past {MIN_TAGLINE} characters, and say what it is for and when to reach for it"),
                )],
            ),
            Some(t) => Outcome::perfect(vec![Evidence::at(
                &name,
                title_line,
                format!("title \"{title}\" followed by \"{}\"", truncate(t, 40)),
            )]),
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n).collect();
        format!("{head}…")
    }
}
