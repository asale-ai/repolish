//! 终端报告渲染。
//!
//! 默认只展示总分、类别分、P1/P2 与覆盖限制；`-v` 展开全部检查项。
//!
//! 配色与字形分别在 [`theme`] 与 [`glyph`]，SVG 卡片用的是同两个模块——
//! 终端里长什么样，README 里就长什么样。

pub mod badge;
pub mod cast;
pub mod draw;
pub mod glyph;
pub mod i18n;
pub mod markdown;
pub mod overview;
pub mod svg;
pub mod table;
pub mod theme;

pub use badge::{
    badge_json, snippet, styled_snippet, styled_snippet_html, BADGE_PATH, REPOLISH_URL,
};
pub use cast::{cast, Line, Screen, Span, Step, Timing};
pub use draw::Options;
pub use i18n::Lang;
pub use markdown::{comment, markdown, COMMENT_MARKER};
pub use overview::{has_star_history, overview, Facts, OVERVIEW_PATH};
/// README 顶上那张通栏横幅，由 `artifacts` 阶段画进使用者自己的仓库
pub const HERO_PATH: &str = ".repolish/hero.svg";
pub use svg::{card, CARD_PATH};
pub use table::table;
pub use theme::{ColorLevel, Palette, DARK, PORCELAIN};

use std::fmt::Write as _;

use repolish_core::{Category, Outcome, Report, Severity};
use theme::Rgb;

/// 正文排版宽度。80 列终端里两侧还各留得下一点余量。
pub(crate) const WIDTH: usize = 72;
pub(crate) const INDENT: &str = "  ";

pub struct RenderOptions {
    pub verbose: bool,
    pub level: ColorLevel,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            verbose: false,
            level: ColorLevel::TrueColor,
        }
    }
}

/// 一支按终端能力上色的笔。所有转义序列都从这里出，
/// 免得某处忘了降级，在 16 色终端上吐出一串裸的 truecolor 序列。
pub(crate) struct Pen {
    pub(crate) level: ColorLevel,
}

impl Pen {
    pub(crate) fn ink(&self, text: &str, c: Rgb) -> String {
        format!(
            "{}{text}{}",
            theme::fg(c, self.level),
            theme::reset(self.level)
        )
    }

    pub(crate) fn strong(&self, text: &str, c: Rgb) -> String {
        format!(
            "{}{}{text}{}",
            theme::bold(self.level),
            theme::fg(c, self.level),
            theme::reset(self.level)
        )
    }

    pub(crate) fn dim(&self, text: &str) -> String {
        self.ink(text, theme::MUTED)
    }

    /// 反白色块，用于严重度标签
    fn chip(&self, text: &str, c: Rgb) -> String {
        format!(
            "{}{}{} {text} {}",
            theme::bg(c, self.level),
            theme::fg(theme::INK, self.level),
            theme::bold(self.level),
            theme::reset(self.level)
        )
    }
}

pub fn terminal(report: &Report, opts: &RenderOptions) -> String {
    let pen = Pen { level: opts.level };
    let mut out = String::new();

    banner(&mut out, &pen);
    headline(&mut out, &pen, report);
    score_block(&mut out, &pen, report);
    delta_block(&mut out, &pen, report);
    check_grid(&mut out, &pen, report, opts.verbose);
    findings(&mut out, &pen, report, opts.verbose);
    if opts.verbose {
        passing(&mut out, &pen, report);
    }
    limits(&mut out, &pen, report);
    footer(&mut out, &pen, report);

    out
}

// ── 横幅 ────────────────────────────────────────────────────

/// wordmark 的每一列取一档渐变色：紫 → 粉 → 青。
fn banner(out: &mut String, pen: &Pen) {
    let lines = glyph::blocks("REPOLISH");
    let width = lines.first().map_or(1, |l| l.chars().count()).max(1);
    out.push('\n');
    for line in &lines {
        let mut row = String::from(INDENT);
        for (i, ch) in line.chars().enumerate() {
            if ch == ' ' {
                row.push(' ');
                continue;
            }
            let t = i as f32 / (width - 1).max(1) as f32;
            row.push_str(&pen.ink(&ch.to_string(), theme::sweep(t)));
        }
        let _ = writeln!(out, "{row}");
    }
    let _ = writeln!(
        out,
        "{INDENT}{}",
        pen.dim("discoverability · comprehensibility · credibility")
    );
}

