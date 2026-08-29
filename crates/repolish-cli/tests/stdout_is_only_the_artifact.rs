//! `--stdout` 的约定是 stdout 上只有那一份产物。
//!
//! 这条约定曾经是破的：进度输出按 `--format text` 走 println，于是
//! `repolish --stages artifacts --artifact score --stdout > card.svg` 写出来的
//! 文件在 `</svg>` 之后还拖着一段「NOT RUN — these stages are opt-in」。
//! 浏览器容忍这种尾巴，`svgo` 和任何一个当真的 XML 解析器不容忍——而
//! 重定向到文件正是我们自己的 SKILL.md 教的用法。
//!
//! 静态读代码守不住这一条：say! 是个宏，散在几十处调用点上，漏掉哪一处
//! 都要把二进制真的跑一遍才看得见。

use std::process::Command;

fn artifact(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_repolish"))
        .args([
            "--stages",
            "artifacts",
            "--stdout",
            "--no-remote",
            "--no-stars",
        ])
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("could not run repolish {args:?}: {e}"));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn an_svg_artifact_reaches_stdout_with_nothing_after_it() {
    for theme in ["dark", "porcelain", "carbon", "paper"] {
        let svg = artifact(&["--artifact", "score", "--theme", theme]);
        assert!(
            svg.starts_with("<svg "),
            "--theme {theme}: stdout does not begin with the SVG:\n{}",
            &svg[..svg.len().min(200)]
        );
        assert!(
            svg.trim_end().ends_with("</svg>"),
            "--theme {theme}: something followed the SVG on stdout:\n{}",
            &svg[svg.len().saturating_sub(300)..]
        );
    }
}

/// 每一套 `--theme` 都必须真的画得出来。
///
/// `Theme` 枚举、`Palette` 常量、`palette()` 的 match 是三处分开写的东西，
/// 少一处就是一个在 `--help` 里列着、跑起来却报错的取值。
#[test]
fn every_theme_the_help_lists_renders_a_card() {
    let help = Command::new(env!("CARGO_BIN_EXE_repolish"))
        .arg("--help")
        .output()
        .expect("could not run repolish --help");
    let help = String::from_utf8_lossy(&help.stdout).into_owned();
    let (_, after) = help.split_once("--theme").expect("--help lists no --theme");
    let listed: Vec<&str> = after
        .lines()
        .skip_while(|l| !l.contains("Possible values:"))
        .skip(1)
        .take_while(|l| l.trim().starts_with('-'))
        .filter_map(|l| l.trim().trim_start_matches("- ").split(':').next())
        .collect();
    assert!(
        listed.len() >= 14,
        "expected every palette in --help, found {listed:?}"
    );
    for theme in listed {
        let svg = artifact(&["--artifact", "score", "--theme", theme]);
        assert!(
            svg.starts_with("<svg "),
            "--theme {theme} is offered by --help but draws nothing"
        );
    }
}
