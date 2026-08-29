//! `.repolish/overview.svg` —— **被检查的那个项目**的概览卡片。
//!
//! 和分数卡片（[`crate::svg`]）分工明确，别搞混：
//!
//! - **概览卡片**说的是「这个项目是什么」：语言构成、文件用途、提交活跃度、
//!   许可证、最近一次提交。它贴在 README 顶上，给**第一次点进来的人**看。
//! - **分数卡片**说的是「这个项目的门面打了多少分」。那是给**作者**看的
//!   诊断结果，位置在 README 末尾的「用 repolish 打磨过」一节里。
//!
//! 把分数卡片贴在顶上是早期的一个错误：一个陌生人点进你的仓库，第一眼
//! 看到的应该是你的项目，不是我们的工具给你打了几分。
//!
//! 三条硬约束与分数卡片一致：自包含、确定性、深浅两套色板各自恒定。
//! 数据全部来自本地 —— `--remote` 只多出星标与主题两行。

use std::fmt::Write as _;

use repolish_core::RepoContext;
use repolish_ingest::lang::{self, Kind, LangStat};

use crate::draw::{self, Anchor, Options};
use crate::i18n::{Lang, Strings};
use crate::theme::{Palette, Rgb};

/// 概览卡片在用户仓库中的位置
pub const OVERVIEW_PATH: &str = ".repolish/overview.svg";

const W: i32 = 880;
const PAD: i32 = 36;
const RIGHT: i32 = W - PAD;
const INNER: i32 = W - PAD * 2;
/// 语言条最多列几行，其余并成「+N others」。
/// 一张列了 14 门语言的卡片没有人会读完。
const MAX_LANGS: usize = 5;

/// star 曲线在 SVG 里的标记。见 [`has_star_history`]。
pub const STAR_MARK: &str = "  <!--star-history-->";

/// 这张已经画好的概览卡里有没有 star 曲线。
///
/// 曲线要 `--remote --stars` 才有,而那是显式开关。不带这两个参数重新生成
/// 一次,曲线就被静默洗掉——提交进仓库的卡片会因此悄悄退化,而 diff 里
/// 只是一大片 SVG 变化,没人看得出少了什么。
pub fn has_star_history(svg: &str) -> bool {
    svg.contains(STAR_MARK)
}

/// 画卡片要用到的全部事实，一次性从 [`RepoContext`] 里取齐。
///
/// 单独立一个结构体，是为了让渲染函数**不碰文件系统也不碰 git**：
/// 卡片的可复现性就靠这条——同一组事实必然画出同一张图。
pub struct Facts {
    pub name: String,
    pub owner: Option<String>,
    pub description: Option<String>,
    pub profile: &'static str,
    pub langs: Vec<LangStat>,
    pub total_files: usize,
    pub commits: usize,
    pub commits_truncated: bool,
    pub activity: Vec<u32>,
    pub shallow: bool,
    pub tags: usize,
    pub latest_tag: Option<String>,
    pub days_since_commit: Option<i64>,
    pub license: Option<String>,
    pub stars: Option<u64>,
    /// star 增长曲线，按时间升序。空 = 没取或取不到。
    pub star_history: Vec<repolish_ingest::StarPoint>,
    pub topics: Vec<String>,
    pub ecosystem: Option<String>,
    pub branch: Option<String>,
}

impl Facts {
    /// `lang` 是卡片要用的语言。它影响的不只是标签——**简介那一行也得跟着走**。
    ///
    /// 一份中英双语的仓库里，`ctx.readme` 是主 README（通常英文），而 GitHub
    /// 的 description 也是英文。照直取的话，中文卡片上标签全是中文、简介却是
    /// 一句英文，看着像翻译漏了一半。所以先在译本里找同语言的那一份。
    pub fn from_ctx(ctx: &RepoContext, lang: Lang) -> Facts {
        let translated = translated_tagline(ctx, lang);
        let langs = lang::stats(&ctx.files);
        let git = ctx.git.as_ref();

        // 许可证优先信远端：SPDX 标识符是 GitHub 认过的，
        // 本地只能看到「有一个 LICENSE 文件」，说不出是哪一份
        let license = ctx
            .remote
            .as_ref()
            .and_then(|r| r.license.clone())
            .or_else(|| {
                ctx.files
                    .find_at_root(&["LICENSE", "LICENSE.md", "COPYING"])
                    .map(|_| "yes".into())
            });

        Facts {
            name: ctx.display_name(),
            owner: ctx.slug.as_ref().map(|s| s.owner.clone()),
            // 同语言译本的标语优先。它是作者用这个语言亲手写的一句话，
            // 比 GitHub 上那条（只有一种语言的）仓库简介更贴。
            description: translated
                .or_else(|| ctx.remote.as_ref().and_then(|r| r.description.clone()))
                .or_else(|| ctx.readme.as_ref().and_then(|r| r.tagline.clone())),
            profile: ctx.profile.as_str(),
            total_files: langs.iter().map(|l| l.files).sum(),
            langs,
            commits: git.map_or(0, |g| g.commits_seen),
            commits_truncated: git.is_some_and(|g| g.commits_truncated),
            activity: git.map(|g| g.activity.clone()).unwrap_or_default(),
            shallow: git.is_some_and(|g| g.shallow),
            tags: git.map_or(0, |g| g.tags.len()),
            latest_tag: git.and_then(latest_semver_tag),
            days_since_commit: git.map(|g| g.days_since_head()),
            license,
            stars: ctx.remote.as_ref().map(|r| r.stars),
            star_history: ctx
                .remote
                .as_ref()
                .map(|r| r.star_history.clone())
                .unwrap_or_default(),
            topics: ctx
                .remote
                .as_ref()
                .map(|r| r.topics.clone())
                .unwrap_or_default(),
            ecosystem: ctx
                .manifests
                .first()
                .map(|m| m.ecosystem.as_str().to_string()),
            branch: git.and_then(|g| g.branch.clone()),
        }
    }
}