fn headline(out: &mut String, pen: &Pen, report: &Report) {
    let repo = match &report.repository.owner {
        Some(o) => format!("{}/{}", o, report.repository.name),
        None => report.repository.name.clone(),
    };
    let profile = if report.profile.overridden {
        format!("{} (specified)", report.profile.detected.as_str())
    } else {
        format!("{} (detected)", report.profile.detected.as_str())
    };

    let mut meta = vec![profile, report.mode.as_str().to_string()];
    if let Some(commit) = &report.repository.commit {
        meta.push(commit.chars().take(8).collect());
    }

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{INDENT}{}  {}",
        pen.strong(&repo, theme::TEXT),
        pen.dim(&format!("· {}", meta.join(" · ")))
    );
}

// ── 分数 ────────────────────────────────────────────────────

/// 分数行 + 三条类别分。
///
/// 分数用的是普通字，点阵字形只留给横幅上的 wordmark：上特效的是**标识**，
/// 不是数据。一个把数字画成艺术字的报告，读者第一反应是「这数准吗」。
fn score_block(out: &mut String, pen: &Pen, report: &Report) {
    let _ = writeln!(out);

    match report.score {
        Some(s) => {
            let color = theme::band(s);
            let _ = writeln!(
                out,
                "{INDENT}{}   {}   {}   {}",
                pen.dim("SCORE"),
                pen.strong(&pad(&format!("{s} / 100"), 9), color),
                pen.ink(&pad(i18n::band_word(s, &i18n::EN), 9), color),
                meter(pen, s, 24),
            );
        }
        None => {
            let _ = writeln!(
                out,
                "{INDENT}{}   {}",
                pen.dim("SCORE"),
                pen.ink("not scored", theme::AMBER),
            );
        }
    }

    let _ = writeln!(out);
    for cat in Category::ALL {
        let _ = writeln!(
            out,
            "{INDENT}{}",
            category_row(pen, cat, report.category_score(cat))
        );
    }

    // 本地分把三个远程检查项剔出了分母，与远程分不是同一个基准。
    // 不标出来，用户会拿本地分去和别人的远程分横向比较。
    if report.mode == repolish_core::Mode::Local && report.score.is_some() {
        let _ = writeln!(out);
        for line in wrap(
            "local score — description / topics / homepage were not checked, so it is not comparable with a --remote score",
            WIDTH,
        ) {
            let _ = writeln!(out, "{INDENT}{}", pen.dim(&line));
        }
    } else if report.score.is_none() {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{INDENT}{}",
            pen.dim(&format!(
                "only {:.0}% of the registered checks produced a score, below the 50% floor",
                report.coverage * 100.0
            ))
        );
    }
}

/// 与 `--base` 那个 commit 的差异。没给 `--base` 就整块不出现。
///
/// 摆在分数正下方，是因为**变化量比绝对值更该先被读到**：一个正在评审 PR
/// 的人要判断的是「这次改动让它变好还是变坏」，不是「它现在几分」。
fn delta_block(out: &mut String, pen: &Pen, report: &Report) {
    let Some(d) = &report.delta else {
        return;
    };

    let _ = writeln!(out);
    let (word, colour) = match d.points {
        Some(p) if p < 0 => (format!("{p}"), theme::RED),
        Some(p) if p > 0 => (format!("+{p}"), theme::LIME),
        Some(_) => ("no change".to_string(), theme::MUTED),
        None => ("not comparable".to_string(), theme::AMBER),
    };
    let base = match d.base_score {
        Some(b) => format!(
            "{b} → {}",
            report
                .score
                .map(|s| s.to_string())
                .unwrap_or_else(|| "—".into())
        ),
        None => "the baseline produced no total score".to_string(),
    };
    let _ = writeln!(
        out,
        "{INDENT}{}   {}   {}",
        pen.dim("SINCE"),
        pen.strong(&pad(&d.base_ref, 9), theme::TEXT),
        format_args!("{}  {}", pen.strong(&word, colour), pen.dim(&base)),
    );

    if d.checks.is_empty() {
        let _ = writeln!(out, "{INDENT}        {}", pen.dim("no check changed state"));
        return;
    }
    // 只列动了的。22 项全列出来，读者要自己找哪一行变了——那正是这一段
    // 本来想替他省掉的功夫。
    for c in &d.checks {
        let cell = |v: Option<u8>, status: &str| match v {
            Some(n) => format!("{n}"),
            None => status.replace('_', " "),
        };
        let colour = if c.is_regression() {
            theme::RED
        } else {
            theme::LIME
        };
        let _ = writeln!(
            out,
            "{INDENT}        {} {}  {}",
            pen.ink(if c.is_regression() { "▼" } else { "▲" }, colour),
            pen.dim(&pad(c.id, 28)),
            pen.ink(
                &format!(
                    "{} → {}",
                    cell(c.before, c.before_status),
                    cell(c.after, c.after_status)
                ),
                colour
            ),
        );
    }
}

