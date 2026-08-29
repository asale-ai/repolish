//! `REPOLISH.md`：提交进仓库、给人读的那一份报告。
//!
//! 结构固定（docs/02-cli-design.zh-CN.md）：总分与类别分 → 分级发现 → 已验证 → 覆盖限制 → 页脚。
//! 顺序对同一 commit 必须稳定，否则每次 CI 跑完都会产生一个无意义的 diff。
//!
//! 框架文案用英文，与主 README 一致。**检查项自身的建议文案目前仍是中文**，
//! 因此当前产出是混合语言的——这一项待整体 i18n 决定后统一。

use std::fmt::Write as _;

use repolish_core::{Category, Outcome, Report, Severity};

use crate::badge::REPOLISH_URL;

pub fn markdown(report: &Report) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "# Repository health\n");
    write_summary(&mut out, report);
    write_delta(&mut out, report, "##");
    write_findings(&mut out, report);
    write_verified(&mut out, report);
    write_limits(&mut out, report);
    write_not_applicable(&mut out, report);
    write_footer(&mut out, report);

    out
}

fn write_summary(out: &mut String, report: &Report) {
    let profile = report.profile.detected.as_str();
    let how = if report.profile.overridden {
        "specified"
    } else {
        "detected"
    };

    match report.score {
        Some(s) => {
            let _ = writeln!(
                out,
                "**{s} / 100** — `{profile}` ({how}) · {} mode\n",
                report.mode.as_str()
            );
        }
        None => {
            let _ = writeln!(
                out,
                "**No total score.** Only {:.0}% of the registered check weight could be \
                 scored, which is below the 50% floor — a number built on that little \
                 evidence would be misleading.\n",
                report.coverage * 100.0
            );
            let _ = writeln!(
                out,
                "Profile: `{profile}` ({how}) · {} mode\n",
                report.mode.as_str()
            );
        }
    }

    let _ = writeln!(out, "| Category | Score |");
    let _ = writeln!(out, "| --- | ---: |");
    for cat in Category::ALL {
        let score = report
            .category_score(cat)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "—".to_string());
        let _ = writeln!(out, "| {} | {score} |", cat.label());
    }
    out.push('\n');

    if report.mode == repolish_core::Mode::Local {
        let _ = writeln!(
            out,
            "> **This is a local score.** `repo-description`, `repo-topics` and \
             `repo-homepage` were not checked, so they are excluded from the denominator. \
             It is not comparable with a score produced by `--remote`.\n"
        );
    }
}

/// 一个 PR 评论里能容下的全部内容。
///
/// 与 `markdown()` 分开，是因为两者的读者不同：`REPOLISH.md` 是提交进仓库的
/// 存档，读它的人想看全貌；PR 评论是给正在评审的人看的，他只想知道
/// **这次改动动了什么**。把整份报告贴进 PR，结果是没人读。
///
/// 第一行是一个 HTML 注释标记，让 CI 能找到上一条评论**改写**它，
/// 而不是每次推送都新增一条。
pub const COMMENT_MARKER: &str = "<!-- repolish-report -->";

pub fn comment(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{COMMENT_MARKER}");

    match (&report.delta, report.score) {
        (Some(d), Some(score)) => {
            let points = d.points.unwrap_or(0);
            let arrow = if points > 0 {
                "▲"
            } else if points < 0 {
                "▼"
            } else {
                "="
            };
            let _ = writeln!(
                out,
                "### repolish {score} / 100 &nbsp; {arrow} {points:+} vs `{}`\n",
                d.base_ref
            );
        }
        (_, Some(score)) => {
            let _ = writeln!(out, "### repolish {score} / 100\n");
        }
        _ => {
            let _ = writeln!(
                out,
                "### repolish — no total score\n\nOnly {:.0}% of the registered check \
                 weight could be scored, below the 50% floor.\n",
                report.coverage * 100.0
            );
        }
    }

    write_delta(&mut out, report, "####");

    // P1 与 P2 就够了。评论里塞满 P3「锦上添花」,读者会学会忽略整条评论。
    let mut findings: Vec<(Severity, &str, &str)> = Vec::new();
    for r in &report.checks {
        for f in r.outcome.fixes() {
            if f.severity != Severity::P3 {
                findings.push((f.severity, r.id, f.message.as_str()));
            }
        }
    }
    findings.sort_by_key(|(s, id, _)| (*s, *id));

    if findings.is_empty() {
        let _ = writeln!(out, "Nothing at P1 or P2.\n");
    } else {
        let _ = writeln!(out, "**To fix**\n");
        for (sev, id, msg) in findings.iter().take(8) {
            let _ = writeln!(out, "- `{}` **{id}** — {msg}", sev.as_str());
            if let Some(r) = report.checks.iter().find(|r| r.id == *id) {
                for e in r.outcome.evidence().iter().take(3) {
                    let _ = writeln!(out, "  - `{}` — {}", location(e), e.note);
                }
            }
        }
        if findings.len() > 8 {
            let _ = writeln!(out, "- … and {} more", findings.len() - 8);
        }
        out.push('\n');
    }

    let _ = writeln!(
        out,
        "<sub>{} mode · commit `{}` · <a href=\"{REPOLISH_URL}\">repolish</a> v{}</sub>",
        report.mode.as_str(),
        report
            .repository
            .commit
            .as_deref()
            .map(|c| &c[..c.len().min(8)])
            .unwrap_or("—"),
        report.repolish_version
    );
    out
}

