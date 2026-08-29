//! 所有子命令共用的一段：载入仓库 → 可选拉取远程 → 跑注册表。
//!
//! 抽出来是为了让 `check` / `badge` / `report` 走的是同一条路径——
//! 三者给出不同的分数会立刻毁掉信任。

use std::collections::HashSet;
use std::io::IsTerminal;
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
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Path to the config file; defaults to .repolish.toml in the repository root
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Run only these checks (comma-separated)
    #[arg(long, global = true, value_delimiter = ',')]
    pub only: Vec<String>,

    /// Skip these checks (comma-separated)
    #[arg(long, global = true, value_delimiter = ',')]
    pub skip: Vec<String>,

    /// Also fetch the star history curve for the overview card.
    /// Costs about a dozen extra API calls, so it is off by default
    #[arg(long, global = true)]
    pub stars: bool,

    #[arg(long, global = true)]
    pub no_color: bool,

    /// Colour palette for the SVG it writes
    #[arg(long, global = true, value_enum)]
    pub theme: Option<crate::style::Theme>,

    /// Language for the text inside the SVG it writes.
    /// Defaults to whatever language the README is written in
    #[arg(long, global = true, value_enum)]
    pub lang: Option<crate::style::CardLang>,
}

impl Common {
    /// 终端色彩能力。探测规则在 `repolish_render::theme`——
    /// `--no-color` 只是把用户的意图递进去，别的判断不在这里重写一遍。
    pub fn level(&self) -> repolish_render::ColorLevel {
        repolish_render::ColorLevel::detect(self.no_color, std::io::stdout().is_terminal())
    }

    /// 卡片渲染选项。命令行 > 配置文件 > 默认，语言的默认是「跟着 README 走」。
    pub fn card_options(
        &self,
        ctx: &RepoContext,
        cfg: &crate::config::Readme,
    ) -> repolish_render::Options {
        let readme = ctx.readme.as_ref().map(|r| r.raw.as_str()).unwrap_or("");
        repolish_render::Options {
            palette: self.theme.or(cfg.theme).unwrap_or_default().palette(),
            lang: self.lang.or(cfg.lang).unwrap_or_default().resolve(readme),
        }
    }
}

pub struct Analysis {
    pub ctx: RepoContext,
    pub report: Report,
    /// 配置文件里的 `min_score`。命令行给了就用命令行的。
    pub min_score: Option<u8>,
    /// 产出这份报告用的那一套选项。`--base` 必须用**同一套**去跑基线：
    /// 两侧的 mode、`--only`、`--skip` 有任何一处不同,差值就是拿两把不同的
    /// 尺子相减。
    pub opts: RunOptions,
}

/// 失败时已经打印过错误，直接返回退出码。
pub fn analyze(common: &Common) -> Result<Analysis, u8> {
    let root = dunce::canonicalize(&common.path).map_err(|e| {
        eprintln!("error: cannot access {}: {e}", common.path.display());
        let _ = e;
        exit::NOT_A_REPO
    })?;

    let config = crate::config::load(common.config.as_deref(), &root).map_err(|e| {
        eprintln!("error: {e}");
        exit::BAD_USAGE
    })?;

    // 命令行 > 配置文件 > 自动探测
    let requested = common.profile.as_deref().or(config.profile.as_deref());
    let profile_override = match requested {
        None | Some("auto") => None,
        Some(name) => match Profile::parse(name) {
            Some(p) => Some(p),
            None => {
                eprintln!(
                    "error: unknown profile \"{name}\" — expected one of: auto, library, app, cli, docs, collection, meta"
                );
                return Err(exit::BAD_USAGE);
            }
        },
    };

    let mut ctx = RepoContext::load(&root, profile_override).map_err(|e| {
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
    if common.stars && !common.remote {
        eprintln!("warning: --stars needs --remote; no star history was fetched");
    }
    if common.remote {
        let token = repolish_ingest::remote::token_from_env();
        // 曲线要十几次请求。匿名配额一小时只有 60 次，用掉五分之一还没提示，
        // 使用者只会看到下一条命令莫名其妙地 429。
        if common.stars && token.is_none() {
            eprintln!(
                "warning: --stars costs about a dozen API calls and no token is set; \
                 the anonymous quota is 60 per hour"
            );
        }
        if token.is_none() {
            eprintln!("warning: GITHUB_TOKEN is not set; falling back to the anonymous quota of 60 requests per hour");
        }
        ctx.fetch_remote(token.as_deref(), common.stars)
            .map_err(|e| {
                eprintln!("error: {e}");
                exit::REMOTE_FAILED
            })?;
    }

    // 曲线取不到不是错误，但也不能不吭声：使用者对着一张没有曲线的卡片
    // 是猜不出原因的，而这个原因他有办法处理（换一个有权限的令牌）。
    if let Some(note) = ctx.remote.as_ref().and_then(|r| r.star_note.clone()) {
        eprintln!("warning: no star history — {note}");
    }

    let registry = repolish_checks::registry();

    // 空表示「没指定」，此时才轮到配置文件
    let only = pick(&common.only, &config.checks.only);
    let skip = pick(&common.skip, &config.checks.skip);

    let known: HashSet<&str> = registry.ids().into_iter().collect();
    if let Some(bad) = only
        .iter()
        .chain(skip.iter())
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
        only: only.iter().cloned().collect(),
        skip: skip.iter().cloned().collect(),
    };

    let report = registry.run(&ctx, &opts);
    Ok(Analysis {
        ctx,
        report,
        min_score: config.min_score,
        opts,
    })
}

/// 命令行给了就用命令行的，否则用配置文件的
fn pick<'a>(cli: &'a [String], cfg: &'a [String]) -> &'a [String] {
    if cli.is_empty() {
        cfg
    } else {
        cli
    }
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
