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

    /// Read description / topics / homepage from the GitHub API. With a token in
    /// GITHUB_TOKEN or GH_TOKEN this happens by default; passing it explicitly
    /// also tries anonymously, on a quota of 60 requests per hour
    #[arg(long, global = true)]
    pub remote: bool,

    /// Never call the GitHub API. The three checks that need it report as
    /// not verified
    #[arg(long, global = true, conflicts_with = "remote")]
    pub no_remote: bool,

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

    /// Fetch the star history curve for the overview card. Under --remote it is
    /// on by default whenever a token is set; this asks for it either way
    #[arg(long, global = true)]
    pub stars: bool,

    /// Skip the star history curve. It costs about a dozen extra API calls
    #[arg(long, global = true, conflicts_with = "stars")]
    pub no_stars: bool,

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

/// 这次运行会用到 star 曲线吗。
#[derive(Copy, Clone)]
pub struct StarsWanted {
    /// 概览卡会被画出来——那是曲线唯一的去处
    pub overview: bool,
}

/// 这次运行**因为缺东西而没做成**的事。
///
/// 和错误不是一回事：跑完了，只是少做了几件。散落在过程中各说一句，读的人
/// 滚上去就看不见了；攒到最后一起报，并且每条都带上把它补齐的那条命令。
#[derive(Default)]
pub struct Gaps(Vec<Gap>);

pub struct Gap {
    /// 少了什么
    pub what: String,
    /// 怎么补
    pub fix: String,
}

impl Gaps {
    pub fn note(&mut self, what: impl Into<String>, fix: impl Into<String>) {
        self.0.push(Gap {
            what: what.into(),
            fix: fix.into(),
        });
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Gap> {
        self.0.iter()
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
    /// 因为缺东西而没做成的事
    pub gaps: Gaps,
}

/// 失败时已经打印过错误，直接返回退出码。
/// `wanted` 说的是「这次运行会不会真的画那张概览卡」。
///
/// star 曲线**只**出现在概览卡上。不画那张卡还去拉曲线，就是十几次白花的
/// API 请求——Action 的 `overview` 默认是 false，那正是最常见的一种运行。
pub fn analyze(common: &Common, wanted: StarsWanted) -> Result<Analysis, u8> {
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
    //
    // 只在**显式**要了曲线时才提醒。默认是开的，本地模式下每跑一次都提醒
    // 一遍「你要的东西没发生」，那是纯噪音。
    if common.stars && common.no_remote {
        eprintln!("warning: --stars cannot work with --no-remote; no star history was fetched");
    }

    let mut gaps = Gaps::default();
    let token = repolish_ingest::remote::token_from_env();

    // 远程默认走,**但只在拿得到 token 时**。匿名配额一小时 60 次,而且
    // 一次限流会以退出码 4 打断整条流水线——把那当默认，等于让离线的人
    // 每次都撞墙。显式给了 `--remote` 就照办，匿名也去试:那是使用者的取舍。
    let remote = if common.no_remote {
        false
    } else if common.remote {
        true
    } else {
        token.is_some()
    };
    if !remote && !common.no_remote {
        gaps.note(
            "repository description, topics and homepage were not checked",
            "set GITHUB_TOKEN or GH_TOKEN (`export GITHUB_TOKEN=$(gh auth token)`), \
             or pass --remote to try on the anonymous quota",
        );
    }

    if remote {
        // 曲线默认要，**但匿名时自动让路**。它要十几次请求，而匿名配额一小时
        // 只有 60 次——把五分之一花在一段装饰上，代价是真正的评分调用 429。
        // 显式给了 `--stars` 就照办：那是使用者自己的取舍。
        let stars = if common.stars {
            true
        } else if common.no_stars || !wanted.overview {
            false
        } else {
            token.is_some()
        };
        if common.stars && token.is_none() {
            eprintln!(
                "warning: --stars costs about a dozen API calls and no token is set; \
                 the anonymous quota is 60 per hour"
            );
        }
        if token.is_none() {
            eprintln!("warning: GITHUB_TOKEN is not set; falling back to the anonymous quota of 60 requests per hour");
        }
        if !stars && wanted.overview && !common.no_stars {
            gaps.note(
                "the overview card has no star history curve",
                if token.is_none() {
                    "set GITHUB_TOKEN — anonymously the dozen calls it costs would be a \
                     fifth of the 60/hour quota"
                } else {
                    "pass --stars"
                },
            );
        }
        if let Err(e) = ctx.fetch_remote(token.as_deref(), stars) {
            // 显式要了远程,失败就是失败——CI 的门禁靠退出码 4 把「限流」
            // 和「变差了」分开。默认走到这里则降级为本地,并记一笔:一次
            // 网络抖动不该让离线也能做完的那些事全部作废。
            if common.remote {
                eprintln!("error: {e}");
                return Err(exit::REMOTE_FAILED);
            }
            eprintln!("warning: the GitHub API call failed: {e}");
            gaps.note(
                "repository description, topics and homepage were not checked",
                "the GitHub call failed; re-run with --remote to see the error and fail on it",
            );
        }
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
        // **看真的抓没抓，而不是看那个开关。** 默认路径下有 token 就会去抓，
        // 此时标成 Local 会让三个远程检查在数据已经拿到的情况下仍被记为
        // Skipped——分数因此偏低，而报告上那行「local」还在说另一套。
        mode: if remote { Mode::Remote } else { Mode::Local },
        only: only.iter().cloned().collect(),
        skip: skip.iter().cloned().collect(),
    };

    let report = registry.run(&ctx, &opts);
    Ok(Analysis {
        ctx,
        report,
        min_score: config.min_score,
        opts,
        gaps,
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
