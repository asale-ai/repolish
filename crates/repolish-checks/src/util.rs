//! 检查项共用的小工具。
//!
//! 重点是**从 README 代码块里认出「命令行」**。`claim-consistency` 与
//! `readme-install-consistency` 都靠它，而这两项一旦误报就会直接摧毁工具的可信度
//! （见 docs/03 设计原则 4），所以这里的取舍一律偏保守：宁可少认，不可错认。
//!
//! [`command_lines`] 是公开的，因为 `repolish verify` 要执行的正是
//! `claim-consistency` 静态核对过的那同一批命令。两处各自认一遍「什么算命令」，
//! 迟早会给出互相矛盾的两份结论。

use repolish_md::Readme;

/// 被当作 shell 的围栏语言标记。空标记也算——大量 README 的安装命令不写语言。
const SHELL_INFO: &[&str] = &[
    "",
    "sh",
    "bash",
    "shell",
    "zsh",
    "console",
    "shell-session",
    "terminal",
    "cmd",
    "powershell",
    "ps1",
    "fish",
];

pub fn readme_name(readme: &Readme) -> String {
    readme
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// 代码块里的命令行，附带在 README 中的 1-based 行号。
///
/// 会剥掉提示符，并丢弃注释行与命令的输出行。
pub fn command_lines(readme: &Readme) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for cb in &readme.code_blocks {
        let info = cb
            .info
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();
        if !SHELL_INFO.contains(&info.as_str()) {
            continue;
        }
        for (i, raw) in cb.literal.lines().enumerate() {
            if let Some(cmd) = as_command(raw) {
                // 代码块的 sourcepos 指向围栏行，正文从下一行开始
                out.push((cb.line + 1 + i, cmd));
            }
        }
    }
    out
}

/// 指定语言的代码块正文
pub fn blocks_with_info<'a>(
    readme: &'a Readme,
    langs: &'a [&str],
) -> impl Iterator<Item = &'a repolish_md::CodeBlock> {
    readme.code_blocks.iter().filter(move |cb| {
        let info = cb
            .info
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();
        langs.contains(&info.as_str())
    })
}

/// 一行是否是可执行的命令；是则返回剥掉提示符后的内容。
///
/// `console` 块里命令与输出混排，只有带提示符的才是命令；但更多 README 用
/// 无提示符的裸命令。折中：有提示符的取提示符之后，无提示符的整行都算，
/// 再靠「注释行」「明显是输出」两条规则筛掉噪声。
fn as_command(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.starts_with('#') || t.starts_with("//") {
        return None;
    }
    let t = t
        .strip_prefix("$ ")
        .or_else(|| t.strip_prefix("> "))
        .or_else(|| t.strip_prefix("% "))
        .or_else(|| t.strip_prefix("PS> "))
        .unwrap_or(t)
        .trim();

    // 续行、管道续段、明显的输出行（缩进的表格 / JSON）不作为命令起点
    if t.is_empty() || t.starts_with('|') || t.starts_with('{') || t.starts_with('}') {
        return None;
    }

    // 行尾注释要剥掉。ruff 的 README 写成
    // `ruff check path/to/code/*.py    # Lint all `.py` files`，
    // 不剥就会把注释里的 `.py` 当成一个文件路径声明。
    // 代价：命令里带 ` #` 的（如某些 sed 表达式）会被截断，可接受。
    let t = t.split_once(" #").map(|(head, _)| head.trim()).unwrap_or(t);
    (!t.is_empty()).then(|| t.to_string())
}

/// 取命令中动词之后的「参数」，跳过所有以 `-` 开头的选项。
///
/// `pip install -U --pre requests` → `["requests"]`
pub fn args_after(cmd: &str, verb: &str) -> Vec<String> {
    let lower = cmd.to_lowercase();
    let Some(i) = lower.find(verb) else {
        return Vec::new();
    };
    cmd[i + verb.len()..]
        .split_whitespace()
        .take_while(|t| !matches!(*t, "&&" | "||" | ";" | "|"))
        .filter(|t| !t.starts_with('-'))
        .map(str::to_string)
        .collect()
}

/// 首个非选项参数
pub fn first_arg(cmd: &str, verb: &str) -> Option<String> {
    args_after(cmd, verb).into_iter().next()
}

/// 计数名词的复数后缀。中文原文没有这个问题，改英文后一路都是
/// 「1 translations」「2 CI config(s)」，成品文案里很扎眼。
pub fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds(md: &str) -> Vec<String> {
        let r = Readme::parse("README.md", md);
        command_lines(&r).into_iter().map(|(_, c)| c).collect()
    }

    #[test]
    fn strips_prompts_and_drops_comments() {
        let md = "```bash\n# 安装\n$ npm install foo\nnpm run build\n```\n";
        assert_eq!(cmds(md), vec!["npm install foo", "npm run build"]);
    }

    #[test]
    fn non_shell_blocks_are_not_commands() {
        // Rust 示例里的 `let x = ...` 不是命令
        assert!(cmds("```rust\nlet x = 1;\n```\n").is_empty());
    }

    #[test]
    fn line_numbers_point_at_the_command_not_the_fence() {
        let md = "# T\n\n```sh\ncargo install repolish\n```\n";
        let r = Readme::parse("README.md", md);
        let found = command_lines(&r);
        assert_eq!(found[0].0, 4);
    }

    #[test]
    fn options_are_not_package_names() {
        assert_eq!(
            first_arg("pip install -U --pre requests", "pip install").as_deref(),
            Some("requests")
        );
        assert_eq!(first_arg("npm install", "npm install"), None);
    }
}
