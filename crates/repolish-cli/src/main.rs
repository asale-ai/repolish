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
    about = "诊断并优化开源仓库的可发现性、可理解性与可信度"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 诊断仓库并输出评分报告
    Check(CheckArgs),
    /// 写出 .repolish/badge.json，并打印可粘贴的徽章 markdown
    Badge(BadgeArgs),
    /// 写出 REPOLISH.md
    Report(ReportArgs),
    /// 生成 GitHub Actions workflow
    Init(InitArgs),
}

#[derive(Parser)]
struct CheckArgs {
    #[command(flatten)]
    common: Common,

    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// 低于该分数时以退出码 1 结束，可用作 CI 门禁
    #[arg(long)]
    min_score: Option<u8>,

    /// 顺带写出 .repolish/badge.json
    #[arg(long)]
    badge: bool,

    /// 顺带写出 REPOLISH.md
    #[arg(long)]
    report: bool,

    /// 展开 P3 建议与已通过项
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Parser)]
struct BadgeArgs {
    #[command(flatten)]
    common: Common,

    /// 徽章 JSON 所在的分支，写进 snippet 的 URL。默认取当前分支
    #[arg(long)]
    branch: Option<String>,

    /// 打印 JSON 而不写文件
    #[arg(long)]
    stdout: bool,
}

#[derive(Parser)]
struct ReportArgs {
    #[command(flatten)]
    common: Common,

    /// 输出路径，默认仓库根下的 REPOLISH.md
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// 打印到标准输出而不写文件
    #[arg(long)]
    stdout: bool,
}

#[derive(Parser)]
struct InitArgs {
    /// 仓库路径
    #[arg(default_value = ".")]
    path: PathBuf,

    /// workflow 中的门禁分数；不给则只记录不拦截
    #[arg(long, default_value = "60")]
    min_score: Option<u8>,

    /// 不设门禁，只记录分数
    #[arg(long, conflicts_with = "min_score")]
    no_gate: bool,

    /// 覆盖已存在的 workflow
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
                eprintln!("error: 序列化失败: {e}");
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
        eprintln!("已写出 {}", path.display());
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
            "error: 有效检查覆盖率仅 {:.0}%，低于 50%，不生成徽章。\n\
             挂一个基于三分之一证据的分数，比不挂更糟",
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
    println!("已写出 {}", path.display());

    let branch = args
        .branch
        .or_else(|| ctx.git.as_ref().and_then(|g| g.branch.clone()))
        .unwrap_or_else(|| "main".to_string());

    println!("\n把这一行粘进 README：\n");
    match &ctx.slug {
        Some(slug) => println!("{}\n", repolish_render::snippet(&slug.owner, &slug.name, &branch)),
        None => {
            println!("{}\n", repolish_render::snippet("OWNER", "REPO", &branch));
            eprintln!(
                "warning: 没有找到 GitHub 远端，snippet 里的 OWNER / REPO 需要你手填"
            );
        }
    }
    println!("徽章由 shields.io 读取你自己仓库里的 {}，我们不托管任何东西。", repolish_render::BADGE_PATH);
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
    println!("已写出 {}", path.display());
    exit::OK
}

fn run_init(args: InitArgs) -> u8 {
    let root = match dunce::canonicalize(&args.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: 无法访问 {}: {e}", args.path.display());
            return exit::NOT_A_REPO;
        }
    };

    let path = root.join(init::WORKFLOW_PATH);
    if path.exists() && !args.force {
        eprintln!(
            "error: {} 已存在。确认要覆盖就加 --force",
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

    println!("已写出 {}", path.display());
    println!("  · 触发分支: {branch}");
    match min_score {
        Some(n) => println!("  · 门禁: 低于 {n} 分则 CI 失败"),
        None => println!("  · 门禁: 未设置，只记录分数"),
    }
    println!(
        "\n注意：模板引用的 action 是 asale-ai/repolish@v{}，需要该版本已发布才能跑通。",
        env!("CARGO_PKG_VERSION")
    );
    exit::OK
}

fn write_badge(ctx: &repolish_ingest::RepoContext, report: &repolish_core::Report) -> Result<(), u8> {
    let Some(json) = repolish_render::badge_json(report) else {
        eprintln!("warning: 覆盖率不足，未生成徽章");
        return Err(exit::LOW_COVERAGE);
    };
    let path = ctx.root.join(repolish_render::BADGE_PATH);
    write_file(&path, &json)?;
    eprintln!("已写出 {}", path.display());
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