fn category_row(pen: &Pen, cat: Category, score: Option<u8>) -> String {
    let label = pad(&cat.label().to_uppercase(), 19);
    match score {
        Some(s) => format!(
            "{}{}  {}",
            pen.dim(&label),
            meter(pen, s, 12),
            pen.strong(&format!("{s:>3}"), theme::band(s))
        ),
        None => format!(
            "{}{}  {}",
            pen.dim(&label),
            meter(pen, 0, 12),
            pen.dim("  —")
        ),
    }
}

/// 条形图。已填部分按分数上色，未填部分用轨道色——
/// 未填不能用底色，否则在浅色终端上整条会消失。
///
/// 用下半块 `▄` 而不是整块 `█`：整块在字符格里上下都顶满，三条类别分挨着
/// 排就粘成了一整块矩形，反而看不出是三条。
///
/// 空档用更细的 `▁` 而**不是**同一个 `▄`：`--no-color` 下颜色不存在，两头用同
/// 一个字符的话整条就没有信息了——CI 日志里 87 分和 0 分会画得一模一样。
///
/// 向下取整：99 分和 100 分不能长得一模一样，差的那一格就是要给人看的。
/// 但非零分至少留一格，否则 3 分会显示成空条。
fn meter(pen: &Pen, score: u8, width: usize) -> String {
    let filled = if score == 0 {
        0
    } else {
        (score as usize * width / 100).clamp(1, width)
    };
    format!(
        "{}{}",
        pen.ink(&"▄".repeat(filled), theme::band(score)),
        pen.ink(&"▁".repeat(width - filled), theme::TRACK)
    )
}

// ── 检查点阵 ────────────────────────────────────────────────

/// 22 个检查项压成一行圆点：满分青、扣分琥珀、零分红、没打分的灰。
/// 一眼就能看出「这个仓库大面积不行」还是「只差一两项」。
fn check_grid(out: &mut String, pen: &Pen, report: &Report, verbose: bool) {
    if report.checks.is_empty() {
        return;
    }
    let mut dots = String::new();
    let (mut scored, mut na, mut unresolved) = (0, 0, 0);
    for r in &report.checks {
        let (ch, color) = match &r.outcome {
            Outcome::Scored { score: 10, .. } => ('●', theme::CYAN),
            Outcome::Scored { score: 0, .. } => ('●', theme::RED),
            Outcome::Scored { .. } => ('●', theme::AMBER),
            Outcome::NotApplicable { .. } => ('○', theme::LINE),
            _ => ('○', theme::MUTED),
        };
        match &r.outcome {
            Outcome::Scored { .. } => scored += 1,
            Outcome::NotApplicable { .. } => na += 1,
            _ => unresolved += 1,
        }
        dots.push_str(&pen.ink(&ch.to_string(), color));
    }

    let mut tally = vec![format!("{scored} scored")];
    if unresolved > 0 {
        tally.push(format!("{unresolved} not verified"));
    }
    if na > 0 {
        tally.push(format!("{na} not applicable"));
    }

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{INDENT}{}  {dots}   {}",
        pen.dim("CHECKS"),
        pen.dim(&tally.join(" · "))
    );

    if verbose {
        let _ = writeln!(
            out,
            "{INDENT}        {} {}  {} {}  {} {}  {}",
            pen.ink("●", theme::CYAN),
            pen.dim("full"),
            pen.ink("●", theme::AMBER),
            pen.dim("partial"),
            pen.ink("●", theme::RED),
            pen.dim("zero"),
            pen.dim("○ not scored"),
        );
    }
}

// ── 发现 ────────────────────────────────────────────────────

