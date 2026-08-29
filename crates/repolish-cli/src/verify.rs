//! `verify`：README 里的命令，在一台**干净的机器**上真的跑得起来吗。
//!
//! `claim-consistency` 这个检查项只做静态核对——`npm run build` 在
//! `package.json` 里吗，`./scripts/setup.sh` 这个文件存在吗。它能抓到改名和
//! 删除，抓不到「命令还在，但少了一个系统依赖，在别人机器上第一步就炸」。
//! 而后者正是新用户真正会遇到的那一种。
//!
//! 所以这里往前走一步：把 README 的命令**真的执行一遍**，在一个只有基础镜像
//! 的容器里，从零开始。
//!
//! 三条不打算让步的规则：
//!
//! - **不在宿主机上跑。** README 是别人写的，里面的命令可能装东西、改配置、
//!   写文件。没有容器就不执行——降级到宿主机去跑是不能接受的默认行为。
//! - **仓库只读挂载。** 容器把 `/repo` 复制到 `/work` 再动手，使用者的工作区
//!   在任何情况下都不会被 README 里的命令改写。
//! - **跳过的每一条都要说明理由。** 一份「12 条命令全部通过」的报告，如果其中
//!   9 条是被悄悄跳过的，比没有报告更糟。

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use repolish_ingest::{Ecosystem, RepoContext};
use serde::Serialize;

/// 一条命令在计划里的去向。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// 会被执行
    Run,
    /// 不执行，附理由。理由是给使用者看的，不是给日志看的。
    Skip { reason: &'static str },
}

impl Verdict {
    fn skip(reason: &'static str) -> Self {
        Verdict::Skip { reason }
    }

    pub fn is_run(&self) -> bool {
        matches!(self, Verdict::Run)
    }
}

/// 执行结果。`NotRun` 是「计划里要跑，但前面超时/容器挂了所以没轮到」——
/// 与「跳过」是两回事，不能混在一起报。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Status {
    Passed,
    Failed { exit_code: i32 },
    NotRun,
}

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    /// README 中的 1-based 行号
    pub line: usize,
    pub command: String,
    #[serde(flatten)]
    pub verdict: Verdict,
    /// 只有真的跑过才有值。摊平进对象：嵌成 `"status": {"status": …}`
    /// 的话，第一个写 `jq` 的人都会踩一次
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    /// 该条命令的输出（stdout 与 stderr 合流，保持原顺序）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub readme: String,
    pub image: String,
    /// 镜像是怎么选出来的。默认值也要说得出理由，否则跑挂了没人知道该换什么
    pub image_reason: String,
    pub steps: Vec<Step>,
}

impl Plan {
    pub fn runnable(&self) -> usize {
        self.steps.iter().filter(|s| s.verdict.is_run()).count()
    }

    pub fn failed(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.status, Some(Status::Failed { .. })))
            .count()
    }

    pub fn passed(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.status, Some(Status::Passed)))
            .count()
    }

    pub fn not_run(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.status, Some(Status::NotRun)))
            .count()
    }
}

// ── 计划 ────────────────────────────────────────────────────────────────

/// 从 README 建一份执行计划。**不执行任何东西。**
pub fn plan(ctx: &RepoContext, image: Option<&str>, sections: &[String]) -> Result<Plan, String> {
    let readme = ctx
        .readme
        .as_ref()
        .ok_or_else(|| "no README to verify".to_string())?;

    let wanted: Option<BTreeSet<String>> = if sections.is_empty() {
        None
    } else {
        Some(sections.iter().map(|s| s.to_lowercase()).collect())
    };

    let mut steps = Vec::new();
    for (line, command) in join_continuations(repolish_checks::util::command_lines(readme)) {
        if let Some(wanted) = &wanted {
            let in_section = readme
                .sections
                .iter()
                .filter(|s| s.contains_line(line))
                .any(|s| {
                    let title = s.title.to_lowercase();
                    wanted.iter().any(|w| title.contains(w))
                });
            if !in_section {
                continue;
            }
        }
        let verdict = classify(&command);
        steps.push(Step {
            line,
            command,
            verdict,
            status: None,
            output: None,
        });
    }

    let (image, image_reason) = match image {
        Some(i) => (i.to_string(), "given with --image".to_string()),
        None => default_image(ctx),
    };

    Ok(Plan {
        readme: readme
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        image,
        image_reason,
        steps,
    })
}

