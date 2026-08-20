//! 评分引擎的模型与聚合逻辑。
//!
//! 硬性边界：本 crate **不依赖 LLM**。评分必须纯确定性、可复现，
//! 否则徽章没有公信力。见 docs/01-技术架构.md。

pub mod check;
pub mod outcome;
pub mod registry;
pub mod score;

pub use check::{Category, Check, Risk};
pub use outcome::{Evidence, Fix, Outcome, Severity};
pub use registry::Registry;
pub use score::{
    CategoryScore, CheckResult, Mode, ProfileInfo, Report, Repository, SCHEMA_VERSION,
};

pub use repolish_ingest::{Profile, RepoContext};

/// 分母保护阈值：`Scored` 权重和低于本次注册项总权重的这个比例时，不输出总分。
pub const MIN_COVERAGE: f64 = 0.5;
