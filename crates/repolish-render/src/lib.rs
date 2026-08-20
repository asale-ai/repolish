//! 终端报告渲染。
//!
//! 默认只展示总分、类别分、P1/P2 与覆盖限制；`-v` 展开全部检查项。

pub mod badge;
pub mod markdown;

pub use badge::{badge_json, snippet, BADGE_PATH, REPOLISH_URL};
pub use markdown::markdown;

use std::fmt::Write as _;

use owo_colors::OwoColorize;
use repolish_core::{Category, Outcome, Report, Severity};

pub struct RenderOptions {
    pub verbose: bool,
    pub color: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            verbose: false,
            color: true,
        }
    }
}

pub fn terminal(report: &Report, opts: &RenderOptions) -> String {
    let mut out = String::new();
    let c = opts.color;

    // ── 标题行 ────────────────────────────────────────────
    let profile_note = if report.profile.overridden {
        format!("{} (specified)", report.profile.detected.as_str())
    } else {
        format!("{} (detected)", report.profile.detected.as_str())
    };
    let _ = writeln!(
        out,
        "\n  {}  ·  profile: {}  ·  mode: {}",
        paint("repolish", c, Paint::Bold),
        profile_note,
        report.mode.as_str()
    );

    // ── 总分 ──────────────────────────────────────────────
    match report.score {
        Some(s) => {
            let bar = meter(s);
            let _ = writeln!(
                out,
                "\n  {}   {}   {}",
                paint("Score", c, Paint::Dim),
                paint_score(&format!("{s:>3}/100"), s, c),
                paint(&bar, c, Paint::Dim),
            );
        }
        None => {
            let _ = writeln!(
                out,
                "\n  {}  only {:.0}% of the registered checks produced a score, below the 50% floor — no total",
                paint("Not scored", c, Paint::Yellow),
                report.coverage * 100.0
            );
        }
    }

    // 本地分把三个远程检查项剔出了分母，与远程分不是同一个基准。
    // 不标出来，用户会拿本地分去和别人的远程分横向比较。
    if report.mode == repolish_core::Mode::Local && report.score.is_some() {
        let _ = writeln!(
            out,
            "  {}",
            paint(
                "         Local score: description / topics / homepage were not checked, so this is not comparable with a --remote score",
                c,
                Paint::Dim
            )
        );
    }

    // ── 类别分 ────────────────────────────────────────────
    let _ = writeln!(out);
    for cat in Category::ALL {
        let label = cat.label();
        match report.category_score(cat) {
            Some(s) => {
                let _ = writeln!(
                    out,
                    "    {}{}   {}",
                    pad(label, 20),
                    paint_score(&format!("{s:>3}"), s, c),
                    paint(&meter(s), c, Paint::Dim)
                );
            }
            None => {
                let _ = writeln!(out, "    {}{}", pad(label, 20), paint("  —", c, Paint::Dim));
            }
        }
    }

    // ── 发现 ──────────────────────────────────────────────
    let mut findings: Vec<(Severity, &str, &str)> = Vec::new();
    for r in &report.checks {
        for f in r.outcome.fixes() {
            findings.push((f.severity, r.id, f.message.as_str()));
        }
    }
    findings.sort_by_key(|(s, id, _)| (*s, *id));

    let shown: Vec<_> = if opts.verbose {
        findings.iter().collect()
    } else {
        findings
            .iter()
            .filter(|(s, _, _)| *s != Severity::P3)
            .collect()
    };

    if !shown.is_empty() {
        let _ = writeln!(out, "\n  {}", paint("To fix", c, Paint::Bold));
        for (sev, id, msg) in &shown {
            let tag = match sev {
                Severity::P1 => paint("P1", c, Paint::Red),
                Severity::P2 => paint("P2", c, Paint::Yellow),
                Severity::P3 => paint("P3", c, Paint::Dim),
            };
            let _ = writeln!(out, "\n    {tag} {}", paint(id, c, Paint::Bold));
            let _ = writeln!(out, "       {msg}");

            if let Some(r) = report.checks.iter().find(|r| r.id == *id) {
                for e in r.outcome.evidence() {
                    let loc = match e.line {
                        Some(l) => format!("{}:{}", e.file.display(), l),
                        None => e.file.display().to_string(),
                    };
                    let _ = writeln!(
                        out,
                        "       {} {}  {}",
                        paint("└", c, Paint::Dim),
                        paint(&loc, c, Paint::Cyan),
                        paint(&e.note, c, Paint::Dim)
                    );
                }
            }
        }
        if !opts.verbose && findings.iter().any(|(s, _, _)| *s == Severity::P3) {
            let n = findings
                .iter()
                .filter(|(s, _, _)| *s == Severity::P3)
                .count();
            let plural = if n == 1 { "suggestion" } else { "suggestions" };
            let _ = writeln!(
                out,
                "\n    {}",
                paint(
                    &format!("{n} more P3 {plural} — run with -v to see them"),
                    c,
                    Paint::Dim
                )
            );
        }
    }

    // ── 通过项 ────────────────────────────────────────────
    if opts.verbose {
        let passed: Vec<_> = report
            .checks
            .iter()
            .filter(|r| r.outcome.score() == Some(10))
            .collect();
        if !passed.is_empty() {
            let _ = writeln!(out, "\n  {}", paint("Passing", c, Paint::Bold));
            for r in passed {
                let _ = writeln!(out, "    {} {}", paint("✓", c, Paint::Green), r.id);
            }
        }
    }

    // ── 覆盖限制 ──────────────────────────────────────────
    if !report.coverage_limits.is_empty() {
        let _ = writeln!(out, "\n  {}", paint("Coverage limits", c, Paint::Bold));
        for limit in &report.coverage_limits {
            let _ = writeln!(
                out,
                "    {} {}",
                paint("·", c, Paint::Dim),
                paint(limit, c, Paint::Dim)
            );
        }
    }

    // ── 不适用项 ──────────────────────────────────────────
    let na: Vec<&str> = report
        .checks
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::NotApplicable { .. }))
        .map(|r| r.id)
        .collect();
    if !na.is_empty() {
        let _ = writeln!(
            out,
            "\n  {}",
            paint(
                &format!(
                    "not applicable to {} projects, excluded: {}",
                    report.profile.detected.as_str(),
                    na.join(", ")
                ),
                c,
                Paint::Dim
            )
        );
    }

    out.push('\n');
    out
}

