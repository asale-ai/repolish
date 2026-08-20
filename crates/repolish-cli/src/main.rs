//! repolish —— 开源仓库诊断 / 优化工具
//!
//! M3 实现 `check` / `badge` / `report` / `init`。`polish` 在 M4。

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use repolish_render::RenderOptions;

mod analyze;
mod init;

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
    /// Generate a GitHub Actions workflow
    Init(InitArgs),
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
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check(args) => run_check(args),
        Command::Badge(args) => run_badge(args),
        Command::Report(args) => run_report(args),
        Command::Init(args) => run_init(args),
    };
    ExitCode::from(code)
}

fn run_check(args: CheckArgs) -> u8 {
    let Analysis { ctx, report } = match analyze(&args.common) {
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
                    color: args.common.color(),
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

    verdict(&report, args.min_score)
}

fn run_badge(args: BadgeArgs) -> u8 {
    let Analysis { ctx, report } = match analyze(&args.common) {
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
    let Analysis { ctx, report } = match analyze(&args.common) {
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
