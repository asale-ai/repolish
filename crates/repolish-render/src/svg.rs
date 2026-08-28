//! `.repolish/card.svg` —— 可以直接贴进 README 的报告卡片。
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
//! - **深底恒定。** 不做 `prefers-color-scheme` 切换：GitHub 把 SVG 当图片经
//!   camo 代理渲染，媒体查询在那条链路上并不可靠。霓虹配色在亮暗两种页面
//!   底色上都立得住。

use std::fmt::Write as _;

use repolish_core::{Category, Outcome, Report, Severity};

use crate::glyph;
use crate::theme::{self, Rgb};

/// 卡片在用户仓库中的位置
pub const CARD_PATH: &str = ".repolish/card.svg";

const W: i32 = 880;
const PAD: i32 = 36;
/// 右栏（类别条、页脚右侧）的对齐右边界
const RIGHT: i32 = W - PAD;
/// 卡片里最多列几条发现。再多就该去看终端输出了，塞满的卡片没人读。
const MAX_FINDINGS: usize = 3;
/// 类别条的段数，与终端的 `meter` 取同一个值
const SEGMENTS: i32 = 12;
const SEG_W: i32 = 22;
const SEG_GAP: i32 = 6;
const BAR_W: i32 = SEGMENTS * SEG_W + (SEGMENTS - 1) * SEG_GAP;

pub fn card(report: &Report) -> String {
    let mut body = String::new();
    let mut y = PAD;

    y = header(&mut body, report, y);
    y = divider(&mut body, y + 18);
    y = score_block(&mut body, report, y + 30);
    y = checks_row(&mut body, report, y + 34);
    y = findings(&mut body, report, y + 30);
    let height = footer(&mut body, report, y + 20);

    document(&body, height)
}

// ── 外壳 ────────────────────────────────────────────────────

fn document(body: &str, height: i32) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{height}" viewBox="0 0 {W} {height}" role="img" aria-label="repolish report card">"#
    );
    s.push_str(&brand_defs());
    let _ = writeln!(
        s,
        r#"  <style>
    .t {{ font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace; }}
  </style>"#
    );
    let _ = writeln!(
        s,
        r#"  <rect x="0" y="0" width="{W}" height="{height}" rx="18" fill="{}"/>"#,
        theme::INK
    );
    let _ = writeln!(
        s,
        r#"  <rect x="0.5" y="0.5" width="{}" height="{}" rx="17.5" fill="none" stroke="{}"/>"#,
        W - 1,
        height - 1,
        theme::LINE
    );
    s.push_str(body);
    let _ = writeln!(s, "</svg>");
    s
}

// ── 页眉 ────────────────────────────────────────────────────

fn header(out: &mut String, report: &Report, y: i32) -> i32 {
    let size = 30;
    out.push_str(&mark(PAD, y, size));

    // wordmark 与 logo 同源：点阵 → 矩形，落到哪台机器上都是同一个形状
    let cell = 4;
    out.push_str(&blocks("REPOLISH", PAD + size + 14, y + 1, cell));

    let meta = format!(
        "{} · {}",
        report.profile.detected.as_str(),
        report.mode.as_str()
    );
    out.push_str(&text(
        &meta,
        RIGHT,
        y + size / 2 + 5,
        13,
        theme::MUTED,
        Anchor::End,
        false,
    ));

    y + size
}

/// `mark()` 引用的渐变定义。凡是画了标记的文档都得先带上这一段。
fn brand_defs() -> String {
    format!(
        "  <defs>\n    <linearGradient id=\"brand\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\n      \
         <stop offset=\"0\" stop-color=\"{}\"/>\n      \
         <stop offset=\"0.55\" stop-color=\"{}\"/>\n      \
         <stop offset=\"1\" stop-color=\"{}\"/>\n    </linearGradient>\n  </defs>\n",
        theme::PURPLE,
        theme::PINK,
        theme::CYAN
    )
}

/// 只有标记的方形 logo，可直接当文件用（favicon、README 头像）。
pub fn logo(size: i32) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" \
         viewBox=\"0 0 {size} {size}\" role=\"img\" aria-label=\"repolish\">\n{}{}</svg>\n",
        brand_defs(),
        mark(0, 0, size)
    )
}

