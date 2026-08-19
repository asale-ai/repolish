use serde::Serialize;

use crate::outcome::Outcome;
use repolish_ingest::{Profile, RepoContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Critical,
    High,
    Medium,
    Low,
}

impl Risk {
    /// 风险加权，沿用 scorecard 的档位
    pub fn weight(self) -> f64 {
        match self {
            Risk::Critical => 10.0,
            Risk::High => 7.5,
            Risk::Medium => 5.0,
            Risk::Low => 2.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// 可发现性
    Discoverability,
    /// 可理解性
    Comprehensibility,
    /// 可信度
    Credibility,
}

impl Category {
    pub const ALL: [Category; 3] = [
        Category::Discoverability,
        Category::Comprehensibility,
        Category::Credibility,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Discoverability => "Discoverability",
            Category::Comprehensibility => "Comprehensibility",
            Category::Credibility => "Credibility",
        }
    }
}

pub trait Check: Send + Sync {
    fn id(&self) -> &'static str;
    fn category(&self) -> Category;
    fn risk(&self) -> Risk;

    /// 需要 GitHub API 才能执行
    fn requires_remote(&self) -> bool {
        false
    }

    /// 该项目类型是否需要此项。返回 false → `NotApplicable`，不计入分母。
    fn applies_to(&self, _profile: Profile) -> bool {
        true
    }

    fn run(&self, ctx: &RepoContext) -> Outcome;
}