/// 在译本里找出用 `lang` 写的那一份 README，取它的标语。
///
/// 命名约定与 `readme-i18n` 检查项一致：`README.zh-CN.md`、`README_zh.md`、
/// `docs/README-zh-hans.md` 都算。主 README 本身不算译本——它已经是
/// 兜底的下一档了。
fn translated_tagline(ctx: &RepoContext, lang: Lang) -> Option<String> {
    let main = ctx
        .readme
        .as_ref()
        .map(|r| r.path.display().to_string().replace('\\', "/"))
        .unwrap_or_default();

    for path in ctx.files.iter() {
        if path == main || !path.to_lowercase().contains("readme") {
            continue;
        }
        let Some(code) = repolish_md::translation_code(path) else {
            continue;
        };
        if !lang.matches_code(&code) {
            continue;
        }
        let Some(raw) = ctx.files.read(path) else {
            continue;
        };
        if let Some(tagline) = repolish_md::Readme::parse(path, raw).tagline {
            return Some(tagline);
        }
    }
    None
}

/// 最新的语义化版本 tag。
///
/// 按版本号的数值比大小，不按字符串：字符串序里 `v0.10.0` 排在 `v0.9.0`
/// 前面，卡片上就会写着一个早就发布过的版本。
fn latest_semver_tag(git: &repolish_ingest::GitFacts) -> Option<String> {
    git.semver_tags()
        .max_by_key(|t| {
            let s = t.name.strip_prefix('v').unwrap_or(&t.name);
            let head = s.split(['-', '+']).next().unwrap_or(s);
            let mut parts = head.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
            (
                parts.next().unwrap_or(0),
                parts.next().unwrap_or(0),
                parts.next().unwrap_or(0),
            )
        })
        .map(|t| t.name.clone())
}

pub fn overview(facts: &Facts, opts: &Options) -> String {
    let p = opts.palette;
    let s = opts.lang.strings();
    let mut body = String::new();
    let mut y = PAD;

    y = header(&mut body, facts, p, s, y);
    y = headline(&mut body, facts, p, s, y + 26);
    y = tally(&mut body, facts, p, s, y + 26);
    y = languages(&mut body, facts, p, s, y + 30);
    y = composition(&mut body, facts, p, s, y + 26);
    y = activity(&mut body, facts, p, s, y + 28);
    y = star_history(&mut body, facts, p, s, y + 28);
    let height = footer(&mut body, facts, p, s, y + 22);

    let aria = match &facts.owner {
        Some(o) => format!("{o}/{} — repository overview", facts.name),
        None => format!("{} — repository overview", facts.name),
    };
    draw::document(&body, W, height, p, opts.lang.tag(), &aria)
}

// ── 页眉 ────────────────────────────────────────────────────

/// 抬头上的字标：**这个项目的名字**，不是我们的。
///
/// 这张卡贴在别人的 README 顶上，第一眼该看到的是他们的项目。我们的署名在
/// 页脚那一行就够了。
///
/// 点阵字体只有 `A-Z0-9.-`，装不下的名字退回普通文字：
///
/// - 非拉丁名（中文、日文）在点阵下会渲染成**一片空白**——空白抬头比朴素
///   字体糟得多，所以这里宁可换字体也不能出空白。
/// - 太长的名字会一路压到右边的 PROFILE 上，所以先算宽度再决定。
///
/// `_` 先换成 `-`：仓库名里下划线很常见，而它恰好是这套字体没有的那几个
/// 字符之一，换成连字符比开个空洞好看。
fn wordmark(name: &str, x: i32, y: i32, size: i32, p: &Palette) -> String {
    const CELL: i32 = 3;
    /// 字标右侧要给 PROFILE 那一列留出的空间
    const RESERVED: i32 = 140;

    if let Some(blocks) = draw::as_blocks(name, CELL, RIGHT - RESERVED - x) {
        return draw::blocks(&blocks, x, y + 1, CELL, p);
    }
    // 兜底：原样的名字，画成和点阵字标差不多高的普通文字
    let text = draw::fit(name, (RIGHT - RESERVED - x) as f32, 20.0);
    draw::text(&text, x, y + size - 6, 20, p.text, Anchor::Start, true)
}

