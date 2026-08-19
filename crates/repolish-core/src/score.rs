//! 聚合：风险加权平均 + 分母保护。

use serde::Serialize;

use crate::check::{Category, Risk};
use crate::outcome::Outcome;
use crate::MIN_COVERAGE;
use repolish_ingest::Profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// 仅本地检查
    Local,
    /// 含 GitHub API 元数据
    Remote,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Local => "local",
            Mode::Remote => "remote",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub id: &'static str,
    pub category: Category,
    pub risk: Risk,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryScore {
    pub category: Category,
    /// 0-100；该类别下无 `Scored` 项时为 None
    pub score: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// 0-100。覆盖不足时为 None
    pub score: Option<u8>,
    /// `Scored` 权重和 / 注册总权重
    pub coverage: f64,
    pub profile: Profile,
    pub profile_overridden: bool,
    pub mode: Mode,
    pub categories: Vec<CategoryScore>,
    pub checks: Vec<CheckResult>,
    /// `Inconclusive` 与 `Skipped` 的合并列表，强制消费方看到「哪些没验证」
    pub coverage_limits: Vec<String>,
}

impl Report {
    pub fn build(
        checks: Vec<CheckResult>,
        profile: Profile,
        profile_overridden: bool,
        mode: Mode,
    ) -> Self {
        let coverage = coverage_ratio(&checks);
        let score = if coverage >= MIN_COVERAGE {
            weighted_score(checks.iter())
        } else {
            None
        };

        let categories = Category::ALL
            .iter()
            .map(|c| CategoryScore {
                category: *c,
                score: weighted_score(checks.iter().filter(|r| r.category == *c)),
            })
            .collect();

        let coverage_limits = checks
            .iter()
            .filter_map(|r| match &r.outcome {
                Outcome::Inconclusive { reason } | Outcome::Skipped { reason } => {
                    Some(format!("{}: {}", r.id, reason))
                }
                _ => None,
            })
            .collect();

        Report {
            score,
            coverage,
            profile,
            profile_overridden,
            mode,
            categories,
            checks,
            coverage_limits,
        }
    }

    pub fn category_score(&self, c: Category) -> Option<u8> {
        self.categories
            .iter()
            .find(|cs| cs.category == c)
            .and_then(|cs| cs.score)
    }
}

/// 注册项中被实际打分的权重占比。`NotApplicable` 不算「没覆盖」——
/// 它本就不该出现在这个项目里，因此从分子分母同时剔除。
fn coverage_ratio(checks: &[CheckResult]) -> f64 {
    let mut scored = 0.0;
    let mut total = 0.0;
    for r in checks {
        if matches!(r.outcome, Outcome::NotApplicable { .. }) {
            continue;
        }
        let w = r.risk.weight();
        total += w;
        if r.outcome.counts() {
            scored += w;
        }
    }
    if total == 0.0 {
        0.0
    } else {
        scored / total
    }
}

/// 总分 = Σ(score_i × weight_i) / Σ(weight_i) × 10
fn weighted_score<'a, I: Iterator<Item = &'a CheckResult>>(iter: I) -> Option<u8> {
    let mut num = 0.0;
    let mut den = 0.0;
    for r in iter {
        if let Some(s) = r.outcome.score() {
            let w = r.risk.weight();
            num += s as f64 * w;
            den += w;
        }
    }
    if den == 0.0 {
        return None;
    }
    Some((num / den * 10.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::Outcome;

    fn r(id: &'static str, risk: Risk, outcome: Outcome) -> CheckResult {
        CheckResult {
            id,
            category: Category::Credibility,
            risk,
            outcome,
        }
    }

    #[test]
    fn weighted_average_matches_spec() {
        // 10 分(权重10) + 5 分(权重5) → (100+25)/15*10 = 83.3 → 83
        let checks = vec![
            r("a", Risk::Critical, Outcome::perfect(vec![])),
            r(
                "b",
                Risk::Medium,
                Outcome::Scored {
                    score: 5,
                    evidence: vec![],
                    fixes: vec![],
                },
            ),
        ];
        let rep = Report::build(checks, Profile::Unknown, false, Mode::Local);
        assert_eq!(rep.score, Some(83));
    }

    #[test]
    fn not_applicable_is_excluded_from_both_sides() {
        let checks = vec![
            r("a", Risk::Critical, Outcome::perfect(vec![])),
            r(
                "b",
                Risk::Critical,
                Outcome::NotApplicable {
                    profile: Profile::Docs,
                },
            ),
        ];
        let rep = Report::build(checks, Profile::Docs, false, Mode::Local);
        assert_eq!(rep.score, Some(100));
        assert_eq!(rep.coverage, 1.0);
    }

    #[test]
    fn low_coverage_suppresses_score() {
        // 只有 1/4 权重被打分 → 低于 50%，不出总分
        let checks = vec![
            r("a", Risk::Low, Outcome::perfect(vec![])),
            r("b", Risk::Critical, Outcome::skipped("需要 --remote")),
        ];
        let rep = Report::build(checks, Profile::Unknown, false, Mode::Local);
        assert!(rep.score.is_none());
        assert_eq!(rep.coverage_limits.len(), 1);
    }
}
