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
            return Outcome::inconclusive("no README, so there are no relative links to check");
        };
        let name = readme.path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let relative: Vec<_> = readme.links.iter().filter(|l| l.is_relative()).collect();
        if relative.is_empty() {
            return Outcome::inconclusive("no relative links in the README to check");
        }

        let mut broken = Vec::new();
        for link in &relative {
            let path = link.repo_path();
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
                format!(
                    "all {} relative link{} resolve",
                    relative.len(),
                    crate::util::plural(relative.len())
                ),
            )]);
        }

        let ratio = 1.0 - broken.len() as f64 / relative.len() as f64;
        let score = (ratio * 10.0).floor() as u8;

        let evidence = broken
            .iter()
            .take(8)
            .map(|(link, path)| {
                let what = if link.is_image { "image" } else { "link" };
                Evidence::at(&name, link.line, format!("{what} target does not exist: {path}"))
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
                    "Fix {} broken relative links — every one of them is a 404 on the GitHub page",
                    broken.len()
                ),
            )],
        )
    }
}