fn findings(out: &mut String, pen: &Pen, report: &Report, verbose: bool) {
    let mut all: Vec<(Severity, &str, &str)> = Vec::new();
    for r in &report.checks {
        for f in r.outcome.fixes() {
            all.push((f.severity, r.id, f.message.as_str()));
        }
    }
    all.sort_by_key(|(s, id, _)| (*s, *id));

    let shown: Vec<_> = if verbose {
        all.iter().collect()
    } else {
        all.iter().filter(|(s, _, _)| *s != Severity::P3).collect()
    };
    let hidden = if verbose {
        0
    } else {
        all.iter().filter(|(s, _, _)| *s == Severity::P3).count()
    };
    // 全部发现都是 P3 时 shown 为空，但「还有 N 条建议」这行仍然得出，
    // 否则默认输出会让人以为一条问题都没有
    if shown.is_empty() && hidden == 0 {
        return;
    }

    rule(out, pen, "TO FIX");
    for (sev, id, msg) in &shown {
        let (label, color) = match sev {
            Severity::P1 => ("P1", theme::RED),
            Severity::P2 => ("P2", theme::AMBER),
            Severity::P3 => ("P3", theme::PURPLE),
        };
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{INDENT}{} {}",
            pen.chip(label, color),
            pen.strong(id, theme::TEXT)
        );
        for line in wrap(msg, WIDTH - 5) {
            let _ = writeln!(out, "{INDENT}     {line}");
        }

        if let Some(r) = report.checks.iter().find(|r| r.id == *id) {
            for e in r.outcome.evidence() {
                let loc = match e.line {
                    Some(l) => format!("{}:{}", e.file.display(), l),
                    None => e.file.display().to_string(),
                };
                // 路径要能被终端识别成可点击的链接，绝不能被折断；
                // 说明接在后面，放不下就换行挂到路径下方
                let head = format!("{INDENT}     {} {}  ", "└", loc);
                let room = WIDTH.saturating_sub(display_width(&head).saturating_sub(2));
                let mut note = wrap(&e.note, room).into_iter();
                let _ = writeln!(
                    out,
                    "{INDENT}     {} {}  {}",
                    pen.dim("└"),
                    pen.ink(&loc, theme::CYAN),
                    pen.dim(note.next().unwrap_or_default().as_str())
                );
                for rest in note {
                    let _ = writeln!(out, "{INDENT}       {}", pen.dim(&rest));
                }
            }
        }
    }

    if hidden > 0 {
        let (noun, it) = if hidden == 1 {
            ("suggestion", "it")
        } else {
            ("suggestions", "them")
        };
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{INDENT}     {}",
            pen.dim(&format!(
                "{hidden} P3 {noun} not shown — run with -v to see {it}"
            ))
        );
    }
}

fn passing(out: &mut String, pen: &Pen, report: &Report) {
    let passed: Vec<&str> = report
        .checks
        .iter()
        .filter(|r| r.outcome.score() == Some(10))
        .map(|r| r.id)
        .collect();
    if passed.is_empty() {
        return;
    }
    rule(out, pen, "PASSING");
    let _ = writeln!(out);
    // 两列排布：22 项一列一列往下铺会把发现挤出一屏
    let col = passed.iter().map(|s| s.len()).max().unwrap_or(0) + 4;
    for pair in passed.chunks(2) {
        let mut row = String::new();
        for id in pair {
            let _ = write!(row, "{} {}", pen.ink("✓", theme::CYAN), pad(id, col));
        }
        let _ = writeln!(out, "{INDENT}   {}", row.trim_end());
    }
}

fn limits(out: &mut String, pen: &Pen, report: &Report) {
    let na: Vec<&str> = report
        .checks
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::NotApplicable { .. }))
        .map(|r| r.id)
        .collect();
    if report.coverage_limits.is_empty() && na.is_empty() {
        return;
    }

    rule(out, pen, "NOT VERIFIED");
    let _ = writeln!(out);
    for limit in &report.coverage_limits {
        bullet(out, pen, limit);
    }
    if !na.is_empty() {
        bullet(
            out,
            pen,
            &format!(
                "not applicable to {} projects: {}",
                report.profile.detected.as_str(),
                na.join(", ")
            ),
        );
    }
}

/// 一条带项目符号的说明，超宽时折行并悬挂缩进对齐到文字。
///
/// 这些文案里嵌着检查项自己写的原因（浅克隆的 tag 情况能写到两百列），
/// 不折行的话终端会自己硬折在任意一个字符上。
fn bullet(out: &mut String, pen: &Pen, text: &str) {
    for (i, line) in wrap(text, WIDTH - 5).into_iter().enumerate() {
        if i == 0 {
            let _ = writeln!(out, "{INDENT}   {} {}", pen.dim("·"), pen.dim(&line));
        } else {
            let _ = writeln!(out, "{INDENT}     {}", pen.dim(&line));
        }
    }
}

