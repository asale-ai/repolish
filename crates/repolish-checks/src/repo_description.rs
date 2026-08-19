use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// 仓库 description 是否填了、且有信息量。
///
/// 这是 GitHub 搜索结果、Google 摘要、社交分享卡片里唯一会出现的一句话。
/// README 写得再好，description 空着就等于在搜索结果里放弃了自我介绍。
///
/// 分档：空 = 0；只是重复项目名 = 4；过短 = 6；完整一句话 = 10
pub struct RepoDescription;

const SHORT: usize = 20;
/// 中文描述在同样信息量下字数远少于英文，阈值单独放宽
const SHORT_CJK: usize = 10;

impl Check for RepoDescription {
    fn id(&self) -> &'static str {
        "repo-description"
    }
    fn category(&self) -> Category {
        Category::Discoverability
    }
    fn risk(&self) -> Risk {
        Risk::High
    }
    fn requires_remote(&self) -> bool {
        true
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(remote) = &ctx.remote else {
            return Outcome::inconclusive("未取到 GitHub 元数据");
        };

        let Some(desc) = remote.description.as_deref() else {
            let suggestion = ctx
                .readme
                .as_ref()
                .and_then(|r| r.tagline.as_deref())
                .map(|t| format!("可直接用 README 里的这句：「{}」", truncate(t, 80)))
                .unwrap_or_else(|| "先在 README 首屏写一句「这是什么」，再填到这里".to_string());

            return Outcome::scored(
                0,
                vec![Evidence::new(".", "GitHub 仓库 description 为空")],
                vec![Fix::new(
                    Severity::P1,
                    format!(
                        "填写仓库 description。它是搜索结果与分享卡片里唯一露出的一句话。{suggestion}"
                    ),
                )],
            );
        };

        let repo_name = ctx.slug.as_ref().map(|s| s.name.as_str()).unwrap_or("");
        if is_just_the_name(desc, repo_name) {
            return Outcome::scored(
                4,
                vec![Evidence::new(".", format!("description 只是重复项目名：「{desc}」"))],
                vec![Fix::new(
                    Severity::P1,
                    "description 里重复项目名等于没写。写清「解决什么问题」——\
                     读者看到它时已经知道项目叫什么了",
                )],
            );
        }

        let chars = desc.chars().count();
        let threshold = if has_cjk(desc) { SHORT_CJK } else { SHORT };
        if chars < threshold {
            return Outcome::scored(
                6,
                vec![Evidence::new(".", format!("description 偏短：「{desc}」"))],
                vec![Fix::new(
                    Severity::P2,
                    "把 description 写成完整的一句话：做什么 + 给谁用 + 关键特点",
                )],
            );
        }

        Outcome::perfect(vec![Evidence::new(".", format!("description：「{}」", truncate(desc, 80)))])
    }
}

/// 去掉分隔符后与仓库名相同，即视为「只是重复项目名」。
/// `ripgrep`、`rip-grep`、`RipGrep` 都算。
fn is_just_the_name(desc: &str, repo_name: &str) -> bool {
    if repo_name.is_empty() {
        return false;
    }
    let squash = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    squash(desc) == squash(repo_name)
}

fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| matches!(c as u32, 0x4E00..=0x9FFF | 0x3040..=0x30FF | 0xAC00..=0xD7AF))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::is_just_the_name;

    #[test]
    fn name_repetition_is_detected_across_separators() {
        assert!(is_just_the_name("ripgrep", "ripgrep"));
        assert!(is_just_the_name("Rip-Grep", "ripgrep"));
        assert!(!is_just_the_name("ripgrep recursively searches directories", "ripgrep"));
    }
}
