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
mod config;
mod init;
mod polish;
mod scaffold;
mod scan;
mod style;
mod tree;

use analyze::{analyze, write_file, Analysis, Common};

/// 退出码。工具自身失败与「检查不通过」必须区分，否则 CI 无法判断。
mod exit {
    pub const OK: u8 = 0;
    pub const BELOW_MIN_SCORE: u8 = 1;
    pub const BAD_USAGE: u8 = 2;
    pub const NOT_A_REPO: u8 = 3;
    pub const REMOTE_FAILED: u8 = 4;
    pub const LOW_COVERAGE: u8 = 5;
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
    /// Score every repository in a directory and rank them
    Scan(ScanArgs),
    /// Write .repolish/card.svg, a report card to embed in your README
    Card(CardArgs),
    /// Generate a GitHub Actions workflow
    Init(InitArgs),
    /// Apply the fixes that can be made mechanically, and print the rest
    Polish(PolishArgs),
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

    /// Width in pixels for the logo
    #[arg(long)]
    logo_width: Option<u32>,

    /// Append a project structure tree this many levels deep
    #[arg(long)]
    tree_depth: Option<usize>,
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

    /// Also write .repolish/card.svg
    #[arg(long)]
    card: bool,

    /// Show P3 suggestions and passing checks as well
    #[arg(short, long)]
    verbose: bool,
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
struct ScanArgs {
    #[command(flatten)]
    common: Common,

    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Exit with code 1 if any repository scores below this
    #[arg(long)]
    min_score: Option<u8>,
}

#[derive(Parser)]
struct CardArgs {
    #[command(flatten)]
    common: Common,

    /// Output path; defaults to .repolish/card.svg in the repository root
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print the SVG instead of writing the file
    #[arg(long)]
    stdout: bool,
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

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
    Markdown,
}

fn main() -> ExitCode {
    // Windows 的控制台默认不解释 ANSI，得在第一次输出之前把 VT 模式打开
    repolish_render::theme::enable_ansi();
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check(args) => run_check(args),
        Command::Badge(args) => run_badge(args),
        Command::Report(args) => run_report(args),
        Command::Scan(args) => run_scan(args),
        Command::Card(args) => run_card(args),
        Command::Init(args) => run_init(args),
        Command::Polish(args) => run_polish(args),
    };
    ExitCode::from(code)
}

fn run_check(args: CheckArgs) -> u8 {
    let Analysis {
        ctx,
        report,
        min_score,
    } = match analyze(&args.common) {
        Ok(a) => a,
        Err(code) => return code,
    };

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
    if args.card {
        let path = ctx.root.join(repolish_render::svg::CARD_PATH);
        if let Err(code) = write_file(&path, &repolish_render::card(&report)) {
            return code;
        }
        eprintln!("wrote {}", path.display());
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

fn run_scan(args: ScanArgs) -> u8 {
    let root = match dunce::canonicalize(&args.common.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot access {}: {e}", args.common.path.display());
            // scan 的第一步永远是「先把仓库弄到本地」。不提这一句，
            // 第一次用的人只会看到一个操作系统的路径错误。
            eprintln!(
                "note: scan reads repositories that are already on disk. \
                 To fetch a whole organisation first: ./scripts/clone-org.sh <org>"
            );
            return exit::NOT_A_REPO;
        }
    };

    let entries = scan::run(&root, &args.common, &args.common.only, &args.common.skip);
    if entries.is_empty() {
        eprintln!(
            "error: no repositories found in {}. Each repository has to be a direct subdirectory — clone them side by side, or point scan one level up",
            root.display()
        );
        return exit::NOT_A_REPO;
    }

    match args.format {
        Format::Json => {
            // 每个仓库原样吐出冻结的 Report，不另造一套 schema：
            // 消费方已经在解析这个形状了
            let reports: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| match &e.report {
                    Ok(r) => serde_json::json!({ "name": e.name, "report": r }),
                    Err(msg) => serde_json::json!({ "name": e.name, "error": msg }),
                })
                .collect();
            let doc = serde_json::json!({
                "repolishVersion": env!("CARGO_PKG_VERSION"),
                "schemaVersion": repolish_core::SCHEMA_VERSION,
                "repositories": reports,
            });
            match serde_json::to_string_pretty(&doc) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("error: serialization failed: {e}");
                    return exit::BAD_USAGE;
                }
            }
        }
        _ => print!(
            "{}",
            repolish_render::scan(
                &entries,
                &RenderOptions {
                    verbose: false,
                    level: args.common.level(),
                }
            )
        ),
    }

    // 一个仓库拉不到就算整次扫描失败：一张缺了几行的表会被当成完整的表读
    if entries.iter().any(|e| e.report.is_err()) {
        return exit::REMOTE_FAILED;
    }
    match args.min_score {
        Some(min)
            if entries
                .iter()
                .filter_map(|e| e.report.as_ref().ok())
                .any(|r| r.score.is_none_or(|s| s < min)) =>
        {
            exit::BELOW_MIN_SCORE
        }
        _ => exit::OK,
    }
}

fn run_card(args: CardArgs) -> u8 {
    let Analysis { ctx, report, .. } = match analyze(&args.common) {
        Ok(a) => a,
        Err(code) => return code,
    };

    let svg = repolish_render::card(&report);
    if args.stdout {
        print!("{svg}");
        return exit::OK;
    }

    let path = args
        .output
        .unwrap_or_else(|| ctx.root.join(repolish_render::svg::CARD_PATH));
    if let Err(code) = write_file(&path, &svg) {
        return code;
    }
    println!("wrote {}", path.display());

    let rel = path
        .strip_prefix(&ctx.root)
        .unwrap_or(&path)
        .display()
        .to_string()
        .replace(std::path::MAIN_SEPARATOR, "/");
    println!(
        "
Paste this into your README:
"
    );
    println!(
        "![repolish]({rel})
"
    );
    println!("The card is a plain file in your own repository: no fonts, no scripts, nothing hosted by us.");
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
    };

    let plan = polish::plan(&ctx, &report, &style);
    if plan.is_empty() {
        println!("Nothing to apply — everything polish can fix mechanically is already in place.");
        println!("Run `repolish check .` for the findings that still need a human.");
        return exit::OK;
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
        if !plan.side_files.is_empty() && !args.verbose {
            println!("  Run with -v to print what each new file would contain.");
        }
        println!("\n  Dry run — nothing written. Re-run with --apply to write it.");
        return exit::OK;
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
    exit::OK
}
