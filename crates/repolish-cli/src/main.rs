//! repolish —— 开源仓库诊断 / 优化工具
//!
//! `check` / `badge` / `report` / `init` / `polish`。
//!
//! `polish` 只做能机械落实的改动，且**只增量插入**——为什么不能让 AST
//! 产出文本，见 repolish-md 的 crate 文档。

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use repolish_render::RenderOptions;

mod analyze;
mod base;
mod config;
mod demo;
mod init;
mod polish;
mod record;
mod sarif;
mod scaffold;
mod skill;
mod style;
mod suggest;
mod tables;
mod tree;
mod verify;

use analyze::{analyze, write_file, Analysis, Common};
use repolish_md::Readme;

/// 退出码。工具自身失败与「检查不通过」必须区分，否则 CI 无法判断。
///
/// 新增的两个都在「环境没跑起来」那一侧,与 4 同类:一个拉不到的镜像、一个
/// 浅克隆里不存在的基线,都不是质量回归。把它们报成 1,CI 上就分不出
/// 「这个 PR 变差了」和「今天 docker 挂了」。
mod exit {
    pub const OK: u8 = 0;
    /// 分数低于 `--min-score`；`verify` 下是「有 README 命令跑失败了」
    pub const BELOW_MIN_SCORE: u8 = 1;
    pub const BAD_USAGE: u8 = 2;
    pub const NOT_A_REPO: u8 = 3;
    pub const REMOTE_FAILED: u8 = 4;
    pub const LOW_COVERAGE: u8 = 5;
    /// `verify --run` 跑不起来：没有容器引擎、镜像拉不下来、容器超时
    pub const RUNNER_FAILED: u8 = 6;
    /// `--base` 的基线取不到：浅克隆、ref 不存在、没有 git
    pub const BASE_FAILED: u8 = 7;
}

#[derive(Parser)]
#[command(
    name = "repolish",
    version,
    about = "Diagnose and improve how discoverable, understandable, and credible an open-source repository is"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Score a repository and print the report
    Check(CheckArgs),
    /// Write .repolish/badge.json and print the badge markdown to paste
    Badge(BadgeArgs),
    /// Write REPOLISH.md
    Report(ReportArgs),
    /// Write the SVG cards to embed in your README
    Card(CardArgs),
    /// Record this project's CLI and write the animation
    Demo(DemoArgs),
    /// Write SKILL.md, which teaches a coding agent how to drive repolish
    Skill(SkillArgs),
    /// Generate a GitHub Actions workflow
    Init(InitArgs),
    /// Apply the fixes that can be made mechanically, and print the rest
    Polish(PolishArgs),
    /// Run the commands the README promises, in a clean container, and report what broke
    Verify(VerifyArgs),
}

#[derive(Parser)]
struct PolishArgs {
    #[command(flatten)]
    common: Common,

    /// Write the changes. Without it, polish only prints what it would do
    #[arg(long)]
    apply: bool,

    /// Apply outside a git repository too, where there is nothing to undo with
    #[arg(long)]
    force: bool,

    /// Print the full contents of every file it would create
    #[arg(short, long)]
    verbose: bool,

    // ── 排版。只影响插入物的外观，不影响任何一个分数。
    //    同名项也可写在 .repolish.toml 的 [readme] 段里，命令行优先。
    /// shields.io badge style for the badge it inserts
    #[arg(long, value_enum)]
    badge_style: Option<style::BadgeStyle>,

    /// Alignment for blocks it creates
    #[arg(long, value_enum)]
    align: Option<style::Align>,

    /// Table of contents layout
    #[arg(long, value_enum)]
    toc_style: Option<style::TocStyle>,

    /// Image to place above the title, as a path inside the repository
    #[arg(long)]
    logo: Option<String>,

    /// Width for the logo: a pixel count, or `full` for a full-width banner
    #[arg(long)]
    logo_width: Option<style::LogoWidth>,

    /// Append a project structure tree this many levels deep
    #[arg(long)]
    tree_depth: Option<usize>,

    /// Insert a project overview card below the badges, and write it
    #[arg(long)]
    overview: bool,

    /// Insert the repolish report card at the end, under its own heading
    #[arg(long)]
    footer_card: bool,

    /// Render README tables as SVG, folding the original into <details>
    #[arg(long, value_enum)]
    tables: Option<style::TableStyle>,

    /// Shorthand for --overview --footer-card --tables svg
    #[arg(long)]
    visuals: bool,

    /// Also ask a model to draft the three pieces polish cannot write mechanically:
    /// the tagline, the quick start and the usage example. Needs
    /// REPOLISH_LLM_API_KEY (or ANTHROPIC_API_KEY). It PRINTS them — nothing is
    /// written, not even with --apply — and no score is affected
    #[arg(long)]
    suggest: bool,
}

#[derive(Parser)]
struct CheckArgs {
    #[command(flatten)]
    common: Common,

    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Exit with code 1 below this score, for use as a CI gate
    #[arg(long)]
    min_score: Option<u8>,

    /// Also write .repolish/badge.json
    #[arg(long)]
    badge: bool,

    /// Also write REPOLISH.md
    #[arg(long)]
    report: bool,

    /// Also write .repolish/card.svg, the score card
    #[arg(long)]
    card: bool,

    /// Also write .repolish/overview.svg, the project overview card
    #[arg(long)]
    overview: bool,

    /// Show P3 suggestions and passing checks as well
    #[arg(short, long)]
    verbose: bool,