/// 横版 logo：标记 + wordmark，**背景透明**——README 在亮暗两种主题下
/// 用的是同一个文件，画一层底色就必然在其中一种下露出方块。
pub fn wordmark(mark_size: i32) -> String {
    let cell = (mark_size / glyph::H as i32).max(1);
    let text_x = mark_size + mark_size / 3;
    let text_h = cell * glyph::H as i32;
    let width = text_x + glyph::blocks_width("REPOLISH") as i32 * cell;
    let height = mark_size.max(text_h);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" \
         viewBox=\"0 0 {width} {height}\" role=\"img\" aria-label=\"repolish\">\n{}{}{}</svg>\n",
        brand_defs(),
        mark(0, (height - mark_size) / 2, mark_size),
        blocks("REPOLISH", text_x, (height - text_h) / 2, cell)
    )
}

/// 品牌标记：渐变圆角块 + 一大一小两颗「打磨」的星芒。
/// 纯几何，不依赖字体，16px 下也认得出来。
pub fn mark(x: i32, y: i32, size: i32) -> String {
    let s = size as f32;
    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"  <rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{:.1}" fill="url(#brand)"/>"#,
        s * 0.26
    );
    let _ = writeln!(
        out,
        r#"  <path d="{}" fill="{}"/>"#,
        sparkle(x as f32 + s * 0.44, y as f32 + s * 0.56, s * 0.34),
        theme::INK
    );
    let _ = writeln!(
        out,
        r#"  <path d="{}" fill="{}"/>"#,
        sparkle(x as f32 + s * 0.76, y as f32 + s * 0.26, s * 0.16),
        theme::INK
    );
    out
}

/// 四角星芒。凹边用二次贝塞尔，`waist` 越小尖角越利。
fn sparkle(cx: f32, cy: f32, r: f32) -> String {
    let w = r * 0.2;
    format!(
        "M{:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Z",
        cx, cy - r,
        cx + w, cy - w, cx + r, cy,
        cx + w, cy + w, cx, cy + r,
        cx - w, cy + w, cx - r, cy,
        cx - w, cy - w, cx, cy - r,
    )
}

// ── 分数 ────────────────────────────────────────────────────

fn score_block(out: &mut String, report: &Report, y: i32) -> i32 {
    let (digits, color) = match report.score {
        Some(s) => (s.to_string(), theme::band(s)),
        None => ("--".to_string(), theme::MUTED),
    };

    // 分数是数据，不是标识：用普通字排大号即可。点阵字形只留给 wordmark——
    // 数字换一副等宽字无非是宽了几像素，logo 换一副面孔就不是同一个 logo 了。
    out.push_str(&text(&digits, PAD, y + 56, 64, color, Anchor::Start, true));

    let verdict = match report.score {
        Some(s) => format!("/ 100  ·  {}", theme::band_word(s)),
        None => "not scored".to_string(),
    };
    out.push_str(&text(
        &verdict,
        PAD,
        y + 82,
        13,
        theme::MUTED,
        Anchor::Start,
        false,
    ));

    let col = PAD + 200;
    let bar_x = RIGHT - 42 - BAR_W;
    for (i, cat) in Category::ALL.iter().enumerate() {
        let row = y + 26 + i as i32 * 28;
        out.push_str(&text(
            &cat.label().to_uppercase(),
            col,
            row,
            11,
            theme::MUTED,
            Anchor::Start,
            false,
        ));
        let score = report.category_score(*cat);
        out.push_str(&bar(bar_x, row - 9, score.unwrap_or(0)));
        match score {
            Some(s) => out.push_str(&text(
                &s.to_string(),
                RIGHT,
                row,
                13,
                theme::band(s),
                Anchor::End,
                true,
            )),
            None => out.push_str(&text("—", RIGHT, row, 13, theme::MUTED, Anchor::End, false)),
        }
    }

    y + 88
}

/// 一条贯穿正文宽度的分隔线，返回它所在的 y
fn divider(out: &mut String, y: i32) -> i32 {
    let _ = writeln!(
        out,
        r#"  <rect x="{PAD}" y="{y}" width="{}" height="1" fill="{}"/>"#,
        RIGHT - PAD,
        theme::LINE
    );
    y
}

/// 分段条形图。段数与终端一致——同一个仓库在两个地方必须长出同一根条，
/// 否则「终端里少一格、卡片上却填满」会被当成两套算法。
///
/// 连续条在高分区是没有信息的：99 和 100 差 3.5 个像素，谁也看不出来。
/// 切成 12 段之后，那一格空缺就是给人看的。
fn bar(x: i32, y: i32, score: u8) -> String {
    let filled = if score == 0 {
        0
    } else {
        (score as i32 * SEGMENTS / 100).clamp(1, SEGMENTS)
    };
    let mut out = String::new();
    for i in 0..SEGMENTS {
        let fill = if i < filled {
            theme::band(score)
        } else {
            theme::TRACK
        };
        let _ = writeln!(
            out,
            r#"  <rect x="{}" y="{y}" width="{SEG_W}" height="9" rx="2" fill="{fill}"/>"#,
            x + i * (SEG_W + SEG_GAP)
        );
    }
    out
}

