//! 项目类型探测。
//!
//! 类型**不改变分数线**，只决定某些检查项是否适用（`NotApplicable`）。
//! 见 docs/03-评分维度.md。

use repolish_md::Readme;
use serde::Serialize;

use crate::files::FileIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Library,
    App,
    Cli,
    Docs,
    Collection,
    /// 探测不出来时的兜底：所有检查项都适用
    Unknown,
}

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Library => "library",
            Profile::App => "app",
            Profile::Cli => "cli",
            Profile::Docs => "docs",
            Profile::Collection => "collection",
            Profile::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "library" | "lib" => Some(Profile::Library),
            "app" => Some(Profile::App),
            "cli" => Some(Profile::Cli),
            "docs" | "doc" => Some(Profile::Docs),
            "collection" | "awesome" => Some(Profile::Collection),
            "auto" | "unknown" => None,
            _ => None,
        }
    }
}

const CODE_EXTS: &[&str] = &[
    "rs", "go", "py", "js", "ts", "tsx", "jsx", "java", "kt", "c", "h", "cpp", "hpp", "cs", "rb",
    "php", "swift", "scala", "ex", "exs", "dart", "lua", "zig",
];

pub fn detect(files: &FileIndex, readme: Option<&Readme>) -> Profile {
    let code_files: usize = CODE_EXTS.iter().map(|e| files.content_extension_count(e)).sum();
    let md_files = files.content_extension_count("md");

    // 资源集合：README 巨长、外链极多、几乎没有代码
    if code_files <= 2 {
        if let Some(r) = readme {
            let external_links = r.links.iter().filter(|l| !l.is_relative()).count();
            if external_links >= 50 && r.raw.lines().count() >= 200 {
                return Profile::Collection;
            }
        }
    }

    // 文档站：Markdown 为主
    if code_files <= 5 && md_files >= 5 && md_files > code_files {
        return Profile::Docs;
    }

    if has_executable_entry(files) {
        return Profile::Cli;
    }
    if has_package_manifest(files) {
        return Profile::Library;
    }
    if has_deployment_config(files) {
        return Profile::App;
    }

    Profile::Unknown
}

fn has_executable_entry(files: &FileIndex) -> bool {
    // 单包仓库
    if files.contains("src/main.rs") && !is_fixture("src/main.rs") {
        return true;
    }
    // workspace / monorepo：任一成员有可执行入口即算。
    // 必须排除测试夹具——serde 的 test_suite/no_std/src/main.rs、
    // tokio 的 tests-integration/src/bin/ 都是内部测试用的二进制，
    // 把它们算作入口会让纯库项目被判成 CLI。
    if files.any_matching(|p| {
        !is_fixture(p) && (p.ends_with("/src/main.rs") || p.contains("/src/bin/") || p.starts_with("src/bin/"))
    }) {
        return true;
    }
    // 清单里显式声明的入口
    let manifest_declares_bin = files
        .iter()
        .filter(|p| {
            !is_fixture(p)
                && (p.ends_with("Cargo.toml")
                    || p.ends_with("package.json")
                    || p.ends_with("pyproject.toml"))
        })
        .take(64)
        .any(|p| {
            files.read(p).is_some_and(|t| {
                t.contains("[[bin]]") || t.contains("[project.scripts]")
            })
        });
    if manifest_declares_bin {
        return true;
    }
    files.any_matching(|p| p.starts_with("cmd/") && p.ends_with("main.go"))
}

/// 测试夹具 / 示例 / 基准目录。这些目录里的可执行文件不代表项目本身是 CLI。
fn is_fixture(path: &str) -> bool {
    const FIXTURE_DIRS: &[&str] = &[
        "test", "tests", "test_suite", "tests-integration", "testing",
        "example", "examples", "bench", "benches", "fixture", "fixtures", "demo",
    ];
    path.split(0x2Fu8 as char)
        .any(|seg| FIXTURE_DIRS.contains(&seg.to_lowercase().as_str()))
}

fn has_package_manifest(files: &FileIndex) -> bool {
    const MANIFESTS: &[&str] = &[
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "setup.py",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "Gemfile",
        "composer.json",
    ];
    MANIFESTS.iter().any(|m| files.contains(m))
}

fn has_deployment_config(files: &FileIndex) -> bool {
    const DEPLOY: &[&str] = &[
        "Dockerfile",
        "docker-compose.yml",
        "docker-compose.yaml",
        "Procfile",
        "fly.toml",
        "vercel.json",
        "netlify.toml",
    ];
    DEPLOY.iter().any(|d| files.contains(d))
        || files.any_matching(|p| p.starts_with("k8s/") || p.starts_with("deploy/"))
}
