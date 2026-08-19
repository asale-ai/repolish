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
            return Outcome::inconclusive("未取到 GitHub 元数据");
        };

        let Some(url) = remote.homepage.as_deref() else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "未设置 homepage")],
                vec![Fix::new(
                    Severity::P3,
                    "在仓库设置里填 homepage。没有官网就填文档站——\
                     docs.rs / Read the Docs / npm 页面都行，它会显示在仓库首页右侧",
                )],
            );
        };

        if points_at_itself(url, ctx) {
            return Outcome::scored(
                4,
                vec![Evidence::new(".", format!("homepage 指回仓库自己：{url}"))],
                vec![Fix::new(
                    Severity::P3,
                    "homepage 指回本仓库没有信息量。改成文档站、官网或包管理页",
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