    /// Also score this git ref and report the difference, e.g. `origin/main`.
    /// The baseline is checked out into a temporary worktree; your working tree is
    /// never touched
    #[arg(long, value_name = "REF")]
    base: Option<String>,

    /// Also write a SARIF file. GitHub renders each finding on its own line in the
    /// pull request diff
    #[arg(long, value_name = "PATH")]
    sarif: Option<PathBuf>,

    /// Also write the short form meant for a pull request comment. With --base it
    /// leads with what this change did to the score
    #[arg(long, value_name = "PATH")]
    comment: Option<PathBuf>,
}

#[derive(Parser)]
struct BadgeArgs {
    #[command(flatten)]
    common: Common,

    /// Branch the badge JSON lives on, used in the snippet URL. Defaults to the current branch
    #[arg(long)]
    branch: Option<String>,

    /// Print the JSON instead of writing the file
    #[arg(long)]
    stdout: bool,
}

#[derive(Parser)]
struct ReportArgs {
    #[command(flatten)]
    common: Common,

    /// Output path; defaults to REPOLISH.md in the repository root
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print to stdout instead of writing the file
    #[arg(long)]
    stdout: bool,
}

#[derive(Parser)]
struct CardArgs {
    #[command(flatten)]
    common: Common,

    /// Which cards to write. Comma-separated; repeatable
    #[arg(long, value_enum, value_delimiter = ',', default_value = "overview")]
    kind: Vec<CardKind>,

    /// Output path. Only valid with a single --kind; the defaults are
    /// .repolish/overview.svg, .repolish/card.svg and .repolish/tables/
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print the SVG instead of writing the file. Only valid with a single --kind
    #[arg(long)]
    stdout: bool,
}

/// 这几张图说的是不同的事，在 README 里的位置也不同——
/// 见 `repolish_render::overview` 的模块说明。
///
/// **`card` 会覆盖已有文件，`polish` 不会。** 分工是有意的：`polish` 负责
/// 第一次把引用插进 README，`card` 负责此后每一次重画。不能重画的话，
/// README 上迟早挂着一张过期的图。
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum CardKind {
    /// What this project is: languages, activity, licence. Goes at the top of the README
    Overview,
    /// What repolish scored it. Goes at the bottom, under a "Polished with repolish" heading
    Score,
    /// Redraw the SVG for every table in the README that polish has already wrapped
    Tables,
    /// All of the above
    All,
}

#[derive(Parser)]
#[command(after_help = "\
This RUNS the commands it records — that is the whole point, and it is why the score in \
the recording is a real score rather than a staged one. Only point it at a repository \
whose commands you are willing to execute. Use --dry-run to see the list first.")]
struct DemoArgs {
    #[command(flatten)]
    common: Common,

    /// Output path; defaults to .repolish/demo.svg in the repository root
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// The command to record. Repeat for several. Defaults to the detected binary's --help
    #[arg(long = "cmd")]
    commands: Vec<String>,

    /// List the commands it would run, and run nothing
    #[arg(long)]
    dry_run: bool,

    /// Also write a VHS tape, for rendering a GIF with charmbracelet/vhs instead
    #[arg(long)]
    tape: bool,

    /// Milliseconds per keystroke in the animation
    #[arg(long, default_value = "45")]
    type_ms: u32,

    /// Print the SVG instead of writing the file
    #[arg(long)]
    stdout: bool,
}

#[derive(Parser)]
#[command(after_help = "\
Without --target this writes SKILL.md into a repository, so it travels with the code. \
With --target it installs into the agent's own directory on this machine, where it \
applies to every project you open.")]
struct SkillArgs {
    /// Path to write into
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Install into an agent's skill directory instead of writing into a repository.
    /// Comma-separated; `detect` picks the agents that are actually installed
    #[arg(long, value_delimiter = ',')]
    target: Vec<String>,

    /// List the known agents and whether each one is installed here
    #[arg(long)]
    list: bool,

    /// Output path; defaults to SKILL.md in that directory
    #[arg(short, long, conflicts_with = "target")]
    output: Option<PathBuf>,

    /// Print it instead of writing the file
    #[arg(long)]
    stdout: bool,

    /// Overwrite an existing file
    #[arg(long)]
    force: bool,
}

#[derive(Parser)]
struct InitArgs {
    /// Path to the repository
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Gate score for the generated workflow; without it the workflow only records the score
    #[arg(long, default_value = "60")]
    min_score: Option<u8>,

    /// Record the score without gating on it
    #[arg(long, conflicts_with = "min_score")]
    no_gate: bool,

    /// Overwrite an existing workflow
    #[arg(long)]
    force: bool,
}

#[derive(Parser)]
#[command(after_help = "\
Without --run this only prints the plan: which commands it would execute, and why it \
would skip the rest. With --run it executes them inside a container, with your \
repository mounted READ-ONLY and copied in — nothing a README command does can reach \
your working tree.

It never runs anything that publishes, needs root, or does not exit on its own. Every \
skipped command is listed with its reason: a report that says \"12 passed\" while \
quietly skipping nine of them is worse than no report.")]
struct VerifyArgs {
    #[command(flatten)]
    common: Common,

    /// Actually execute the commands. Without it, nothing runs
    #[arg(long)]
    run: bool,

    /// Container image to run them in. Defaults to one picked from the package manifest
    #[arg(long)]
    image: Option<String>,

    /// Container engine. Defaults to docker, then podman
    #[arg(long)]
    engine: Option<String>,

