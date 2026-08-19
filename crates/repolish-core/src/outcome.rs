use std::path::PathBuf;

use serde::Serialize;

use repolish_ingest::Profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Severity {
    P1,
    P2,
    P3,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::P1 => "P1",
            Severity::P2 => "P2",
            Severity::P3 => "P3",
        }
    }
}

/// 一条证据。`line` 为 1-based；`None` 表示证据指向文件整体或其缺失。
#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub note: String,
}

impl Evidence {
    pub fn new(file: impl Into<PathBuf>, note: impl Into<String>) -> Self {
        Evidence {
            file: file.into(),
            line: None,
            note: note.into(),
        }
    }

    pub fn at(file: impl Into<PathBuf>, line: usize, note: impl Into<String>) -> Self {
        Evidence {
            file: file.into(),
            line: Some(line),
            note: note.into(),
        }
    }
}

/// 改进建议。**每条扣分都必须给出至少一条 Fix**——见 docs/05 设计原则 2。
#[derive(Debug, Clone, Serialize)]
pub struct Fix {
    pub severity: Severity,
    pub message: String,
    /// M4 起：能否由 `polish --apply` 自动落地
    pub autofixable: bool,
}

impl Fix {
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Fix {
            severity,
            message: message.into(),
            autofixable: false,
        }
    }
}

/// 检查项的四种终态。区别不只在语义，更在报告与徽章行为——见 docs/03。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Scored {
        score: u8,
        evidence: Vec<Evidence>,
        fixes: Vec<Fix>,
    },
    /// 该项目类型不需要此项
    NotApplicable { profile: Profile },
    /// 想查但客观查不了
    Inconclusive { reason: String },
    /// 用户配置或运行模式导致未执行
    Skipped { reason: String },
}

impl Outcome {
    pub fn scored(score: u8, evidence: Vec<Evidence>, fixes: Vec<Fix>) -> Self {
        debug_assert!(score <= 10, "score 必须在 0..=10");
        debug_assert!(
            score == 10 || !fixes.is_empty(),
            "扣分必须给出可执行的 Fix"
        );
        Outcome::Scored {
            score: score.min(10),
            evidence,
            fixes,
        }
    }

    pub fn perfect(evidence: Vec<Evidence>) -> Self {
        Outcome::Scored {
            score: 10,
            evidence,
            fixes: Vec::new(),
        }
    }

    pub fn inconclusive(reason: impl Into<String>) -> Self {
        Outcome::Inconclusive {
            reason: reason.into(),
        }
    }

    pub fn skipped(reason: impl Into<String>) -> Self {
        Outcome::Skipped {
            reason: reason.into(),
        }
    }

    pub fn score(&self) -> Option<u8> {
        match self {
            Outcome::Scored { score, .. } => Some(*score),
            _ => None,
        }
    }

    /// 是否计入总分分母
    pub fn counts(&self) -> bool {
        matches!(self, Outcome::Scored { .. })
    }

    /// 是否进入报告的「覆盖限制」章节
    pub fn is_coverage_limit(&self) -> bool {
        matches!(
            self,
            Outcome::Inconclusive { .. } | Outcome::Skipped { .. }
        )
    }

    pub fn fixes(&self) -> &[Fix] {
        match self {
            Outcome::Scored { fixes, .. } => fixes,
            _ => &[],
        }
    }

    pub fn evidence(&self) -> &[Evidence] {
        match self {
            Outcome::Scored { evidence, .. } => evidence,
            _ => &[],
        }
    }
}