fn header(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    let size = 26;
    out.push_str(&draw::mark(PAD, y, size, p));
    out.push_str(&wordmark(&facts.name, PAD + size + 12, y, size, p));

    out.push_str(&draw::label(s.profile, RIGHT, y + 10, p.muted, Anchor::End));
    out.push_str(&draw::text(
        facts.profile,
        RIGHT,
        y + 26,
        14,
        p.text,
        Anchor::End,
        true,
    ));
    y + size
}

// ── 项目名与一句话 ──────────────────────────────────────────

/// 卡片上最大的那一行是**项目名**，不是任何一个数字。
///
/// 截图里那个位置放的是花掉的钱——因为那张卡片就是一张账单。这张卡片是
/// 一个项目的名片，读者要的第一个信息是「这是什么」。
fn headline(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    // 项目名不在这里——它已经是抬头的字标了。同一个名字印两遍，等于把这张
    // 卡最值钱的那块位置浪费掉一次。
    let mut bottom = y;

    // 星标只在 --remote 下有值。没有就不画那一格，而不是写个 0——
    // 「0 星」和「没查」是两回事
    if let Some(stars) = facts.stars {
        let label = format!("{} {}", human(stars as usize), s.stars);
        out.push_str(&draw::text(
            &label,
            RIGHT,
            y + 13,
            14,
            p.text,
            Anchor::End,
            true,
        ));
        bottom = y + 13;
    }
    if let Some(tag) = &facts.latest_tag {
        let ty = if facts.stars.is_some() {
            y + 31
        } else {
            y + 13
        };
        out.push_str(&draw::text(tag, RIGHT, ty, 13, p.muted, Anchor::End, false));
        bottom = bottom.max(ty);
    }

    // 描述独占一行，排在右列下面。和星标挤在同一行的话，一句长描述会直接
    // 压过去。
    if let Some(d) = &facts.description {
        let dy = bottom + 20;
        let text = draw::fit(d, INNER as f32, 13.0);
        out.push_str(&draw::text(
            &text,
            PAD,
            dy,
            13,
            p.muted,
            Anchor::Start,
            false,
        ));
        bottom = dy;
    }
    bottom
}

/// `61 FILES · 3 LANGUAGES · 12 TAGS · MIT` 那一行。
///
/// 只列**查得出来**的项。查不到的整项不出现，而不是写一个 `—`：
/// 一行占位符会让读者以为这个仓库缺了什么，其实只是我们没查。
fn tally(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    let mut parts: Vec<(String, String)> = Vec::new();
    parts.push((facts.total_files.to_string(), s.files.to_string()));

    let kinds = facts.langs.iter().filter(|l| l.kind == Kind::Code).count();
    if kinds > 0 {
        parts.push((kinds.to_string(), s.languages_unit.to_string()));
    }
    if facts.commits > 0 {
        let n = if facts.commits_truncated {
            format!("{}+", human(facts.commits))
        } else {
            human(facts.commits)
        };
        parts.push((n, s.commits.to_string()));
    }
    if facts.tags > 0 {
        parts.push((facts.tags.to_string(), s.tags.to_string()));
    }
    if !facts.topics.is_empty() {
        parts.push((facts.topics.len().to_string(), s.topics.to_string()));
    }

    // 数字用正文色，单位用弱色：一行数据里真正要被读到的是数字
    let mut x = PAD;
    for (i, (value, unit)) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(&draw::text("·", x, y, 13, p.line, Anchor::Start, false));
            x += 14;
        }
        out.push_str(&draw::text(value, x, y, 14, p.text, Anchor::Start, true));
        x += width(value, 14) + 6;
        out.push_str(&draw::text(unit, x, y, 12, p.muted, Anchor::Start, false));
        x += width(unit, 12) + 12;
    }

    // 许可证靠右：它是一个是非题，读者要么在找它要么不在乎
    let license = facts.license.as_deref().unwrap_or(s.none);
    let license = if license == "yes" { s.license } else { license };
    out.push_str(&draw::text(
        license,
        RIGHT,
        y,
        13,
        p.text,
        Anchor::End,
        true,
    ));

    out.push_str(&draw::hline(PAD, y + 16, INNER, p.line));
    y + 16
}

// ── 语言构成 ────────────────────────────────────────────────

