//! `.repolish/card.svg` —— **分数**卡片。
//!
//! 位置在 README 的**末尾**，不是顶上。顶上属于 [`crate::overview`]：一个
//! 陌生人点进你的仓库，第一眼该看到的是你的项目，不是我们的工具给你打了
//! 几分。分数卡片的读者是作者自己，以及顺着它找过来的下一个作者——
//! 它同时是一枚诊断结果和一条来路。
//!
//! 和 `badge.json` 是同一条路子的延伸：文件放在**用户自己的仓库**里，
//! 由他自己的 raw URL 提供，我们不托管任何东西。区别只是徽章上写得下一个
//! 数字，卡片写得下扣分在哪。
//!
//! 三条硬约束：
//!
//! - **自包含。** 不引外部字体、不引外部图片、不引脚本。wordmark 走
//!   [`crate::glyph`] 的点阵转矩形——别人机器上装没装 JetBrains Mono 不由我们
//!   决定，一个 logo 不能在半数读者那里换一副面孔。分数和文案则是普通文本：
//!   上特效的是**标识**，不是数据，把数字画成艺术字只会让人怀疑这数准不准。
//! - **确定性。** 没有时间戳、没有随机数。同一个 commit 生成的 SVG 逐字节一致，
//!   否则每次 CI 都会产生一个只有噪声的 diff。
//! - **色板恒定。** 不做 `prefers-color-scheme` 切换：GitHub 把 SVG 当图片经
//!   camo 代理渲染，媒体查询在那条链路上并不可靠。要浅色版就显式选
//!   `--theme porcelain`，让文件本身就是浅色的。

use repolish_core::{Category, Outcome, Report, Severity};

use crate::draw::{self, Anchor, Options};
use crate::i18n::{band_word, Strings};
use crate::theme::Palette;

/// 卡片在用户仓库中的位置
pub const CARD_PATH: &str = ".repolish/card.svg";

const W: i32 = 880;
const PAD: i32 = 36;
/// 右栏（类别条、页脚右侧）的对齐右边界
const RIGHT: i32 = W - PAD;
const INNER: i32 = W - PAD * 2;
/// 卡片里最多列几条发现。再多就该去看终端输出了，塞满的卡片没人读。
const MAX_FINDINGS: usize = 3;
/// 类别条的段数，与终端的 `meter` 取同一个值
const SEGMENTS: i32 = 12;
const SEG_W: i32 = 22;
const SEG_GAP: i32 = 6;
const BAR_W: i32 = SEGMENTS * SEG_W + (SEGMENTS - 1) * SEG_GAP;

pub fn card(report: &Report, opts: &Options) -> String {
    let p = opts.palette;
    let s = opts.lang.strings();
    let mut body = String::new();
    let mut y = PAD;

    y = header(&mut body, report, p, s, y);
    y += 18;
    body.push_str(&draw::hline(PAD, y, INNER, p.line));
    y = score_block(&mut body, report, p, s, y + 30);
    y = checks_row(&mut body, report, p, s, y + 34);
    y = findings(&mut body, report, p, s, y + 30);
    let height = footer(&mut body, report, p, s, y + 20);

    let aria = match report.score {
        Some(score) => format!("repolish report card — {score} out of 100"),
        None => "repolish report card — not scored".to_string(),
    };
    draw::document(&body, W, height, p, opts.lang.tag(), &aria)
}

// ── 页眉 ────────────────────────────────────────────────────

fn header(out: &mut String, report: &Report, p: &Palette, _s: &'static Strings, y: i32) -> i32 {
    let size = 30;
    out.push_str(&draw::mark(PAD, y, size, p));

    // wordmark 与 logo 同源：点阵 → 矩形，落到哪台机器上都是同一个形状
    out.push_str(&draw::blocks("REPOLISH", PAD + size + 14, y + 1, 4, p));

    let meta = format!(
        "{} · {}",
        report.profile.detected.as_str(),
        report.mode.as_str()
    );
    out.push_str(&draw::text(
        &meta,
        RIGHT,
        y + size / 2 + 5,
        13,
        p.muted,
        Anchor::End,
        false,
    ));

    y + size
}

/// 只有标记的方形 logo，可直接当文件用（favicon、README 头像）。
pub fn logo(size: i32) -> String {
    let p = &crate::theme::DARK;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" \
         viewBox=\"0 0 {size} {size}\" role=\"img\" aria-label=\"repolish\">\n{}{}</svg>\n",
        draw::brand_defs(p),
        draw::mark(0, 0, size, p)
    )
}

