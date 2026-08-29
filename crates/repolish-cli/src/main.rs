//! repolish —— 开源仓库诊断 / 优化工具
//!
//! **一条命令,一条流水线。** `repolish` 不带参数就跑完 check → polish →
//! artifacts → ci 四个阶段,并且**默认一个字节都不写**——先给出完整的
//! 「会改哪些文件」,`--apply` 才落盘。子命令已经取消:同一件事只有一种敲法。
//!
//! 分析只做一次,四个阶段共用。分开跑意味着多打几次 GitHub API,也意味着
//! 几份产物有可能来自不同的评分结果。
//!
//! `polish` 只做能机械落实的改动，且**只增量插入**——为什么不能让 AST
//! 产出文本，见 repolish-md 的 crate 文档。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
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

use analyze::{analyze, write_file, Analysis, Common, Gaps, StarsWanted};
use repolish_md::Readme;

/// 退出码。工具自身失败与「检查不通过」必须区分，否则 CI 无法判断。
///
/// 7 在「环境没跑起来」那一侧,与 4 同类:浅克隆里不存在的基线不是质量回归。
/// 把它报成 1,CI 上就分不出「这个 PR 变差了」和「今天基线取不到」。
///
/// 6 曾经是 `verify --run` 跑不起来。`verify` 已移除,**这个值不再复用**——
/// 老版本的脚本里可能还留着对 6 的判断,让它保持空缺比换个含义安全。
mod exit {
    pub const OK: u8 = 0;
    /// 分数低于 `--min-score`
    pub const BELOW_MIN_SCORE: u8 = 1;
    pub const BAD_USAGE: u8 = 2;
    pub const NOT_A_REPO: u8 = 3;
    pub const REMOTE_FAILED: u8 = 4;
    pub const LOW_COVERAGE: u8 = 5;
    /// `--base` 的基线取不到：浅克隆、ref 不存在、没有 git
    pub const BASE_FAILED: u8 = 7;
}

/// 提示里该怎么称呼自己。
///
/// `npx @asale/repolish` 里的那份二进制跑完就没了，`repolish` 从来没进过
/// PATH。照着我们印出来的 ``Run `repolish --apply``` 敲下去，得到的是
/// command not found——提示把人送进了死路。
///
/// 只有启动器知道自己是怎么被调起来的（npx 缓存、项目依赖、还是全局装），
/// 所以由它用 `REPOLISH_INVOKED_AS` 把这件事告诉我们。没有这个变量就是
/// 直接执行的二进制，裸 `repolish` 本来就对。
fn invocation() -> String {
    // 这段字符串会被原样印进提示里，所以只放行命令行里该有的字符——
    // 环境变量不是可信输入。
    fn plausible(v: &str) -> bool {
        !v.is_empty()
            && v.len() <= 40
            && v.chars()
                .all(|c| c.is_ascii_alphanumeric() || " @/._-".contains(c))
    }
    match std::env::var("REPOLISH_INVOKED_AS") {
        Ok(v) if plausible(&v) => v,
        _ => "repolish".to_string(),
    }
}

/// 过程性输出走哪条流。
///
/// `--format` 不是 text 时,stdout 必须只有那一份报告——否则下游第一个
/// `jq` 就会炸。进度是进度,不是数据。
macro_rules! say {
    ($cli:expr, $($arg:tt)*) => {
        if $cli.format == Format::Text { println!($($arg)*) } else { eprintln!($($arg)*) }
    };
}

// ── 命令行 ──────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "repolish",
    version,
    about = "Diagnose and improve how discoverable, understandable, and credible an open-source repository is",
    after_help = "\
Run it with no arguments and it scores the repository, then reports every file it \
would create or change — and writes NOTHING. Add --apply to write.

  repolish                 score, and print the whole plan
  repolish --apply         do it
  repolish --stages check  score only, nothing else

There are no subcommands. --stages picks which parts of the pipeline run."
)]
struct Cli {
    #[command(flatten)]
    common: Common,

    // ── 流水线 ──
    /// Write the changes. Without it, repolish only reports what it would do
    #[arg(long)]
    apply: bool,

    /// Overwrite files that already exist, and write outside a git repository
    /// too, where there is nothing to undo with
    #[arg(long)]
    force: bool,

    /// Which parts of the pipeline to run. Comma-separated.
    /// Defaults to check,polish,artifacts,ci
    #[arg(long, value_enum, value_delimiter = ',')]
    stages: Vec<Stage>,

    /// Show P3 suggestions, passing checks, and the full contents of every new file
    #[arg(short, long)]
    verbose: bool,

    // ── 报告 ──
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Exit with code 1 below this score, and gate the generated CI workflow on it
    #[arg(long)]
    min_score: Option<u8>,

    /// Generate a CI workflow that records the score without enforcing it
    #[arg(long, conflicts_with = "min_score")]
    no_gate: bool,

    /// Also score this git ref and report the difference, e.g. `origin/main`.
    /// The baseline is checked out into a temporary worktree; your working tree is
    /// never touched
    #[arg(long, value_name = "REF")]
    base: Option<String>,

    /// Write a SARIF file. GitHub renders each finding on its own line in the
    /// pull request diff. Written even without --apply: you named the path
    #[arg(long, value_name = "PATH")]
    sarif: Option<PathBuf>,

    /// Write the short form meant for a pull request comment. With --base it
    /// leads with what this change did to the score. Written even without --apply
    #[arg(long, value_name = "PATH")]
    comment: Option<PathBuf>,