    /// Only take commands from README sections whose heading contains this.
    /// Comma-separated; repeatable
    #[arg(long, value_delimiter = ',')]
    section: Vec<String>,

    /// Run with no network, to check what the README promises works offline
    #[arg(long)]
    offline: bool,

    /// Give up after this many seconds
    #[arg(long, default_value = "600")]
    timeout: u64,

    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Print the output of the commands that passed, too
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
    Markdown,
    /// SARIF 2.1.0. GitHub renders each finding on its own line in the pull request diff
    Sarif,
    /// What fits in a pull request comment: the score, the difference from --base, P1 and P2
    Comment,
}

fn main() -> ExitCode {
    // Windows 的控制台默认不解释 ANSI，得在第一次输出之前把 VT 模式打开
    repolish_render::theme::enable_ansi();
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check(args) => run_check(args),
        Command::Badge(args) => run_badge(args),
        Command::Report(args) => run_report(args),
        Command::Card(args) => run_card(args),
        Command::Demo(args) => run_demo(args),
        Command::Skill(args) => run_skill(args),
        Command::Init(args) => run_init(args),
        Command::Polish(args) => run_polish(args),
        Command::Verify(args) => run_verify(args),
    };
    ExitCode::from(code)
}

fn run_check(args: CheckArgs) -> u8 {
    let Analysis {
        ctx,
        mut report,
        min_score,
        opts,
    } = match analyze(&args.common) {
        Ok(a) => a,
        Err(code) => return code,
    };

    // 差值必须在渲染之前算出来：四种格式都要能看到它,分开算等于四份代码
    if let Some(base_ref) = &args.base {
        match base::compare(&ctx.root, base_ref, &ctx, &report, &opts) {
            Ok(b) => report.delta = Some(b.delta),
            Err(e) => {
                eprintln!("error: {e}");
                // 基线取不到不是质量回归。报成 1 的话,CI 上分不出
                // 「这个 PR 变差了」和「浅克隆里没有那个 commit」
                return exit::BASE_FAILED;
            }
        }
    }

    match args.format {
        Format::Text => print!(
            "{}",
            repolish_render::terminal(
                &report,
                &RenderOptions {
                    verbose: args.verbose,
                    level: args.common.level(),
                }
            )
        ),
        Format::Json => match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: serialization failed: {e}");
                return exit::BAD_USAGE;
            }
        },
        Format::Markdown => print!("{}", repolish_render::markdown(&report)),
        Format::Sarif => print!("{}", sarif::sarif(&report)),
        Format::Comment => print!("{}", repolish_render::comment(&report)),
    }

    // `--format text` 之后再单独落一份 SARIF：CI 里通常两者都要——
    // 人看日志,GitHub 读文件
    if let Some(path) = &args.sarif {
        if let Err(code) = write_file(path, &sarif::sarif(&report)) {
            return code;
        }
        eprintln!("wrote {}", path.display());
    }
    if let Some(path) = &args.comment {
        if let Err(code) = write_file(path, &repolish_render::comment(&report)) {
            return code;
        }
        eprintln!("wrote {}", path.display());
    }

    // 副产物在一次运行里全部写出。分开跑意味着多打几次 GitHub API，
    // 也意味着几份产物有可能来自不同的评分结果。
    if args.badge {
        if let Err(code) = write_badge(&ctx, &report) {
            return code;
        }
    }
    if args.report {
        let path = ctx.root.join("REPOLISH.md");
        if let Err(code) = write_file(&path, &repolish_render::markdown(&report)) {
            return code;
        }
        eprintln!("wrote {}", path.display());
    }
    if args.card || args.overview {
        let cfg = match crate::config::load(args.common.config.as_deref(), &ctx.root) {
            Ok(c) => c.readme,
            Err(e) => {
                eprintln!("error: {e}");
                return exit::BAD_USAGE;
            }
        };
        let opts = args.common.card_options(&ctx, &cfg);
        if args.overview {
            let facts = repolish_render::Facts::from_ctx(&ctx, opts.lang);
            let path = ctx.root.join(repolish_render::OVERVIEW_PATH);
            if let Err(code) = write_file(&path, &repolish_render::overview(&facts, &opts)) {
                return code;
            }
            eprintln!("wrote {}", path.display());
        }
        if args.card {
            let path = ctx.root.join(repolish_render::CARD_PATH);
            if let Err(code) = write_file(&path, &repolish_render::card(&report, &opts)) {
                return code;
            }
            eprintln!("wrote {}", path.display());
        }
    }

    // 命令行永远赢：CI 里能改的只有那一行
    verdict(&report, args.min_score.or(min_score))
}

fn run_badge(args: BadgeArgs) -> u8 {
    let Analysis { ctx, report, .. } = match analyze(&args.common) {
        Ok(a) => a,
        Err(code) => return code,
    };

    let Some(json) = repolish_render::badge_json(&report) else {
        eprintln!(
            "error: only {:.0}% of the registered checks produced a score, below the 50% floor, \
             so no badge was written.\n\
             A badge backed by a third of the evidence is worse than no badge at all",
            report.coverage * 100.0
        );
        return exit::LOW_COVERAGE;
    };

    if args.stdout {
        print!("{json}");
        return exit::OK;
    }

    let path = ctx.root.join(repolish_render::BADGE_PATH);
    if let Err(code) = write_file(&path, &json) {
        return code;
    }
    println!("wrote {}", path.display());

    let branch = args
        .branch
        .or_else(|| ctx.git.as_ref().and_then(|g| g.branch.clone()))
        .unwrap_or_else(|| "main".to_string());

    println!("\nPaste this into your README:\n");
    match &ctx.slug {
        Some(slug) => println!(
            "{}\n",
            repolish_render::snippet(&slug.owner, &slug.name, &branch)
        ),
        None => {
            println!("{}\n", repolish_render::snippet("OWNER", "REPO", &branch));
            eprintln!(
                "warning: no GitHub remote found — fill in OWNER / REPO in the snippet yourself"
            );
        }
    }
    println!(
        "shields.io renders the badge by reading {} out of your own repository. Nothing is hosted by us.",
        repolish_render::BADGE_PATH
    );
    exit::OK
}