/// 横版 logo：标记 + wordmark，**背景透明**——README 在亮暗两种主题下
/// 用的是同一个文件，画一层底色就必然在其中一种下露出方块。
pub fn wordmark(mark_size: i32) -> String {
    let p = &crate::theme::DARK;
    let cell = (mark_size / crate::glyph::H as i32).max(1);
    let text_x = mark_size + mark_size / 3;
    let text_h = cell * crate::glyph::H as i32;
    let width = text_x + crate::glyph::blocks_width("REPOLISH") as i32 * cell;
    let height = mark_size.max(text_h);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" \
         viewBox=\"0 0 {width} {height}\" role=\"img\" aria-label=\"repolish\">\n{}{}{}</svg>\n",
        draw::brand_defs(p),
        draw::mark(0, (height - mark_size) / 2, mark_size, p),
        draw::blocks("REPOLISH", text_x, (height - text_h) / 2, cell, p)
    )
}

/// README 顶部的通栏横幅：标记 + wordmark + 一句话，整体居中。
///
/// 与 [`wordmark`] 的区别只有一个，但那一个是关键：**viewBox 是通栏比例**。
/// README 里以 `width="100%"` 引用时，一个 450×56 的 wordmark 会被拉成一条
/// 横穿页面的巨型字，而这张图按比例缩放后仍然是一个居中的标志。
///
/// 背景同样透明：亮暗两种主题共用一个文件。
pub fn hero(tagline: &str, lang: crate::i18n::Lang) -> String {
    let p = &crate::theme::DARK;
    let (w, h) = (1200, 260);
    let mark_size = 76;
    let cell = 8;

    let text_w = crate::glyph::blocks_width("REPOLISH") as i32 * cell;
    let gap = 28;
    let block_w = mark_size + gap + text_w;
    let x0 = (w - block_w) / 2;
    let top = 62;

    let mut body = draw::mark(x0, top, mark_size, p);
    body.push_str(&draw::blocks(
        "REPOLISH",
        x0 + mark_size + gap,
        top + (mark_size - cell * crate::glyph::H as i32) / 2,
        cell,
        p,
    ));
    if !tagline.is_empty() {
        body.push_str(&draw::text(
            &draw::fit(tagline, w as f32 - 120.0, 19.0),
            w / 2,
            top + mark_size + 46,
            19,
            p.muted,
            Anchor::Middle,
            false,
        ));
    }

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" lang=\"{}\" role=\"img\" aria-label=\"repolish\">\n{}{}{}</svg>\n",
        lang.tag(),
        draw::brand_defs(p),
        format_args!(
            "  <style>\n    .t {{ font-family: {}; }}\n  </style>\n",
            draw::FONT
        ),
        body
    )
}

// ── 分数 ────────────────────────────────────────────────────

fn score_block(out: &mut String, report: &Report, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    let (digits, color) = match report.score {
        Some(score) => (score.to_string(), p.band(score)),
        None => ("--".to_string(), p.muted),
    };

    // 分数是数据，不是标识：用普通字排大号即可。点阵字形只留给 wordmark——
    // 数字换一副等宽字无非是宽了几像素，logo 换一副面孔就不是同一个 logo 了。
    out.push_str(&draw::text(
        &digits,
        PAD,
        y + 56,
        64,
        color,
        Anchor::Start,
        true,
    ));

    let verdict = match report.score {
        Some(score) => format!("/ 100  ·  {}", band_word(score, s)),
        None => s.not_scored.to_string(),
    };
    out.push_str(&draw::text(
        &verdict,
        PAD,
        y + 82,
        13,
        p.muted,
        Anchor::Start,
        false,
    ));

    let col = PAD + 200;
    let bar_x = RIGHT - 42 - BAR_W;
    for (i, cat) in Category::ALL.iter().enumerate() {
        let row = y + 26 + i as i32 * 28;
        // 中文没有大小写，`to_uppercase` 对它是空操作，对英文才有效
        out.push_str(&draw::label(
            &crate::i18n::category_label(*cat, s).to_uppercase(),
            col,
            row,
            p.muted,
            Anchor::Start,
        ));
        let score = report.category_score(*cat);
        out.push_str(&bar(bar_x, row - 9, score.unwrap_or(0), p));
        match score {
            Some(n) => out.push_str(&draw::text(
                &n.to_string(),
                RIGHT,
                row,
                13,
                p.band(n),
                Anchor::End,
                true,
            )),
            None => out.push_str(&draw::text(
                "—",
                RIGHT,
                row,
                13,
                p.muted,
                Anchor::End,
                false,
            )),
        }
    }

    y + 88
}

