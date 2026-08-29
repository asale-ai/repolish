//! `verify --run` 的端到端测试。
//!
//! 用一个**假引擎**顶替 docker：一段 shell 脚本，把 repolish 交给它的那段
//! sh 拿出来，把 `/repo` 与 `/work` 改写成临时目录，然后本地跑掉。
//!
//! 这样测到的是真东西：真的分类器、真的脚本生成、真的哨兵解析、真的退出码。
//! 没测到的只有 `docker run` 那一行参数本身——而 CI 上装一个 docker 守护进程
//! 只为了确认「命令失败会被报成失败」，代价和收益不成比例。

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_repolish")
}

/// 假引擎。repolish 的调用形状是
/// `<engine> run --rm --name N -v SRC:/repo:ro -w /work … IMAGE sh -c SCRIPT`，
/// 所以脚本永远是最后一个参数。
fn fake_engine(dir: &Path, repo: &Path) -> PathBuf {
    let work = dir.join("work");
    let path = dir.join("fake-engine.sh");
    let script = format!(
        r#"#!/bin/sh
# repolish 的最后一个参数就是要跑的那段 sh
for a in "$@"; do last="$a"; done
rm -rf '{work}'
printf '%s' "$last" \
  | sed -e "s#/work#{work}#g" -e "s#/repo#{repo}#g" \
  | sh
"#,
        work = work.display(),
        repo = repo.display(),
    );
    std::fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

fn repo_with(readme: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("repolish-verify-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("README.md"), readme).unwrap();
    dir
}

struct Run {
    code: i32,
    /// stdout 与 stderr 合起来，给人读的断言用
    out: String,
    /// 只有 stdout。`--format json` 的断言必须用这个
    stdout: String,
}

fn verify(repo: &Path, extra: &[&str]) -> Run {
    let engine = fake_engine(repo, repo);
    let out = Command::new(bin())
        .arg("verify")
        .arg(repo)
        .arg("--run")
        .arg("--engine")
        .arg(&engine)
        .arg("--no-color")
        .args(extra)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    Run {
        code: out.status.code().unwrap_or(-1),
        out: format!("{stdout}{}", String::from_utf8_lossy(&out.stderr)),
        stdout,
    }
}

/// 这是整件事的核心：README 上写着一条命令，它跑不起来，
/// 于是 CI 变红并指出是 README 的第几行。
#[test]
fn a_command_that_fails_is_reported_with_its_readme_line_and_fails_the_run() {
    let repo = repo_with(
        "# t\n\n## Install\n\n```sh\necho hello\n./scripts/setup.sh\n```\n",
        "failing",
    );
    let r = verify(&repo, &[]);

    assert!(r.out.contains("FAILED"), "{}", r.out);
    // 好的那条通过，坏的那条带着行号被点名
    assert!(r.out.contains("1 passed"), "{}", r.out);
    assert!(r.out.contains("README.md:7"), "{}", r.out);
    assert!(r.out.contains("exited"), "{}", r.out);
    // 「命令没跑通」与「工具自身失败」是两回事,退出码 1 是前者
    assert_eq!(r.code, 1, "{}", r.out);

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn a_readme_whose_commands_all_work_exits_zero() {
    let repo = repo_with(
        "# t\n\n## Install\n\n```sh\necho one\ntrue\n```\n",
        "passing",
    );
    let r = verify(&repo, &[]);
    assert!(r.out.contains("PASSED"), "{}", r.out);
    assert!(r.out.contains("2 passed · 0 failed"), "{}", r.out);
    assert_eq!(r.code, 0, "{}", r.out);
    let _ = std::fs::remove_dir_all(&repo);
}

/// 会话是连着的：`cd` 之后的命令在新目录里跑。
/// README 的命令序列几乎总是依赖这一点。
#[test]
fn state_carries_from_one_command_to_the_next() {
    let repo = repo_with(
        "# t\n\n## Build\n\n```sh\nmkdir -p sub\ncd sub\npwd\n```\n",
        "session",
    );
    let r = verify(&repo, &["-v"]);
    assert!(r.out.contains("3 passed"), "{}", r.out);
    assert!(r.out.contains("/sub"), "{}", r.out);
    let _ = std::fs::remove_dir_all(&repo);
}

/// 一份「全部通过」的报告，如果一半是被悄悄跳过的，比没有报告更糟。
#[test]
fn every_skipped_command_is_listed_with_its_reason() {
    let repo = repo_with(
        "# t\n\n## Publish\n\n```sh\ntrue\nnpm publish\nsudo apt-get install jq\n```\n",
        "skips",
    );
    let r = verify(&repo, &[]);
    assert!(r.out.contains("2 skipped"), "{}", r.out);
    assert!(
        r.out.contains("publishes or writes to a remote"),
        "{}",
        r.out
    );
    assert!(r.out.contains("needs root"), "{}", r.out);
    // 跳过的不算失败
    assert_eq!(r.code, 0, "{}", r.out);
    let _ = std::fs::remove_dir_all(&repo);
}

/// `--section` 只取那一节的命令
#[test]
fn a_section_filter_takes_commands_from_that_heading_only() {
    let repo = repo_with(
        "# t\n\n## Install\n\n```sh\necho install\n```\n\n## Development\n\n```sh\necho dev\n```\n",
        "section",
    );
    let r = verify(&repo, &["--section", "Install", "-v"]);
    assert!(r.out.contains("echo install"), "{}", r.out);
    assert!(!r.out.contains("echo dev"), "{}", r.out);
    let _ = std::fs::remove_dir_all(&repo);
}

/// JSON 里每条命令都带着结论。CI 上要拿它做别的事的人读的是这个。
///
/// 断言用的是 **stdout**：进度信息必须全部走 stderr，否则下游第一个 `jq`
/// 就会炸在「Running 2 command(s)」上。
#[test]
fn the_json_carries_a_status_for_every_command() {
    let repo = repo_with("# t\n\n```sh\ntrue\nfalse\n```\n", "json");
    let r = verify(&repo, &["--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    let steps = v["steps"].as_array().unwrap();
    assert_eq!(steps[0]["status"], "passed");
    assert_eq!(steps[1]["status"], "failed");
    assert_eq!(steps[1]["exit_code"], 1);
    let _ = std::fs::remove_dir_all(&repo);
}
