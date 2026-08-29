//! 聚合：风险加权平均 + 分母保护，以及**冻结的 JSON 输出契约**。
//!
//! `schemaVersion` 自 M2 起为 1。字段只增不改：删字段或改含义必须递增
//! `schemaVersion`，因为 CI 门禁与徽章都在消费它。见 docs/02-cli-design.zh-CN.md。

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
    /// 与某个基线 commit 的差异。只有 `--base` 给了才有。
    ///
    /// 字段只增不改：不给 `--base` 时这个键根本不出现，v1 的消费方看到的
    /// JSON 与从前逐字节一致。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<Delta>,
}

/// 与一个基线 commit 的差异。
///
/// **变化量比绝对值更有行动力。** 「78 分」对一个正在评审 PR 的人没有意义，
/// 「这个 PR 让分数掉了 4 分，因为 README.md:42 的链接失效了」才有。
/// 这也是让那些「配好就永远绿」的检查项重新产生价值的唯一方式——
/// 它们平时不说话,回归的那一次会说。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    /// 用户给的那个 ref，原样保留：报告里要说清基线是什么
    pub base_ref: String,
    pub base_commit: String,
    pub base_score: Option<u8>,
    /// 总分变化。任一侧没有总分时为 None——不能拿 0 去减
    pub points: Option<i16>,
    pub categories: Vec<CategoryDelta>,
    /// **只列变了的。** 22 项里通常只有一两项动过，全列出来等于没列
    pub checks: Vec<CheckDelta>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDelta {
    pub category: Category,
    pub before: Option<u8>,
    pub after: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckDelta {
    pub id: &'static str,
    /// 0-10。基线上不是 `Scored` 时为 None
    pub before: Option<u8>,
    pub after: Option<u8>,
    pub before_status: &'static str,
    pub after_status: &'static str,
}

impl CheckDelta {
    /// 分数掉了，或者从「打过分」变成了「没验证」。
    ///
    /// 后半句不能漏：一个检查项从 10 分变成 `Inconclusive`，总分可能纹丝不动
    /// （它退出了分母），但仓库确实少了一份证据。
    pub fn is_regression(&self) -> bool {
        match (self.before, self.after) {
            (Some(b), Some(a)) => a < b,
            (Some(_), None) => true,
            _ => false,
        }
    }
}

fn status_of(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Scored { .. } => "scored",
        Outcome::NotApplicable { .. } => "not_applicable",
        Outcome::Inconclusive { .. } => "inconclusive",
        Outcome::Skipped { .. } => "skipped",
    }
}

/// `head` 相对 `base` 的差异。
///
/// **两侧必须用同一个 mode 跑出来。** 本地分与远程分的分母不同，拿它们相减
/// 得到的数字没有任何含义——调用方负责保证这一点，见 `docs/03-scoring.md`。
pub fn diff(base: &Report, head: &Report, base_ref: &str, base_commit: &str) -> Delta {
    let points = match (base.score, head.score) {
        (Some(b), Some(h)) => Some(h as i16 - b as i16),
        _ => None,
    };

    let categories = Category::ALL
        .iter()
        .map(|c| CategoryDelta {
            category: *c,
            before: base.category_score(*c),
            after: head.category_score(*c),
        })
        .collect();

    let mut checks = Vec::new();
    for h in &head.checks {
        let Some(b) = base.checks.iter().find(|b| b.id == h.id) else {
            continue;
        };
        let before = b.outcome.score();
        let after = h.outcome.score();
        let before_status = status_of(&b.outcome);
        let after_status = status_of(&h.outcome);
        if before == after && before_status == after_status {
            continue;
        }
        checks.push(CheckDelta {
            id: h.id,
            before,
            after,
            before_status,
            after_status,
        });
    }

    Delta {
        base_ref: base_ref.to_string(),
        base_commit: base_commit.to_string(),
        base_score: base.score,
        points,
        categories,
        checks,
    }
}

/// 分数落在第几档，0 最好、4 最差。
///
/// **阈值只写在这一处。** 徽章颜色、终端配色、卡片配色、以及分数旁边那个词，
/// 全部由它派生。分档标准散成好几份的话，改一次阈值就要改好几处，漏掉一处的
/// 表现是「徽章是绿的、卡片写着 fair」——同一个仓库两种说法，比说错更伤。
///
/// 放在 core 而不是 render，是因为分档是**分数的属性**，不是配色的属性：
/// render 依赖 core，反过来不行，所以这里是唯一能让两边都够得到的地方。
pub fn band_index(score: u8) -> usize {
    match score {
        90..=255 => 0,
        75..=89 => 1,
        60..=74 => 2,
        40..=59 => 3,
        _ => 4,
    }
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
            delta: None,
        }
    }

    pub fn category_score(&self, c: Category) -> Option<u8> {
        self.categories
            .iter()
            .find(|cs| cs.category == c)
            .and_then(|cs| cs.score)
    }

    /// 徽章配色阈值，见 docs/02-cli-design.zh-CN.md
    pub fn color(&self) -> &'static str {
        match self.score {
            Some(s) => ["brightgreen", "green", "yellow", "orange", "red"][band_index(s)],
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

    /// 只列变了的那几项：22 项里通常只有一两项动过
    #[test]
    fn a_diff_reports_only_the_checks_that_moved() {
        let base = build(
            vec![
                r("a", Risk::Critical, Outcome::perfect(vec![])),
                r("b", Risk::Critical, Outcome::perfect(vec![])),
            ],
            Profile::Cli,
        );
        let head = build(
            vec![
                r("a", Risk::Critical, Outcome::perfect(vec![])),
                r(
                    "b",
                    Risk::Critical,
                    Outcome::Scored {
                        score: 4,
                        evidence: vec![],
                        fixes: vec![],
                    },
                ),
            ],
            Profile::Cli,
        );
        let d = diff(&base, &head, "origin/main", "deadbeef");
        assert_eq!(d.base_score, Some(100));
        assert_eq!(d.points, Some(-30));
        assert_eq!(d.checks.len(), 1);
        assert_eq!(d.checks[0].id, "b");
        assert!(d.checks[0].is_regression());
    }

    /// 从「打过分」掉到「没验证」，总分可能纹丝不动（它退出了分母），
    /// 但仓库确实少了一份证据——那也是回归
    #[test]
    fn losing_a_scored_check_counts_as_a_regression() {
        let base = build(
            vec![r("a", Risk::Critical, Outcome::perfect(vec![]))],
            Profile::Cli,
        );
        let head = build(
            vec![r("a", Risk::Critical, Outcome::inconclusive("no README"))],
            Profile::Cli,
        );
        let d = diff(&base, &head, "main", "cafe");
        assert_eq!(d.checks[0].after_status, "inconclusive");
        assert!(d.checks[0].is_regression());
    }

    /// 不给 `--base` 时 `delta` 键根本不出现，v1 的消费方看到的 JSON 不变
    #[test]
    fn the_delta_key_is_absent_unless_a_base_was_given() {
        let mut rep = build(
            vec![r("a", Risk::Critical, Outcome::perfect(vec![]))],
            Profile::Cli,
        );
        let v = serde_json::to_value(&rep).unwrap();
        assert!(v.as_object().unwrap().get("delta").is_none());

        rep.delta = Some(diff(&rep.clone(), &rep.clone(), "main", "cafe"));
        let v = serde_json::to_value(&rep).unwrap();
        assert_eq!(v["delta"]["baseRef"], "main");
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