/// 按终端显示宽度补齐（CJK 字符占两格），format! 的 width 按字符数算，会错位。
fn pad(s: &str, width: usize) -> String {
    let w: usize = s.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum();
    format!("{s}{}", " ".repeat(width.saturating_sub(w)))
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6)
}

fn meter(score: u8) -> String {
    let filled = (score as usize).div_ceil(10);
    let mut s = String::new();
    for i in 0..10 {
        s.push(if i < filled { '█' } else { '░' });
    }
    s
}

enum Paint {
    Bold,
    Dim,
    Red,
    Yellow,
    Green,
    Cyan,
}

fn paint(s: &str, color: bool, p: Paint) -> String {
    if !color {
        return s.to_string();
    }
    match p {
        Paint::Bold => s.bold().to_string(),
        Paint::Dim => s.dimmed().to_string(),
        Paint::Red => s.red().to_string(),
        Paint::Yellow => s.yellow().to_string(),
        Paint::Green => s.green().to_string(),
        Paint::Cyan => s.cyan().to_string(),
    }
}

fn paint_score(s: &str, score: u8, color: bool) -> String {
    if !color {
        return s.to_string();
    }
    match score {
        90..=100 => s.bright_green().bold().to_string(),
        75..=89 => s.green().bold().to_string(),
        60..=74 => s.yellow().bold().to_string(),
        40..=59 => s.bright_red().bold().to_string(),
        _ => s.red().bold().to_string(),
    }
}
