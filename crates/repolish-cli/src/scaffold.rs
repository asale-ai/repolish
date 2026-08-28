//! `polish` 会写出去的新文件。
//!
//! 收进这里的标准只有一条：**内容能从仓库里已有的事实推出来，不需要猜。**
//!
//! - issue / PR 模板是纯脚手架。GitHub 自己的表单 schema，问的是版本号、
//!   复现步骤、改了什么——没有一处是项目特有的，因此没有可猜的余地。
//! - CONTRIBUTING 里的构建与测试命令来自**探测到的包清单**，不是模板占位符。
//!   探测不出生态就不生成：与其写一份 `<your build command here>`，
//!   不如让检查项继续扣分。
//!
//! 明确**不**生成行为准则。Contributor Covenant 是一份标准文本，唯一项目
//! 特有的是举报邮箱——而那个我们推不出来。一份留着占位符的行为准则，
//! 承诺了一条并不存在的举报通道，比没有更糟。

use repolish_ingest::{Ecosystem, Manifest, RepoSlug};

/// 一套能真正跑起来的本地开发命令。
pub struct Toolchain {
    /// 前置条件里提到的名字，如 "Rust"
    pub needs: &'static str,
    /// 代码块的语言标记
    pub lang: &'static str,
    pub build: &'static str,
    /// 没有可用的测试命令时为 `None`——编一条跑不通的命令，
    /// 正是这个工具用 `claim-consistency` 抓别人的事情
    pub test: Option<String>,
}

/// 从包清单推出本地开发命令。多个清单时取第一个认得的。
pub fn toolchain(manifests: &[Manifest]) -> Option<Toolchain> {
    let m = manifests.first()?;
    let t = match m.ecosystem {
        Ecosystem::Cargo => Toolchain {
            needs: "Rust",
            lang: "bash",
            build: "cargo build",
            test: Some("cargo test".into()),
        },
        Ecosystem::Npm => Toolchain {
            needs: "Node.js",
            lang: "bash",
            build: "npm install",
            // package.json 里没有 test 脚本时 `npm test` 会直接报错
            test: m
                .scripts
                .iter()
                .any(|s| s == "test")
                .then(|| "npm test".to_string()),
        },
        Ecosystem::Pypi => Toolchain {
            needs: "Python",
            lang: "bash",
            build: "pip install -e .",
            test: Some("pytest".into()),
        },
        Ecosystem::Go => Toolchain {
            needs: "Go",
            lang: "bash",
            build: "go build ./...",
            test: Some("go test ./...".into()),
        },
        Ecosystem::Maven => Toolchain {
            needs: "Java and Maven",
            lang: "bash",
            build: "mvn compile",
            test: Some("mvn test".into()),
        },
        Ecosystem::Gem => Toolchain {
            needs: "Ruby",
            lang: "bash",
            build: "bundle install",
            test: Some("bundle exec rake test".into()),
        },
        Ecosystem::Composer => Toolchain {
            needs: "PHP and Composer",
            lang: "bash",
            build: "composer install",
            test: None,
        },
    };
    Some(t)
}

/// GitHub 的 issue 表单。问的三件事，正是没有模板时最常缺的三件事：
/// 发生了什么、怎么复现、什么版本。
pub fn bug_report(project: &str) -> String {
    format!(
        r#"name: Bug report
description: Something in {project} does not work the way it should
labels: [bug]
body:
  - type: textarea
    id: what-happened
    attributes:
      label: What happened
      description: What you did, what you expected, and what you got instead
    validations:
      required: true

  - type: textarea
    id: reproduce
    attributes:
      label: How to reproduce
      description: The smallest set of steps that shows the problem
      placeholder: |-
        1.
        2.
    validations:
      required: true

  - type: input
    id: version
    attributes:
      label: Version
      description: Which version of {project} are you running
    validations:
      required: true

  - type: input
    id: platform
    attributes:
      label: Operating system
"#
    )
}

pub fn feature_request(project: &str) -> String {
    format!(
        r#"name: Feature request
description: Suggest something {project} should be able to do
labels: [enhancement]
body:
  - type: textarea
    id: problem
    attributes:
      label: The problem
      description: What are you trying to do that is hard or impossible today
    validations:
      required: true

  - type: textarea
    id: proposal
    attributes:
      label: What you have in mind
      description: Roughly how it would work, if you have a shape in mind

  - type: textarea
    id: alternatives
    attributes:
      label: What you tried instead
      description: Workarounds you are using today, and why they fall short
"#
    )
}

/// PR 模板。清单最后一条用**真实的**测试命令，测不了就不写那一条。
pub fn pull_request_template(test: Option<&str>) -> String {
    let mut out = String::from(
        "## What changed\n\n\
         <!-- One or two sentences. Link the issue this closes, if there is one. -->\n\n\
         ## Why\n\n\
         <!-- What problem does this solve? -->\n\n\
         ## Checklist\n\n\
         - [ ] The change is covered by a test, or there is a reason it cannot be\n\
         - [ ] Documentation is updated if the behaviour changed\n",
    );
    if let Some(cmd) = test {
        out.push_str(&format!("- [ ] `{cmd}` passes locally\n"));
    }
    out
}