fn languages(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    out.push_str(&draw::label(
        &format!("{} · {}", s.languages, s.by_file),
        PAD,
        y,
        p.muted,
        Anchor::Start,
    ));
    out.push_str(&draw::text(
        &format!(
            "{} {} · {} {}",
            facts.total_files,
            s.files,
            facts.langs.len(),
            s.kinds
        ),
        RIGHT,
        y,
        12,
        p.muted,
        Anchor::End,
        false,
    ));

    if facts.total_files == 0 {
        return y + 10;
    }

    // 尾巴并成一行。前五名之外逐条列出来，读者也记不住。
    let mut rows: Vec<(String, usize, Kind)> = facts
        .langs
        .iter()
        .take(MAX_LANGS)
        .map(|l| (l.name.to_string(), l.files, l.kind))
        .collect();
    if facts.langs.len() > MAX_LANGS {
        let rest = &facts.langs[MAX_LANGS..];
        rows.push((
            format!("{}{}{}", s.more_prefix, rest.len(), s.more_suffix),
            rest.iter().map(|l| l.files).sum(),
            Kind::Other,
        ));
    }

    let bar_x = PAD + 168;
    let bar_w = 300;
    let mut cursor = y;
    for (i, (name, files, _)) in rows.iter().enumerate() {
        cursor += 26;
        let ratio = *files as f32 / facts.total_files as f32;
        let color = p.series(i);

        // 每行左端一小截色条，把这一行和图例对上——
        // 只靠条形本身的颜色，读者得来回扫两遍才能配对
        out.push_str(&draw::rect(PAD, cursor - 9, 3, 12, 1.5, color));
        out.push_str(&draw::text(
            &draw::fit(name, 140.0, 13.0),
            PAD + 12,
            cursor,
            13,
            p.text,
            Anchor::Start,
            false,
        ));
        out.push_str(&draw::ratio_bar(
            bar_x,
            cursor - 8,
            bar_w,
            10,
            ratio,
            color,
            p.track,
        ));
        out.push_str(&draw::text(
            &format!("{}%", (ratio * 100.0).round() as u32),
            bar_x + bar_w + 56,
            cursor,
            13,
            p.text,
            Anchor::End,
            true,
        ));
        out.push_str(&draw::text(
            &format!("{files}"),
            RIGHT,
            cursor,
            13,
            p.muted,
            Anchor::End,
            false,
        ));
    }
    cursor
}

// ── 文件用途 ────────────────────────────────────────────────

/// 代码 / 文档 / 配置 / 其他，一条通栏堆叠条。
///
/// 语言构成回答「用什么写的」，这一条回答「这个仓库里装的是什么」——
/// 一个 80% 是文档的「库」和一个 80% 是代码的库，不是同一种东西。
fn composition(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    let kinds = lang::by_kind(&facts.langs);
    if kinds.is_empty() {
        return y;
    }
    out.push_str(&draw::label(s.composition, PAD, y, p.muted, Anchor::Start));

    let parts: Vec<(f32, Rgb)> = kinds
        .iter()
        .enumerate()
        .map(|(i, (_, n))| (*n as f32, p.series(i)))
        .collect();
    out.push_str(&draw::stacked(PAD, y + 12, INNER, 12, &parts));

    // 图例接在条下面，色块 + 名称 + 占比，一行摆完
    let mut x = PAD;
    let total: usize = kinds.iter().map(|(_, n)| *n).sum();
    for (i, (kind, n)) in kinds.iter().enumerate() {
        out.push_str(&draw::swatch(x, y + 38, p.series(i)));
        let name = kind_word(*kind, s);
        out.push_str(&draw::text(
            name,
            x + 14,
            y + 46,
            12,
            p.muted,
            Anchor::Start,
            false,
        ));
        let pct = format!("{}%", (*n as f32 / total as f32 * 100.0).round() as u32);
        let nx = x + 14 + width(name, 12) + 6;
        out.push_str(&draw::text(
            &pct,
            nx,
            y + 46,
            12,
            p.text,
            Anchor::Start,
            false,
        ));
        x = nx + width(&pct, 12) + 20;
    }
    y + 46
}

fn kind_word(k: Kind, s: &Strings) -> &'static str {
    match k {
        Kind::Code => s.kind_code,
        Kind::Docs => s.kind_docs,
        Kind::Config => s.kind_config,
        Kind::Other => s.kind_other,
    }
}

// ── 提交活跃度 ──────────────────────────────────────────────

/// 一年的每周提交数。
///
/// 这是卡片上唯一一条**时间**信息，也是「这个项目还活着吗」的唯一直接
/// 答案——一个陌生人在决定要不要用你的库时，问的就是这个。
///
/// 窗口终点是 HEAD 的提交时间而不是「现在」：一个停更两年的仓库若按当前
/// 时间开窗，画出来是一条整齐的零线，看着像没数据而不像停更。
fn activity(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    out.push_str(&draw::label(
        &format!("{} · {}", s.activity, s.weeks),
        PAD,
        y,
        p.muted,
        Anchor::Start,
    ));

    let peak = facts.activity.iter().copied().max().unwrap_or(0);
    if facts.activity.len() < 2 || peak == 0 {
        let why = if facts.shallow {
            s.shallow_note
        } else {
            s.no_history
        };
        out.push_str(&draw::text(
            why,
            PAD,
            y + 22,
            12,
            p.muted,
            Anchor::Start,
            false,
        ));
        return y + 22;
    }

    out.push_str(&draw::text(
        &format!("{peak} {}", s.peak),
        RIGHT,
        y,
        12,
        p.muted,
        Anchor::End,
        false,
    ));

    let (top, h) = (y + 14, 66);
    out.push_str(&draw::rect(PAD, top, INNER, h, 6.0, p.panel));
    out.push_str(&draw::dashed(PAD + 8, top + 8, INNER - 16, p.line));
    out.push_str(&draw::area(
        PAD + 8,
        top + 8,
        INNER - 16,
        h - 16,
        &facts.activity,
        peak,
        p.series(2),
    ));

    // 横轴只标两端。中间再插刻度就得决定「第 26 周」是哪一天，
    // 而那取决于读者什么时候看这张图——不是一个能标死的东西。
    let weeks = facts.activity.len();
    out.push_str(&draw::text(
        &format!("{}{weeks}{}", s.weeks_ago_prefix, s.weeks_ago_suffix),
        PAD,
        top + h + 14,
        11,
        p.muted,
        Anchor::Start,
        false,
    ));
    let last = match facts.days_since_commit {
        Some(0) => s.today.to_string(),
        Some(d) => format!("{d}{}", s.days_ago),
        None => String::new(),
    };
    out.push_str(&draw::text(
        &last,
        RIGHT,
        top + h + 14,
        11,
        p.muted,
        Anchor::End,
        false,
    ));

    if facts.shallow {
        out.push_str(&draw::text(
            s.shallow_note,
            W / 2,
            top + h + 14,
            11,
            p.warn,
            Anchor::Middle,
            false,
        ));
    }
    top + h + 14
}