fn run_report(args: ReportArgs) -> u8 {
    let Analysis { ctx, report, .. } = match analyze(&args.common) {
        Ok(a) => a,
        Err(code) => return code,
    };

    let md = repolish_render::markdown(&report);
    if args.stdout {
        print!("{md}");
        return exit::OK;
    }

    let path = args.output.unwrap_or_else(|| ctx.root.join("REPOLISH.md"));
    if let Err(code) = write_file(&path, &md) {
        return code;
    }
    println!("wrote {}", path.display());
    exit::OK
}

fn run_card(args: CardArgs) -> u8 {
    let Analysis { ctx, report, .. } = match analyze(&args.common) {
        Ok(a) => a,
        Err(code) => return code,
    };

    let cfg = match crate::config::load(args.common.config.as_deref(), &ctx.root) {
        Ok(c) => c.readme,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    let opts = args.common.card_options(&ctx, &cfg);

    let wants = |k: CardKind| args.kind.contains(&k) || args.kind.contains(&CardKind::All);
    let single =
        args.kind.len() == 1 && args.kind[0] != CardKind::All && args.kind[0] != CardKind::Tables;

    // 每张图的默认路径不同，所以 --output 和 --stdout 只在选定一张时说得清
    if (args.output.is_some() || args.stdout) && !single {
        eprintln!(
            "error: --output and --stdout each handle a single SVG, but --kind {} produces several",
            args.kind
                .iter()
                .map(|k| format!("{k:?}").to_lowercase())
                .collect::<Vec<_>>()
                .join(",")
        );
        eprintln!("note: run it once per kind, or drop the flag to write the default paths");
        return exit::BAD_USAGE;
    }

    let mut written: Vec<String> = Vec::new();
    if wants(CardKind::Overview) {
        let facts = repolish_render::Facts::from_ctx(&ctx, opts.lang);
        let svg = repolish_render::overview(&facts, &opts);
        if args.stdout {
            print!("{svg}");
            return exit::OK;
        }
        let path = args
            .output
            .clone()
            .unwrap_or_else(|| ctx.root.join(repolish_render::OVERVIEW_PATH));
        if let Err(code) = write_file(&path, &svg) {
            return code;
        }
        written.push(relative(&ctx.root, &path));
    }
    if wants(CardKind::Score) {
        let svg = repolish_render::card(&report, &opts);
        if args.stdout {
            print!("{svg}");
            return exit::OK;
        }
        let path = args
            .output
            .clone()
            .unwrap_or_else(|| ctx.root.join(repolish_render::CARD_PATH));
        if let Err(code) = write_file(&path, &svg) {
            return code;
        }
        written.push(relative(&ctx.root, &path));
    }
    if wants(CardKind::Tables) {
        let Some(readme) = ctx.readme.as_ref() else {
            eprintln!("error: no README to take tables from");
            return exit::NOT_A_REPO;
        };
        // 主 README 加上每一份译本。译本要是漏掉，它们的表格图就再也没有
        // 重画的途径——polish 从不覆盖，`card` 是唯一会重写的那条路。
        let mut sheets: Vec<(Readme, repolish_render::Options)> = vec![(readme.clone(), opts)];
        for path in tables::translations(&ctx, readme) {
            let Some(raw) = ctx.files.read(&path) else {
                continue;
            };
            let translated = Readme::parse(&path, raw);
            // 每一份都用它自己的语言画，不是主 README 的语言
            let lang = repolish_render::Lang::detect(&translated.raw);
            sheets.push((translated, repolish_render::Options { lang, ..opts }));
        }

        // 只重画 polish 已经包过的那些。给一张没人引用的表生成 SVG，
        // 落下的是一个孤儿文件——它会被提交、被一直带着，而没有任何东西
        // 指向它。要新增一张，先让 polish 去包：
        //     repolish polish . --apply --tables svg
        let mut unwrapped = 0usize;
        for (sheet, sheet_opts) in &sheets {
            for table in tables::render(sheet, sheet_opts, |w| eprintln!("note: {w}")) {
                if !tables::already_wrapped(sheet, table.start_line) {
                    unwrapped += 1;
                    continue;
                }
                let path = table.path(&ctx.root);
                if let Err(code) = write_file(&path, &table.svg) {
                    return code;
                }
                written.push(table.rel);
            }
        }
        if unwrapped > 0 {
            println!(
                "{unwrapped} table(s) in the README are not wrapped yet, so nothing was \
                 drawn for them.\n  To wrap them: repolish polish . --apply --tables svg"
            );
        }
        if written.is_empty() {
            println!(
                "No table in {} is worth drawing (needs {}–{} rows and at least two columns).",
                relative(&ctx.root, &readme.path),
                tables::MIN_ROWS,
                tables::MAX_ROWS
            );
        }
    }

    for rel in &written {
        println!("wrote {rel}");
    }
    if written.is_empty() {
        return exit::OK;
    }

    // 两张卡片的位置不一样，这一点比路径本身更值得说：
    // 贴反了，一个陌生人点进仓库第一眼看到的就是我们的分数而不是这个项目
    if wants(CardKind::Overview) || wants(CardKind::Score) {
        println!("\nWhere they go:\n");
    }
    if wants(CardKind::Overview) {
        println!(
            "  near the top, under the badges:\n\n    <img src=\"{}\" alt=\"{} at a glance\" width=\"880\">\n",
            repolish_render::OVERVIEW_PATH,
            ctx.display_name()
        );
    }
    if wants(CardKind::Score) {
        println!(
            "  at the end, under a \"Polished with repolish\" heading:\n\n    <img src=\"{}\" alt=\"repolish report card\" width=\"880\">\n",
            repolish_render::CARD_PATH
        );
    }
    println!(
        "Or let polish place them: `repolish polish . --apply --visuals`.\n\
         Everything here is a plain file in your own repository: no fonts, no scripts, \
         nothing hosted by us."
    );
    exit::OK
}

/// 仓库相对路径，分隔符统一成 `/`——打印出来的路径要能直接贴进 README
fn relative(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn run_demo(args: DemoArgs) -> u8 {
    let root = match dunce::canonicalize(&args.common.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot access {}: {e}", args.common.path.display());
            return exit::NOT_A_REPO;
        }
    };
    let ctx = match repolish_ingest::RepoContext::load(&root, None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            return exit::NOT_A_REPO;
        }
    };

    let commands = if args.commands.is_empty() {
        // 录一段跑不起来的命令比没有录屏更糟。认不出可执行文件就明说，
        // 并告诉他怎么手动指定——而不是拿仓库名去赌。
        let Some(bin) = demo::binary(&ctx) else {
            eprintln!(
                "error: no command-line binary detected in {}.\n\
                 note: repolish demo records a CLI. If this project has one, name the \
                 commands yourself:\n      repolish demo . --cmd \"yourtool --help\"",
                root.display()
            );
            return exit::BAD_USAGE;
        };
        demo::default_commands(&bin)
    } else {
        args.commands.clone()
    };

    // 执行别人机器上的程序这件事，必须让使用者看得见——干跑时更是唯一的输出
    if args.dry_run {
        println!("Would run these, in {}:\n", relative(&root, &root));
        for c in &commands {
            println!("  $ {c}");
        }
        println!("\nNothing was run. Drop --dry-run to record.");
        return exit::OK;
    }

    println!("Recording in {}:", root.display());
    let recording = match record::run(&commands, &root, |c| println!("  $ {c}")) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "note: the command has to be on PATH. For a project you have not installed, \
                 build it first and put the build directory on PATH"
            );
            return exit::BAD_USAGE;
        }
    };

    // 失败的命令照录——一条报错也是真实输出——但绝不能不声不响：
    // 一段悄悄录进了错误的演示，比没有演示伤得更久
    for (cmd, code) in &recording.failures {
        eprintln!("warning: `{cmd}` exited with {code}; its output is in the recording as-is");
    }

    let cfg = match crate::config::load(args.common.config.as_deref(), &ctx.root) {
        Ok(c) => c.readme,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    let opts = args.common.card_options(&ctx, &cfg);
    let timing = repolish_render::Timing {
        type_ms: args.type_ms,
        ..Default::default()
    };
    let svg = repolish_render::cast(&recording.steps, &timing, &opts);

    if args.stdout {
        print!("{svg}");
        return exit::OK;
    }

    let path = args
        .output
        .clone()
        .unwrap_or_else(|| root.join(demo::SVG_PATH));
    if let Err(code) = write_file(&path, &svg) {
        return code;
    }
    let rel = relative(&root, &path);
    println!("\nwrote {rel}");

    if args.tape {
        let bin = demo::binary(&ctx).unwrap_or_else(|| "your-tool".into());
        let tape_path = root.join(demo::TAPE_PATH);
        if let Err(code) = write_file(&tape_path, &demo::tape(&bin, &commands, demo::GIF_PATH)) {
            return code;
        }
        println!("wrote {}", relative(&root, &tape_path));
        println!(
            "  · render it to a GIF with: vhs {}",
            relative(&root, &tape_path)
        );
    }

    println!(
        "\nPaste this into your README:\n\n    {}\n",
        demo::snippet(&rel, "terminal recording")
    );
    println!(
        "It is a plain SVG: no fonts, no scripts, nothing hosted by us, and the commands \n\
         in it are real text you can select and copy."
    );
    exit::OK
}