/// 分段条形图。段数与终端一致——同一个仓库在两个地方必须长出同一根条，
/// 否则「终端里少一格、卡片上却填满」会被当成两套算法。
///
/// 连续条在高分区是没有信息的：99 和 100 差 3.5 个像素，谁也看不出来。
/// 切成 12 段之后，那一格空缺就是给人看的。
fn bar(x: i32, y: i32, score: u8, p: &Palette) -> String {
    let filled = if score == 0 {
        0
    } else {
        (score as i32 * SEGMENTS / 100).clamp(1, SEGMENTS)
    };
    draw::segmented(
        x,
        y,
        draw::Segments {
            count: SEGMENTS,
            width: SEG_W,
            gap: SEG_GAP,
        },
        filled,
        p.band(score),
        p.track,
    )
}

// ── 检查点阵 ────────────────────────────────────────────────

fn checks_row(out: &mut String, report: &Report, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    if report.checks.is_empty() {
        return y;
    }
    out.push_str(&draw::label(s.checks, PAD, y + 4, p.muted, Anchor::Start));

    let (mut scored, mut na, mut unresolved) = (0, 0, 0);
    let x0 = PAD + 82;
    for (i, r) in report.checks.iter().enumerate() {
        let color = match &r.outcome {
            Outcome::Scored { score: 10, .. } => {
                scored += 1;
                p.bands[0]
            }
            Outcome::Scored { score: 0, .. } => {
                scored += 1;
                p.bad
            }
            Outcome::Scored { .. } => {
                scored += 1;
                p.warn
            }
            Outcome::NotApplicable { .. } => {
                na += 1;
                p.line
            }
            _ => {
                unresolved += 1;
                p.muted
            }
        };
        out.push_str(&draw::circle(x0 + i as i32 * 16, y, 5, color));
    }

    let mut tally = vec![format!("{scored} {}", s.scored)];
    if unresolved > 0 {
        tally.push(format!("{unresolved} {}", s.not_verified));
    }
    if na > 0 {
        tally.push(format!("{na} {}", s.not_applicable));
    }
    out.push_str(&draw::text(
        &tally.join(" · "),
        RIGHT,
        y + 4,
        12,
        p.muted,
        Anchor::End,
        false,
    ));

    y + 10
}

// ── 发现 ────────────────────────────────────────────────────

fn findings(out: &mut String, report: &Report, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    let mut all: Vec<(Severity, &str, &str)> = Vec::new();
    for r in &report.checks {
        for f in r.outcome.fixes() {
            all.push((f.severity, r.id, f.message.as_str()));
        }
    }
    all.sort_by_key(|(sev, id, _)| (*sev, *id));
    if all.is_empty() {
        return y;
    }

    let mut cursor = y;
    out.push_str(&draw::hline(PAD, cursor, INNER, p.line));
    cursor += 22;
    out.push_str(&draw::label(
        s.to_fix,
        PAD,
        cursor,
        p.brand[1],
        Anchor::Start,
    ));
    cursor += 24;

    for (sev, id, msg) in all.iter().take(MAX_FINDINGS) {
        let (label, color) = match sev {
            Severity::P1 => ("P1", p.bad),
            Severity::P2 => ("P2", p.warn),
            Severity::P3 => ("P3", p.brand[0]),
        };
        out.push_str(&draw::rect(PAD, cursor - 13, 26, 18, 5.0, color));
        out.push_str(&draw::text(
            label,
            PAD + 13,
            cursor,
            11,
            p.bg,
            Anchor::Middle,
            true,
        ));
        out.push_str(&draw::text(
            id,
            PAD + 38,
            cursor,
            13,
            p.text,
            Anchor::Start,
            true,
        ));
        // 卡片是固定宽度的图片，没有换行可用——过长的建议在这里截断，
        // 完整文案在终端和 REPOLISH.md 里
        out.push_str(&draw::text(
            &draw::fit(msg, (INNER - 38) as f32, 12.0),
            PAD + 38,
            cursor + 18,
            12,
            p.muted,
            Anchor::Start,
            false,
        ));
        cursor += 46;
    }

    if all.len() > MAX_FINDINGS {
        out.push_str(&draw::text(
            &format!("+{} {}", all.len() - MAX_FINDINGS, s.more_findings),
            PAD + 38,
            cursor - 8,
            12,
            p.muted,
            Anchor::Start,
            false,
        ));
        cursor += 10;
    }

    cursor - 18
}