fn footer(out: &mut String, pen: &Pen, report: &Report) {
    let _ = writeln!(out);
    let _ = writeln!(out, "{INDENT}{}", pen.ink(&"─".repeat(WIDTH), theme::LINE));
    let _ = writeln!(
        out,
        "{INDENT}{}",
        pen.dim(&format!(
            "repolish v{} · {} checks · scoring is deterministic, no model involved",
            report.repolish_version,
            report.checks.len()
        ))
    );
    out.push('\n');
}

/// 分节线：`── LABEL ─────────`
pub(crate) fn rule(out: &mut String, pen: &Pen, label: &str) {
    let dashes = WIDTH.saturating_sub(label.chars().count() + 4);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{INDENT}{} {} {}",
        pen.ink("──", theme::LINE),
        pen.strong(label, theme::PINK),
        pen.ink(&"─".repeat(dashes), theme::LINE)
    );
}

/// 按词折行。检查项的建议文案长度不受这里控制，硬折会把 `--min-score`
/// 这类参数从中间劈开，所以只在空格处断。单词本身超宽时整个留在一行。
///
/// 按**显示宽度**算而不是字符数：证据里会出现中文（README 标题就是原样带出来的），
/// 一个汉字占两格，按字符数折出来的行会有两倍宽。
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && display_width(&line) + 1 + display_width(word) > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// 按终端显示宽度补齐（CJK 字符占两格），format! 的 width 按字符数算，会错位。
pub(crate) fn pad(s: &str, width: usize) -> String {
    format!("{s}{}", " ".repeat(width.saturating_sub(display_width(s))))
}

/// 终端列数。CJK 与全角标点占两格。
pub(crate) fn display_width(s: &str) -> usize {
    s.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repolish_core::{CheckResult, Mode, Profile, ProfileInfo, Repository, Risk};

    fn report(score: u8) -> Report {
        let checks = vec![CheckResult {
            id: "license",
            category: Category::Credibility,
            risk: Risk::Critical,
            outcome: Outcome::Scored {
                score: score / 10,
                evidence: vec![],
                fixes: vec![],
            },
        }];
        Report::build(
            checks,
            Repository {
                owner: Some("acme".into()),
                name: "widget".into(),
                commit: Some("deadbeefcafe".into()),
            },
            ProfileInfo {
                detected: Profile::Cli,
                overridden: false,
            },
            Mode::Remote,
        )
    }

    /// 关色之后不能剩下任何转义序列——重定向到文件的输出要能直接读
    #[test]
    fn no_color_output_is_free_of_escapes() {
        let out = terminal(
            &report(80),
            &RenderOptions {
                verbose: true,
                level: ColorLevel::None,
            },
        );
        assert!(!out.contains('\x1b'), "关色后仍有转义序列");
    }

    #[test]
    fn truecolor_output_carries_the_brand_purple() {
        let out = terminal(&report(80), &RenderOptions::default());
        assert!(out.contains(&theme::fg(theme::PURPLE, ColorLevel::TrueColor)));
    }

    /// 横幅、分数、类别、页脚四段都要在
    #[test]
    fn plain_output_contains_every_section() {
        let out = terminal(
            &report(80),
            &RenderOptions {
                verbose: false,
                level: ColorLevel::None,
            },
        );
        assert!(out.contains("/ 100"));
        assert!(out.contains("good"));
        assert!(out.contains("DISCOVERABILITY"));
        assert!(out.contains("CREDIBILITY"));
        assert!(out.contains("acme/widget"));
        assert!(out.contains("deadbeef"), "commit 应截断到 8 位");
        assert!(!out.contains("deadbeefcafe"));
        assert!(out.contains("no model involved"));
    }

    #[test]
    fn meter_fills_proportionally_and_never_overflows() {
        let pen = Pen {
            level: ColorLevel::None,
        };
        // 关色之后颜色不存在，填充与空档必须靠字符本身区分
        assert_eq!(meter(&pen, 0, 10).chars().filter(|&c| c == '▄').count(), 0);
        assert_eq!(meter(&pen, 0, 10).chars().filter(|&c| c == '▁').count(), 10);
        assert_eq!(
            meter(&pen, 100, 10).chars().filter(|&c| c == '▄').count(),
            10
        );
        assert_eq!(meter(&pen, 50, 10).chars().filter(|&c| c == '▄').count(), 5);
        assert_eq!(meter(&pen, 55, 10).chars().count(), 10);
    }
}