fn run_skill(args: SkillArgs) -> u8 {
    let md = skill::markdown();
    if args.stdout {
        print!("{md}");
        return exit::OK;
    }
    if args.list {
        return list_skill_targets();
    }
    if !args.target.is_empty() {
        return install_skill(&args.target, &md, args.force);
    }

    // 没给 --target 就是写进一个仓库：技能跟着代码走，谁 clone 谁就有
    let root = match dunce::canonicalize(&args.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot access {}: {e}", args.path.display());
            return exit::NOT_A_REPO;
        }
    };
    let path = args.output.unwrap_or_else(|| root.join(skill::SKILL_PATH));
    if path.exists() && !args.force {
        eprintln!(
            "error: {} already exists. Pass --force to overwrite it",
            path.display()
        );
        return exit::BAD_USAGE;
    }
    if let Err(code) = write_file(&path, &md) {
        return code;
    }
    println!("wrote {}", relative(&root, &path));
    println!(
        "\nIt teaches an agent to measure before it edits, and never to rewrite a README \
         wholesale.\nTo install it for every project instead: `repolish skill --target detect`."
    );
    exit::OK
}

fn list_skill_targets() -> u8 {
    let Some(home) = skill::home() else {
        eprintln!("error: could not determine your home directory (HOME / USERPROFILE)");
        return exit::BAD_USAGE;
    };
    for t in skill::TARGETS {
        // 装没装是一条事实，直说。列一个「可用」的目录而那家工具根本没装，
        // 只会让人以为技能生效了。
        let mark = if t.detected(&home) {
            "· installed"
        } else {
            "  not found"
        };
        println!("{mark}  {:<10} {}", t.id, t.label);
        println!("               ~/{}", t.skills_dir);
        println!("               {}", t.docs);
    }
    println!("\n  · = detected on this machine");
    println!("\n  repolish skill --target detect     install into the ones marked above");
    println!("  repolish skill --target all        install into every one of them");
    exit::OK
}

