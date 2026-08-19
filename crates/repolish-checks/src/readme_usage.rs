use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};

/// 是否有可复制的使用示例。
///
/// 分档：无代码块 = 0；仅 1 个 = 5；≥2 个但无语言标记 = 8；≥2 个且带语言标记 = 10
pub struct ReadmeUsageExample;

impl Check for ReadmeUsageExample {
    fn id(&self) -> &'static str {
        "readme-usage-example"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::High
    }

    fn applies_to(&self, profile: Profile) -> bool {
        !matches!(profile, Profile::Docs | Profile::Collection)
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "没有 README")],
                vec![Fix::new(Severity::P1, "添加 README 并给出使用示例")],
            );
        };
        let name = readme.path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let total = readme.code_blocks.len();
        let tagged = readme
            .code_blocks
            .iter()
            .filter(|cb| !cb.info.is_empty())
            .count();

        match total {
            0 => Outcome::scored(
                0,
                vec![Evidence::new(&name, "README 中没有任何代码块")],
                vec![Fix::new(
                    Severity::P1,
                    "加一段最小可运行示例——读者判断要不要用你的项目，主要看这个",
                )],
            ),
            1 => Outcome::scored(
                5,
                vec![Evidence::at(
                    &name,
                    readme.code_blocks[0].line,
                    "只有 1 个代码块，通常只够覆盖安装",
                )],
                vec![Fix::new(
                    Severity::P2,
                    "除安装命令外，再补一个真实用法示例",
                )],
            ),
            _ if tagged * 2 < total => Outcome::scored(
                8,
                vec![Evidence::new(
                    &name,
                    format!("{total} 个代码块中仅 {tagged} 个标注了语言"),
                )],
                vec![Fix::new(
                    Severity::P3,
                    "给代码块加语言标记（```rust / ```bash），启用语法高亮",
                )],
            ),
            _ => Outcome::perfect(vec![Evidence::new(
                &name,
                format!("{total} 个代码块，{tagged} 个带语言标记"),
            )]),
        }
    }
}
