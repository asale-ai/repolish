//! 工具吐给使用者的每一个字都必须是英文（CONTRIBUTING 第三条）。
//!
//! `repolish-checks` 那边已经有一条测试守着**检查项的文案**，但那条守不到
//! 这里：`--help` 是 clap 从 `///` 文档注释生成的，而这个仓库的文档注释
//! 按约定是中文。两者一撞，中文就会从 `--help` 漏出去——而且确实漏过：
//! `--theme` 与 `--tables` 的取值说明曾经整段是中文。
//!
//! 唯一可靠的判据是**真的把 help 跑出来看**。静态扫源码分不清哪一条 `///`
//! 会被 clap 渲染、哪一条只是给读代码的人看的。
//!
//! 卡片上的字不在此列：那是贴进别人 README 的东西，走 `repolish-render`
//! 的 i18n 文案表，本来就该跟着那份 README 的语言走。

use std::process::Command;

/// 流水线的每一段。子命令取消之后，`--stages` 的取值说明是唯一还会被 clap
/// 渲染出来的一组文档注释，漏掉一个就没人守。
const STAGES: &[&str] = &["check", "polish", "artifacts", "ci", "skill", "demo"];

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_repolish")
}

fn help(args: &[&str]) -> String {
    let out = Command::new(bin())
        .args(args)
        .arg("--help")
        .output()
        .unwrap_or_else(|e| panic!("could not run repolish {args:?} --help: {e}"));
    assert!(
        out.status.success(),
        "repolish {args:?} --help exited with {}",
        out.status
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn cjk(s: &str) -> Vec<String> {
    s.lines()
        .filter(|l| {
            l.chars()
                .any(|c| matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF))
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn every_help_screen_is_english() {
    let mut offenders: Vec<(String, String)> = Vec::new();

    // 子命令没了，`--help` 就是全部的 help。长格式才会渲染 value enum
    // 的说明，所以这里必须用 `--help` 而不是 `-h`。
    for line in cjk(&help(&[])) {
        offenders.push(("repolish".into(), line));
    }

    assert!(
        offenders.is_empty(),
        "Chinese reached --help. Those doc comments are user-facing output; \
         move the reasoning to a plain // comment and write the /// in English:\n{}",
        offenders
            .iter()
            .map(|(c, l)| format!("  {c}: {}", l.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// 每一段都要在 `--help` 里露面。少一个，它的说明就没人守。
#[test]
fn help_documents_every_stage() {
    let top = help(&[]);
    for stage in STAGES {
        assert!(
            top.contains(stage),
            "`{stage}` is a --stages value but does not appear in --help, so its \
             documentation is unchecked"
        );
    }
}

/// 子命令已经取消。留一个还能解析的子命令，等于同一件事有两种敲法。
#[test]
fn there_are_no_subcommands() {
    for word in ["check", "polish", "badge", "report", "card", "demo", "skill", "init"] {
        let out = Command::new(bin()).arg(word).output().expect("run");
        assert!(
            !out.status.success(),
            "`repolish {word}` still parses; it should be rejected as a path"
        );
    }
}

/// 错误提示同样是工具输出。它们往往是最长、最容易顺手写成中文的一类文案。
#[test]
fn error_messages_are_english_too() {
    let cases: &[&[&str]] = &[
        &["/definitely/not/a/path"],
        &["--stages", "skill", "--target", "not-an-agent", "--apply"],
        &[".", "--profile", "nonsense"],
    ];
    for args in cases {
        let out = Command::new(bin()).args(*args).output().expect("run");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let bad = cjk(&text);
        assert!(
            bad.is_empty(),
            "repolish {args:?} printed Chinese:\n{bad:#?}"
        );
    }
}