fn install_skill(requested: &[String], md: &str, force: bool) -> u8 {
    let Some(home) = skill::home() else {
        eprintln!("error: could not determine your home directory (HOME / USERPROFILE)");
        return exit::BAD_USAGE;
    };

    let mut targets: Vec<&skill::Target> = Vec::new();
    for name in requested {
        match name.as_str() {
            "all" => targets.extend(skill::TARGETS.iter()),
            // 只装到真的存在的工具里。往一个没装 Codex 的机器上写
            // ~/.codex/skills 会凭空造出一个目录，看着像那工具装了。
            "detect" => targets.extend(skill::TARGETS.iter().filter(|t| t.detected(&home))),
            id => match skill::Target::find(id) {
                Some(t) => targets.push(t),
                None => {
                    eprintln!("error: unknown target \"{id}\"");
                    eprintln!(
                        "available: all, detect, {}",
                        skill::TARGETS
                            .iter()
                            .map(|t| t.id)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    return exit::BAD_USAGE;
                }
            },
        }
    }
    targets.sort_by_key(|t| t.id);
    targets.dedup_by_key(|t| t.id);

    if targets.is_empty() {
        println!("No agent detected on this machine, so nothing was installed.");
        println!("Run `repolish skill --list` to see what is supported, or name one:");
        println!("    repolish skill --target claude");
        // 一台机器上没装任何智能体不是错误，只是没事可做
        return exit::OK;
    }

    for t in &targets {
        let path = t.skill_path(&home);
        if path.exists() && !force {
            println!(
                "skipped {} — already installed (pass --force to replace)",
                path.display()
            );
            continue;
        }
        if let Err(code) = write_file(&path, md) {
            return code;
        }
        println!("installed {} ({})", path.display(), t.label);

        if t.gemini_manifest {
            // 清单点名了一个上下文文件，两个必须一起写：只写清单会让
            // Gemini CLI 每次启动都指向一个不存在的路径
            let dir = path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf());
            let Some(dir) = dir else { continue };
            if let Err(code) = write_file(
                &dir.join("gemini-extension.json"),
                &skill::gemini_manifest(),
            ) {
                return code;
            }
            if let Err(code) = write_file(&dir.join("GEMINI.md"), &skill::gemini_context()) {
                return code;
            }
            println!("installed {}", dir.join("gemini-extension.json").display());
        }
    }

    println!(
        "\nThe skill calls `repolish` by name, so it has to be on PATH.\n\
         Check with: repolish --version"
    );
    exit::OK
}

fn run_init(args: InitArgs) -> u8 {
    let root = match dunce::canonicalize(&args.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot access {}: {e}", args.path.display());
            return exit::NOT_A_REPO;
        }
    };

    let path = root.join(init::WORKFLOW_PATH);
    if path.exists() && !args.force {
        eprintln!(
            "error: {} already exists. Pass --force to overwrite it",
            path.display()
        );
        return exit::BAD_USAGE;
    }

    // 分支名要与仓库实际的默认分支一致，否则 workflow 永远不会被 push 触发
    let branch = repolish_ingest::RepoContext::load(&root, None)
        .ok()
        .and_then(|c| c.git.and_then(|g| g.branch))
        .unwrap_or_else(|| "main".to_string());

    let min_score = if args.no_gate { None } else { args.min_score };
    if let Err(code) = write_file(&path, &init::workflow(&branch, min_score)) {
        return code;
    }

    println!("wrote {}", path.display());
    println!("  · triggers on: {branch}");
    match min_score {
        Some(n) => println!("  · gate: CI fails below {n}"),
        None => println!("  · gate: none — the score is recorded, not enforced"),
    }
    println!(
        "\nNote: the template pins asale-ai/repolish@v{}, which has to be released before this workflow can run.",
        env!("CARGO_PKG_VERSION")
    );
    exit::OK
}

