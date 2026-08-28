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
    ///
    /// 默认对 [`Profile::Meta`] 不适用：组织的 `.github` 资料仓库不是项目，
    /// 要求它有 license、CI、测试只会产出满屏假警报。真正衡量「这段 README
    /// 读得懂吗」的那几项显式覆盖回来。
    ///
    /// 默认值取「不适用」而不是「适用」，是因为这个方向的错是安全的：
    /// 新加的检查项不会在没人过问的情况下突然对资料仓库开火。
    fn applies_to(&self, profile: Profile) -> bool {
        profile != Profile::Meta
    }

    fn run(&self, ctx: &RepoContext) -> Outcome;
}
