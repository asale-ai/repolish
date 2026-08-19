use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// README 里的相对链接与图片是否真的指向存在的文件。
///
/// 没有相对链接时返回 `Inconclusive`——「没得查」不等于「合格」。
pub struct ReadmeLinkHealth;

impl Check for ReadmeLinkHealth {
    fn id(&self) -> &'static str {
        "readme-link-health"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::Medium
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::inconclusive("没有 README，无相对链接可校验");
        };
        let name = readme.path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let relative: Vec<_> = readme.links.iter().filter(|l| l.is_relative()).collect();
        if relative.is_empty() {
            return Outcome::inconclusive("README 中没有相对链接可校验");
        }

        let mut broken = Vec::new();
        for link in &relative {
            let path = link.path_part().trim_start_matches("./");
            if path.is_empty() {
                continue;
            }
            if !ctx.root.join(path).exists() {
                broken.push((*link, path.to_string()));
            }
        }

        if broken.is_empty() {
            return Outcome::perfect(vec![Evidence::new(
                &name,
                format!("{} 个相对链接全部有效", relative.len()),
            )]);
        }

        let ratio = 1.0 - broken.len() as f64 / relative.len() as f64;
        let score = (ratio * 10.0).floor() as u8;

        let evidence = broken
            .iter()
            .take(8)
            .map(|(link, path)| {
                let what = if link.is_image { "图片" } else { "链接" };
                Evidence::at(&name, link.line, format!("{what}目标不存在: {path}"))
            })
            .collect();

        Outcome::scored(
            score,
            evidence,
            vec![Fix::new(
                if broken.len() > relative.len() / 2 {
                    Severity::P1
                } else {
                    Severity::P2
                },
                format!(
                    "修正 {} 个失效的相对链接——它们在 GitHub 页面上是 404",
                    broken.len()
                ),
            )],
        )
    }
}
