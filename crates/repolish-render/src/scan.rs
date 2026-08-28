//! 多仓库扫描的表格渲染。
//!
//! 单仓库报告回答「这个仓库哪里不行」；这张表回答的是另一个问题——
//! **一个组织里几十个仓库，先动哪一个、哪一条修一次能覆盖最多仓库**。
//! 因此除了按分数排序，还必须给出跨仓库的共性缺项：`issue-pr-template`
//! 在八个仓库里缺四个，那就是一次性写完、收益乘以四的一刀。

use std::fmt::Write as _;

use repolish_core::{Category, Outcome, Report, Severity};

use crate::theme::{self};
use crate::{Pen, RenderOptions, INDENT, WIDTH};

/// 一个被扫描的仓库。载入或远程调用失败时 `report` 为 `Err`——
/// 失败的仓库单独列出来，不能悄悄从表里消失。
pub struct Entry {
    pub name: String,
    pub report: Result<Report, String>,
}

/// 共性缺项至少出现在这么多仓库里才值得单列
const RECURRING: usize = 2;

pub fn scan(entries: &[Entry], opts: &RenderOptions) -> String {
    let pen = Pen { level: opts.level };
    let mut out = String::new();

    let ok: Vec<(&str, &Report)> = entries
        .iter()
        .filter_map(|e| e.report.as_ref().ok().map(|r| (e.name.as_str(), r)))
        .collect();

    table(&mut out, &pen, &ok);
    summary(&mut out, &pen, &ok);
    recurring(&mut out, &pen, &ok);
    failures(&mut out, &pen, entries);

    out
}

