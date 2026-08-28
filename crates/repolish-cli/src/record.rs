//! 跑命令，把输出录下来。
//!
//! 渲染在 [`repolish_render::cast`]，这里只管**执行与捕获**。
//!
//! 三条决定值得写下来：
//!
//! - **真的跑。** 不排演、不摆拍。README 上那段录屏里出现的分数，就是那台
//!   机器上跑出来的分数。一个专门检查「README 承诺的命令是否真的存在」的
//!   工具，自己的演示不能是编的。
//! - **强制开色。** 输出接的是管道不是终端，绝大多数 CLI 会自动关掉颜色。
//!   `CLICOLOR_FORCE` 与 `FORCE_COLOR` 是既成约定，两个都设。仍然不认的
//!   程序录出来就是黑白的——那是它的选择，我们不去伪造一个 PTY。
//! - **不带 PTY。** 拉一个伪终端进来能让最后那一小撮程序也吐颜色，代价是
//!   一个平台相关的依赖，以及 Windows 上另一条实现。为一个演示功能不值当。
//!   代价写在这儿，将来有人要加，至少知道换的是什么。

use std::process::{Command, Stdio};

use repolish_render::cast::{Screen, Step};

/// 一次录制的结果。
#[derive(Debug)]
pub struct Recording {
    pub steps: Vec<Step>,
    /// 有非零退出码的命令。录屏照录——一条失败的命令也是真实的输出，
    /// 而悄悄扔掉它会让使用者对着一段缺了一节的录屏发呆。
    pub failures: Vec<(String, i32)>,
}

/// 依次跑每条命令，捕获输出。
///
/// `cwd` 是每条命令的工作目录。`announce` 收到即将执行的命令——
/// **执行别人机器上的程序这件事，必须让使用者看得见。**
pub fn run(
    commands: &[String],
    cwd: &std::path::Path,
    mut announce: impl FnMut(&str),
) -> Result<Recording, String> {
    let mut steps = Vec::new();
    let mut failures = Vec::new();

    for command in commands {
        announce(command);
        let (program, args) = split(command)?;

        let out = Command::new(&program)
            .args(&args)
            .current_dir(cwd)
            // 管道后面绝大多数 CLI 会自动关色。这两个变量是既成约定。
            .env("CLICOLOR_FORCE", "1")
            .env("FORCE_COLOR", "1")
            .env("TERM", "xterm-256color")
            .env_remove("NO_COLOR")
            // 终端宽度。不设的话有些工具会按 80 折行，而我们的画布更宽
            .env("COLUMNS", "100")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run `{program}`: {e}"))?;

        // stdout 与 stderr 都要：repolish 自己就把「wrote …」写在 stderr 上，
        // 只录 stdout 的话，演示 `polish --apply` 会得到一段空白
        let mut screen = Screen::new();
        screen.feed(&String::from_utf8_lossy(&out.stdout));
        screen.feed(&String::from_utf8_lossy(&out.stderr));

        if !out.status.success() {
            failures.push((command.clone(), out.status.code().unwrap_or(-1)));
        }
        steps.push(Step {
            command: command.clone(),
            output: screen.finish(),
        });
    }

    Ok(Recording { steps, failures })
}

/// 把一条命令行切成程序名与参数。
///
/// **不经过 shell。** `sh -c` 会让 tape 里的一行字获得管道、重定向和变量展开
/// 的全部能力——对一个「跑一跑给人看」的功能来说，那个能力面太大了。
/// 认单双引号，认到此为止；需要管道的人自己写脚本，然后录那个脚本。
fn split(command: &str) -> Result<(String, Vec<String>), String> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;

    for c in command.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '\'') | (None, '"') => {
                quote = Some(c);
                // 空引号也是一个参数：`--sep ""` 不能被丢掉
                any = true;
            }
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() || any {
                    parts.push(std::mem::take(&mut current));
                    any = false;
                }
            }
            (None, c) => current.push(c),
        }
    }
    if quote.is_some() {
        return Err(format!("unterminated quote in `{command}`"));
    }
    if !current.is_empty() || any {
        parts.push(current);
    }
    if parts.is_empty() {
        return Err("empty command".to_string());
    }
    let program = parts.remove(0);
    Ok((program, parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_command_splits_on_whitespace() {
        let (p, a) = split("repolish check .").unwrap();
        assert_eq!(p, "repolish");
        assert_eq!(a, vec!["check", "."]);
    }

    #[test]
    fn quoted_arguments_stay_together() {
        let (_, a) = split(r#"tool --only "a b" --x 'c d'"#).unwrap();
        assert_eq!(a, vec!["--only", "a b", "--x", "c d"]);
    }

    #[test]
    fn an_empty_quoted_argument_survives() {
        let (_, a) = split(r#"tool --sep "" x"#).unwrap();
        assert_eq!(a, vec!["--sep", "", "x"]);
    }

    #[test]
    fn an_unterminated_quote_is_an_error_rather_than_a_guess() {
        assert!(split(r#"tool "unclosed"#).is_err());
        assert!(split("   ").is_err());
    }

    /// 不经过 shell：管道和重定向只是普通参数，不会被执行
    #[test]
    fn shell_metacharacters_are_arguments_not_operators() {
        let (p, a) = split("tool a | rm -rf /").unwrap();
        assert_eq!(p, "tool");
        assert_eq!(a, vec!["a", "|", "rm", "-rf", "/"]);
    }

    #[test]
    fn extra_whitespace_does_not_produce_empty_arguments() {
        let (_, a) = split("tool   a    b").unwrap();
        assert_eq!(a, vec!["a", "b"]);
    }

    /// 录制要真的跑命令，并且把 stderr 也收进去
    #[test]
    fn it_records_real_output_from_both_streams() {
        let dir = std::env::temp_dir();
        let mut announced = Vec::new();
        let rec = run(&["echo hello".to_string()], &dir, |c| {
            announced.push(c.to_string())
        })
        .unwrap();
        assert_eq!(announced, vec!["echo hello"]);
        assert_eq!(rec.steps.len(), 1);
        assert_eq!(rec.steps[0].output[0].plain().trim(), "hello");
        assert!(rec.failures.is_empty());
    }

    /// 一条失败的命令也是真实输出，照录，但要报出来
    #[test]
    fn a_failing_command_is_recorded_and_reported() {
        let dir = std::env::temp_dir();
        let rec = run(&["false".to_string()], &dir, |_| {}).unwrap();
        assert_eq!(rec.steps.len(), 1);
        assert_eq!(rec.failures.len(), 1);
        assert_eq!(rec.failures[0].0, "false");
    }

    #[test]
    fn a_missing_binary_is_an_error_with_its_name_in_it() {
        let dir = std::env::temp_dir();
        let err = run(
            &["definitely-not-a-real-binary-xyz".to_string()],
            &dir,
            |_| {},
        )
        .unwrap_err();
        assert!(err.contains("definitely-not-a-real-binary-xyz"), "{err}");
    }
}
