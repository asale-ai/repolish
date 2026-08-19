use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// README 首屏是否说清「这是什么」。
///
/// 分档：无 README = 0；无 H1 = 3；有 H1 无 tagline = 5；tagline 过短 = 7；完整 = 10
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

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "仓库根目录没有 README")],
                vec![Fix::new(
                    Severity::P1,
                    "添加 README.md，首行写项目名，紧接一句话说明它是什么、解决什么问题",
                )],
            );
        };

        let name = readme.path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let Some(title) = &readme.title else {
            return Outcome::scored(
                3,
                vec![Evidence::at(&name, 1, "未找到一级标题（# 项目名）")],
                vec![Fix::new(
                    Severity::P1,
                    "在 README 顶部加一个 `# 项目名` 一级标题",
                )],
            );
        };

        let title_line = readme.title_line.unwrap_or(1);

        match &readme.tagline {
            None => Outcome::scored(
                5,
                vec![Evidence::at(
                    &name,
                    title_line,
                    format!("有标题「{title}」，但其后没有说明性段落"),
                )],
                vec![Fix::new(
                    Severity::P1,
                    "在标题下方补一句话：这个项目是什么、给谁用、解决什么问题",
                )],
            ),
            Some(t) if t.chars().count() < MIN_TAGLINE => Outcome::scored(
                7,
                vec![Evidence::at(
                    &name,
                    title_line,
                    format!("说明过短（{} 字）：「{t}」", t.chars().count()),
                )],
                vec![Fix::new(
                    Severity::P3,
                    format!("把首段说明扩充到 {MIN_TAGLINE} 字以上，点明用途与适用场景"),
                )],
            ),
            Some(t) => Outcome::perfect(vec![Evidence::at(
                &name,
                title_line,
                format!("标题「{title}」+ 说明「{}」", truncate(t, 40)),
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