fn run_verify(args: VerifyArgs) -> u8 {
    let root = match dunce::canonicalize(&args.common.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot access {}: {e}", args.common.path.display());
            return exit::NOT_A_REPO;
        }
    };
    let ctx = match repolish_ingest::RepoContext::load(&root, None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            return exit::NOT_A_REPO;
        }
    };

    let mut plan = match verify::plan(&ctx, args.image.as_deref(), &args.section) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::NOT_A_REPO;
        }
    };

    if plan.steps.is_empty() {
        println!(
            "No commands found in {}.{}",
            plan.readme,
            if args.section.is_empty() {
                String::new()
            } else {
                format!(
                    " (looking only in sections matching: {})",
                    args.section.join(", ")
                )
            }
        );
        return exit::OK;
    }

    let mut runner_failed = false;
    if args.run {
        let opts = verify::RunOptions {
            engine: args.engine.as_deref(),
            offline: args.offline,
            timeout: std::time::Duration::from_secs(args.timeout),
        };
        // 引擎先解析出来再宣布。先说「即将执行 26 条命令」、再说跑不了,
        // 读的人会以为跑了一半。
        let engine = match verify::engine(&opts) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("error: {e}");
                eprintln!(
                    "note: verify runs README commands in a container on purpose. \
                     Running them on your own machine is not a fallback we are willing \
                     to take without being asked.\n      \
                     Name one explicitly with --engine if yours is called something else."
                );
                return exit::RUNNER_FAILED;
            }
        };

        // 在别人的机器上执行别人 README 里的命令,必须让使用者看得见——
        // 与 `demo` 是同一条规矩。
        //
        // 走 stderr 而不是 stdout：`--format json` 的 stdout 必须只有 JSON,
        // 否则下游第一个 `jq` 就会炸。进度是进度,不是数据。
        eprintln!(
            "Running {} command(s) from {} in {} ({}):",
            plan.runnable(),
            plan.readme,
            plan.image,
            engine
        );
        if let Err(e) = verify::run(&mut plan, &root, &engine, &opts, |c| eprintln!("  $ {c}")) {
            eprintln!("error: {e}");
            // 超时之类：部分结果是有价值的,照常报出来,但退出码要说这次没跑完
            runner_failed = true;
        }
    }

    match args.format {
        Format::Text => print!(
            "{}",
            verify::render(&plan, args.common.level(), args.verbose)
        ),
        Format::Json => match serde_json::to_string_pretty(&plan) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: serialization failed: {e}");
                return exit::BAD_USAGE;
            }
        },
        Format::Markdown | Format::Comment | Format::Sarif => {
            eprintln!(
                "error: verify supports --format text and json. \
                 {} describes a score, and verify does not produce one",
                match args.format {
                    Format::Sarif => "sarif",
                    Format::Comment => "comment",
                    _ => "markdown",
                }
            );
            return exit::BAD_USAGE;
        }
    }

    if runner_failed {
        return exit::RUNNER_FAILED;
    }
    if !args.run {
        return exit::OK;
    }
    // 一条命令失败,与「分数低于门槛」是同一类事件:检查没通过
    if plan.failed() > 0 || plan.not_run() > 0 {
        exit::BELOW_MIN_SCORE
    } else {
        exit::OK
    }
}

