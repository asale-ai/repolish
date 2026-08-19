use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// 仓库是否设置了 homepage。
///
/// 分档：未设置 = 0；指回仓库自己 = 4；指向文档站 / 官网 = 10
///
/// 权重 Low：没有官网的库很常见，填个 docs.rs 链接也算合格。
pub struct RepoHomepage;

impl Check for RepoHomepage {
    fn id(&self) -> &'static str {
        "repo-homepage"
    }
    fn category(&self) -> Category {
        Category::Discoverability
    }
    fn risk(&self) -> Risk {
        Risk::Low
    }
    fn requires_remote(&self) -> bool {
        true
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(remote) = &ctx.remote else {
            return Outcome::inconclusive("GitHub metadata was not fetched");
        };

        let Some(url) = remote.homepage.as_deref() else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "no homepage set")],
                vec![Fix::new(
                    Severity::P3,
                    "Set the homepage in the repository settings. With no dedicated site, \
                     the documentation will do — docs.rs, Read the Docs, the npm page. \
                     GitHub shows it in the sidebar of the repository home page",
                )],
            );
        };

        if points_at_itself(url, ctx) {
            return Outcome::scored(
                4,
                vec![Evidence::new(".", format!("the homepage points back at the repository itself: {url}"))],
                vec![Fix::new(
                    Severity::P3,
                    "A homepage pointing back at this repository adds nothing. Point it at the documentation, the project site, or the package page",
                )],
            );
        }

        Outcome::perfect(vec![Evidence::new(".", format!("homepage：{url}"))])
    }
}

fn points_at_itself(url: &str, ctx: &RepoContext) -> bool {
    let Some(slug) = &ctx.slug else {
        return false;
    };
    let u = url.to_lowercase();
    let self_url = format!("github.com/{}/{}", slug.owner, slug.name).to_lowercase();
    u.trim_end_matches('/').ends_with(&self_url)
}