/// star 增长曲线。
///
/// **只有 `--remote --stars` 才有数据**，没有就整节不画——一个空的图表框
/// 比没有图表更糟：它看起来像这个仓库一颗星都没有。
///
/// 曲线的点是**精确**的：GitHub 没有「历年 star 数」这种接口，但
/// `/stargazers` 按加星时间升序返回，所以第 k 页的第一个人就是第
/// `(k-1)*100+1` 颗星落下的那一刻。抽样抽的是页，不是插值出来的数——
/// 近似的只有点与点之间那几段直线。
fn star_history(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    let pts = &facts.star_history;
    if pts.len() < 2 {
        return y;
    }
    // 语言无关的标记，给「你正要洗掉这条曲线」那道检查用。曲线的标签是
    // 翻译过的，靠文字去认，换个 --lang 就认不出来了。
    out.push_str(STAR_MARK);
    out.push('\n');
    let total = pts.last().map(|p| p.count).unwrap_or(0);
    let first = pts.first().expect("上面判过长度");

    out.push_str(&draw::label(s.star_history, PAD, y, p.muted, Anchor::Start));
    out.push_str(&draw::text(
        &format!("{} {}", human(total as usize), s.stars),
        RIGHT,
        y,
        12,
        p.muted,
        Anchor::End,
        false,
    ));

    let (top, h) = (y + 14, 66);
    out.push_str(&draw::rect(PAD, top, INNER, h, 6.0, p.panel));

    // 按**时间**取横坐标，不按点的序号：抽样是按页均匀的，而 star 增长
    // 不是均匀的，照序号画会把一段爆发期和一段沉寂期画成同样宽。
    let (t0, t1) = (first.at, pts.last().expect("上面判过长度").at);
    let span = (t1 - t0).max(1) as f32;
    let w = (INNER - 16) as f32;
    let x_of = |at: i64| PAD as f32 + 8.0 + w * ((at - t0) as f32 / span);
    let y_of = |c: u64| (top + 8) as f32 + (h - 16) as f32 * (1.0 - c as f32 / total.max(1) as f32);

    let mut line = String::new();
    for (i, pt) in pts.iter().enumerate() {
        let _ = std::fmt::Write::write_fmt(
            &mut line,
            format_args!(
                "{}{:.1} {:.1} ",
                if i == 0 { "M" } else { "L" },
                x_of(pt.at),
                y_of(pt.count)
            ),
        );
    }
    let line = line.trim_end().to_string();
    let color = p.series(4);
    let _ = std::fmt::Write::write_fmt(
        out,
        format_args!(
            "  <path d=\"{line} L{:.1} {} L{:.1} {} Z\" fill=\"{color}\" fill-opacity=\"0.28\"/>\n  \
             <path d=\"{line}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\" \
             stroke-linejoin=\"round\" stroke-linecap=\"round\"/>\n",
            x_of(t1),
            top + h - 8,
            x_of(t0),
            top + h - 8,
        ),
    );

    // 两端各标一个日期。中间不标：抽样是按页来的，中间那些点的横坐标
    // 精确，但标注密了只会挡住曲线本身。
    out.push_str(&draw::text(
        &format!("{} {}", s.stars_since, year_month(first.at)),
        PAD,
        top + h + 14,
        11,
        p.muted,
        Anchor::Start,
        false,
    ));
    out.push_str(&draw::text(
        &year_month(t1),
        RIGHT,
        top + h + 14,
        11,
        p.muted,
        Anchor::End,
        false,
    ));
    top + h + 14
}

/// Unix 秒 → `2024-03`。只需要年月，所以不引日期库。
fn year_month(at: i64) -> String {
    let days = at.div_euclid(86_400);
    // days_from_civil 的逆运算，同一篇 Hinnant 的算法
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}")
}

// ── 页脚 ────────────────────────────────────────────────────