    /// Also write REPOLISH.md, the full report as markdown
    #[arg(long)]
    report: bool,

    /// Skip the badge JSON in the artifacts stage
    #[arg(long)]
    no_badge: bool,

    /// Restrict the artifacts stage to these. Comma-separated. Without it the
    /// stage writes the badge and redraws whatever the README already references
    #[arg(long, value_enum, value_delimiter = ',')]
    artifact: Vec<Artifact>,

    // ── 单阶段的出口 ──
    /// Output path. Only valid with a single --stages that produces one file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print the artifact instead of writing it. Only valid with a single --stages
    #[arg(long)]
    stdout: bool,

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

    /// Draw a banner carrying this project's name, and put it above the title
    #[arg(long)]
    hero: bool,

    /// Insert a project overview card below the badges, and draw it
    #[arg(long)]
    overview: bool,

    /// Insert the repolish report card at the end, under its own heading
    #[arg(long)]
    footer_card: bool,

    /// Render README tables as SVG, folding the original into <details>
    #[arg(long, value_enum)]
    tables: Option<style::TableStyle>,

    /// Shorthand for --overview --footer-card --tables svg. On by default;
    /// this only reasserts it after --no-visuals
    #[arg(long)]
    visuals: bool,

    /// Leave the README's visuals alone: no overview card, no report card, and
    /// tables stay as markdown
    #[arg(long)]
    no_visuals: bool,

    /// Also ask a model to draft the three pieces repolish cannot write
    /// mechanically: the tagline, the quick start and the usage example. Needs
    /// REPOLISH_LLM_API_KEY (or ANTHROPIC_API_KEY). It PRINTS them — nothing is
    /// written, not even with --apply — and no score is affected
    #[arg(long)]
    suggest: bool,

    /// Branch the badge JSON lives on, used in the snippet URL.
    /// Defaults to the current branch
    #[arg(long)]
    branch: Option<String>,

    // ── stage: demo ──
    /// The command to record for the `demo` stage. Repeat for several.
    /// Defaults to the detected binary's --help
    #[arg(long = "cmd")]
    commands: Vec<String>,

    /// Also write a VHS tape, for rendering a GIF with charmbracelet/vhs instead
    #[arg(long)]
    tape: bool,

    /// Milliseconds per keystroke in the recorded animation
    #[arg(long, default_value = "45")]
    type_ms: u32,

    // ── stage: skill ──
    /// Install the agent skill into an agent's own directory instead of writing
    /// SKILL.md into the repository. Comma-separated; `detect` picks the agents
    /// that are actually installed
    #[arg(long, value_delimiter = ',')]
    target: Vec<String>,

    /// List the known agents and whether each one is installed here
    #[arg(long)]
    list: bool,
}

/// 流水线的一段。顺序即执行顺序,而顺序是有意义的:`polish` 可能刚往 README
/// 里插入了一张卡片的引用,`artifacts` 紧接着才画得出那张图。
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Stage {
    /// Score the repository and print the report
    Check,
    /// Apply the fixes that follow mechanically from the findings
    Polish,
    /// Redraw everything the README already references, and the badge JSON
    Artifacts,
    /// Write the GitHub Actions workflow
    Ci,
    /// Write SKILL.md, which teaches a coding agent how to drive repolish
    Skill,
    /// Record the CLI as an animated SVG. RUNS the commands it records
    Demo,
}

/// 不给 `--stages` 时跑的四段。
///
/// `skill` **刻意不在默认里**:它往仓库(或用户主目录)里放一份只有用智能体的
/// 人才需要的文档。
///
/// `demo` 在默认里，但**只有 `--apply` 才会真的执行**那些命令。不加 `--apply`
/// 时它只列清单——那份清单就是执行前的知情同意。
const DEFAULT_STAGES: &[Stage] = &[
    Stage::Check,
    Stage::Polish,
    Stage::Artifacts,
    Stage::Ci,
    Stage::Demo,
];