fn write_badge(
    ctx: &repolish_ingest::RepoContext,
    report: &repolish_core::Report,
) -> Result<(), u8> {
    let Some(json) = repolish_render::badge_json(report) else {
        eprintln!("warning: coverage too low, no badge written");
        return Err(exit::LOW_COVERAGE);
    };
    let path = ctx.root.join(repolish_render::BADGE_PATH);
    write_file(&path, &json)?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

fn verdict(report: &repolish_core::Report, min_score: Option<u8>) -> u8 {
    let Some(score) = report.score else {
        return exit::LOW_COVERAGE;
    };
    match min_score {
        Some(min) if score < min => exit::BELOW_MIN_SCORE,
        _ => exit::OK,
    }
}

fn run_polish(args: PolishArgs) -> u8 {
    let Analysis { ctx, report, .. } = match analyze(&args.common) {
        Ok(a) => a,
        Err(code) => return code,
    };

    // 命令行 > 配置文件 > 默认；徽章样式没给时跟着 README 里已有的走
    let cfg = match crate::config::load(args.common.config.as_deref(), &ctx.root) {
        Ok(c) => c.readme,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    let readme_raw = ctx.readme.as_ref().map(|r| r.raw.as_str()).unwrap_or("");
    let style = style::ReadmeStyle {
        badge: args
            .badge_style
            .or(cfg.badge_style)
            .or_else(|| style::BadgeStyle::detect(readme_raw))
            .unwrap_or_default(),
        align: args.align.or(cfg.align).unwrap_or_default(),
        toc: args.toc_style.or(cfg.toc_style).unwrap_or_default(),
        logo: args.logo.or(cfg.logo),
        logo_width: args.logo_width.or(cfg.logo_width),
        tree_depth: args.tree_depth.or(cfg.tree_depth),
        theme: args.common.theme.or(cfg.theme).unwrap_or_default(),
        lang: args
            .common
            .lang
            .or(cfg.lang)
            .unwrap_or_default()
            .resolve(readme_raw),
        // --visuals 是三个开关的简写。命令行开了就是开，配置文件里的
        // false 不该把命令行显式给的开关关掉——命令行永远赢。
        overview: args.overview || args.visuals || cfg.overview.unwrap_or(false),
        footer_card: args.footer_card || args.visuals || cfg.footer_card.unwrap_or(false),
        tables: match (args.tables, args.visuals) {
            (Some(t), _) => t,
            (None, true) => style::TableStyle::Svg,
            (None, false) => cfg.tables.unwrap_or_default(),
        },
    };

    let plan = polish::plan(&ctx, &report, &style);
    if plan.is_empty() {
        println!("Nothing to apply — everything polish can fix mechanically is already in place.");
        if !args.suggest {
            println!("Run `repolish check .` for the findings that still need a human.");
            return exit::OK;
        }
        // --suggest 要的正是「机械修不了的那部分」,没有机械改动恰恰是它该上场的时候
    }

    let rel = |p: &std::path::Path| {
        p.strip_prefix(&ctx.root)
            .unwrap_or(p)
            .display()
            .to_string()
            .replace(std::path::MAIN_SEPARATOR, "/")
    };

    println!();
    if let Some(readme) = ctx.readme.as_ref() {
        if !plan.inserts.is_empty() {
            println!("  {}", rel(&readme.path));
            for insert in &plan.inserts {
                for line in insert.lines.iter().filter(|l| !l.is_empty()) {
                    println!("    + {}", line);
                }
                println!("      {}", insert.reason);
            }
            println!();
        }
    }
    for edit in &plan.translations {
        println!("  {}", rel(&edit.path));
        for insert in &edit.inserts {
            for line in insert.lines.iter().filter(|l| !l.is_empty()) {
                println!("    + {line}");
            }
            println!("      {}", insert.reason);
        }
        println!();
    }
    for f in &plan.side_files {
        println!(
            "  {}  ({} lines, new file)",
            rel(&f.path),
            f.contents.lines().count()
        );
        println!("      {}", f.reason);
        // README 的每一行插入都看得见，整个新文件却只报个路径，是说不过去的：
        // 落进别人仓库的东西，落盘前该能看全。
        if args.verbose {
            for line in f.contents.lines() {
                println!("      | {line}");
            }
        }
        println!();
    }

    if !args.apply {
        if !plan.is_empty() {
            if !plan.side_files.is_empty() && !args.verbose {
                println!("  Run with -v to print what each new file would contain.");
            }
            println!("\n  Dry run — nothing written. Re-run with --apply to write it.");
        }
        return run_suggest(args.suggest, &args.common, &ctx, &report);
    }

    // 没有 git 就没有撤销键。`polish --apply` 改的是别人的 README，
    // 在一个连 `git checkout` 都用不了的目录里默默改文件是不能接受的。
    if ctx.git.is_none() && !args.force {
        eprintln!(
            "error: {} is not a git repository, so there is no way to undo this.\n\
             Re-run with --force if you have another way to recover the file",
            ctx.root.display()
        );
        return exit::BAD_USAGE;
    }

    if let Some(readme) = ctx.readme.as_ref() {
        if !plan.inserts.is_empty() {
            let out = polish::polished(readme, &plan);
            if let Err(code) = write_file(&readme.path, &out) {
                return code;
            }
        }
    }
    // 译本和主 README 一样，只增量插入——切开原文拼回去，其余字节不碰
    for edit in &plan.translations {
        let out = repolish_md::edit::apply(&edit.raw, &edit.inserts);
        if let Err(code) = write_file(&edit.path, &out) {
            return code;
        }
    }
    for f in &plan.side_files {
        if let Err(code) = write_file(&f.path, &f.contents) {
            return code;
        }
    }

    // `git diff` 看不到未跟踪的新文件。照着那句话去检查，会以为 polish
    // 只改了 README —— 而它刚往仓库里放了四个文件。
    if plan.side_files.is_empty() {
        println!("  Written. Review with `git diff`, undo with `git checkout -- .`");
    } else {
        println!(
            "  Written. Review with `git add -A && git diff --staged` (plain `git diff` does not show new files).\n  \
             Undo with `git checkout -- . && git clean -fd`"
        );
    }
    run_suggest(args.suggest, &args.common, &ctx, &report)
}

/// `--suggest`：请模型写那三段机械方法写不出来的文字。
///
/// **一个字都不落盘**,`--apply` 也不例外。理由写在 [`suggest`] 的模块文档里:
/// 一段模型写的文字进了别人的 README 而他没有逐字看过,是不能接受的。
fn run_suggest(
    wanted: bool,
    common: &Common,
    ctx: &repolish_ingest::RepoContext,
    report: &repolish_core::Report,
) -> u8 {
    if !wanted {
        return exit::OK;
    }

    let kinds = suggest::wanted(report);
    if kinds.is_empty() {
        println!(
            "\n  No wording to suggest — the title, quick start and usage example all \
             score full marks.\n  Those are the only three things --suggest writes for."
        );
        return exit::OK;
    }

    let cfg = match crate::config::load(common.config.as_deref(), &ctx.root) {
        Ok(c) => c.suggest,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    let model = match suggest::Model::resolve(&cfg) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("\nerror: {e}");
            return exit::BAD_USAGE;
        }
    };

    let facts = suggest::Facts::from_ctx(ctx);
    let prompt = suggest::prompt(&facts, &kinds);
    eprintln!(
        "\n  asking {} for: {}",
        model.model,
        kinds
            .iter()
            .map(|k| k.label())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let answer = match suggest::ask(&model, &prompt) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    let suggestions = match suggest::parse(&answer) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    if suggestions.is_empty() {
        println!(
            "\n  The model had nothing to add. That usually means the repository does not \
             carry\n  the facts it would need — an install command it can point at, \
             a binary name.\n  Making one of those true is the fix; inventing one is not."
        );
        return exit::OK;
    }

    print!(
        "{}",
        suggest::render(&suggestions, &model.model, common.level())
    );
    exit::OK
}