// ── 检查点阵 ────────────────────────────────────────────────

fn checks_row(out: &mut String, report: &Report, y: i32) -> i32 {
    if report.checks.is_empty() {
        return y;
    }
    out.push_str(&text(
        "CHECKS",
        PAD,
        y + 4,
        11,
        theme::MUTED,
        Anchor::Start,
        false,
    ));

    let (mut scored, mut na, mut unresolved) = (0, 0, 0);
    let x0 = PAD + 72;
    for (i, r) in report.checks.iter().enumerate() {
        let color = match &r.outcome {
            Outcome::Scored { score: 10, .. } => {
                scored += 1;
                theme::CYAN
            }
            Outcome::Scored { score: 0, .. } => {
                scored += 1;
                theme::RED
            }
            Outcome::Scored { .. } => {
                scored += 1;
                theme::AMBER
            }
            Outcome::NotApplicable { .. } => {
                na += 1;
                theme::LINE
            }
            _ => {
                unresolved += 1;
                theme::MUTED
            }
        };
        let _ = writeln!(
            out,
            r#"  <circle cx="{}" cy="{y}" r="5" fill="{color}"/>"#,
            x0 + i as i32 * 16
        );
    }

    let mut tally = vec![format!("{scored} scored")];
    if unresolved > 0 {
        tally.push(format!("{unresolved} not verified"));
    }
    if na > 0 {
        tally.push(format!("{na} not applicable"));
    }
    out.push_str(&text(
        &tally.join(" · "),
        RIGHT,
        y + 4,
        12,
        theme::MUTED,
        Anchor::End,
        false,
    ));

    y + 10
}

// ── 发现 ────────────────────────────────────────────────────

fn findings(out: &mut String, report: &Report, y: i32) -> i32 {
    let mut all: Vec<(Severity, &str, &str)> = Vec::new();
    for r in &report.checks {
        for f in r.outcome.fixes() {
            all.push((f.severity, r.id, f.message.as_str()));
        }
    }
    all.sort_by_key(|(s, id, _)| (*s, *id));
    if all.is_empty() {
        return y;
    }

    let mut cursor = y;
    let _ = writeln!(
        out,
        r#"  <rect x="{PAD}" y="{cursor}" width="{}" height="1" fill="{}"/>"#,
        RIGHT - PAD,
        theme::LINE
    );
    cursor += 22;
    out.push_str(&text(
        "TO FIX",
        PAD,
        cursor,
        11,
        theme::PINK,
        Anchor::Start,
        true,
    ));
    cursor += 24;

    for (sev, id, msg) in all.iter().take(MAX_FINDINGS) {
        let (label, color) = match sev {
            Severity::P1 => ("P1", theme::RED),
            Severity::P2 => ("P2", theme::AMBER),
            Severity::P3 => ("P3", theme::PURPLE),
        };
        let _ = writeln!(
            out,
            r#"  <rect x="{PAD}" y="{}" width="26" height="18" rx="5" fill="{color}"/>"#,
            cursor - 13
        );
        out.push_str(&text(
            label,
            PAD + 13,
            cursor,
            11,
            theme::INK,
            Anchor::Middle,
            true,
        ));
        out.push_str(&text(
            id,
            PAD + 38,
            cursor,
            13,
            theme::TEXT,
            Anchor::Start,
            true,
        ));
        // 卡片是固定宽度的图片，没有换行可用——过长的建议在这里截断，
        // 完整文案在终端和 REPOLISH.md 里
        out.push_str(&text(
            &clip(msg, 96),
            PAD + 38,
            cursor + 18,
            12,
            theme::MUTED,
            Anchor::Start,
            false,
        ));
        cursor += 46;
    }

    if all.len() > MAX_FINDINGS {
        out.push_str(&text(
            &format!("+{} more — run repolish check", all.len() - MAX_FINDINGS),
            PAD + 38,
            cursor - 8,
            12,
            theme::MUTED,
            Anchor::Start,
            false,
        ));
        cursor += 10;
    }

    cursor - 18
}

