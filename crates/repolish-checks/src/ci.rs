use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};

use crate::util;

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

/// 构建工具 / 测试运行器。判定规则是「同一行里出现运行器 + test 字样」，
/// 而不是匹配 `cargo test` 这类固定串——ripgrep 的 workflow 写的是
/// `${{ env.CARGO }} test`，固定串永远匹配不到。
const RUNNERS: &[&str] = &[
    "cargo", "npm", "yarn", "pnpm", "npx", "go ", "mvn", "gradle", "make", "just",
    "nox", "tox", "uv ", "poetry", "bundle", "dotnet", "ctest", "swift", "mix",
];

/// 本身就代表「在跑测试」的工具名，无需再看上下文
const TEST_TOOLS: &[&str] = &[
    "pytest", "nextest", "jest", "vitest", "mocha", "karma", "phpunit", "rspec",
    "gotestsum", "unittest",
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
                vec![Evidence::new(".", "no CI configuration found")],
                vec![Fix::new(
                    Severity::P1,
                    "Add a CI workflow. A green build badge is the first thing a stranger uses to judge whether the code actually works",
                )],
            );
        }

        let with_tests = configs
            .iter()
            .find(|c| ctx.files.read(c).is_some_and(|t| runs_tests(&t)));

        match with_tests {
            Some(c) => Outcome::perfect(vec![Evidence::new(
                c,
                format!("{} CI config{}, this one running tests", configs.len(), util::plural(configs.len())),
            )]),
            None => Outcome::scored(
                7,
                vec![Evidence::new(
                    &configs[0],
                    format!("{} CI config{}, none of them running tests", configs.len(), util::plural(configs.len())),
                )],
                vec![Fix::new(
                    Severity::P2,
                    "Run the test suite in CI. A pipeline that only lints or builds proves the code compiles, not that it works",
                )],
            ),
        }
    }
}

fn runs_tests(config: &str) -> bool {
    config.lines().any(|line| {
        let l = line.to_lowercase();
        if TEST_TOOLS.iter().any(|t| l.contains(t)) {
            return true;
        }
        // 同一行里「运行器 + test」
        if l.contains("test") && RUNNERS.iter().any(|r| l.contains(r)) {
            return true;
        }
        // 显式命名的测试步骤。requests 的 workflow 写成
        // `- name: Run tests` 加换行后的 `run: |`，命令与 test 字样不在同一行。
        is_step_name(&l) && l.contains("test")
    })
}

/// 缩进的 `name:`（步骤或 job 名），区别于顶格的 workflow 名
fn is_step_name(line_lower: &str) -> bool {
    if !line_lower.starts_with(char::is_whitespace) {
        return false;
    }
    let t = line_lower.trim_start().trim_start_matches("- ");
    t.starts_with("name:")
}