/// 反斜杠续行的命令要合成一条。分开跑的话，第二行会以 `--flag` 开头，
/// 它自己不是命令——报一条 `--flag: not found` 只会让人以为 README 坏了。
fn join_continuations(lines: Vec<(usize, String)>) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut pending: Option<(usize, String)> = None;

    for (line, cmd) in lines {
        let continues = cmd.ends_with('\\');
        let body = cmd.trim_end_matches('\\').trim_end().to_string();
        match &mut pending {
            Some((_, acc)) => {
                acc.push(' ');
                acc.push_str(body.trim());
            }
            None => pending = Some((line, body)),
        }
        if !continues {
            if let Some(done) = pending.take() {
                out.push(done);
            }
        }
    }
    // 最后一行还带着续行符：README 写错了，把已有的部分收下,不静默丢掉
    if let Some(done) = pending.take() {
        out.push(done);
    }
    out
}

/// 会把宿主机或使用者的账号搞坏的动词。容器隔离了文件系统，
/// **隔离不了网络那一头**——`npm publish` 在容器里一样会发布。
const OUTBOUND: &[&str] = &[
    "git push",
    "npm publish",
    "yarn publish",
    "pnpm publish",
    "cargo publish",
    "cargo owner",
    "gh release create",
    "gh pr create",
    "gh issue create",
    "docker push",
    "twine upload",
    "gem push",
    "mvn deploy",
    "helm push",
    "terraform apply",
    "kubectl apply",
    "aws ",
    "gcloud ",
    "az ",
];

/// 不会自己结束的命令。跑它们等于挂着直到超时，报出来的「失败」是假的。
const LONG_RUNNING: &[&str] = &[
    "npm start",
    "npm run dev",
    "npm run serve",
    "npm run start",
    "yarn dev",
    "yarn start",
    "pnpm dev",
    "pnpm start",
    "bun dev",
    "cargo watch",
    "cargo run",
    "go run",
    "flask run",
    "rails server",
    "rails s",
    "python -m http.server",
    "jupyter notebook",
    "jupyter lab",
    "mkdocs serve",
    "hugo server",
    "vite",
    "nodemon",
    "watchexec",
    "tail -f",
    "ssh ",
];

/// 需要一个我们没有提供的容器/集群运行时。验证本身就跑在容器里，里面没有
/// docker 守护进程——照跑只会得到一句 `docker: not found`，那是**我们**的
/// 环境限制，不是这份 README 的错。误报一条，比漏报一条伤得多。
const NESTED_RUNTIME: &[&str] = &[
    "docker ",
    "docker-compose ",
    "podman ",
    "kubectl ",
    "minikube ",
    "helm ",
    "vagrant ",
];

/// 交互式的东西，没有 TTY 就是挂着或者立刻崩
const INTERACTIVE: &[&str] = &[
    "vim ", "vi ", "nano ", "emacs ", "less ", "more ", "man ", "top", "htop", "watch ",
];

/// 明显的破坏性操作。容器里也不跑——一份会 `rm -rf` 的报告，
/// 没人敢在自己机器上第二次运行。
const DESTRUCTIVE: &[&str] = &[
    "rm -rf /",
    "mkfs",
    "dd if=",
    "shutdown",
    "reboot",
    "chmod -R 777 /",
    ":(){",
];