/// 与基线的差异。
///
/// **只列动了的那几项。** 22 项全列出来,读者要自己找哪一行变了,
/// 那正是这一段本来想省掉的功夫。
fn write_delta(out: &mut String, report: &Report, heading: &str) {
    let Some(d) = &report.delta else {
        return;
    };

    let _ = writeln!(out, "{heading} Since `{}`\n", d.base_ref);

    match (d.base_score, report.score, d.points) {
        (Some(b), Some(h), Some(p)) => {
            let verdict = if p < 0 {
                " — this change lowers the score"
            } else if p > 0 {
                ""
            } else {
                " — unchanged"
            };
            let _ = writeln!(out, "**{b} → {h}** ({p:+}){verdict}\n");
        }
        _ => {
            let _ = writeln!(
                out,
                "No comparable total: one of the two sides did not produce a score.\n"
            );
        }
    }

    if d.checks.is_empty() {
        let _ = writeln!(out, "No check changed state.\n");
        return;
    }

    let _ = writeln!(out, "| Check | Before | After |");
    let _ = writeln!(out, "| --- | ---: | ---: |");
    for c in &d.checks {
        let cell = |v: Option<u8>, status: &str| match v {
            Some(n) => format!("{n}/10"),
            None => status.replace('_', " "),
        };
        let mark = if c.is_regression() { "⚠️ " } else { "" };
        let _ = writeln!(
            out,
            "| {mark}`{}` | {} | {} |",
            c.id,
            cell(c.before, c.before_status),
            cell(c.after, c.after_status)
        );
    }
    out.push('\n');
}

fn write_findings(out: &mut String, report: &Report) {
    let mut findings: Vec<(Severity, &str, &str)> = Vec::new();
    for r in &report.checks {
        for f in r.outcome.fixes() {
            findings.push((f.severity, r.id, f.message.as_str()));
        }
    }
    if findings.is_empty() {
        return;
    }
    // 与终端报告同一套排序，两处输出才对得上
    findings.sort_by_key(|(s, id, _)| (*s, *id));

    let _ = writeln!(out, "## What to fix\n");

    let mut current: Option<Severity> = None;
    for (sev, id, msg) in &findings {
        if current != Some(*sev) {
            let _ = writeln!(out, "### {}\n", severity_heading(*sev));
            current = Some(*sev);
        }
        let _ = writeln!(out, "- **`{id}`** — {msg}");
        if let Some(r) = report.checks.iter().find(|r| r.id == *id) {
            for e in r.outcome.evidence() {
                let _ = writeln!(out, "  - `{}` — {}", location(e), e.note);
            }
        }
        out.push('\n');
    }
}

fn write_verified(out: &mut String, report: &Report) {
    let passed: Vec<_> = report
        .checks
        .iter()
        .filter(|r| r.outcome.score() == Some(10))
        .collect();
    if passed.is_empty() {
        return;
    }
    let _ = writeln!(out, "## Verified\n");
    for r in passed {
        match r.outcome.evidence().first() {
            Some(e) => {
                let _ = writeln!(out, "- `{}` — {}", r.id, e.note);
            }
            None => {
                let _ = writeln!(out, "- `{}`", r.id);
            }
        }
    }
    out.push('\n');
}

