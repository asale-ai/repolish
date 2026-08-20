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
            return Outcome::inconclusive("GitHub metadata was not fetched");
        };

        let Some(desc) = remote.description.as_deref() else {
            let suggestion = ctx
                .readme
                .as_ref()
                .and_then(|r| r.tagline.as_deref())
                .map(|t| format!("The opening line of the README would do: \"{}\"", truncate(t, 80)))
                .unwrap_or_else(|| "Write one sentence at the top of the README saying what this is, then reuse it here.".to_string());

            return Outcome::scored(
                0,
                vec![Evidence::new(".", "the GitHub repository description is empty")],
                vec![Fix::new(
                    Severity::P1,
                    format!(
                        "Set the repository description. It is the only sentence that shows up in search results and link previews. {suggestion}"
                    ),
                )],
            );
        };

        let repo_name = ctx.slug.as_ref().map(|s| s.name.as_str()).unwrap_or("");
        if is_just_the_name(desc, repo_name) {
            return Outcome::scored(
                4,
                vec![Evidence::new(".", format!("the description only repeats the project name: \"{desc}\""))],
                vec![Fix::new(
                    Severity::P1,
                    "A description that repeats the name says nothing. Say what problem it \
                     solves — by the time anyone reads it, they already know what the \
                     project is called",
                )],
            );
        }

        let chars = desc.chars().count();
        let threshold = if has_cjk(desc) { SHORT_CJK } else { SHORT };
        if chars < threshold {
            return Outcome::scored(
                6,
                vec![Evidence::new(".", format!("the description is short: \"{desc}\""))],
                vec![Fix::new(
                    Severity::P2,
                    "Make the description a full sentence: what it does, who it is for, and what sets it apart",
                )],
            );
        }

        Outcome::perfect(vec![Evidence::new(
            ".",
            format!("description: \"{}\"", truncate(desc, 80)),
        )])
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