/// `artifacts` 阶段能产出的东西。`--artifact` 点名时**只**产出点到的那些,
/// 并且不再要求「README 已经引用过」——点名本身就是那个要求。
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Artifact {
    /// .repolish/badge.json, read by shields.io out of your own repository
    Badge,
    /// .repolish/hero.svg — the banner above the README's title
    Hero,
    /// REPOLISH.md, the full report as markdown
    Report,
    /// .repolish/overview.svg — what this project is
    Overview,
    /// .repolish/card.svg — what repolish scored it
    Score,
    /// Redraw the SVG for every README table that is already wrapped
    Tables,
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

// ── 落盘账本 ────────────────────────────────────────────────────────────

/// 每个阶段都往这里记一笔,最后统一汇报。
///
/// 「会写什么」和「写了什么」必须是同一份清单——两处各自维护,迟早会出现
/// 干跑说三个文件、`--apply` 却写了四个。
struct Ledger {
    apply: bool,
    entries: Vec<Entry>,
}

struct Entry {
    rel: String,
    note: String,
}

impl Ledger {
    fn new(apply: bool) -> Self {
        Self {
            apply,
            entries: Vec::new(),
        }
    }

    /// 记一笔,`--apply` 时同时落盘。
    ///
    /// **同一个路径只占一行。** 两个阶段写同一个文件是正常的:`--visuals` 下
    /// `polish` 要把卡片画出来,否则它刚插进 README 的 `<img>` 指向一个不存在
    /// 的文件;`artifacts` 随后又会重画同一张——它才是重画的归属方。内容一致,
    /// 但报成两行就等于告诉使用者「9 个文件」而实际只有 7 个,而这份清单唯一
    /// 的价值就是它说的和落盘的是同一件事。后写的赢:`artifacts` 是权威。
    fn write(
        &mut self,
        root: &Path,
        path: &Path,
        contents: &str,
        note: impl Into<String>,
    ) -> Result<(), u8> {
        let rel = relative(root, path);
        if let Some(e) = self.entries.iter_mut().find(|e| e.rel == rel) {
            if self.apply {
                write_file(path, contents)?;
            }
            e.note = note.into();
            return Ok(());
        }
        if self.apply {
            write_file(path, contents)?;
        }
        self.entries.push(Entry {
            rel,
            note: note.into(),
        });
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── 入口 ────────────────────────────────────────────────────────────────

fn main() -> ExitCode {
    // Windows 的控制台默认不解释 ANSI，得在第一次输出之前把 VT 模式打开
    repolish_render::theme::enable_ansi();
    ExitCode::from(run(Cli::parse()))
}

fn run(cli: Cli) -> u8 {
    let stages: Vec<Stage> = if cli.stages.is_empty() {
        DEFAULT_STAGES.to_vec()
    } else {
        let mut s = cli.stages.clone();
        s.dedup();
        s
    };

    // 每张产物的默认路径都不同,所以 `--output` / `--stdout` 只在选定一个
    // 阶段时说得清。给四个阶段一个输出路径是没有意义的。
    if (cli.output.is_some() || cli.stdout) && stages.len() != 1 {
        eprintln!(
            "error: --output and --stdout each handle a single artifact, but {} stages are selected",
            stages.len()
        );
        eprintln!(
            "note: name one, e.g. `{} --stages artifacts --stdout`",
            invocation()
        );
        return exit::BAD_USAGE;
    }

    // `--list` 是纯查询,不碰仓库,也不需要分析
    if cli.list {
        return list_skill_targets();
    }

    let root = match dunce::canonicalize(&cli.common.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot access {}: {e}", cli.common.path.display());
            return exit::NOT_A_REPO;
        }
    };

    let needs_analysis = stages
        .iter()
        .any(|s| matches!(s, Stage::Check | Stage::Polish | Stage::Artifacts));

    // **分析只做一次。** 四个阶段共用同一份 ctx 与 report:分开跑意味着多打
    // 几次 GitHub API,也意味着几份产物有可能来自不同的评分结果。
    let mut analysis = if needs_analysis {
        // star 曲线只画在概览卡上。这次到底会不会画那张卡,取决于视觉产物的
        // 开关和文件在不在——两处在这里就算得出来,而 analyze 必须在阶段跑
        // 起来之前就知道该不该多花那十几次请求。Action 的 `overview` 默认
        // 是 false,那正是最常见的一种运行。
        let cfg = crate::config::load(cli.common.config.as_deref(), &root)
            .map(|c| c.readme)
            .unwrap_or_default();
        let v = visuals(&cli, &cfg);
        let wanted = StarsWanted {
            overview: (stages.contains(&Stage::Polish) && v.overview)
                || (stages.contains(&Stage::Artifacts)
                    && (cli.artifact.contains(&Artifact::Overview)
                        || root.join(repolish_render::OVERVIEW_PATH).exists())),
        };
        match analyze(&cli.common, wanted) {
            Ok(a) => Some(a),
            Err(code) => return code,
        }
    } else {
        None
    };

    let mut ledger = Ledger::new(cli.apply);
    let mut gaps = analysis
        .as_mut()
        .map(|a| std::mem::take(&mut a.gaps))
        .unwrap_or_default();
    // 分数门禁留到最后:分数不达标时产物照样要写出来,否则 CI 上一次未达标
    // 的运行会连徽章都拿不到。
    let mut gate = exit::OK;

    for stage in &stages {
        let code = match stage {
            Stage::Check => {
                let a = analysis.as_mut().expect("check needs an analysis");
                let c = stage_check(&cli, a);
                if c == exit::OK {
                    gate = verdict(&a.report, cli.min_score.or(a.min_score));
                }
                c
            }
            Stage::Polish => stage_polish(
                &cli,
                analysis.as_ref().expect("polish needs an analysis"),
                &mut ledger,
            ),
            Stage::Artifacts => stage_artifacts(
                &cli,
                analysis.as_ref().expect("artifacts needs an analysis"),
                &mut ledger,
            ),
            Stage::Ci => stage_ci(&cli, &root, &mut ledger),
            Stage::Skill => stage_skill(&cli, &root, &mut ledger),
            Stage::Demo => stage_demo(&cli, &root, &mut ledger, &mut gaps),
        };
        // 工具自身失败就地中止。继续往下跑只会在一个已知坏掉的前提上
        // 堆更多产物。
        if code != exit::OK {
            return code;
        }
    }

    report_ledger(&cli, &ledger);
    report_untouched(&cli, &stages, &root);
    report_gaps(&cli, &gaps);
    gate
}

/// 跑完了，但有几件事因为缺东西而没做成。
///
/// 这些不是错误——流水线照常走完了。但它们散落在过程里各说一句，读的人滚上去
/// 就看不见了；而每一条都恰好是「补一个输入就能解决」的那种。所以攒到最后
/// 一起报，并且每条都带上把它补齐的那条命令。
fn report_gaps(cli: &Cli, gaps: &Gaps) {
    if gaps.is_empty() {
        return;
    }
    say!(cli, "");
    say!(
        cli,
        "  NEEDS INPUT — these were skipped for want of something"
    );
    for g in gaps.iter() {
        say!(cli, "    · {}", g.what);
        say!(cli, "      {}", g.fix);
    }
}

/// 默认流水线跑完之后，说一句还有哪两段没跑。
///
/// `demo` 和 `skill` 刻意不在默认里，但「刻意」这件事只有我们知道——使用者
/// 看到的是一份没有录屏、没有 SKILL.md 的产出，而输出里一个字都没提它们
/// 存在过。两次被问「其他功能呢」之后，这段就是答案。
///
/// **产物已经在了就不再提。** 每次运行都催一遍已经做过的事，是噪音，
/// 而噪音会让人连有用的那几行一起略过。
fn report_untouched(cli: &Cli, stages: &[Stage], root: &Path) {
    let missing = |s: Stage, path: &str| !stages.contains(&s) && !root.join(path).exists();
    let demo = missing(Stage::Demo, demo::SVG_PATH);
    let skill = missing(Stage::Skill, skill::SKILL_PATH);
    if !demo && !skill {
        return;
    }

    let inv = invocation();
    say!(cli, "");
    say!(cli, "  NOT RUN — these stages are opt-in");
    if demo {
        say!(
            cli,
            "    demo    record this CLI as an animated SVG   {inv} --stages demo"
        );
    }
    if skill {
        say!(
            cli,
            "    skill   teach a coding agent to drive this   {inv} --stages skill"
        );
    }
    if demo {
        say!(
            cli,
            "\n  `demo` RUNS the commands it records, so it prints the list first and \
             only records under --apply."
        );
    }
}

/// 统一汇报「会写 / 写了」哪些文件。
fn report_ledger(cli: &Cli, ledger: &Ledger) {
    if ledger.is_empty() {
        return;
    }
    let n = ledger.entries.len();
    let width = ledger
        .entries
        .iter()
        .map(|e| e.rel.chars().count())
        .max()
        .unwrap_or(0)
        .min(44);

    say!(cli, "");
    say!(
        cli,
        "  {} ({} file{})",
        if ledger.apply { "WROTE" } else { "WOULD WRITE" },
        n,
        if n == 1 { "" } else { "s" }
    );
    for e in &ledger.entries {
        say!(cli, "    {:<width$}  {}", e.rel, e.note, width = width);
    }

    if !ledger.apply {
        say!(cli, "");
        say!(
            cli,
            "  Nothing was written. Apply with: {} --apply",
            invocation()
        );
        return;
    }
    // `git diff` 看不到未跟踪的新文件。照着那句话去检查，会以为它只改了
    // README —— 而它刚往仓库里放了四个文件。
    say!(cli, "");
    say!(
        cli,
        "  Review with `git add -A && git diff --staged` (plain `git diff` hides new files).\n  \
         Undo with `git checkout -- . && git clean -fd`"
    );
}

// ── 各阶段 ──────────────────────────────────────────────────────────────

fn stage_check(cli: &Cli, a: &mut Analysis) -> u8 {
    // 差值必须在渲染之前算出来：四种格式都要能看到它,分开算等于四份代码
    if let Some(base_ref) = &cli.base {
        match base::compare(&a.ctx.root, base_ref, &a.ctx, &a.report, &a.opts) {
            Ok(b) => a.report.delta = Some(b.delta),
            Err(e) => {
                eprintln!("error: {e}");
                // 基线取不到不是质量回归。报成 1 的话,CI 上分不出
                // 「这个 PR 变差了」和「浅克隆里没有那个 commit」
                return exit::BASE_FAILED;
            }
        }
    }

    match cli.format {
        Format::Text => print!(
            "{}",
            repolish_render::terminal(
                &a.report,
                &RenderOptions {
                    verbose: cli.verbose,
                    level: cli.common.level(),
                }
            )
        ),
        Format::Json => match serde_json::to_string_pretty(&a.report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: serialization failed: {e}");
                return exit::BAD_USAGE;
            }
        },
        Format::Markdown => print!("{}", repolish_render::markdown(&a.report)),
        Format::Sarif => print!("{}", sarif::sarif(&a.report)),
        Format::Comment => print!("{}", repolish_render::comment(&a.report)),
    }

    // `--sarif` / `--comment` 不受 `--apply` 约束:使用者明确点了名的输出
    // 路径,那本身就是请求,不是对仓库的改动。
    if let Some(path) = &cli.sarif {
        if let Err(code) = write_file(path, &sarif::sarif(&a.report)) {
            return code;
        }
        eprintln!("wrote {}", path.display());
    }
    if let Some(path) = &cli.comment {
        if let Err(code) = write_file(path, &repolish_render::comment(&a.report)) {
            return code;
        }
        eprintln!("wrote {}", path.display());
    }
    exit::OK
}

/// README 里那三样视觉产物开不开。
///
/// **默认全开。** `polish` 和 `artifacts` 必须算出同一个答案——前者决定往
/// README 里插什么引用,后者决定画哪几张图,两边不一致就会留下一个指向
/// 空文件的 `<img>`,或者一张没人引用的孤儿图。所以这里只有一份实现。
///
/// 优先级:单项命令行开关 > `--no-visuals` > 配置文件 > 默认(开)。
/// 单项开关排在 `--no-visuals` 前面是有意的——同时给了这两个,说的是
/// 「除了这一样,其余都别动」。
struct Visuals {
    hero: bool,
    overview: bool,
    footer_card: bool,
    tables: style::TableStyle,
}

fn visuals(cli: &Cli, cfg: &crate::config::Readme) -> Visuals {
    let on = !cli.no_visuals;
    Visuals {
        hero: cli.hero || (on && cfg.hero.unwrap_or(true)),
        overview: cli.overview || (on && cfg.overview.unwrap_or(true)),
        footer_card: cli.footer_card || (on && cfg.footer_card.unwrap_or(true)),
        tables: cli.tables.or(cfg.tables).unwrap_or(if on {
            style::TableStyle::Svg
        } else {
            style::TableStyle::Keep
        }),
    }
}

/// 这个阶段是不是被单独点名跑的。
///
/// 跑完整流水线时,`polish` 已经把徽章和图的引用插进 README 了,再printing
/// 一遍「把这段贴进 README」纯属噪音。单独跑某一段时正相反——那时使用者
/// 拿到的是一个文件和零条线索。
fn only_stage(cli: &Cli, s: Stage) -> bool {
    cli.stages == [s]
}

fn stage_polish(cli: &Cli, a: &Analysis, ledger: &mut Ledger) -> u8 {
    let ctx = &a.ctx;

    // 命令行 > 配置文件 > 默认；徽章样式没给时跟着 README 里已有的走
    let cfg = match crate::config::load(cli.common.config.as_deref(), &ctx.root) {
        Ok(c) => c.readme,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    let readme_raw = ctx.readme.as_ref().map(|r| r.raw.as_str()).unwrap_or("");
    let v = visuals(cli, &cfg);
    let style = style::ReadmeStyle {
        badge: cli
            .badge_style
            .or(cfg.badge_style)
            .or_else(|| style::BadgeStyle::detect(readme_raw))
            .unwrap_or_default(),
        align: cli.align.or(cfg.align).unwrap_or_default(),
        toc: cli.toc_style.or(cfg.toc_style).unwrap_or_default(),
        logo: cli.logo.clone().or(cfg.logo),
        logo_width: cli.logo_width.or(cfg.logo_width),
        tree_depth: cli.tree_depth.or(cfg.tree_depth),
        theme: cli.common.theme.or(cfg.theme).unwrap_or_default(),
        lang: cli
            .common
            .lang
            .or(cfg.lang)
            .unwrap_or_default()
            .resolve(readme_raw),
        hero: v.hero,
        overview: v.overview,
        footer_card: v.footer_card,
        tables: v.tables,
    };

    let plan = polish::plan(ctx, &a.report, &style);

    if plan.is_empty() {
        say!(
            cli,
            "\n  Nothing to fix mechanically — every finding that is left needs a human."
        );
        return run_suggest(cli.suggest, &cli.common, ctx, &a.report);
    }

    // 落盘前拒绝:没有 git 就没有撤销键。改的是别人的 README，在一个连
    // `git checkout` 都用不了的目录里默默改文件是不能接受的。
    if cli.apply && ctx.git.is_none() && !cli.force {
        eprintln!(
            "error: {} is not a git repository, so there is no way to undo this.\n\
             Re-run with --force if you have another way to recover the file",
            ctx.root.display()
        );
        return exit::BAD_USAGE;
    }

    say!(cli, "");
    if let Some(readme) = ctx.readme.as_ref() {
        if !plan.inserts.is_empty() {
            say!(cli, "  {}", relative(&ctx.root, &readme.path));
            for insert in &plan.inserts {
                for line in insert.lines.iter().filter(|l| !l.is_empty()) {
                    say!(cli, "    + {}", line);
                }
                say!(cli, "      {}", insert.reason);
            }
            say!(cli, "");
            let out = polish::polished(readme, &plan);
            let n = plan.inserts.iter().map(|i| i.lines.len()).sum::<usize>();
            if let Err(code) = ledger.write(&ctx.root, &readme.path, &out, format!("+{n} lines")) {
                return code;
            }
        }
    }
    // 译本和主 README 一样，只增量插入——切开原文拼回去，其余字节不碰
    for edit in &plan.translations {
        say!(cli, "  {}", relative(&ctx.root, &edit.path));
        for insert in &edit.inserts {
            for line in insert.lines.iter().filter(|l| !l.is_empty()) {
                say!(cli, "    + {line}");
            }
            say!(cli, "      {}", insert.reason);
        }
        say!(cli, "");
        let out = repolish_md::edit::apply(&edit.raw, &edit.inserts);
        let n = edit.inserts.iter().map(|i| i.lines.len()).sum::<usize>();
        if let Err(code) = ledger.write(&ctx.root, &edit.path, &out, format!("+{n} lines")) {
            return code;
        }
    }
    for f in &plan.side_files {
        say!(
            cli,
            "  {}  ({} lines, new file)",
            relative(&ctx.root, &f.path),
            f.contents.lines().count()
        );
        say!(cli, "      {}", f.reason);
        // README 的每一行插入都看得见，整个新文件却只报个路径，是说不过去的：
        // 落进别人仓库的东西，落盘前该能看全。
        if cli.verbose {
            for line in f.contents.lines() {
                say!(cli, "      | {line}");
            }
        }
        say!(cli, "");
        if let Err(code) = ledger.write(&ctx.root, &f.path, &f.contents, "new file") {
            return code;
        }
    }
    if !plan.side_files.is_empty() && !cli.verbose {
        say!(
            cli,
            "  Run with -v to print what each new file would contain."
        );
    }

    run_suggest(cli.suggest, &cli.common, ctx, &a.report)
}

/// 重画 README **已经引用**的那些图,外加徽章 JSON。
///
/// 判据是「已经被引用」而不是「用户点了名」:给一张没人引用的表生成 SVG,
/// 落下的是一个孤儿文件——它会被提交、被一直带着,而没有任何东西指向它。
/// `polish` 负责第一次把引用插进去,这一段负责此后每一次重画。
fn stage_artifacts(cli: &Cli, a: &Analysis, ledger: &mut Ledger) -> u8 {
    let ctx = &a.ctx;
    let cfg = match crate::config::load(cli.common.config.as_deref(), &ctx.root) {
        Ok(c) => c.readme,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    let opts = cli.common.card_options(ctx, &cfg);

    // 点名了就只做点到的；没点名就是「徽章 + README 已经引用的那些」。
    let named = !cli.artifact.is_empty();
    let want = |k: Artifact| {
        if named {
            cli.artifact.contains(&k)
        } else {
            match k {
                Artifact::Badge => !cli.no_badge,
                Artifact::Report => cli.report,
                // **只重画已经在那儿的。** 第一次是 `polish` 的活:它插入引用，
                // 并同时把文件生成出来。这里再按「视觉产物默认开」去画一遍，
                // 落下的是一个没有任何东西指向的孤儿文件——本仓库顶上挂的是
                // 作者自己的 `assets/hero.svg`，`polish` 会正确让位，而这里
                // 却照画不误。要强行画某一张，用 `--artifact`。
                Artifact::Hero => ctx.root.join(repolish_render::HERO_PATH).exists(),
                Artifact::Overview => ctx.root.join(repolish_render::OVERVIEW_PATH).exists(),
                Artifact::Score => ctx.root.join(repolish_render::CARD_PATH).exists(),
                Artifact::Tables => true,
            }
        }
    };
    // `--output` / `--stdout` 只在恰好一件产物时说得清
    if (cli.output.is_some() || cli.stdout) && cli.artifact.len() != 1 {
        eprintln!(
            "error: --output and --stdout each handle a single artifact; name one with \
             --artifact (badge, report, overview, score or tables)"
        );
        return exit::BAD_USAGE;
    }

    // 徽章 JSON。覆盖率过低时不写——一个由三分之一证据撑起来的徽章
    // 比没有徽章更糟。
    match if want(Artifact::Badge) {
        repolish_render::badge_json(&a.report)
    } else {
        None
    } {
        Some(json) => {
            if cli.stdout {
                print!("{json}");
                return exit::OK;
            }
            let path = cli
                .output
                .clone()
                .unwrap_or_else(|| ctx.root.join(repolish_render::BADGE_PATH));
            if let Err(code) = ledger.write(&ctx.root, &path, &json, "score badge") {
                return code;
            }
            if only_stage(cli, Stage::Artifacts) && !named {
                let branch = cli
                    .branch
                    .clone()
                    .or_else(|| ctx.git.as_ref().and_then(|g| g.branch.clone()))
                    .unwrap_or_else(|| "main".to_string());
                let (owner, name) = match &ctx.slug {
                    Some(sl) => (sl.owner.as_str(), sl.name.as_str()),
                    None => {
                        eprintln!(
                            "warning: no GitHub remote found — fill in OWNER / REPO yourself"
                        );
                        ("OWNER", "REPO")
                    }
                };
                say!(cli, "\n  Paste this into your README:\n");
                say!(
                    cli,
                    "    {}",
                    repolish_render::snippet(owner, name, &branch)
                );
                say!(
                    cli,
                    "\n  shields.io renders it by reading {} out of your own repository. \
                     Nothing is hosted by us.",
                    repolish_render::BADGE_PATH
                );
            }
        }
        None if !want(Artifact::Badge) => {}
        None => {
            eprintln!(
                "warning: only {:.0}% of the registered checks produced a score, below the \
                 50% floor, so no badge was written",
                a.report.coverage * 100.0
            );
        }
    }

    if want(Artifact::Report) {
        let md = repolish_render::markdown(&a.report);
        if cli.stdout {
            print!("{md}");
            return exit::OK;
        }
        let path = cli
            .output
            .clone()
            .unwrap_or_else(|| ctx.root.join("REPOLISH.md"));
        if let Err(code) = ledger.write(&ctx.root, &path, &md, "full report") {
            return code;
        }
    }

    if want(Artifact::Hero) {
        let facts = repolish_render::Facts::from_ctx(ctx, opts.lang);
        let tagline = facts.description.clone().unwrap_or_default();
        let svg = repolish_render::svg::hero(&facts.name, &tagline, opts.lang);
        if cli.stdout {
            print!("{svg}");
            return exit::OK;
        }
        let path = cli
            .output
            .clone()
            .unwrap_or_else(|| ctx.root.join(repolish_render::HERO_PATH));
        if let Err(code) = ledger.write(&ctx.root, &path, &svg, "banner") {
            return code;
        }
    }

    if want(Artifact::Overview) {
        let facts = repolish_render::Facts::from_ctx(ctx, opts.lang);
        let svg = repolish_render::overview(&facts, &opts);
        // 正要用一张没有曲线的卡覆盖一张有曲线的。这不是错误——本地不带
        // token 重画是很正常的事——但静默覆盖就意味着提交进仓库的卡片会
        // 悄悄退化,而 diff 里只是一大片 SVG 变化,没人看得出少了什么。
        if !repolish_render::has_star_history(&svg) && !cli.common.no_stars {
            let path = ctx.root.join(repolish_render::OVERVIEW_PATH);
            if std::fs::read_to_string(&path)
                .map(|old| repolish_render::has_star_history(&old))
                .unwrap_or(false)
            {
                eprintln!(
                    "warning: {} has a star history curve and this run has none — \
                     it needs --remote --stars. Re-run with both to keep it.",
                    repolish_render::OVERVIEW_PATH
                );
            }
        }
        if cli.stdout {
            print!("{svg}");
            return exit::OK;
        }
        let path = cli
            .output
            .clone()
            .unwrap_or_else(|| ctx.root.join(repolish_render::OVERVIEW_PATH));
        if let Err(code) = ledger.write(&ctx.root, &path, &svg, "overview card") {
            return code;
        }
    }
    if want(Artifact::Score) {
        let svg = repolish_render::card(&a.report, &opts);
        if cli.stdout {
            print!("{svg}");
            return exit::OK;
        }
        let path = cli
            .output
            .clone()
            .unwrap_or_else(|| ctx.root.join(repolish_render::CARD_PATH));
        if let Err(code) = ledger.write(&ctx.root, &path, &svg, "report card") {
            return code;
        }
    }

    if !want(Artifact::Tables) {
        return exit::OK;
    }

    // 表格:主 README 加上每一份译本。译本要是漏掉，它们的表格图就再也没有
    // 重画的途径——polish 从不覆盖，这一段是唯一会重写的那条路。
    let Some(readme) = ctx.readme.as_ref() else {
        return exit::OK;
    };
    let mut sheets: Vec<(Readme, repolish_render::Options)> = vec![(readme.clone(), opts)];
    for path in tables::translations(ctx, readme) {
        let Some(raw) = ctx.files.read(&path) else {
            continue;
        };
        let translated = Readme::parse(&path, raw);
        // 每一份都用它自己的语言画，不是主 README 的语言
        let lang = repolish_render::Lang::detect(&translated.raw);
        sheets.push((translated, repolish_render::Options { lang, ..opts }));
    }
    for (sheet, sheet_opts) in &sheets {
        for table in tables::render(sheet, sheet_opts, |w| eprintln!("note: {w}")) {
            if !tables::already_wrapped(sheet, table.start_line) {
                continue;
            }
            let path = table.path(&ctx.root);
            if let Err(code) = ledger.write(&ctx.root, &path, &table.svg, "table") {
                return code;
            }
        }
    }
    exit::OK
}

fn stage_ci(cli: &Cli, root: &Path, ledger: &mut Ledger) -> u8 {
    let path = root.join(init::WORKFLOW_PATH);
    // 已经有一份就别动它。流水线里跑到这一步时,使用者要的是「把仓库
    // 补齐」,不是「把我改过的 workflow 覆盖掉」。
    if path.exists() && !cli.force {
        say!(
            cli,
            "\n  {} already exists — left alone. Pass --force to regenerate it.",
            init::WORKFLOW_PATH
        );
        return exit::OK;
    }

    // 分支名要与仓库实际的默认分支一致，否则 workflow 永远不会被 push 触发
    let branch = repolish_ingest::RepoContext::load(root, None)
        .ok()
        .and_then(|c| c.git.and_then(|g| g.branch))
        .unwrap_or_else(|| "main".to_string());

    let min_score = if cli.no_gate { None } else { cli.min_score };
    let note = match min_score {
        Some(n) => format!("CI workflow, gate at {n}"),
        None => "CI workflow, score recorded not enforced".to_string(),
    };
    if let Err(code) = ledger.write(root, &path, &init::workflow(&branch, min_score), note) {
        return code;
    }
    say!(cli, "\n  CI workflow triggers on: {branch}");
    say!(
        cli,
        "  The template pins asale-ai/repolish@v{}, which has to be released before it can run.",
        env!("CARGO_PKG_VERSION")
    );
    exit::OK
}

fn stage_skill(cli: &Cli, root: &Path, ledger: &mut Ledger) -> u8 {
    let md = skill::markdown();
    if cli.stdout {
        print!("{md}");
        return exit::OK;
    }
    // `--target` 装的是这台机器上的智能体目录,在仓库之外,所以它不进账本:
    // 账本讲的是「这个仓库会变成什么样」。
    if !cli.target.is_empty() {
        if !cli.apply {
            say!(
                cli,
                "\n  Would install the agent skill into: {}\n  Apply with: {} --stages skill --target {} --apply",
                cli.target.join(", "),
                invocation(),
                cli.target.join(",")
            );
            return exit::OK;
        }
        return install_skill(&cli.target, &md, cli.force);
    }

    // 没给 --target 就是写进一个仓库：技能跟着代码走，谁 clone 谁就有
    let path = cli
        .output
        .clone()
        .unwrap_or_else(|| root.join(skill::SKILL_PATH));
    if path.exists() && !cli.force {
        say!(
            cli,
            "\n  {} already exists — left alone. Pass --force to regenerate it.",
            relative(root, &path)
        );
        return exit::OK;
    }
    if let Err(code) = ledger.write(root, &path, &md, "agent skill") {
        return code;
    }
    say!(
        cli,
        "\n  SKILL.md teaches an agent to measure before it edits, and never to rewrite a \
         README wholesale.\n  To install it for every project instead: {} --stages skill \
         --target detect --apply",
        invocation()
    );
    exit::OK
}

/// **这一段会真的执行 README 里的命令**,所以没有 `--apply` 时它只列清单。
fn stage_demo(cli: &Cli, root: &Path, ledger: &mut Ledger, gaps: &mut Gaps) -> u8 {
    let ctx = match repolish_ingest::RepoContext::load(root, None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            return exit::NOT_A_REPO;
        }
    };

    let commands = if cli.commands.is_empty() {
        // 录一段跑不起来的命令比没有录屏更糟。认不出可执行文件就明说，
        // 并告诉他怎么手动指定——而不是拿仓库名去赌。
        // 认不出可执行文件不是错误——大多数仓库根本不是 CLI。记一笔就走，
        // 中止整条流水线会让前面四段的产出一起作废。
        let Some(bin) = demo::binary(&ctx) else {
            gaps.note(
                "no terminal recording — no command-line binary was detected here",
                format!(
                    "if this project has a CLI, name the commands: \
                     {} --stages demo --cmd \"yourtool --help\" --apply",
                    invocation()
                ),
            );
            return exit::OK;
        };
        demo::default_commands(&bin)
    } else {
        cli.commands.clone()
    };

    // 执行别人机器上的程序这件事，必须让使用者看得见——干跑时更是唯一的输出
    if !cli.apply {
        say!(
            cli,
            "\n  Would RUN these in {}, and record them:\n",
            root.display()
        );
        for c in &commands {
            say!(cli, "    $ {c}");
        }
        say!(
            cli,
            "\n  Nothing was run. Recording executes them for real: {} --stages demo --apply",
            invocation()
        );
        return exit::OK;
    }

    say!(cli, "Recording in {}:", root.display());
    let recording = match record::run(&commands, root, |c| eprintln!("  $ {c}")) {
        Ok(r) => r,
        Err(e) => {
            // 命令跑不起来同样不该中止:录屏是产出里最不关键的一件,
            // 而它失败的常见原因只是「这个项目还没构建」。
            eprintln!("warning: the recording could not run: {e}");
            let inv = invocation();
            let fix = match demo::how_to_build(&ctx) {
                // 两条可以直接粘贴的命令。第二条把构建目录塞进这一次调用的
                // PATH 里,不动使用者的 shell 配置——录一次屏不该要求他改
                // 自己的环境。
                Some((build, dir)) => format!(
                    "build it, then put the build directory on PATH for one run:\n      \
                     {build}\n      PATH=\"$PWD/{dir}:$PATH\" {inv} --stages demo --apply"
                ),
                None => format!(
                    "`{}` has to be on PATH. Build the project, then:\n      \
                     PATH=\"$PWD/<build-dir>:$PATH\" {inv} --stages demo --apply",
                    commands
                        .first()
                        .map(|c| c.split_whitespace().next().unwrap_or("the binary"))
                        .unwrap_or("the binary")
                ),
            };
            gaps.note(
                format!(
                    "no terminal recording — `{}` is not on PATH",
                    commands
                        .first()
                        .map(|c| c.split_whitespace().next().unwrap_or("the binary"))
                        .unwrap_or("the binary")
                ),
                fix,
            );
            return exit::OK;
        }
    };

    // 失败的命令照录——一条报错也是真实输出——但绝不能不声不响：
    // 一段悄悄录进了错误的演示，比没有演示伤得更久
    for (cmd, code) in &recording.failures {
        eprintln!("warning: `{cmd}` exited with {code}; its output is in the recording as-is");
    }

    let cfg = match crate::config::load(cli.common.config.as_deref(), &ctx.root) {
        Ok(c) => c.readme,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::BAD_USAGE;
        }
    };
    let opts = cli.common.card_options(&ctx, &cfg);
    let timing = repolish_render::Timing {
        type_ms: cli.type_ms,
        ..Default::default()
    };
    let svg = repolish_render::cast(&recording.steps, &timing, &opts);

    if cli.stdout {
        print!("{svg}");
        return exit::OK;
    }

    let path = cli
        .output
        .clone()
        .unwrap_or_else(|| root.join(demo::SVG_PATH));
    if let Err(code) = ledger.write(root, &path, &svg, "terminal recording") {
        return code;
    }
    if only_stage(cli, Stage::Demo) {
        say!(cli, "\n  Paste this into your README:\n");
        say!(
            cli,
            "    {}",
            demo::snippet(&relative(root, &path), "terminal recording")
        );
        say!(
            cli,
            "\n  It is a plain SVG: no fonts, no scripts, nothing hosted by us, and the \
             commands in it are real text you can select and copy."
        );
    }
    if cli.tape {
        let bin = demo::binary(&ctx).unwrap_or_else(|| "your-tool".into());
        let tape_path = root.join(demo::TAPE_PATH);
        let tape = demo::tape(&bin, &commands, demo::GIF_PATH);
        if let Err(code) = ledger.write(root, &tape_path, &tape, "VHS tape") {
            return code;
        }
    }
    exit::OK
}

// ── 共用 ────────────────────────────────────────────────────────────────

/// 仓库相对路径，分隔符统一成 `/`——打印出来的路径要能直接贴进 README
fn relative(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace(std::path::MAIN_SEPARATOR, "/")
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
    let inv = invocation();
    println!("\n  {inv} skill --target detect     install into the ones marked above");
    println!("  {inv} skill --target all        install into every one of them");
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
        let inv = invocation();
        println!("Run `{inv} skill --list` to see what is supported, or name one:");
        println!("    {inv} skill --target claude");
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
        "\nThe skill calls `repolish` by name. Check it is reachable with:\n    \
         repolish --version\n\
         If it is not on PATH, the skill falls back to `npx -y @asale/repolish`, \
         which needs nothing installed."
    );
    exit::OK
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
