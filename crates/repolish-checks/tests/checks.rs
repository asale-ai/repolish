//! 检查项的端到端回归：在真实目录上跑整个注册表，而不是单测某个函数。
//!
//! 这两项是 M2 的差异化重点，也是最容易误报的地方：README 的用法示例里
//! 到处是使用者自己的文件路径与占位符，把它们当成「仓库的承诺」去校验，
//! 就会给一批健康项目判失效。验收时 ruff 与 fzf 各贡献了一个这样的误报，
//! 下面第二个用例就是照着它们的 README 写的。

use std::fs;
use std::path::{Path, PathBuf};

use repolish_core::{registry::RunOptions, Mode, Outcome, RepoContext};
use repolish_ingest::RemoteFacts;

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

    let Outcome::Scored {
        score, evidence, ..
    } = outcome_of(&fx, "claim-consistency")
    else {
        panic!("应当判分");
    };

    // 5 条可校验，2 条失效 → floor(3/5 * 10)
    assert_eq!(score, 6);
    let notes: Vec<&str> = evidence.iter().map(|e| e.note.as_str()).collect();
    assert_eq!(
        notes.len(),
        2,
        "只有 bootstrap 与 deploy 应被判失效：{notes:?}"
    );
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
        matches!(
            outcome_of(&fx, "claim-consistency"),
            Outcome::Inconclusive { .. }
        ),
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

    let Outcome::Scored {
        score, evidence, ..
    } = outcome_of(&fx, "readme-install-consistency")
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
    assert!(report
        .coverage_limits
        .iter()
        .any(|l| l.starts_with("repo-topics")));
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
            (
                "README.md",
                "# X

A thing.
",
            ),
            (
                "tests/integration.rs",
                "#[test]
fn it_works() {}
",
            ),
            ("src/a.rs", inline),
            ("src/b.rs", inline),
            ("src/c.rs", inline),
        ],
    );

    let Outcome::Scored {
        score, evidence, ..
    } = outcome_of(&fx, "tests-present")
    else {
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

    // 三个远程检查项在没有 token 时根本不执行，等于永久在防线之外——
    // `repo-topics` 的一个中文句号就是这么漏过去的。这里直接塞假的远程数据，
    // 让它们也跑起来：一份「什么都没填」和一份「填了但对不上」。
    let empty_remote = RemoteFacts {
        description: None,
        homepage: None,
        topics: Vec::new(),
        license: None,
        archived: false,
        stars: 0,
        default_branch: Some("main".to_string()),
    };
    let mismatched_remote = RemoteFacts {
        description: Some("thing".to_string()),
        homepage: Some("https://github.com/o/thing".to_string()),
        topics: vec!["quantum".to_string(), "biology".to_string()],
        ..empty_remote.clone()
    };

    let mut cases: Vec<(RepoContext, Mode)> = Vec::new();
    for fx in [&bare, &full] {
        cases.push((
            RepoContext::load(fx.path(), None).expect("载入仓库"),
            Mode::Local,
        ));
        for facts in [&empty_remote, &mismatched_remote] {
            let mut ctx = RepoContext::load(fx.path(), None).expect("载入仓库");
            ctx.remote = Some(facts.clone());
            cases.push((ctx, Mode::Remote));
        }
    }

    for (ctx, mode) in &cases {
        let report = repolish_checks::registry().run(
            ctx,
            &RunOptions {
                mode: *mode,
                ..RunOptions::default()
            },
        );
        for c in &report.checks {
            let mut messages: Vec<String> = Vec::new();
            match &c.outcome {
                Outcome::Scored {
                    evidence, fixes, ..
                } => {
                    messages.extend(evidence.iter().map(|e| e.note.clone()));
                    messages.extend(fixes.iter().map(|f| f.message.clone()));
                }
                Outcome::Inconclusive { reason } | Outcome::Skipped { reason } => {
                    messages.push(reason.clone())
                }
                Outcome::NotApplicable { .. } => {}
            }
            for m in messages {
                assert!(!m.chars().any(is_cjk), "{} 的产出文案含非英文：{m}", c.id);
            }
        }
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x303F | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xFF00..=0xFFEF)
}

