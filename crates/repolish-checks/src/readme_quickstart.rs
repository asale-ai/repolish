use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};
use repolish_md::SectionKind;

/// 是否存在能让人跑起来的安装 / 快速开始区块。
///
/// 分档：无区块 = 0；有区块无命令 = 4；有命令 = 8；命令 + 前置条件说明 = 10
pub struct ReadmeQuickstart;

const PREREQ_HINTS: &[&str] = &[
    "require", "prerequisite", "depend", "version", "node ", "python ", "rust", "go ",
    "前置", "依赖", "需要", "要求", "环境",
];

impl Check for ReadmeQuickstart {
    fn id(&self) -> &'static str {
        "readme-quickstart"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::Critical
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "没有 README，无从判断如何上手")],
                vec![Fix::new(Severity::P1, "添加 README 并写「快速开始」区块")],
            );
        };
        let name = readme.path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let section = readme
            .section(SectionKind::Quickstart)
            .or_else(|| readme.section(SectionKind::Install));

        let Some(section) = section else {
            return Outcome::scored(
                0,
                vec![Evidence::new(
                    &name,
                    "未找到「快速开始 / 安装」区块",
                )],
                vec![Fix::new(
                    Severity::P1,
                    "加一个「快速开始」区块，写清安装命令和最小可运行示例",
                )],
            );
        };

        // 该区块内是否有命令块
        let has_command = readme
            .code_blocks
            .iter()
            .any(|cb| cb.line > section.line && cb.line <= section.line + section.body.lines().count() + 1);

        if !has_command {
            return Outcome::scored(
                4,
                vec![Evidence::at(
                    &name,
                    section.line,
                    format!("「{}」区块里没有可复制的命令", section.title),
                )],
                vec![Fix::new(
                    Severity::P2,
                    "在该区块加一个代码块，给出可直接粘贴执行的安装命令",
                )],
            );
        }

        let body_lower = section.body.to_lowercase();
        let has_prereq = PREREQ_HINTS.iter().any(|h| body_lower.contains(h));

        if has_prereq {
            Outcome::perfect(vec![Evidence::at(
                &name,
                section.line,
                format!("「{}」含命令与前置条件说明", section.title),
            )])
        } else {
            Outcome::scored(
                8,
                vec![Evidence::at(
                    &name,
                    section.line,
                    format!("「{}」有命令，但未说明前置条件", section.title),
                )],
                vec![Fix::new(
                    Severity::P3,
                    "补一行前置条件（语言/运行时版本、系统依赖），减少「照做却跑不起来」",
                )],
            )
        }
    }
}