fn write_limits(out: &mut String, report: &Report) {
    if report.coverage_limits.is_empty() {
        return;
    }
    // 这一节是强制的：不写出来，读者会把「没查」当成「查过且没问题」
    let _ = writeln!(out, "## Not verified\n");
    let _ = writeln!(
        out,
        "These checks could not be decided. They are excluded from the score rather than \
         guessed at.\n"
    );
    for limit in &report.coverage_limits {
        match limit.split_once(": ") {
            Some((id, reason)) => {
                let _ = writeln!(out, "- `{id}` — {reason}");
            }
            None => {
                let _ = writeln!(out, "- {limit}");
            }
        }
    }
    out.push('\n');
}

fn write_not_applicable(out: &mut String, report: &Report) {
    let na: Vec<&str> = report
        .checks
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::NotApplicable { .. }))
        .map(|r| r.id)
        .collect();
    if na.is_empty() {
        return;
    }
    let ids = na
        .iter()
        .map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        out,
        "## Not applicable\n\n{ids} — not expected of a `{}` project, and excluded from \
         both sides of the score.\n",
        report.profile.detected.as_str()
    );
}

fn write_footer(out: &mut String, report: &Report) {
    let commit = report
        .repository
        .commit
        .as_deref()
        .map(|c| format!(" from commit `{}`", &c[..c.len().min(8)]))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "---\n\nGenerated by [repolish]({REPOLISH_URL}) v{}{commit}.",
        report.repolish_version
    );
}

fn location(e: &repolish_core::Evidence) -> String {
    match e.line {
        Some(l) => format!("{}:{}", e.file.display(), l),
        None => e.file.display().to_string(),
    }
}

fn severity_heading(s: Severity) -> &'static str {
    match s {
        Severity::P1 => "P1 — fix these first",
        Severity::P2 => "P2",
        Severity::P3 => "P3 — nice to have",
    }
}

/// 英文类别名。`Category::label()` 返回的是终端用的中文名，
/// 两者会在整体 i18n 决定后合并。
#[cfg(test)]
mod tests {
    use super::*;
    use repolish_core::{CheckResult, Evidence, Fix, Mode, Profile, ProfileInfo, Repository, Risk};

    fn sample() -> Report {
        let checks = vec![
            CheckResult {
                id: "license",
                category: Category::Credibility,
                risk: Risk::Critical,
                outcome: Outcome::perfect(vec![Evidence::new("LICENSE", "identified as MIT")]),
            },
            CheckResult {
                id: "readme-quickstart",
                category: Category::Comprehensibility,
                risk: Risk::Critical,
                outcome: Outcome::scored(
                    0,
                    vec![Evidence::at("README.md", 12, "no install section")],
                    vec![Fix::new(Severity::P1, "add a quick start section")],
                ),
            },
            CheckResult {
                id: "release-hygiene",
                category: Category::Credibility,
                risk: Risk::Medium,
                outcome: Outcome::inconclusive("shallow clone, tags unavailable"),
            },
            CheckResult {
                id: "tests-present",
                category: Category::Credibility,
                risk: Risk::High,
                outcome: Outcome::NotApplicable {
                    profile: Profile::Docs,
                },
            },
        ];
        Report::build(
            checks,
            Repository {
                owner: Some("acme".into()),
                name: "widget".into(),
                commit: Some("3fce3b5bb0236da2df6d99672afb8a719642eca7".into()),
            },
            ProfileInfo {
                detected: Profile::Docs,
                overridden: false,
            },
            Mode::Local,
        )
    }

    #[test]
    fn contains_every_mandated_section() {
        let md = markdown(&sample());
        for section in [
            "# Repository health",
            "| Category | Score |",
            "## What to fix",
            "### P1",
            "## Verified",
            "## Not verified",
            "## Not applicable",
        ] {
            assert!(md.contains(section), "缺少 {section}\n---\n{md}");
        }
    }

    #[test]
    fn evidence_carries_file_and_line() {
        let md = markdown(&sample());
        assert!(md.contains("`README.md:12` — no install section"));
    }

    #[test]
    fn unverified_checks_are_named_so_they_cannot_read_as_passing() {
        let md = markdown(&sample());
        assert!(md.contains("`release-hygiene` — shallow clone, tags unavailable"));
    }

    #[test]
    fn local_mode_is_called_out_and_footer_pins_the_commit() {
        let md = markdown(&sample());
        assert!(md.contains("**This is a local score.**"));
        assert!(md.contains("from commit `3fce3b5b`."));
    }

    #[test]
    fn output_is_stable_across_runs() {
        // 这份文件会被 CI 提交；不稳定的顺序会制造无意义的 diff
        assert_eq!(markdown(&sample()), markdown(&sample()));
    }
}
