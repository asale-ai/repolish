use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// 是否有贡献指南。
///
/// GitHub 会在「新建 Issue / PR」页面自动链接 CONTRIBUTING，位置固定在
/// 仓库根、`.github/` 或 `docs/` 三处之一，别处放了等于没放。
///
/// 分档：无 = 0；只有骨架 = 5；有内容但没写怎么跑起来 = 8；写了本地开发命令 = 10
pub struct Contributing;

const DIRS: &[&str] = &["", ".github/", "docs/"];
const NAMES: &[&str] = &["contributing.md", "contributing.rst", "contributing.txt", "contributing"];

/// 「怎么在本地跑起来」的信号。贡献指南只讲行为规范而不讲怎么构建，
/// 对第一次提 PR 的人没有实际帮助。
const SETUP_HINTS: &[&str] = &[
    "```", "npm ", "yarn ", "pnpm ", "cargo ", "make ", "just ", "pytest", "pip ", "uv ",
    "go test", "mvn ", "gradle", "docker",
];

const MIN_LINES: usize = 15;

impl Check for Contributing {
    fn id(&self) -> &'static str {
        "contributing"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::Medium
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(path) = find(ctx) else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "no CONTRIBUTING in the repository root, .github/, or docs/")],
                vec![Fix::new(
                    Severity::P2,
                    "Add CONTRIBUTING.md covering how to build locally, how to run the tests, \
                     and what a pull request needs. It decides whether a passer-by ever opens \
                     their first PR",
                )],
            );
        };

        let text = match ctx.files.read(&path) {
            Some(t) => t,
            None => return Outcome::inconclusive(format!("{path} exists but could not be read")),
        };

        let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
        if lines < MIN_LINES {
            return Outcome::scored(
                5,
                vec![Evidence::new(&path, format!("only {lines} non-empty lines — closer to a template than a guide"))],
                vec![Fix::new(
                    Severity::P2,
                    "Fill it in: the build command, the test command, the style expectations, and a PR checklist",
                )],
            );
        }

        let lower = text.to_lowercase();
        if SETUP_HINTS.iter().any(|h| lower.contains(h)) {
            return Outcome::perfect(vec![Evidence::new(
                &path,
                format!("{lines} lines, including local development commands"),
            )]);
        }

        Outcome::scored(
            8,
            vec![Evidence::new(&path, format!("{lines} lines, but no runnable development commands"))],
            vec![Fix::new(
                Severity::P3,
                "Add the commands that get someone running locally (install / build / test). \
                 Contributors get stuck on the environment far more often than on the rules",
            )],
        )
    }
}

fn find(ctx: &RepoContext) -> Option<String> {
    for dir in DIRS {
        for name in NAMES {
            let candidate = format!("{dir}{name}");
            if let Some(p) = ctx
                .files
                .iter()
                .find(|p| p.to_lowercase() == candidate)
            {
                return Some(p.to_string());
            }
        }
    }
    None
}