/// 占位符标记。README 里的 `<YOUR_TOKEN>` 不是命令的一部分，是留给读者填的。
const PLACEHOLDERS: &[&str] = &[
    "your-",
    "your_",
    "youruser",
    "yourname",
    "path/to/",
    "/path/to",
    "example.com",
    "<owner>",
    "<repo>",
    "user@",
    "…",
];

/// 一条命令跑还是不跑。
///
/// 取舍方向是**宁可多跳过，不可乱跑**：漏掉一条本可以验证的命令，代价是
/// 报告少了一行；跑了一条不该跑的，代价是别人的仓库被发布了一个版本。
pub fn classify(cmd: &str) -> Verdict {
    let t = cmd.trim();
    let lower = t.to_lowercase();

    if t.is_empty() {
        return Verdict::skip("empty");
    }
    if DESTRUCTIVE.iter().any(|d| lower.contains(d)) {
        return Verdict::skip("destructive");
    }
    if lower.starts_with("sudo ") || lower.contains(" sudo ") {
        return Verdict::skip("needs root");
    }
    if OUTBOUND.iter().any(|d| lower.contains(d)) {
        return Verdict::skip("publishes or writes to a remote");
    }
    // 这一条排在 LONG_RUNNING 前面：对 docker 命令，「容器里没有守护进程」
    // 才是真正的原因，「不会自己退出」只是碰巧也对其中一部分成立
    if NESTED_RUNTIME.iter().any(|d| lower.starts_with(d)) {
        return Verdict::skip("needs a container runtime we do not provide inside the container");
    }
    if LONG_RUNNING.iter().any(|d| lower.starts_with(d)) {
        return Verdict::skip("does not exit on its own");
    }
    if INTERACTIVE.iter().any(|d| lower.starts_with(d)) {
        return Verdict::skip("interactive");
    }
    // heredoc 是多行结构，我们逐行取命令，拿到的必然是残缺的一半
    if t.contains("<<") {
        return Verdict::skip("heredoc spans lines we cannot reassemble");
    }
    // `<...>` 与 `path/to/` 是给读者填的空。照着跑必然失败，而那不是 README 的错
    if PLACEHOLDERS.iter().any(|p| lower.contains(p)) {
        return Verdict::skip("contains a placeholder for the reader to fill in");
    }
    if t.contains('<') && t.contains('>') && !t.contains("2>") && !t.contains(">>") {
        return Verdict::skip("contains a placeholder for the reader to fill in");
    }
    // 未定义的变量展开成空串，命令就变成了另一条命令
    if t.contains('$') && !lower.starts_with("export ") {
        return Verdict::skip("expands a shell variable we cannot resolve");
    }
    // 提示符没剥干净的输出行、表格行之类
    if t.starts_with('|') || t.starts_with('+') {
        return Verdict::skip("looks like output, not a command");
    }

    Verdict::Run
}

/// 默认镜像。跟着仓库的第一份清单走，选不出来就用一个通用的 Debian。
///
/// 多份清单时取第一份（Rust 库外面套一层 npm 包装是常见组合）。挑不准的时候
/// 让使用者用 `--image` 说清楚，比我们替他排一个优先级要诚实——报告里那句
/// 「detected from Cargo.toml」就是给他看的，好让他知道该不该改。
///
/// 版本**故意不钉死**到补丁号：README 的承诺是「在这个生态的当前版本上能跑」，
/// 钉到补丁号会让验证结果随镜像标签过期而漂移。
fn default_image(ctx: &RepoContext) -> (String, String) {
    let Some(m) = ctx.manifests.first() else {
        return (
            "debian:stable-slim".to_string(),
            "no package manifest detected — pass --image for a toolchain".to_string(),
        );
    };
    let (image, why) = match m.ecosystem {
        Ecosystem::Cargo => ("rust:slim", "Cargo.toml"),
        Ecosystem::Npm => ("node:lts", "package.json"),
        Ecosystem::Pypi => ("python:3-slim", "a Python manifest"),
        Ecosystem::Go => ("golang:latest", "go.mod"),
        Ecosystem::Maven => ("maven:eclipse-temurin", "pom.xml"),
        Ecosystem::Gem => ("ruby:slim", "a Gemfile"),
        Ecosystem::Composer => ("composer:latest", "composer.json"),
    };
    (image.to_string(), format!("detected from {why}"))
}

