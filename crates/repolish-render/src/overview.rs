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
        let Some(code) = readme_lang_code(path) else {
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

/// `README.zh-CN.md` → `zh-cn`。认不出来返回 `None`。
fn readme_lang_code(path: &str) -> Option<String> {
    let file = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let stem = file
        .strip_suffix(".md")
        .or_else(|| file.strip_suffix(".rst"))?;
    let rest = stem.strip_prefix("readme")?;
    let code = rest.trim_start_matches(['.', '_', '-']);
    if code.is_empty() {
        return None;
    }
    Some(code.to_string())
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
    let height = footer(&mut body, facts, p, s, y + 22);

    let aria = match &facts.owner {
        Some(o) => format!("{o}/{} — repository overview", facts.name),
        None => format!("{} — repository overview", facts.name),
    };
    draw::document(&body, W, height, p, opts.lang.tag(), &aria)
}

// ── 页眉 ────────────────────────────────────────────────────

fn header(out: &mut String, facts: &Facts, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    let size = 26;
    out.push_str(&draw::mark(PAD, y, size, p));
    out.push_str(&draw::blocks("REPOLISH", PAD + size + 12, y + 1, 3, p));

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
    let size = 40;
    let name = draw::fit(&facts.name, INNER as f32 * 0.62, size as f32);
    out.push_str(&draw::text(
        &name,
        PAD,
        y + size,
        size,
        p.text,
        Anchor::Start,
        true,
    ));

    // 星标只在 --remote 下有值。没有就不画那一格，而不是写个 0——
    // 「0 星」和「没查」是两回事
    if let Some(stars) = facts.stars {
        let label = format!("{} {}", human(stars as usize), s.stars);
        out.push_str(&draw::text(
            &label,
            RIGHT,
            y + size - 16,
            14,
            p.text,
            Anchor::End,
            true,
        ));
    }
    if let Some(tag) = &facts.latest_tag {
        out.push_str(&draw::text(
            tag,
            RIGHT,
            y + size + 4,
            13,
            p.muted,
            Anchor::End,
            false,
        ));
    }

    if let Some(d) = &facts.description {
        let text = draw::fit(d, INNER as f32, 13.0);
        out.push_str(&draw::text(
            &text,
            PAD,
            y + size + 24,
            13,
            p.muted,
            Anchor::Start,
            false,
        ));
        return y + size + 24;
    }
    y + size + 4
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

    /// 每次 CI 都提交一张只有噪声在变的卡片是不能接受的
    #[test]
    fn the_same_facts_render_byte_identical_svg() {
        let o = Options::default();
        assert_eq!(overview(&facts(), &o), overview(&facts(), &o));
    }

    #[test]
    fn the_project_name_is_the_biggest_thing_on_the_card() {
        let svg = overview(&facts(), &Options::default());
        assert!(svg.contains(r#"font-size="40""#));
        assert!(svg.contains(">taskvault<"));
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
        for lang in [crate::i18n::Lang::En, crate::i18n::Lang::ZhCn] {
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
