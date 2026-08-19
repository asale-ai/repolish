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
                vec![Evidence::new(".", "没有 CONTRIBUTING（根目录 / .github/ / docs/ 均无）")],
                vec![Fix::new(
                    Severity::P2,
                    "加 CONTRIBUTING.md，写清如何本地构建、如何跑测试、PR 的基本要求。\
                     它决定了路过的人会不会真的提交第一个 PR",
                )],
            );
        };

        let text = match ctx.files.read(&path) {
            Some(t) => t,
            None => return Outcome::inconclusive(format!("{path} 存在但无法读取")),
        };

        let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
        if lines < MIN_LINES {
            return Outcome::scored(
                5,
                vec![Evidence::new(&path, format!("只有 {lines} 行有效内容，更像模板骨架"))],
                vec![Fix::new(
                    Severity::P2,
                    "把贡献指南写实：本地构建命令、测试命令、代码风格要求、PR 检查清单",
                )],
            );
        }

        let lower = text.to_lowercase();
        if SETUP_HINTS.iter().any(|h| lower.contains(h)) {
            return Outcome::perfect(vec![Evidence::new(
                &path,
                format!("{lines} 行，包含本地开发命令"),
            )]);
        }

        Outcome::scored(
            8,
            vec![Evidence::new(&path, format!("{lines} 行，但没有可执行的开发命令"))],
            vec![Fix::new(
                Severity::P3,
                "补一段「本地跑起来」的命令（安装依赖 / 构建 / 测试）——\
                 贡献者卡住最多的地方是环境，不是规范",
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
