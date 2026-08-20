use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};

/// 是否有 README 之外的文档。
///
/// 文档站链接与本地 `docs/` 等价——docs.rs、Read the Docs 这类托管站
/// 对使用者而言就是文档，不该因为「文件不在仓库里」而扣分。
///
/// 分档：什么都没有 = 0；全靠一篇厚 README = 4；只有一篇附加文档 = 5；
/// `docs/` 有若干篇 = 8；有文档站或成体系的文档 = 10
pub struct DocsPresence;

/// 文档站生成器的配置文件——出现即说明有独立文档站
const GENERATORS: &[&str] = &[
    "mkdocs.yml",
    "mkdocs.yaml",
    "docusaurus.config.js",
    "docusaurus.config.ts",
    "book.toml",
    "docs/conf.py",
    "doc/conf.py",
    "typedoc.json",
    "docs/.vitepress/config.js",
    "docs/.vitepress/config.ts",
    "astro.config.mjs",
];

/// 专用文档托管站。域名本身已经说明用途，命中即认。
const DOC_HOSTS: &[&str] = &[
    "docs.rs/",
    "readthedocs.io",
    "readthedocs.org",
    "gitbook.io",
    "pkg.go.dev/",
    "javadoc.io",
    "godoc.org",
    "hexdocs.pm/",
    "rubydoc.info/",
];

/// 自建文档站的路径特征。这两条太宽——`https://docs.github.com/...`
/// 也会命中——所以必须再加一条「URL 里出现项目名」才算数。
const SELF_HOSTED_HINTS: &[&str] = &["/docs", "docs."];

const DOC_DIRS: &[&str] = &["docs/", "doc/", "website/", "site/", "wiki/", "man/"];

/// 文档文件的扩展名。`doc/` 下常见 vim 帮助（`.txt`）与 man 手册（`.1`）。
const DOC_EXTS: &[&str] = &[".md", ".mdx", ".rst", ".adoc", ".txt", ".1", ".7"];

/// 根目录里这些不是「文档」，是社区元文件。除去它们，剩下的 `*.md`
/// 就是真正的附加文档——fzf 的 `ADVANCED.md`、awesome 的 `create-list.md`
/// 都在根目录而不在 `docs/` 下，只数 `docs/` 会把它们当成没有文档。
const COMMUNITY_FILES: &[&str] = &[
    "readme",
    "contributing",
    "changelog",
    "changes",
    "history",
    "news",
    "releases",
    "code_of_conduct",
    "code-of-conduct",
    "codeofconduct",
    "license",
    "licence",
    "copying",
    "security",
    "authors",
    "notice",
    "support",
    "governance",
    "funding",
    "maintainers",
    "pull_request_template",
    "issue_template",
    "citation",
];

/// README 已经足够厚时，「文档只在 README 里」是取舍而非缺失
const SUBSTANTIAL_README: usize = 300;

impl Check for DocsPresence {
    fn id(&self) -> &'static str {
        "docs-presence"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::Medium
    }

    /// 文档站项目本身就是文档，无从再要求「另有文档」
    fn applies_to(&self, profile: Profile) -> bool {
        profile != Profile::Docs
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        if let Some(g) = GENERATORS.iter().find(|g| ctx.files.contains(g)) {
            return Outcome::perfect(vec![Evidence::new(
                *g,
                "dedicated documentation site configured",
            )]);
        }

        let pages: Vec<&str> = ctx
            .files
            .iter()
            .filter(|p| {
                let l = p.to_lowercase();
                if !DOC_EXTS.iter().any(|e| l.ends_with(e)) {
                    return false;
                }
                match l.split_once('/') {
                    Some(_) => DOC_DIRS.iter().any(|d| l.starts_with(d)),
                    // 根目录：除社区元文件外的 md / rst 都算附加文档
                    None => {
                        let stem = l.rsplit_once('.').map(|(s, _)| s).unwrap_or(&l);
                        !COMMUNITY_FILES.contains(&stem) && !l.ends_with(".txt")
                    }
                }
            })
            .collect();

        if pages.len() >= 5 {
            return Outcome::perfect(vec![Evidence::new(
                pages[0],
                format!("{} documentation pages", pages.len()),
            )]);
        }

        if let Some((file, line, url)) = doc_site_link(ctx) {
            return Outcome::perfect(vec![Evidence::at(
                file,
                line,
                format!("README links to a documentation site: {url}"),
            )]);
        }

        match pages.len() {
            0 => no_docs(ctx),
            1 => Outcome::scored(
                5,
                vec![Evidence::new(pages[0], "only one page beyond the README")],
                vec![Fix::new(
                    Severity::P3,
                    "Move what no longer fits in the README — configuration reference, advanced usage, architecture — into `docs/`",
                )],
            ),
            n => Outcome::scored(
                8,
                vec![Evidence::new(pages[0], format!("{n} pages beyond the README"))],
                vec![Fix::new(
                    Severity::P3,
                    "There is enough documentation now to warrant an index page or a documentation site, so readers can navigate instead of opening files one by one",
                )],
            ),
        }
    }
}

fn no_docs(ctx: &RepoContext) -> Outcome {
    let words = ctx.readme.as_ref().map(|r| r.word_count()).unwrap_or(0);
    if words >= SUBSTANTIAL_README {
        return Outcome::scored(
            4,
            vec![Evidence::new(
                ".",
                format!("no `docs/`; everything lives in the README (~{words} words)"),
            )],
            vec![Fix::new(
                Severity::P3,
                "The README is carrying all of the documentation. Split `docs/` out before it \
                 grows further — a README should answer \"what is this\" and \"how do I start\", \
                 and stop there",
            )],
        );
    }
    Outcome::scored(
        0,
        vec![Evidence::new(".", "no `docs/`, and the README is too thin to serve as documentation")],
        vec![Fix::new(
            Severity::P2,
            "Write some documentation: configuration reference, advanced usage, common problems. `docs/` or a hosted site both work",
        )],
    )
}

/// README 里指向文档托管站的外链
fn doc_site_link(ctx: &RepoContext) -> Option<(String, usize, String)> {
    let readme = ctx.readme.as_ref()?;
    let name = crate::util::readme_name(readme);
    let project = repolish_ingest::normalize_package_name(&ctx.display_name());

    readme
        .links
        .iter()
        .filter(|l| !l.is_image && !l.is_relative())
        // URL 里必须出现项目名。docs.rs / pkg.go.dev 这类站点按包分页，
        // ripgrep 的 README 里链的 `docs.rs/regex` 是它依赖的文档，不是它自己的；
        // 只看域名会把别人家的文档算到自己头上。
        .find(|l| {
            if project.is_empty() {
                return false;
            }
            let u = l.url.to_lowercase();
            u.contains(&project)
                && (DOC_HOSTS.iter().any(|h| u.contains(h))
                    || SELF_HOSTED_HINTS.iter().any(|h| u.contains(h)))
        })
        .map(|l| (name, l.line, l.url.clone()))
}
