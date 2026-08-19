use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};

/// 是否配置了持续集成。
///
/// 分档：无配置 = 0；有配置但看不出跑测试 = 7；配置中出现测试步骤 = 10
pub struct CiPresent;

const ROOT_CONFIGS: &[&str] = &[
    ".gitlab-ci.yml",
    ".travis.yml",
    "azure-pipelines.yml",
    "Jenkinsfile",
    ".drone.yml",
    "appveyor.yml",
];

const TEST_HINTS: &[&str] = &[
    "cargo test", "npm test", "npm run test", "pytest", "go test", "mvn test",
    "gradle test", "jest", "vitest", "tox", "make test", "yarn test", "pnpm test",
];

impl Check for CiPresent {
    fn id(&self) -> &'static str {
        "ci-present"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::High
    }

    fn applies_to(&self, profile: Profile) -> bool {
        profile != Profile::Collection
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let mut configs: Vec<String> = ctx
            .files
            .under(".github/workflows/")
            .filter(|p| p.ends_with(".yml") || p.ends_with(".yaml"))
            .map(|s| s.to_string())
            .collect();

        for c in ROOT_CONFIGS {
            if ctx.files.contains(c) {
                configs.push((*c).to_string());
            }
        }
        if ctx.files.contains(".circleci/config.yml") {
            configs.push(".circleci/config.yml".into());
        }

        if configs.is_empty() {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "未找到任何 CI 配置")],
                vec![Fix::new(
                    Severity::P1,
                    "加一个 CI workflow。绿色的构建标记是使用者判断项目是否可靠的第一信号",
                )],
            );
        }

        let runs_tests = configs.iter().any(|c| {
            ctx.files
                .read(c)
                .map(|t| {
                    let lower = t.to_lowercase();
                    TEST_HINTS.iter().any(|h| lower.contains(h))
                })
                .unwrap_or(false)
        });

        if runs_tests {
            Outcome::perfect(vec![Evidence::new(
                &configs[0],
                format!("{} 个 CI 配置，其中包含测试步骤", configs.len()),
            )])
        } else {
            Outcome::scored(
                7,
                vec![Evidence::new(
                    &configs[0],
                    "有 CI 配置，但未发现执行测试的步骤",
                )],
                vec![Fix::new(
                    Severity::P2,
                    "在 CI 中加入测试步骤——只跑 lint 或 build 的 CI 无法证明代码是对的",
                )],
            )
        }
    }
}