// ── 执行 ────────────────────────────────────────────────────────────────

pub struct RunOptions<'a> {
    pub engine: Option<&'a str>,
    /// 断网跑。装依赖的命令会失败——这是刻意的，用来验证「离线可用」的承诺
    pub offline: bool,
    pub timeout: Duration,
}

/// 容器引擎失败的原因。与「README 的命令失败了」是完全不同的一类事件，
/// 退出码也不同——把环境问题报成质量回归，CI 里没人能分辨。
#[derive(Debug)]
pub enum RunError {
    NoEngine,
    Launch(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::NoEngine => write!(
                f,
                "no container engine found. `verify --run` needs docker or podman on PATH"
            ),
            RunError::Launch(e) => write!(f, "{e}"),
        }
    }
}

/// 在容器里跑计划里标了 `Run` 的那些命令，把结果填回 `plan`。
///
/// `announce` 收到即将执行的命令——**在别人的机器上执行别人 README 里的命令，
/// 必须让使用者看得见**，这与 `demo` 的规矩是同一条。
pub fn run(
    plan: &mut Plan,
    root: &Path,
    engine: &str,
    opts: &RunOptions,
    mut announce: impl FnMut(&str),
) -> Result<(), RunError> {
    let engine = engine.to_string();

    let indices: Vec<usize> = plan
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.verdict.is_run())
        .map(|(i, _)| i)
        .collect();

    if indices.is_empty() {
        return Ok(());
    }
    for &i in &indices {
        announce(&plan.steps[i].command);
    }

    let script = script(&indices, plan);
    // 容器名让超时有一个可靠的杀法：`docker kill` 打得中，而 `Child::kill`
    // 只杀得掉客户端进程，容器会继续跑下去
    let name = format!("repolish-verify-{}", std::process::id());

    let mut cmd = Command::new(&engine);
    cmd.arg("run")
        .arg("--rm")
        .args(["--name", &name])
        // 仓库只读挂载，容器复制一份再动手：README 里的命令不许改使用者的工作区
        .arg("-v")
        .arg(format!("{}:/repo:ro", root.display()))
        .args(["-w", "/work"])
        .args(["-e", "HOME=/work"])
        .args(["-e", "CI=1"])
        .args(["-e", "DEBIAN_FRONTEND=noninteractive"]);
    if opts.offline {
        cmd.args(["--network", "none"]);
    }
    cmd.arg(&plan.image)
        .args(["sh", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| RunError::Launch(format!("cannot run `{engine}`: {e}")))?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    let (stdout, timed_out) = match rx.recv_timeout(opts.timeout) {
        Ok(Ok(out)) => (String::from_utf8_lossy(&out.stdout).to_string(), false),
        Ok(Err(e)) => return Err(RunError::Launch(format!("{engine} failed: {e}"))),
        Err(_) => {
            // 超时。先把容器打掉，再收已经产出的输出——半份日志远好过没有日志
            let _ = Command::new(&engine)
                .args(["rm", "-f", &name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let partial = match rx.recv() {
                Ok(Ok(out)) => String::from_utf8_lossy(&out.stdout).to_string(),
                _ => String::new(),
            };
            (partial, true)
        }
    };

    apply(plan, &stdout);

    // 计划里要跑却没有结果的，一律标 NotRun。默认成「通过」是最坏的选择：
    // 一次超时会变成一份满分报告。
    for &i in &indices {
        if plan.steps[i].status.is_none() {
            plan.steps[i].status = Some(Status::NotRun);
        }
    }

    if timed_out {
        return Err(RunError::Launch(format!(
            "timed out after {}s; the container was killed and the partial output kept",
            opts.timeout.as_secs()
        )));
    }
    Ok(())
}

/// 解析出要用哪个引擎。**在宣布「即将执行 26 条命令」之前调用**——
/// 先说要跑、再说跑不了，读的人会以为跑了一半。
pub fn engine(opts: &RunOptions) -> Result<String, RunError> {
    match opts.engine {
        Some(e) => Ok(e.to_string()),
        None => detect_engine().ok_or(RunError::NoEngine),
    }
}

fn detect_engine() -> Option<String> {
    for e in ["docker", "podman"] {
        let ok = Command::new(e)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(e.to_string());
        }
    }
    None
}

const BEGIN: &str = "__REPOLISH_STEP__";
const END: &str = "__REPOLISH_EXIT__";

/// 生成容器里跑的那段 sh。
///
/// 一条命令一段，用哨兵行分隔，这样既能**逐条**归因失败，又保留了 shell 会话
/// 本身——`cd docs && make` 里的 `cd` 对下一条命令仍然有效。分成 N 次
/// `docker run` 就没有这个性质，而 README 的命令序列几乎总是依赖它。
fn script(indices: &[usize], plan: &Plan) -> String {
    let mut s = String::new();
    // stderr 并进 stdout，保住两者的相对顺序：报错和它上一行的上下文分开看没有意义
    s.push_str("exec 2>&1\n");
    s.push_str("mkdir -p /work && cp -a /repo/. /work/ 2>/dev/null; cd /work || exit 1\n");
    for &i in indices {
        s.push_str(&format!("printf '%s %d\\n' '{BEGIN}' {i}\n"));
        s.push_str(&plan.steps[i].command);
        s.push('\n');
        s.push_str(&format!("printf '%s %d %d\\n' '{END}' {i} \"$?\"\n"));
    }
    s
}

/// 从容器输出里拆出每条命令的退出码与它自己的那段输出
fn apply(plan: &mut Plan, stdout: &str) {
    let mut current: Option<usize> = None;
    let mut buf = String::new();

    for raw in stdout.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix(BEGIN) {
            if let Ok(i) = rest.trim().parse::<usize>() {
                current = Some(i);
                buf.clear();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix(END) {
            let mut parts = rest.split_whitespace();
            let idx: Option<usize> = parts.next().and_then(|s| s.parse().ok());
            let code: i32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(-1);
            if let Some(i) = idx {
                if let Some(step) = plan.steps.get_mut(i) {
                    step.status = Some(if code == 0 {
                        Status::Passed
                    } else {
                        Status::Failed { exit_code: code }
                    });
                    step.output = Some(buf.trim_end().to_string());
                }
            }
            current = None;
            buf.clear();
            continue;
        }
        if current.is_some() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
}

// ── 呈现 ────────────────────────────────────────────────────────────────

use repolish_render::theme::{self, ColorLevel};

/// 终端报告。
///
/// **跳过的每一条都列出来，带理由。** 一份「12 条全部通过」的报告，如果其中
/// 9 条被悄悄跳过了，读它的人会得到一个比没有报告更错的印象。
pub fn render(plan: &Plan, level: ColorLevel, verbose: bool) -> String {
    use std::fmt::Write as _;

    let ink = |t: &str, c: theme::Rgb| format!("{}{t}{}", theme::fg(c, level), theme::reset(level));
    let strong = |t: &str, c: theme::Rgb| {
        format!(
            "{}{}{t}{}",
            theme::bold(level),
            theme::fg(c, level),
            theme::reset(level)
        )
    };
    let dim = |t: &str| ink(t, theme::MUTED);

    let mut s = String::new();
    let ran = plan.steps.iter().any(|st| st.status.is_some());

    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "  {}  ·  {}",
        strong(&plan.readme, theme::TEXT),
        dim(&format!("{} ({})", plan.image, plan.image_reason))
    );
    let _ = writeln!(s);

    if !ran {
        let _ = writeln!(
            s,
            "  {} — {} command{} would run, {} skipped. Nothing was executed.",
            strong("PLAN", theme::AMBER),
            plan.runnable(),
            if plan.runnable() == 1 { "" } else { "s" },
            plan.steps.len() - plan.runnable()
        );
    } else {
        let failed = plan.failed();
        let (word, colour) = if failed > 0 {
            ("FAILED", theme::RED)
        } else if plan.not_run() > 0 {
            ("INCOMPLETE", theme::AMBER)
        } else {
            ("PASSED", theme::LIME)
        };
        let _ = writeln!(
            s,
            "  {}  {} passed · {} failed · {} not run · {} skipped",
            strong(word, colour),
            plan.passed(),
            failed,
            plan.not_run(),
            plan.steps.len() - plan.runnable()
        );
    }
    let _ = writeln!(s);

    for step in &plan.steps {
        let (mark, colour) = match (&step.verdict, &step.status) {
            (Verdict::Skip { .. }, _) => ("○", theme::MUTED),
            (_, Some(Status::Passed)) => ("●", theme::LIME),
            (_, Some(Status::Failed { .. })) => ("●", theme::RED),
            (_, Some(Status::NotRun)) => ("◐", theme::AMBER),
            (_, None) => ("·", theme::CYAN),
        };
        let _ = writeln!(
            s,
            "  {} {}  {}",
            ink(mark, colour),
            dim(&format!("{}:{}", plan.readme, step.line)),
            step.command
        );
        match (&step.verdict, &step.status) {
            (Verdict::Skip { reason }, _) => {
                let _ = writeln!(s, "      {}", dim(&format!("skipped — {reason}")));
            }
            (_, Some(Status::Failed { exit_code })) => {
                let _ = writeln!(
                    s,
                    "      {}",
                    ink(&format!("exited {exit_code}"), theme::RED)
                );
                // 失败的那条一定要看到输出，不管 -v。让人再跑一次才能知道
                // 为什么失败，等于没有报告。
                for line in tail(step.output.as_deref().unwrap_or(""), 12) {
                    let _ = writeln!(s, "      {}", dim(&format!("| {line}")));
                }
            }
            (_, Some(Status::NotRun)) => {
                let _ = writeln!(
                    s,
                    "      {}",
                    dim("not run — the container stopped before reaching it")
                );
            }
            (_, Some(Status::Passed)) if verbose => {
                for line in tail(step.output.as_deref().unwrap_or(""), 6) {
                    let _ = writeln!(s, "      {}", dim(&format!("| {line}")));
                }
            }
            _ => {}
        }
    }

    let _ = writeln!(s);
    if !ran {
        let _ = writeln!(
            s,
            "  Run them for real with {}. They execute in a container, with your\n  \
             repository mounted read-only — nothing here can change your working tree.",
            strong("--run", theme::CYAN)
        );
    }
    s
}

/// 只留最后几行。一次失败的构建能吐出几百行，全贴出来会把报告淹掉，
/// 而错误几乎总在末尾。
fn tail(output: &str, n: usize) -> Vec<&str> {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(cmd: &str) -> Verdict {
        classify(cmd)
    }

    #[test]
    fn ordinary_build_and_test_commands_run() {
        assert_eq!(v("cargo build --release"), Verdict::Run);
        assert_eq!(v("npm ci"), Verdict::Run);
        assert_eq!(v("make test"), Verdict::Run);
        assert_eq!(v("pip install -e ."), Verdict::Run);
    }

    /// 容器挡得住文件系统，挡不住网络那一头
    #[test]
    fn anything_that_reaches_a_remote_is_never_run() {
        assert!(!v("npm publish").is_run());
        assert!(!v("git push origin main").is_run());
        assert!(!v("cargo publish").is_run());
        assert!(!v("gh release create v1.0.0").is_run());
    }

    #[test]
    fn servers_and_watchers_are_skipped_rather_than_timed_out() {
        assert!(!v("npm run dev").is_run());
        assert!(!v("cargo watch -x test").is_run());
        assert!(!v("mkdocs serve").is_run());
    }

    /// 验证本身跑在容器里，里面没有 docker 守护进程。照跑得到的
    /// `docker: not found` 是**我们**的环境限制，报成「README 坏了」是误报
    #[test]
    fn docker_commands_are_skipped_rather_than_reported_as_broken() {
        let d = v("docker compose up -d");
        assert!(!d.is_run());
        assert!(
            matches!(d, Verdict::Skip { reason } if reason.contains("container runtime")),
            "{d:?}"
        );
        assert!(!v("kubectl get pods").is_run());
    }

    #[test]
    fn placeholders_are_the_readers_job_not_a_broken_claim() {
        assert!(!v("export TOKEN=<your-token>").is_run());
        assert!(!v("myapp --config path/to/config.yml").is_run());
        assert!(!v("curl https://api.example.com/v1").is_run());
    }

    #[test]
    fn destructive_and_privileged_commands_are_never_run() {
        assert!(!v("sudo apt-get install -y jq").is_run());
        assert!(!v("rm -rf /").is_run());
    }

    /// 变量在我们这儿展开不出来，跑的就不是 README 承诺的那条命令
    #[test]
    fn unresolvable_variables_are_skipped() {
        assert!(!v("$EDITOR README.md").is_run());
        assert!(!v("./build.sh $VERSION").is_run());
    }

    #[test]
    fn a_backslash_continuation_becomes_one_command() {
        let joined = join_continuations(vec![
            (3, "docker run --rm \\".to_string()),
            (4, "  -v .:/x \\".to_string()),
            (5, "  alpine sh".to_string()),
            (7, "echo done".to_string()),
        ]);
        assert_eq!(
            joined,
            vec![
                (3, "docker run --rm -v .:/x alpine sh".to_string()),
                (7, "echo done".to_string()),
            ]
        );
    }

    fn plan_of(cmds: &[&str]) -> Plan {
        Plan {
            readme: "README.md".into(),
            image: "debian:stable-slim".into(),
            image_reason: "test".into(),
            steps: cmds
                .iter()
                .enumerate()
                .map(|(i, c)| Step {
                    line: i + 1,
                    command: c.to_string(),
                    verdict: Verdict::Run,
                    status: None,
                    output: None,
                })
                .collect(),
        }
    }

    #[test]
    fn exit_codes_and_output_are_attributed_to_the_right_command() {
        let mut p = plan_of(&["true", "false"]);
        let out = format!("{BEGIN} 0\nhello\n{END} 0 0\n{BEGIN} 1\nboom\n{END} 1 3\n");
        apply(&mut p, &out);
        assert_eq!(p.steps[0].status, Some(Status::Passed));
        assert_eq!(p.steps[0].output.as_deref(), Some("hello"));
        assert_eq!(p.steps[1].status, Some(Status::Failed { exit_code: 3 }));
        assert_eq!(p.steps[1].output.as_deref(), Some("boom"));
    }

    /// 容器中途死掉，后面的命令没有结果。默认成「通过」会把一次超时
    /// 变成一份满分报告——那正是这个工具存在的理由的反面。
    #[test]
    fn a_command_with_no_marker_is_not_run_rather_than_passed() {
        let mut p = plan_of(&["true", "false"]);
        apply(&mut p, &format!("{BEGIN} 0\n{END} 0 0\n"));
        assert_eq!(p.steps[0].status, Some(Status::Passed));
        assert_eq!(p.steps[1].status, None);
    }

    /// 会话是连着的：`cd` 对下一条命令仍然有效
    #[test]
    fn the_script_keeps_one_shell_session_for_every_command() {
        let p = plan_of(&["cd docs", "make html"]);
        let s = script(&[0, 1], &p);
        assert!(s.contains("cd docs\n"));
        assert!(s.contains("make html\n"));
        assert_eq!(s.matches("cd /work").count(), 1);
    }
}