fn footer(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    out.push_str(&draw::hline(PAD, y, INNER, p.line));
    let row = y + 24;

    let mut left = match &facts.owner {
        Some(o) => format!("{o}/{}", facts.name),
        None => facts.name.clone(),
    };
    if let Some(b) = &facts.branch {
        let _ = write!(left, " · {b}");
    }
    if let Some(e) = &facts.ecosystem {
        let _ = write!(left, " · {e}");
    }
    out.push_str(&draw::text(
        &draw::fit(&left, INNER as f32 * 0.6, 12.0),
        PAD,
        row,
        12,
        p.muted,
        Anchor::Start,
        false,
    ));
    out.push_str(&draw::text(
        &format!("{} repolish v{}", s.generated_by, env!("CARGO_PKG_VERSION")),
        RIGHT,
        row,
        12,
        p.muted,
        Anchor::End,
        false,
    ));
    row + PAD - 8
}

// ── 小工具 ──────────────────────────────────────────────────

fn width(s: &str, size: i32) -> i32 {
    draw::width_px(s, size as f32).round() as i32
}

/// 千位缩写。`1.4k` 比 `1423` 好读，而在一张概览卡片上，
/// 提交数的个位数没有任何意义。
fn human(n: usize) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => {
            let k = n as f32 / 1000.0;
            if k < 10.0 {
                format!("{k:.1}k")
            } else {
                format!("{}k", k.round() as u32)
            }
        }
        _ => format!("{:.1}M", n as f32 / 1_000_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{DARK, PORCELAIN};
    use repolish_ingest::lang::{Kind, LangStat};

    /// 字标画的是**项目名**，不是 repolish。这张卡贴在别人的 README 顶上，
    /// 抬头印我们的名字是把最值钱的位置拿去做自我推销。
    #[test]
    fn the_masthead_carries_the_project_name() {
        let svg = wordmark("taskvault", 74, 36, 26, &DARK);
        // 点阵路径：一堆 3px 的方块，没有一个 <text>
        assert!(svg.contains(r#"width="3""#), "expected the block wordmark");
        assert!(
            !svg.contains("<text"),
            "a fitting ASCII name should not fall back"
        );
        assert!(!svg.to_lowercase().contains("repolish"));
    }

    /// 点阵字体只有 A-Z0-9.-。非拉丁名字整串都画不出来，硬画就是一片空白，
    /// 所以必须退回普通文字。
    #[test]
    fn a_name_the_block_font_cannot_draw_falls_back_to_text() {
        let svg = wordmark("中文项目", 74, 36, 26, &DARK);
        assert!(svg.contains("<text"), "expected the text fallback");
        assert!(svg.contains("中文项目"));
    }

    /// 太长的名字会一路压到右边的 PROFILE 上。
    #[test]
    fn an_overlong_name_falls_back_rather_than_running_into_the_profile() {
        let long = "a-very-long-repository-name-that-cannot-possibly-fit-up-there";
        let svg = wordmark(long, 74, 36, 26, &DARK);
        assert!(svg.contains("<text"), "expected the text fallback");
    }

    /// 下划线在仓库名里很常见，而它恰好是这套字体没有的字符之一。
    /// 换成连字符，而不是开一个空洞、或者整串退回文字。
    #[test]
    fn an_underscore_becomes_a_hyphen_rather_than_a_hole() {
        let svg = wordmark("my_tool", 74, 36, 26, &DARK);
        assert!(svg.contains(r#"width="3""#), "expected the block wordmark");
        assert!(!svg.contains("<text"));
    }

    fn facts() -> Facts {
        Facts {
            name: "taskvault".into(),
            owner: Some("acme".into()),
            description: Some("A vault for tasks".into()),
            profile: "cli",
            langs: vec![
                LangStat {
                    name: "Rust",
                    kind: Kind::Code,
                    files: 40,
                },
                LangStat {
                    name: "Markdown",
                    kind: Kind::Docs,
                    files: 8,
                },
                LangStat {
                    name: "TOML",
                    kind: Kind::Config,
                    files: 2,
                },
            ],
            total_files: 50,
            commits: 1423,
            commits_truncated: false,
            activity: (0..52).map(|i| (i % 7) as u32).collect(),
            shallow: false,
            tags: 6,
            latest_tag: Some("v0.10.0".into()),
            days_since_commit: Some(3),
            license: Some("MIT".into()),
            stars: Some(1234),
            star_history: Vec::new(),
            topics: vec!["cli".into(), "rust".into()],
            ecosystem: Some("cargo".into()),
            branch: Some("main".into()),
        }
    }

    #[test]
    fn is_a_self_contained_svg_with_no_external_references() {
        let svg = overview(&facts(), &Options::default());
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.trim_end().ends_with("</svg>"));
        crate::draw::assert_self_contained(&svg);
    }

    /// 有曲线的卡片必须带得出标记，否则「你正要洗掉这条曲线」那道检查
    /// 就永远不会触发——而它防的正是一次静默的退化。
    #[test]
    fn a_card_with_a_star_curve_is_detectable_and_one_without_is_not() {
        let mut f = facts();
        f.star_history = vec![
            repolish_ingest::StarPoint { at: 1, count: 1 },
            repolish_ingest::StarPoint { at: 2, count: 9 },
        ];
        assert!(has_star_history(&overview(&f, &Options::default())));
        assert!(!has_star_history(&overview(&facts(), &Options::default())));
    }

    /// 每次 CI 都提交一张只有噪声在变的卡片是不能接受的
    #[test]
    fn the_same_facts_render_byte_identical_svg() {
        let o = Options::default();
        assert_eq!(overview(&facts(), &o), overview(&facts(), &o));
    }

    /// 项目名是这张卡上最大的东西——但它现在是**抬头的字标**，画成点阵方块，
    /// 不再是下面那行 40px 的文字。同一个名字印两遍，等于把最值钱的位置浪费
    /// 掉一次；这条测试守的是「只出现一次，且在抬头」。
    #[test]
    fn the_project_name_is_the_masthead_and_is_not_repeated_below() {
        let svg = overview(&facts(), &Options::default());
        assert!(
            !svg.contains(">taskvault<"),
            "名字应该在字标里，而不是再画一行文字"
        );
        // 字标是点阵方块，而点阵只有在名字画得出来时才会出现
        assert!(svg.contains(r#"width="3""#), "抬头缺了点阵字标");
        assert!(
            wordmark("taskvault", 74, 36, 26, &DARK).contains(r#"width="3""#),
            "taskvault 应该走点阵那条路"
        );
    }

    /// 概览卡片说的是项目，不是我们的分数——一个分数都不该出现在上面
    #[test]
    fn no_repolish_score_appears_on_the_overview() {
        let svg = overview(&facts(), &Options::default());
        for word in ["SCORE", "/ 100", "TO FIX"] {
            assert!(!svg.contains(word), "概览卡片上不该有 {word}");
        }
    }

    /// 卡片是固定宽度的图片：任何一段文字画到框外面，就是一张画坏了的图
    #[test]
    fn no_text_is_drawn_outside_the_card() {
        for lang in [
            crate::i18n::Lang::En,
            crate::i18n::Lang::ZhCn,
            crate::i18n::Lang::Ja,
        ] {
            let svg = overview(
                &facts(),
                &Options {
                    palette: &DARK,
                    lang,
                },
            );
            for (text, left, right) in text_extents(&svg) {
                assert!(
                    left >= 8.0 && right <= W as f32 - 8.0,
                    "{lang:?}: {text:?} 画到了 {left:.0}..{right:.0}，卡宽 {W}"
                );
            }
        }
    }

    /// 估算每个 `<text>` 的左右边界。等宽字的步进恒定，估得够准。
    fn text_extents(svg: &str) -> Vec<(String, f32, f32)> {
        let mut out = Vec::new();
        for node in svg.split("<text ").skip(1) {
            let attr = |k: &str| {
                node.split(&format!("{k}=\""))
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .map(str::to_string)
            };
            let (Some(x), Some(size), Some(anchor)) =
                (attr("x"), attr("font-size"), attr("text-anchor"))
            else {
                continue;
            };
            let body: String = node
                .split_once('>')
                .and_then(|(_, r)| r.split_once("</text>"))
                .map(|(t, _)| t.to_string())
                .unwrap_or_default();
            let x: f32 = x.parse().unwrap_or(0.0);
            let w = crate::draw::width_px(&body, size.parse().unwrap_or(12.0));
            let (l, r) = match anchor.as_str() {
                "end" => (x - w, x),
                "middle" => (x - w / 2.0, x + w / 2.0),
                _ => (x, x + w),
            };
            out.push((body, l, r));
        }
        out
    }

    #[test]
    fn chinese_labels_are_used_when_the_language_says_so() {
        let svg = overview(
            &facts(),
            &Options {
                palette: &DARK,
                lang: crate::i18n::Lang::ZhCn,
            },
        );
        assert!(svg.contains("语言构成"));
        assert!(svg.contains(r#"lang="zh-CN""#));
        assert!(!svg.contains("LANGUAGES"));
    }

    #[test]
    fn the_palette_changes_every_colour_on_the_card() {
        let lang = crate::i18n::Lang::En;
        let dark = overview(
            &facts(),
            &Options {
                palette: &DARK,
                lang,
            },
        );
        let light = overview(
            &facts(),
            &Options {
                palette: &PORCELAIN,
                lang,
            },
        );
        assert!(dark.contains(&DARK.bg.to_string()));
        assert!(light.contains(&PORCELAIN.bg.to_string()));
        assert!(!light.contains(&DARK.bg.to_string()));
    }

    /// 星标只有 --remote 才查得到。查不到就不画那一格，而不是写个 0
    #[test]
    fn unknown_remote_facts_are_left_out_rather_than_shown_as_zero() {
        let mut f = facts();
        f.stars = None;
        f.topics.clear();
        let svg = overview(&f, &Options::default());
        assert!(!svg.contains("stars"));
        assert!(!svg.contains("topics"));
    }

    /// 浅克隆下画出来的曲线是残缺的，卡片必须说出来
    #[test]
    fn a_shallow_clone_is_labelled_rather_than_drawn_as_a_flat_line() {
        let mut f = facts();
        f.shallow = true;
        f.activity = vec![0; 52];
        let svg = overview(&f, &Options::default());
        assert!(svg.contains("shallow clone"));
    }

    #[test]
    fn a_repository_with_no_git_history_still_renders() {
        let mut f = facts();
        f.activity.clear();
        f.commits = 0;
        f.tags = 0;
        f.latest_tag = None;
        f.days_since_commit = None;
        let svg = overview(&f, &Options::default());
        assert!(svg.contains("no commit history"));
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn an_empty_repository_does_not_divide_by_zero() {
        let mut f = facts();
        f.langs.clear();
        f.total_files = 0;
        let svg = overview(&f, &Options::default());
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn repository_names_are_xml_escaped() {
        let mut f = facts();
        f.name = "a<b&c".into();
        let svg = overview(&f, &Options::default());
        assert!(svg.contains("a&lt;b&amp;c"));
    }

    /// 日期换算是自己写的（不引日期库），所以它必须被真的验一遍——
    /// 闰年、世纪、以及 1970 之前都算对了才算数
    #[test]
    fn unix_seconds_convert_to_the_right_year_and_month() {
        assert_eq!(year_month(0), "1970-01");
        assert_eq!(year_month(1_710_663_704), "2024-03");
        // 闰日当天与它的前后
        assert_eq!(year_month(1_709_164_800), "2024-02"); // 2024-02-29
        assert_eq!(year_month(1_709_251_200), "2024-03"); // 2024-03-01
                                                          // 2000 是闰年（能被 400 整除），所以 2000-02-29 存在
        assert_eq!(year_month(951_782_400), "2000-02"); // 2000-02-29
        assert_eq!(year_month(951_868_800), "2000-03"); // 2000-03-01
        assert_eq!(year_month(946_684_800), "2000-01");
        // 1970 之前
        assert_eq!(year_month(-1), "1969-12");
    }

    fn stars(points: &[(i64, u64)]) -> Facts {
        let mut f = facts();
        f.star_history = points
            .iter()
            .map(|(at, count)| repolish_ingest::StarPoint {
                at: *at,
                count: *count,
            })
            .collect();
        f
    }

    /// 一个空的图表框比没有图表更糟：它看起来像这个仓库一颗星都没有
    #[test]
    fn no_star_data_means_no_star_section() {
        let svg = overview(&stars(&[]), &Options::default());
        assert!(!svg.contains("STARS"));
        // 一个点画不出曲线
        let svg = overview(&stars(&[(1_700_000_000, 1)]), &Options::default());
        assert!(!svg.contains("STARS"));
    }

    #[test]
    fn a_star_curve_is_drawn_with_both_ends_dated() {
        let svg = overview(
            &stars(&[
                (1_640_995_200, 1),     // 2022-01
                (1_688_169_600, 500),   // 2023-07
                (1_719_792_000, 1_200), // 2024-07
            ]),
            &Options::default(),
        );
        assert!(svg.contains("STARS"));
        assert!(svg.contains("2022-01"), "起点日期没标");
        assert!(svg.contains("2024-07"), "终点日期没标");
        assert!(svg.contains("1.2k"), "总数没写出来");
        crate::draw::assert_self_contained(&svg);
    }

    /// 横坐标按时间走，不按点的序号——抽样是按页均匀的，而 star 增长不是，
    /// 照序号画会把一段爆发期和一段沉寂期画成同样宽
    #[test]
    fn the_x_axis_follows_time_not_sample_order() {
        // 前两点相隔一年，后两点相隔一天
        let mut f = stars(&[
            (1_600_000_000, 1),
            (1_631_536_000, 100),
            (1_631_622_400, 200),
        ]);
        // 提交活跃度也会画 path，清掉它免得取错那一条
        f.activity.clear();
        let svg = overview(&f, &Options::default());

        // 品牌标记也是 path，所以按描边宽度挑出曲线那一条
        let d = svg
            .split("<path d=\"")
            .skip(1)
            .filter(|seg| {
                seg.split("/>")
                    .next()
                    .is_some_and(|tag| tag.contains("stroke-width=\"2\""))
            })
            .map(|seg| seg.split('"').next().unwrap_or(""))
            .next()
            .expect("star 曲线应该画出一条描边 path");
        let xs: Vec<f32> = d
            .split(['M', 'L'])
            .filter_map(|seg| seg.split_whitespace().next()?.parse::<f32>().ok())
            .collect();
        assert!(xs.len() >= 3, "解析到的点太少: {xs:?} from {d}");

        let (a, b) = (xs[1] - xs[0], xs[2] - xs[1]);
        assert!(
            a > b * 20.0,
            "一年和一天画成了差不多宽: {a} vs {b} ({xs:?})"
        );
    }

    #[test]
    fn thousands_are_abbreviated_rather_than_printed_in_full() {
        assert_eq!(human(999), "999");
        assert_eq!(human(1423), "1.4k");
        assert_eq!(human(24_500), "25k");
        assert_eq!(human(2_400_000), "2.4M");
    }

    #[test]
    fn card_height_stays_within_a_readable_range() {
        let svg = overview(&facts(), &Options::default());
        let h: i32 = svg
            .split(r#"height=""#)
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!((360..700).contains(&h), "卡片高度失控: {h}");
    }
}
