//! 所有子命令共用的一段：载入仓库 → 可选拉取远程 → 跑注册表。
//!
//! 抽出来是为了让 `check` / `badge` / `report` 走的是同一条路径——
//! 三者给出不同的分数会立刻毁掉信任。

use std::collections::HashSet;
use std::path::PathBuf;

use clap::Args;
use repolish_core::registry::RunOptions;
use repolish_core::{Mode, Profile, RepoContext, Report};

use crate::exit;

#[derive(Args, Clone)]
pub struct Common {
    /// Path to the repository
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Also read description / topics / homepage from the GitHub API.
    /// The token comes from GITHUB_TOKEN or GH_TOKEN; without one, the anonymous
    /// quota of 60 requests per hour applies
    #[arg(long, global = true)]
    pub remote: bool,

    /// Override the detected project type
    #[arg(long, global = true, default_value = "auto")]
    pub profile: String,

    /// Run only these checks (comma-separated)
    #[arg(long, global = true, value_delimiter = ',')]
    pub only: Vec<String>,

    /// Skip these checks (comma-separated)
    #[arg(long, global = true, value_delimiter = ',')]
    pub skip: Vec<String>,

    #[arg(long, global = true)]
    pub no_color: bool,
}

impl Common {
    pub fn color(&self) -> bool {
        !self.no_color && std::env::var_os("NO_COLOR").is_none()
    }
}

pub struct Analysis {
    pub ctx: RepoContext,
    pub report: Report,
}

/// 失败时已经打印过错误，直接返回退出码。
pub fn analyze(common: &Common) -> Result<Analysis, u8> {
    let profile_override = if common.profile == "auto" {
        None
    } else {
        match Profile::parse(&common.profile) {
            Some(p) => Some(p),
            None => {
                eprintln!(
                    "error: unknown profile \"{}\" — expected one of: auto, library, app, cli, docs, collection",
                    common.profile
                );
                return Err(exit::BAD_USAGE);
            }
        }
    };

    let mut ctx = RepoContext::load(&common.path, profile_override).map_err(|e| {
        eprintln!("error: {e:#}");
        exit::NOT_A_REPO
    })?;

    if ctx.git.is_none() && !ctx.root.join(".git").exists() {
        eprintln!(
            "warning: {} is not a git repository; checks that need commit history will report inconclusive",
            ctx.root.display()
        );
    }

    // 拉不到就退出，不静默降级：本地分与远程分基准不同，
    // 悄悄换基准会让用户拿到一个看不出差异的错分数。
    if common.remote {
        let token = repolish_ingest::remote::token_from_env();
        if token.is_none() {
            eprintln!("warning: GITHUB_TOKEN is not set; falling back to the anonymous quota of 60 requests per hour");
        }
        ctx.fetch_remote(token.as_deref()).map_err(|e| {
            eprintln!("error: {e}");
            exit::REMOTE_FAILED
        })?;
    }

    let registry = repolish_checks::registry();

    let known: HashSet<&str> = registry.ids().into_iter().collect();
    if let Some(bad) = common
        .only
        .iter()
        .chain(common.skip.iter())
        .find(|id| !known.contains(id.as_str()))
    {
        eprintln!("error: unknown check id \"{bad}\"");
        eprintln!("available: {}", registry.ids().join(", "));
        return Err(exit::BAD_USAGE);
    }

    let opts = RunOptions {
        mode: if common.remote {
            Mode::Remote
        } else {
            Mode::Local
        },
        only: common.only.iter().cloned().collect(),
        skip: common.skip.iter().cloned().collect(),
    };

    let report = registry.run(&ctx, &opts);
    Ok(Analysis { ctx, report })
}

/// 写文件并打印去向。父目录不存在时一并创建。
pub fn write_file(path: &std::path::Path, contents: &str) -> Result<(), u8> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| {
                eprintln!("error: cannot create directory {}: {e}", dir.display());
                exit::BAD_USAGE
            })?;
        }
    }
    std::fs::write(path, contents).map_err(|e| {
        eprintln!("error: cannot write {}: {e}", path.display());
        exit::BAD_USAGE
    })
}