fn footer(out: &mut String, report: &Report, y: i32) -> i32 {
    let _ = writeln!(
        out,
        r#"  <rect x="{PAD}" y="{y}" width="{}" height="1" fill="{}"/>"#,
        RIGHT - PAD,
        theme::LINE
    );
    let row = y + 24;

    let mut left = match &report.repository.owner {
        Some(o) => format!("{}/{}", o, report.repository.name),
        None => report.repository.name.clone(),
    };
    if let Some(commit) = &report.repository.commit {
        let short: String = commit.chars().take(8).collect();
        left.push_str(&format!(" · {short}"));
    }
    out.push_str(&text(
        &left,
        PAD,
        row,
        12,
        theme::MUTED,
        Anchor::Start,
        false,
    ));
    out.push_str(&text(
        &format!("repolish v{}", report.repolish_version),
        RIGHT,
        row,
        12,
        theme::MUTED,
        Anchor::End,
        false,
    ));

    row + PAD - 8
}

// ── 基元 ────────────────────────────────────────────────────

enum Anchor {
    Start,
    Middle,
    End,
}

impl Anchor {
    fn as_str(&self) -> &'static str {
        match self {
            Anchor::Start => "start",
            Anchor::Middle => "middle",
            Anchor::End => "end",
        }
    }
}

fn text(s: &str, x: i32, y: i32, size: i32, fill: Rgb, anchor: Anchor, bold: bool) -> String {
    let weight = if bold { r#" font-weight="700""# } else { "" };
    format!(
        "  <text class=\"t\" x=\"{x}\" y=\"{y}\" font-size=\"{size}\" fill=\"{fill}\" text-anchor=\"{}\"{weight}>{}</text>\n",
        anchor.as_str(),
        esc(s)
    )
}

/// wordmark 的点阵 → 矩形。一个亮点一个方块，圆角一点点，像素味不会太硬。
///
/// 只有 wordmark 走这条路。分数、标签、文案一律是普通文本——点阵是**标识**的
/// 处理方式，不是数据的：数字换一副等宽字无非是宽了几像素，logo 换一副面孔
/// 就不是同一个 logo 了。
///
/// 渐变按**列**上色而不是挂一个 `linearGradient`：objectBoundingBox 的渐变是
/// 相对每个引用它的元素算的，几百个小方块每个都会取满一整条渐变，
/// 结果就是整段 wordmark 一个颜色。
fn blocks(s: &str, x: i32, y: i32, cell: i32) -> String {
    let bm = glyph::bitmap(s);
    let mut out = String::new();
    let rx = (cell / 8).max(1);

    for col in 0..bm.width {
        let rows: Vec<usize> = (0..glyph::H).filter(|&row| bm.bits[row][col]).collect();
        if rows.is_empty() {
            continue;
        }
        let t = col as f32 / (bm.width.saturating_sub(1)).max(1) as f32;
        let _ = write!(out, r#"  <g fill="{}">"#, theme::sweep(t));
        for row in rows {
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{cell}" height="{cell}" rx="{rx}"/>"#,
                x + col as i32 * cell,
                y + row as i32 * cell,
            );
        }
        let _ = writeln!(out, "</g>");
    }
    out
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max - 1).collect();
    format!("{}…", head.trim_end())
}

/// SVG 是 XML：仓库名、检查项文案里的 `&` `<` 必须转义，否则整张卡片解析失败。
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let svg = card(&report(80, "widget"));
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(!svg.contains("http://www.w3.org/1999/xlink"));
        assert!(!svg.contains("<image"));
        assert!(!svg.contains("@import"));
        // 唯一允许出现的 http 是 SVG 自己的命名空间
        assert_eq!(svg.matches("http").count(), 1);
    }

    /// 每次 CI 都提交一张只有噪声在变的卡片是不能接受的
    #[test]
    fn the_same_report_renders_byte_identical_svg() {
        assert_eq!(card(&report(80, "widget")), card(&report(80, "widget")));
    }

    #[test]
    fn repository_names_are_xml_escaped() {
        let svg = card(&report(80, "a<b&c"));
        assert!(svg.contains("a&lt;b&amp;c"));
        assert!(!svg.contains("a<b&c"));
    }

    #[test]
    fn score_band_drives_the_fill_colour() {
        assert!(card(&report(100, "w")).contains(&theme::CYAN.to_string()));
        assert!(card(&report(30, "w")).contains(&theme::RED.to_string()));
    }

    #[test]
    fn long_messages_are_clipped_rather_than_overflowing_the_card() {
        assert_eq!(clip("short", 96), "short");
        let long = "x".repeat(200);
        let out = clip(&long, 96);
        assert_eq!(out.chars().count(), 96);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn height_grows_with_the_number_of_findings() {
        let svg = card(&report(80, "widget"));
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
}