fn table(out: &mut String, pen: &Pen, ok: &[(&str, &Report)]) {
    let mut rows: Vec<&(&str, &Report)> = ok.iter().collect();
    // 分数升序：最该动的排最上面。看这张表的人是来找活干的，不是来领奖的。
    rows.sort_by_key(|(name, r)| (r.score.unwrap_or(0), *name));

    let width = rows
        .iter()
        .map(|(n, _)| crate::display_width(n))
        .max()
        .unwrap_or(4)
        .max(4);

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{INDENT}{}  {}  {}  {}",
        pen.dim("SCORE"),
        pen.dim(&crate::pad("REPOSITORY", width)),
        pen.dim("DISC COMP CRED"),
        pen.dim("FIRST THING TO FIX"),
    );
    let _ = writeln!(out, "{INDENT}{}", pen.ink(&"─".repeat(WIDTH), theme::LINE));

    for (name, r) in rows {
        let (score, color) = match r.score {
            Some(s) => (format!("{s:>3}"), theme::band(s)),
            None => ("  —".to_string(), theme::MUTED),
        };
        let cats: String = Category::ALL
            .iter()
            .map(|c| match r.category_score(*c) {
                Some(s) => format!("{s:>4}"),
                None => "   —".to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ");

        let _ = writeln!(
            out,
            "{INDENT} {}   {}  {}   {}",
            pen.strong(&score, color),
            pen.ink(&crate::pad(name, width), theme::TEXT),
            pen.dim(&cats),
            top_finding(pen, r),
        );
    }
}

/// 排在最前面的一条发现，作为「先动哪里」的指路牌
fn top_finding(pen: &Pen, r: &Report) -> String {
    let mut best: Option<(Severity, &str)> = None;
    for c in &r.checks {
        for f in c.outcome.fixes() {
            if best.is_none_or(|(s, _)| f.severity < s) {
                best = Some((f.severity, c.id));
            }
        }
    }
    match best {
        Some((sev, id)) => {
            let color = match sev {
                Severity::P1 => theme::RED,
                Severity::P2 => theme::AMBER,
                Severity::P3 => theme::PURPLE,
            };
            format!("{} {}", pen.ink(sev.as_str(), color), pen.dim(id))
        }
        None => pen.ink("clean", theme::CYAN),
    }
}

fn summary(out: &mut String, pen: &Pen, ok: &[(&str, &Report)]) {
    if ok.is_empty() {
        return;
    }
    let mut scores: Vec<u8> = ok.iter().filter_map(|(_, r)| r.score).collect();
    scores.sort_unstable();
    let median = scores.get(scores.len() / 2).copied();

    let p1: usize = ok
        .iter()
        .map(|(_, r)| {
            r.checks
                .iter()
                .flat_map(|c| c.outcome.fixes())
                .filter(|f| f.severity == Severity::P1)
                .count()
        })
        .sum();

    let mut parts = vec![format!("{} repositories", ok.len())];
    if let Some(m) = median {
        parts.push(format!("median {m}"));
    }
    parts.push(format!(
        "{} below 80",
        scores.iter().filter(|s| **s < 80).count()
    ));
    parts.push(format!("{p1} P1 in total"));

    let _ = writeln!(out);
    let _ = writeln!(out, "{INDENT}{}", pen.dim(&parts.join(" · ")));
}

/// 跨仓库的共性缺项。这是整张表最有用的一段：它把「修八次」变成「修一次」。
fn recurring(out: &mut String, pen: &Pen, ok: &[(&str, &Report)]) {
    if ok.len() < RECURRING {
        return;
    }
    // 按 (检查项, 严重度) 分组，**不是**只按检查项。
    //
    // 同一个检查项在不同仓库的严重度可以不同：0 分的仓库出 P1，7 分的出 P2。
    // 只按检查项聚合、再取最严重的那档贴标签，就会写出「P1 ci-present，
    // 8 个仓库中有 3 个」——而其中只有一个真的是 P1。那是在夸大问题，
    // 一个专门讲「判不了就说判不了」的工具不能这么算。
    let mut counts: Vec<(&'static str, usize, Severity)> = Vec::new();
    for (_, r) in ok {
        for c in &r.checks {
            // 只数真扣了分的：不适用与没验证的不是「缺项」
            let Outcome::Scored { score, fixes, .. } = &c.outcome else {
                continue;
            };
            if *score == 10 {
                continue;
            }
            let sev = fixes
                .iter()
                .map(|f| f.severity)
                .min()
                .unwrap_or(Severity::P3);
            match counts
                .iter_mut()
                .find(|(id, _, s)| *id == c.id && *s == sev)
            {
                Some(entry) => entry.1 += 1,
                None => counts.push((c.id, 1, sev)),
            }
        }
    }
    counts.retain(|(_, n, _)| *n >= RECURRING);
    // 严重度优先，其次才是出现次数。反过来排的话，一条出现六次的 P3 会盖过
    // 一条出现四次的 P2 —— 这张表是给人排活的，不是统计报表。
    counts.sort_by_key(|(id, n, sev)| (*sev, std::cmp::Reverse(*n), *id));
    if counts.is_empty() {
        return;
    }

    crate::rule(out, pen, "FIX ONCE, LIFTS SEVERAL");
    let _ = writeln!(out);
    for (id, n, sev) in counts.iter().take(5) {
        let color = match sev {
            Severity::P1 => theme::RED,
            Severity::P2 => theme::AMBER,
            Severity::P3 => theme::PURPLE,
        };
        let _ = writeln!(
            out,
            "{INDENT}   {} {}  {}",
            pen.ink(sev.as_str(), color),
            pen.ink(&crate::pad(id, 28), theme::TEXT),
            pen.dim(&format!("{n} of {} repositories", ok.len())),
        );
    }
}

fn failures(out: &mut String, pen: &Pen, entries: &[Entry]) {
    let failed: Vec<(&str, &str)> = entries
        .iter()
        .filter_map(|e| {
            e.report
                .as_ref()
                .err()
                .map(|m| (e.name.as_str(), m.as_str()))
        })
        .collect();
    if failed.is_empty() {
        return;
    }
    crate::rule(out, pen, "NOT SCORED");
    let _ = writeln!(out);
    for (name, why) in failed {
        let _ = writeln!(
            out,
            "{INDENT}   {} {}  {}",
            pen.dim("·"),
            pen.ink(name, theme::TEXT),
            pen.dim(why)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorLevel;
    use repolish_core::{CheckResult, Evidence, Fix, Mode, Profile, ProfileInfo, Repository, Risk};

    fn report(score: u8, missing: &'static str) -> Report {
        let checks = vec![
            CheckResult {
                id: "license",
                category: Category::Credibility,
                risk: Risk::Critical,
                outcome: Outcome::Scored {
                    score: score / 10,
                    evidence: vec![Evidence::new("LICENSE", "MIT")],
                    fixes: vec![Fix::new(Severity::P2, "Add a LICENSE file")],
                },
            },
            CheckResult {
                id: missing,
                category: Category::Credibility,
                risk: Risk::Medium,
                outcome: Outcome::Scored {
                    score: 0,
                    evidence: vec![],
                    fixes: vec![Fix::new(Severity::P1, "Add it")],
                },
            },
        ];
        Report::build(
            checks,
            Repository {
                owner: Some("acme".into()),
                name: "r".into(),
                commit: None,
            },
            ProfileInfo {
                detected: Profile::Cli,
                overridden: false,
            },
            Mode::Remote,
        )
    }

    fn entries() -> Vec<Entry> {
        vec![
            Entry {
                name: "alpha".into(),
                report: Ok(report(90, "issue-pr-template")),
            },
            Entry {
                name: "beta".into(),
                report: Ok(report(50, "issue-pr-template")),
            },
            Entry {
                name: "broken".into(),
                report: Err("not a git repository".into()),
            },
        ]
    }

    fn plain() -> String {
        scan(
            &entries(),
            &RenderOptions {
                verbose: false,
                level: ColorLevel::None,
            },
        )
    }

    /// 看这张表的人是来找活干的：最该动的必须排最上面
    #[test]
    fn the_worst_repository_is_listed_first() {
        let out = plain();
        let beta = out.find("beta").expect("beta 在表里");
        let alpha = out.find("alpha").expect("alpha 在表里");
        assert!(beta < alpha, "低分仓库应排在前面\n{out}");
    }

    /// 严重度压过出现次数：一条出现六次的 P3 不该盖过一条出现四次的 P2
    #[test]
    fn severity_outranks_frequency_in_the_recurring_list() {
        let mut e = entries();
        // 两个仓库都缺 issue-pr-template（P1），两个都少一条 P3
        for entry in e.iter_mut() {
            if let Ok(r) = entry.report.as_mut() {
                r.checks.push(CheckResult {
                    id: "readme-i18n",
                    category: Category::Comprehensibility,
                    risk: Risk::Low,
                    outcome: Outcome::Scored {
                        score: 8,
                        evidence: vec![],
                        fixes: vec![Fix::new(Severity::P3, "Add a translation")],
                    },
                });
            }
        }
        let out = scan(
            &e,
            &RenderOptions {
                verbose: false,
                level: ColorLevel::None,
            },
        );
        let p1 = out.find("issue-pr-template").expect("P1 在表里");
        let p3 = out.find("readme-i18n").expect("P3 在表里");
        assert!(
            p1 < p3,
            "P1 应排在 P3 前面
{out}"
        );
    }

    /// 跨仓库共性是整张表最有用的一段，也是它区别于「跑 N 次 check」的理由
    #[test]
    fn a_gap_shared_by_several_repositories_is_called_out() {
        let out = plain();
        assert!(out.contains("FIX ONCE"), "{out}");
        assert!(out.contains("issue-pr-template"), "{out}");
        assert!(out.contains("2 of 2 repositories"), "{out}");
    }

    /// 跑不了的仓库不能悄悄从表里消失
    #[test]
    fn repositories_that_could_not_be_scored_are_listed_not_dropped() {
        let out = plain();
        assert!(out.contains("NOT SCORED"), "{out}");
        assert!(out.contains("broken"), "{out}");
        assert!(out.contains("not a git repository"), "{out}");
    }

    #[test]
    fn the_summary_reports_the_median_and_the_p1_count() {
        let out = plain();
        assert!(out.contains("2 repositories"), "{out}");
        assert!(out.contains("median"), "{out}");
        assert!(out.contains("P1 in total"), "{out}");
    }

    #[test]
    fn no_color_output_is_free_of_escapes() {
        assert!(!plain().contains('\u{1b}'));
    }
}
