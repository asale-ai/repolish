//! repolish —— 开源仓库诊断 / 优化工具
//!
//! M2 实现 `check`（含 `--remote`）。badge / report / init 在 M3，polish 在 M4。

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use repolish_core::registry::RunOptions;
use repolish_core::{Mode, Profile, RepoContext, Report};
use repolish_render::RenderOptions;

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
}

#[derive(Parser)]
struct CheckArgs {
    /// 仓库路径
    #[arg(default_value = ".")]
    path: PathBuf,

    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// 补充 GitHub 元数据（description / topics / homepage）。
    /// token 取自 GITHUB_TOKEN / GH_TOKEN，缺省则走匿名配额（每小时 60 次）
    #[arg(long)]
    remote: bool,

    /// 低于该分数时以退出码 1 结束，可用作 CI 门禁
    #[arg(long)]
    min_score: Option<u8>,

    /// 覆盖项目类型探测结果
    #[arg(long, default_value = "auto")]
    profile: String,

    /// 只跑这些检查项（逗号分隔）
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,

    /// 跳过这些检查项（逗号分隔）
    #[arg(long, value_delimiter = ',')]
    skip: Vec<String>,

    #[arg(long)]
    no_color: bool,

    /// 展开 P3 建议与已通过项
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check(args) => run_check(args),
    };
    ExitCode::from(code)
}

fn run_check(args: CheckArgs) -> u8 {
    let profile_override = if args.profile == "auto" {
        None
    } else {
        match Profile::parse(&args.profile) {
            Some(p) => Some(p),
            None => {
                eprintln!(
                    "error: 未知的 profile「{}」，可选：auto, library, app, cli, docs, collection",
                    args.profile
                );
                return exit::BAD_USAGE;
            }
        }
    };

    let mut ctx = match RepoContext::load(&args.path, profile_override) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            return exit::NOT_A_REPO;
        }
    };

    // 失败即退出，不静默降级成本地模式——两种模式的分数基准不同，
    // 悄悄换基准会让用户拿到一个看不出差异的错分数。
    if args.remote {
        let token = repolish_ingest::remote::token_from_env();
        if token.is_none() {
            eprintln!("warning: 未设置 GITHUB_TOKEN，将走匿名配额（每小时 60 次）");
        }
        if let Err(e) = ctx.fetch_remote(token.as_deref()) {
            eprintln!("error: {e}");
            return exit::REMOTE_FAILED;
        }
    }

    if ctx.git.is_none() && !ctx.root.join(".git").exists() {
        eprintln!(
            "warning: {} 不是 git 仓库，与提交历史相关的检查项将标记为 inconclusive",
            ctx.root.display()
        );
    }

    let registry = repolish_checks::registry();

    let known: HashSet<&str> = registry.ids().into_iter().collect();
    if let Some(bad) = args
        .only
        .iter()
        .chain(args.skip.iter())
        .find(|id| !known.contains(id.as_str()))
    {
        eprintln!("error: 未知的检查项 id「{bad}」");
        eprintln!("可用: {}", registry.ids().join(", "));
        return exit::BAD_USAGE;
    }

    let opts = RunOptions {
        mode: if args.remote { Mode::Remote } else { Mode::Local },
        only: args.only.iter().cloned().collect(),
        skip: args.skip.iter().cloned().collect(),
    };

    let report = registry.run(&ctx, &opts);

    match args.format {
        Format::Text => {
            print!(
                "{}",
                repolish_render::terminal(
                    &report,
                    &RenderOptions {
                        verbose: args.verbose,
                        color: !args.no_color && std::env::var_os("NO_COLOR").is_none(),
                    }
                )
            );
        }
        Format::Json => match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: 序列化失败: {e}");
                return exit::BAD_USAGE;
            }
        },
    }

    verdict(&report, args.min_score)
}

fn verdict(report: &Report, min_score: Option<u8>) -> u8 {
    let Some(score) = report.score else {
        return exit::LOW_COVERAGE;
    };
    match min_score {
        Some(min) if score < min => exit::BELOW_MIN_SCORE,
        _ => exit::OK,
    }
}
