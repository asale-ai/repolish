//! 聚合：风险加权平均 + 分母保护，以及**冻结的 JSON 输出契约**。
//!
//! `schemaVersion` 自 M2 起为 1。字段只增不改：删字段或改含义必须递增
//! `schemaVersion`，因为 CI 门禁与徽章都在消费它。见 docs/02-CLI设计.md。

use serde::Serialize;

use crate::check::{Category, Risk};
use crate::outcome::Outcome;
use crate::MIN_COVERAGE;
use repolish_ingest::{Profile, RepoContext};

/// 输出结构版本。改变既有字段的含义时必须 +1。
pub const SCHEMA_VERSION: u32 = 1;

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

/// 被诊断的仓库。`owner` 在没有 GitHub 远端时为空。
#[derive(Debug, Clone, Serialize)]
pub struct Repository {
    pub owner: Option<String>,
    pub name: String,
    pub commit: Option<String>,
}

impl Repository {
    pub fn from_ctx(ctx: &RepoContext) -> Self {
        let dir_name = ctx
            .root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        Repository {
            owner: ctx.slug.as_ref().map(|s| s.owner.clone()),
            // 远端名优先：目录名可能被使用者改过，owner/name 才是身份
            name: ctx
                .slug
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or(dir_name),
            commit: ctx.git.as_ref().map(|g| g.head_id.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileInfo {
    pub detected: Profile,
    /// 是否被 `--profile` / 配置文件覆盖
    pub overridden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub repolish_version: &'static str,
    pub schema_version: u32,
    pub repository: Repository,
    pub profile: ProfileInfo,
    pub mode: Mode,
    /// 0-100。覆盖不足时为 None
    pub score: Option<u8>,
    /// `Scored` 权重和 / 注册总权重，保留三位小数
    pub coverage: f64,
    pub categories: Vec<CategoryScore>,
    pub checks: Vec<CheckResult>,
    /// `Inconclusive` 与 `Skipped` 的合并列表，强制消费方看到「哪些没验证」
    pub coverage_limits: Vec<String>,
}

impl Report {
    pub fn build(
        checks: Vec<CheckResult>,
        repository: Repository,
        profile: ProfileInfo,
        mode: Mode,
    ) -> Self {
        let coverage = round3(coverage_ratio(&checks));
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
            repolish_version: env!("CARGO_PKG_VERSION"),
            schema_version: SCHEMA_VERSION,
            repository,
            profile,
            mode,
            score,
            coverage,
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

    /// 徽章配色阈值，见 docs/02-CLI设计.md
    pub fn color(&self) -> &'static str {
        match self.score {
            Some(s) if s >= 90 => "brightgreen",
            Some(s) if s >= 75 => "green",
            Some(s) if s >= 60 => "yellow",
            Some(s) if s >= 40 => "orange",
            Some(_) => "red",
            None => "lightgrey",
        }
    }
}

/// 浮点数进 JSON 前定量化：同一 commit 的输出必须逐字节一致，
/// 而 `7.5/8.75` 这类比值的十进制展开会随权重组合变得很长。
fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
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

    fn build(checks: Vec<CheckResult>, profile: Profile) -> Report {
        Report::build(
            checks,
            Repository {
                owner: Some("acme".into()),
                name: "widget".into(),
                commit: Some("deadbeef".into()),
            },
            ProfileInfo {
                detected: profile,
                overridden: false,
            },
            Mode::Local,
        )
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
        assert_eq!(build(checks, Profile::Unknown).score, Some(83));
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
        let rep = build(checks, Profile::Docs);
        assert_eq!(rep.score, Some(100));
        assert_eq!(rep.coverage, 1.0);
    }

    #[test]
    fn low_coverage_suppresses_score() {
        // 只有 1/4 权重被打分 → 低于 50%，不出总分
        let checks = vec![
            r("a", Risk::Low, Outcome::perfect(vec![])),
            r("b", Risk::Critical, Outcome::skipped("requires --remote")),
        ];
        let rep = build(checks, Profile::Unknown);
        assert!(rep.score.is_none());
        assert_eq!(rep.coverage_limits.len(), 1);
    }

    /// schema 一旦发出去就有人在解析。字段名变动必须是显式决定，
    /// 而不是改结构体时的副作用——所以这里把顶层键钉死。
    #[test]
    fn json_schema_is_frozen() {
        let rep = build(
            vec![r("a", Risk::Critical, Outcome::perfect(vec![]))],
            Profile::Cli,
        );
        let v: serde_json::Value = serde_json::to_value(&rep).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(|s| s.as_str()).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "categories",
                "checks",
                "coverage",
                "coverageLimits",
                "mode",
                "profile",
                "repolishVersion",
                "repository",
                "schemaVersion",
                "score",
            ]
        );
        assert_eq!(v["schemaVersion"], 1);
        assert_eq!(v["profile"]["detected"], "cli");
        assert_eq!(v["checks"][0]["status"], "scored");
        assert_eq!(v["checks"][0]["score"], 10);
    }
}
