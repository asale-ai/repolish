//! 检查项的端到端回归：在真实目录上跑整个注册表，而不是单测某个函数。
//!
//! 这两项是 M2 的差异化重点，也是最容易误报的地方：README 的用法示例里
//! 到处是使用者自己的文件路径与占位符，把它们当成「仓库的承诺」去校验，
//! 就会给一批健康项目判失效。验收时 ruff 与 fzf 各贡献了一个这样的误报，
//! 下面第二个用例就是照着它们的 README 写的。

use std::fs;
use std::path::{Path, PathBuf};

use repolish_core::{registry::RunOptions, Mode, Outcome, RepoContext};

/// 建一个临时仓库目录，测试结束后删掉
struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!("repolish-test-{name}"));
        let _ = fs::remove_dir_all(&root);
        for (path, content) in files {
            let full = root.join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, content).unwrap();
        }
        Fixture(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn outcome_of(fx: &Fixture, id: &str) -> Outcome {
    let ctx = RepoContext::load(fx.path(), None).expect("载入仓库");
    let report = repolish_checks::registry().run(&ctx, &RunOptions::default());
    report
        .checks
        .into_iter()
        .find(|c| c.id == id)
        .expect("检查项存在")
        .outcome
}

#[test]
fn broken_commands_are_reported_with_line_numbers() {
    let fx = Fixture::new(
        "claims-broken",
        &[
            (
                "package.json",
                r#"{"name":"probe","scripts":{"build":"tsc"}}"#,
            ),
            ("Makefile", ".PHONY: lint\nlint:\n\techo lint\n"),
            ("scripts/setup.sh", "echo hi\n"),
            (
                "README.md",
                "# Probe\n\nA probe repository.\n\n```bash\n./scripts/setup.sh\nnpm run build\nnpm run bootstrap\nmake lint\nmake deploy\n```\n",
            ),
        ],
    );

    let Outcome::Scored { score, evidence, .. } = outcome_of(&fx, "claim-consistency") else {
        panic!("应当判分");
    };

    // 5 条可校验，2 条失效 → floor(3/5 * 10)
    assert_eq!(score, 6);
    let notes: Vec<&str> = evidence.iter().map(|e| e.note.as_str()).collect();
    assert_eq!(notes.len(), 2, "只有 bootstrap 与 deploy 应被判失效：{notes:?}");
    assert!(notes.iter().any(|n| n.contains("npm run bootstrap")));
    assert!(notes.iter().any(|n| n.contains("make deploy")));
    // 行号要指向命令本身，而不是围栏行
    assert_eq!(evidence[0].line, Some(8));
}

#[test]
fn usage_example_paths_are_not_treated_as_claims() {
    // 照抄 ruff 与 fzf 的写法：远端安装脚本、占位路径、裸文件名。
    // 三者都不是「本仓库的承诺」，一条都不该被校验。
    let fx = Fixture::new(
        "claims-examples",
        &[
            ("pyproject.toml", "[project]\nname = \"ruffish\"\n"),
            (
                "README.md",
                "# Ruffish\n\nA linter.\n\n```shell\ncurl -LsSf https://example.com/install.sh | sh\nruffish check path/to/code/to/file.py  # Lint `file.py`.\nruffish check preview.sh\n```\n",
            ),
        ],
    );

    assert!(
        matches!(outcome_of(&fx, "claim-consistency"), Outcome::Inconclusive { .. }),
        "用法示例里的路径不构成可校验声明"
    );
}

#[test]
fn install_command_naming_another_package_is_flagged() {
    // 从别的项目抄来 README 模板、忘了改包名，是最常见的复制粘贴残留
    let fx = Fixture::new(
        "install-mismatch",
        &[
            ("package.json", r#"{"name":"widget"}"#),
            (
                "README.md",
                "# Widget\n\nA widget library.\n\n```bash\nnpm install gadget\n```\n",
            ),
        ],
    );

    let Outcome::Scored { score, evidence, .. } =
        outcome_of(&fx, "readme-install-consistency")
    else {
        panic!("应当判分");
    };
    assert_eq!(score, 4);
    assert!(evidence[0].note.contains("gadget") && evidence[0].note.contains("widget"));
}

#[test]
fn remote_checks_are_skipped_without_the_flag() {
    let fx = Fixture::new("local-only", &[("README.md", "# X\n\nA thing.\n")]);
    let ctx = RepoContext::load(fx.path(), None).unwrap();
    let report = repolish_checks::registry().run(&ctx, &RunOptions::default());

    assert_eq!(report.mode, Mode::Local);
    for id in ["repo-description", "repo-topics", "repo-homepage"] {
        let c = report.checks.iter().find(|c| c.id == id).unwrap();
        assert!(
            matches!(&c.outcome, Outcome::Skipped { reason } if reason.contains("--remote")),
            "{id} 在本地模式下应为 skipped"
        );
    }
    // 被跳过的项必须出现在覆盖限制里，否则消费方看不出分数基准变了
    assert!(report.coverage_limits.iter().any(|l| l.starts_with("repo-topics")));
    // 剔掉三项远程检查后仍要能出总分
    assert!(report.score.is_some());
}

#[test]
fn inline_tests_count_even_when_a_tests_dir_exists() {
    // Rust 项目的测试绝大多数写在 `#[cfg(test)] mod tests` 里。若「有 tests/ 目录
    // 就不再扫内联」，一个有几十个内联测试模块的仓库会被判成「只找到 1 处测试」
    // ——repolish 自己就是这么被误判的。
    let inline = "pub fn f() {}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {}
}
";
    let fx = Fixture::new(
        "tests-union",
        &[
            ("README.md", "# X

A thing.
"),
            ("tests/integration.rs", "#[test]
fn it_works() {}
"),
            ("src/a.rs", inline),
            ("src/b.rs", inline),
            ("src/c.rs", inline),
        ],
    );

    let Outcome::Scored { score, evidence, .. } = outcome_of(&fx, "tests-present") else {
        panic!("应当判分");
    };
    // 1 个 tests/ 文件 + 3 个内联模块 = 4 处，落在 3..=9 档
    assert_eq!(score, 8, "{:?}", evidence);
}

/// 产出文案一律英文——REPOLISH.md 会被提交进陌生人的仓库，混合语言等于不能用。
///
/// 用两个全 ASCII 的 fixture 兜住：一个什么都没有（走 0 分与建议分支），
/// 一个尽量齐全（走满分与证据分支）。fixture 里没有一个非 ASCII 字符，
/// 所以输出里出现 CJK 只可能来自我们自己的字符串。
///
/// 覆盖不到中间档，但**用户最常看到的两端**在这里被钉住了。
#[test]
fn all_messages_are_english() {
    let bare = Fixture::new("lang-bare", &[("src/lib.rs", "pub fn f() {}\n")]);

    let full = Fixture::new(
        "lang-full",
        &[
            (
                "README.md",
                "# thing\n\n\
                 A small thing that does one job well, and does not pretend otherwise.\n\n\
                 [![build](https://img.shields.io/badge/build-passing-green)](https://x/ci)\n\
                 [![crates.io](https://img.shields.io/crates/v/thing)](https://crates.io/crates/thing)\n\
                 [![license](https://img.shields.io/badge/license-MIT-blue)](LICENSE)\n\n\
                 ## Quick start\n\nRequires Rust 1.88.\n\n```sh\ncargo add thing\n```\n\n\
                 ## Usage\n\n```rust\nlet t = thing::new();\n```\n",
            ),
            ("LICENSE", "MIT License\n\nPermission is hereby granted, free of charge\n"),
            ("CONTRIBUTING.md", &"Build it with `cargo build`.\n".repeat(20)),
            ("CODE_OF_CONDUCT.md", "Contributor Covenant\n"),
            (".github/workflows/ci.yml", "jobs:\n  t:\n    steps:\n      - run: cargo test\n"),
            (".github/ISSUE_TEMPLATE/bug.yml", "name: Bug\n"),
            (".github/pull_request_template.md", "## What changed\n"),
            ("docs/a.md", "a\n"),
            ("docs/b.md", "b\n"),
            ("docs/c.md", "c\n"),
            ("docs/d.md", "d\n"),
            ("docs/e.md", "e\n"),
            ("tests/t.rs", "#[test]\nfn t() {}\n"),
        ],
    );

    for fx in [&bare, &full] {
        let ctx = RepoContext::load(fx.path(), None).expect("载入仓库");
        let report = repolish_checks::registry().run(&ctx, &RunOptions::default());
        for c in &report.checks {
            let mut messages: Vec<String> = Vec::new();
            match &c.outcome {
                Outcome::Scored { evidence, fixes, .. } => {
                    messages.extend(evidence.iter().map(|e| e.note.clone()));
                    messages.extend(fixes.iter().map(|f| f.message.clone()));
                }
                Outcome::Inconclusive { reason } | Outcome::Skipped { reason } => {
                    messages.push(reason.clone())
                }
                Outcome::NotApplicable { .. } => {}
            }
            for m in messages {
                assert!(
                    !m.chars().any(is_cjk),
                    "{} 的产出文案含非英文：{m}",
                    c.id
                );
            }
        }
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x303F | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xFF00..=0xFFEF)
}