/// topic 建议只从人挑过的词里出，不从 README 正文里捞。
///
/// 这条来自 repolish 自己：CI 第一次跑远程分时，给自己的建议是
/// `cli, command, command-line, first, improve, like`——后三个是从 tagline
/// 「…the **first** thing…」「**improve**」「what it looks **like**」里
/// 逐词切出来的。停用词表挡不住实义动词，所以修的是词源不是过滤：
/// 清单里的 keywords 优先，正文一概不取。
#[test]
fn topic_suggestions_come_from_curated_keywords_not_prose() {
    let fx = Fixture::new(
        "topics-suggest",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"thing\"\nkeywords = [\"readme\", \"lint\", \"open-source\"]\n",
            ),
            (
                "README.md",
                "# thing\n\nScore and improve what your project looks like to the first stranger who finds it.\n",
            ),
            ("src/a.rs", "pub fn a() {}\n"),
            ("src/b.rs", "pub fn b() {}\n"),
            ("src/c.rs", "pub fn c() {}\n"),
        ],
    );

    let mut ctx = RepoContext::load(fx.path(), None).expect("载入仓库");
    ctx.remote = Some(RemoteFacts {
        default_branch: Some("main".to_string()),
        ..RemoteFacts::default()
    });
    let report = repolish_checks::registry().run(
        &ctx,
        &RunOptions {
            mode: Mode::Remote,
            ..RunOptions::default()
        },
    );
    let outcome = report
        .checks
        .into_iter()
        .find(|c| c.id == "repo-topics")
        .expect("检查项存在")
        .outcome;

    let Outcome::Scored { fixes, .. } = outcome else {
        panic!("没有 topic 应当判 0 分");
    };
    let msg = &fixes[0].message;

    for junk in ["first", "improve", "looks", "stranger", "score"] {
        assert!(
            !msg.contains(junk),
            "正文里的词不该出现在建议里：{junk}\n{msg}"
        );
    }
    for good in ["readme", "lint", "open-source", "rust"] {
        assert!(good_is_suggested(msg, good), "缺少 {good}\n{msg}");
    }
    // keywords 排在最前：读的人第一眼看到的应当是最该加的那个
    let list = msg.split("Candidates: ").nth(1).expect("有建议列表");
    assert!(list.starts_with("readme, lint, open-source"), "{list}");
}

fn good_is_suggested(msg: &str, word: &str) -> bool {
    msg.split("Candidates: ")
        .nth(1)
        .is_some_and(|l| l.split(", ").any(|w| w.trim() == word))
}

/// 清单在子目录里的业务项目要判成 app，不能落到 unknown。
///
/// 来自一个 `server/` + `web/` 的真实项目：两个清单分别在
/// `server/requirements.txt` 和 `web/package.json`，根目录一个都没有，
/// 于是三条判据全落空。这是一整类项目的形状，不是个例。
///
/// 反过来，serde / tokio 那种 workspace 有**根** Cargo.toml，在上一步就
/// 判成 library——区别正是「根目录发不发布东西」。
#[test]
fn subdirectory_manifests_make_it_an_app_not_unknown() {
    let app = Fixture::new(
        "profile-component-app",
        &[
            ("README.md", "# crm\n\nAn internal thing.\n"),
            ("server/requirements.txt", "flask\n"),
            ("server/main.py", "print('hi')\n"),
            ("web/package.json", "{\"name\": \"web\"}\n"),
        ],
    );
    let ctx = RepoContext::load(app.path(), None).expect("载入仓库");
    assert_eq!(ctx.profile.as_str(), "app", "子目录清单应当判成 app");

    // 根清单仍然优先判 library，不能被上面的规则抢走
    let lib = Fixture::new(
        "profile-root-workspace",
        &[
            ("README.md", "# thing\n\nA library.\n"),
            ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
            ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
        ],
    );
    let ctx = RepoContext::load(lib.path(), None).expect("载入仓库");
    assert_eq!(ctx.profile.as_str(), "library", "根清单应当判成 library");

    // examples/ 下的清单不算——那是示例，不是这个仓库的组件
    let ex = Fixture::new(
        "profile-example-only",
        &[
            ("README.md", "# thing\n\nSomething.\n"),
            ("examples/demo/package.json", "{\"name\": \"demo\"}\n"),
        ],
    );
    let ctx = RepoContext::load(ex.path(), None).expect("载入仓库");
    assert_ne!(ctx.profile.as_str(), "app", "示例目录里的清单不该判成 app");
}
