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
        !matches!(profile, Profile::Docs | Profile::Collection | Profile::Meta)
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "no README")],
                vec![Fix::new(Severity::P1, "Add a README with a usage example")],
            );
        };
        let name = readme
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let total = readme.code_blocks.len();
        let tagged = readme
            .code_blocks
            .iter()
            .filter(|cb| !cb.info.is_empty())
            .count();

        match total {
            0 => Outcome::scored(
                0,
                vec![Evidence::new(&name, "no code blocks anywhere in the README")],
                vec![Fix::new(
                    Severity::P1,
                    "Add one example small enough to run immediately. It is the main thing readers use to decide whether to adopt a project",
                )],
            ),
            1 => Outcome::scored(
                5,
                vec![Evidence::at(
                    &name,
                    readme.code_blocks[0].line,
                    "a single code block, which usually covers installation and nothing else",
                )],
                vec![Fix::new(
                    Severity::P2,
                    "Add a real usage example alongside the install command",
                )],
            ),
            _ if tagged * 2 < total => Outcome::scored(
                8,
                vec![Evidence::new(
                    &name,
                    // 分词形式绕开一致性：`1 of 4 code blocks are tagged` 别扭
                    format!("{tagged} of {total} code blocks tagged with a language"),
                )],
                vec![Fix::new(
                    Severity::P3,
                    "Tag the code blocks with a language (```rust, ```bash) so they get syntax highlighting",
                )],
            ),
            _ => Outcome::perfect(vec![Evidence::new(
                &name,
                format!("{total} code blocks, {tagged} of them tagged with a language"),
            )]),
        }
    }
}