/// 贡献指南。
///
/// 每一条命令都来自探测到的包清单——这份文件里没有一句是编的，
/// 也因此不接受「探测不出生态」的情况，那时调用方不该走到这里。
pub fn contributing(project: &str, slug: Option<&RepoSlug>, t: &Toolchain) -> String {
    let clone = match slug {
        Some(s) => format!(
            "git clone https://github.com/{}/{}\ncd {}\n",
            s.owner, s.name, s.name
        ),
        None => String::new(),
    };
    let test_line = t
        .test
        .as_ref()
        .map(|c| format!("{c}\n"))
        .unwrap_or_default();
    let before_pr = match &t.test {
        Some(c) => format!("- Run `{c}` and make sure it passes.\n"),
        None => String::new(),
    };

    format!(
        "# Contributing to {project}\n\n\
         Issues and pull requests are welcome. This file covers how to get the project\n\
         running locally and what a pull request is expected to carry.\n\n\
         ## Local development\n\n\
         Requires {needs}.\n\n\
         ```{lang}\n\
         {clone}{build}\n\
         {test_line}\
         ```\n\n\
         ## Before opening a pull request\n\n\
         {before_pr}\
         - Keep the change focused: one concern per pull request is easier to review\n  \
         and easier to revert.\n\
         - If the behaviour changed, update the README in the same commit. Documentation\n  \
         that lags behind the code is worse than no documentation.\n\n\
         ## Reporting a bug\n\n\
         Open an issue with the version you are running and the smallest set of steps\n\
         that reproduces the problem. A report without a reproduction usually costs a\n\
         few round trips before triage can even start.\n",
        needs = t.needs,
        lang = t.lang,
        build = t.build,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(ecosystem: Ecosystem, scripts: &[&str]) -> Manifest {
        Manifest {
            ecosystem,
            path: "manifest".into(),
            name: Some("thing".into()),
            scripts: scripts.iter().map(|s| s.to_string()).collect(),
            bins: vec![],
            deps: vec![],
            keywords: vec![],
        }
    }

    /// package.json 里没有 test 脚本时，`npm test` 是跑不通的——
    /// 编一条跑不通的命令，正是这个工具用 claim-consistency 抓别人的事
    #[test]
    fn npm_without_a_test_script_gets_no_test_command() {
        let t = toolchain(&[manifest(Ecosystem::Npm, &["build"])]).unwrap();
        assert_eq!(t.test, None);
        let t = toolchain(&[manifest(Ecosystem::Npm, &["build", "test"])]).unwrap();
        assert_eq!(t.test.as_deref(), Some("npm test"));
    }

    #[test]
    fn no_manifest_means_no_toolchain() {
        assert!(toolchain(&[]).is_none());
    }

    /// contributing 检查项要求「非空行 ≥ 15 行」且出现本地开发命令，
    /// 生成的文件必须真的过得了这一关，否则等于白写
    #[test]
    fn the_generated_guide_clears_the_bar_the_check_sets() {
        let t = toolchain(&[manifest(Ecosystem::Cargo, &[])]).unwrap();
        let slug = RepoSlug {
            owner: "acme".into(),
            name: "thing".into(),
        };
        let md = contributing("thing", Some(&slug), &t);

        let lines = md.lines().filter(|l| !l.trim().is_empty()).count();
        assert!(lines >= 15, "只有 {lines} 行非空内容:\n{md}");
        assert!(md.contains("```"), "要有代码块:\n{md}");
        assert!(md.contains("cargo test"), "要有真实的测试命令:\n{md}");
        assert!(md.contains("git clone https://github.com/acme/thing"));
    }

    /// 推不出测试命令时，宁可不写那一行，也不写一条假的
    #[test]
    fn a_toolchain_without_tests_omits_the_test_line_rather_than_inventing_one() {
        let t = toolchain(&[manifest(Ecosystem::Composer, &[])]).unwrap();
        let md = contributing("thing", None, &t);
        assert!(!md.contains("Run `"), "不该出现测试命令:\n{md}");
        assert!(md.contains("composer install"));

        let pr = pull_request_template(t.test.as_deref());
        assert!(!pr.contains("passes locally"), "{pr}");
    }

    #[test]
    fn the_pull_request_template_uses_the_real_test_command() {
        let pr = pull_request_template(Some("cargo test"));
        assert!(pr.contains("`cargo test` passes locally"), "{pr}");
    }

    /// 生成的表单要是 GitHub 认得的形状：顶层 name / description / body
    #[test]
    fn issue_forms_have_the_shape_github_expects() {
        for md in [bug_report("thing"), feature_request("thing")] {
            assert!(md.starts_with("name: "), "{md}");
            assert!(md.contains("description: "), "{md}");
            assert!(md.contains("body:"), "{md}");
            assert!(md.contains("thing"), "项目名应被代入:\n{md}");
        }
    }
}