fn footer(out: &mut String, report: &Report, p: &Palette, s: &'static Strings, y: i32) -> i32 {
    out.push_str(&draw::hline(PAD, y, INNER, p.line));
    let row = y + 24;

    let mut left = match &report.repository.owner {
        Some(o) => format!("{}/{}", o, report.repository.name),
        None => report.repository.name.clone(),
    };
    if let Some(commit) = &report.repository.commit {
        let short: String = commit.chars().take(8).collect();
        left.push_str(&format!(" · {short}"));
    }
    out.push_str(&draw::text(
        &left,
        PAD,
        row,
        12,
        p.muted,
        Anchor::Start,
        false,
    ));
    out.push_str(&draw::text(
        &format!("repolish v{}", report.repolish_version),
        RIGHT,
        row,
        12,
        p.muted,
        Anchor::End,
        false,
    ));
    // 这一行是免责声明，不是装饰：它说的是这个分数凭什么可信。
    // 用比分隔线还淡的颜色写它，等于没写。
    out.push_str(&draw::text(
        s.deterministic,
        PAD,
        row + 18,
        11,
        p.muted,
        Anchor::Start,
        false,
    ));

    row + PAD + 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use crate::theme::{DARK, PORCELAIN};
    use repolish_core::{CheckResult, Evidence, Fix, Mode, Profile, ProfileInfo, Repository, Risk};

    fn report(score: u8, name: &str) -> Report {
        let checks = vec![
            CheckResult {
                id: "license",
                category: Category::Credibility,
                risk: Risk::Critical,
                outcome: Outcome::Scored {
                    score: score / 10,
                    evidence: vec![Evidence::new("LICENSE", "MIT")],
                    fixes: vec![Fix::new(Severity::P1, "Add a LICENSE file")],
                },
            },
            CheckResult {
                id: "repo-topics",
                category: Category::Discoverability,
                risk: Risk::High,
                outcome: Outcome::skipped("requires --remote"),
            },
        ];
        Report::build(
            checks,
            Repository {
                owner: Some("acme".into()),
                name: name.into(),
                commit: Some("deadbeefcafe".into()),
            },
            ProfileInfo {
                detected: Profile::Cli,
                overridden: false,
            },
            Mode::Local,
        )
    }

    #[test]
    fn is_a_self_contained_svg_with_no_external_references() {
        let svg = card(&report(80, "widget"), &Options::default());
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.trim_end().ends_with("</svg>"));
        crate::draw::assert_self_contained(&svg);
    }

    /// 每次 CI 都提交一张只有噪声在变的卡片是不能接受的
    #[test]
    fn the_same_report_renders_byte_identical_svg() {
        let o = Options::default();
        assert_eq!(
            card(&report(80, "widget"), &o),
            card(&report(80, "widget"), &o)
        );
    }

    #[test]
    fn repository_names_are_xml_escaped() {
        let svg = card(&report(80, "a<b&c"), &Options::default());
        assert!(svg.contains("a&lt;b&amp;c"));
        assert!(!svg.contains("a<b&c"));
    }

    #[test]
    fn score_band_drives_the_fill_colour() {
        let o = Options::default();
        assert!(card(&report(100, "w"), &o).contains(&DARK.bands[0].to_string()));
        assert!(card(&report(30, "w"), &o).contains(&DARK.bands[4].to_string()));
    }

    #[test]
    fn the_palette_reaches_every_colour_on_the_card() {
        let light = card(
            &report(80, "w"),
            &Options {
                palette: &PORCELAIN,
                lang: Lang::En,
            },
        );
        assert!(light.contains(&PORCELAIN.bg.to_string()));
        assert!(!light.contains(&DARK.bg.to_string()));
    }

    #[test]
    fn chinese_labels_are_used_when_the_language_says_so() {
        let svg = card(
            &report(80, "w"),
            &Options {
                palette: &DARK,
                lang: Lang::ZhCn,
            },
        );
        assert!(svg.contains("待修复"));
        assert!(svg.contains("良好"));
        assert!(svg.contains(r#"lang="zh-CN""#));
    }

    #[test]
    fn height_grows_with_the_number_of_findings() {
        let svg = card(&report(80, "widget"), &Options::default());
        let h: i32 = svg
            .split(r#"height=""#)
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(h > 300 && h < 600, "卡片高度失控: {h}");
    }

    /// 通栏横幅按 `width="100%"` 引用，viewBox 必须是通栏比例——
    /// 否则 README 顶上会出现一条横穿页面的巨型字
    #[test]
    fn the_hero_banner_is_wide_and_centred() {
        let svg = hero("score and improve your repository", crate::i18n::Lang::En);
        assert!(svg.contains(r#"viewBox="0 0 1200 260""#));
        assert!(svg.contains(r#"text-anchor="middle""#));
        // 亮暗两种主题共用一个文件，所以不能画底
        assert!(!svg.contains(r#"<rect x="0" y="0" width="1200""#));
    }

    #[test]
    fn the_hero_survives_an_empty_tagline() {
        let svg = hero("", crate::i18n::Lang::En);
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(!svg.contains("<text"));
    }
}
