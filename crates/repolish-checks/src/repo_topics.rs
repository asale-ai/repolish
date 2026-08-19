//! `repo-topics`：数量分档 + 本地信号交叉验证。
//!
//! **不引入 LLM 参与打分**（docs/05）。相关性拆成两个确定性信号：
//! 数量落在合理区间，以及至少有一个 topic 能与本地事实对上。
//! 语义相关性不判——判不准，而误判会直接摧毁分数的可信度。

use std::collections::BTreeSet;

use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

pub struct RepoTopics;

/// 交叉验证不通过时的分数上限
const UNVALIDATED_CAP: u8 = 5;
const MAX_SUGGESTIONS: usize = 6;

/// 扩展名 → 生态里通行的 topic 名
const LANG_TOPICS: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("py", "python"),
    ("js", "javascript"),
    ("mjs", "javascript"),
    ("ts", "typescript"),
    ("tsx", "react"),
    ("jsx", "react"),
    ("go", "golang"),
    ("java", "java"),
    ("kt", "kotlin"),
    ("rb", "ruby"),
    ("php", "php"),
    ("cs", "dotnet"),
    ("cpp", "cpp"),
    ("cc", "cpp"),
    ("c", "c"),
    ("swift", "swift"),
    ("scala", "scala"),
    ("ex", "elixir"),
    ("dart", "dart"),
    ("lua", "lua"),
    ("zig", "zig"),
    ("vue", "vue"),
    ("svelte", "svelte"),
    ("sh", "shell"),
];

/// 项目形态。这些词几乎总是合适的 topic，用来补足建议列表。
const PROFILE_TOPICS: &[(repolish_core::Profile, &[&str])] = &[
    (repolish_core::Profile::Cli, &["cli", "command-line"]),
    (repolish_core::Profile::Library, &["library"]),
    (repolish_core::Profile::App, &["app"]),
    (repolish_core::Profile::Docs, &["documentation"]),
    (repolish_core::Profile::Collection, &["awesome", "awesome-list"]),
];

impl Check for RepoTopics {
    fn id(&self) -> &'static str {
        "repo-topics"
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

        let vocab = local_vocabulary(ctx);
        let topics = &remote.topics;

        if topics.is_empty() {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "no topics set")],
                vec![Fix::new(
                    Severity::P1,
                    format!(
                        "Set some topics. They are what GitHub search and the \"related repositories\" rail run on. {}",
                        suggest(&vocab, topics)
                    ),
                )],
            );
        }

        let (base, size_advice) = match topics.len() {
            1..=2 => (4, Some("Too few topics. Five or more are needed to cover the different terms people search for")),
            3..=5 => (8, Some("Add a few more: one for the language, one for the domain, one for the shape it ships in")),
            6..=12 => (10, None),
            _ => (8, Some("Piling on topics dilutes relevance. Trim to twelve or fewer")),
        };

        let matched: Vec<&String> = topics.iter().filter(|t| matches_vocab(t, &vocab)).collect();

        if matched.is_empty() {
            return Outcome::scored(
                base.min(UNVALIDATED_CAP),
                vec![Evidence::new(
                    ".",
                    format!(
                        "none of the {} topic{} ({}) match anything in the repository: not the main language, not the dependencies, not the README headings",
                        topics.len(),
                        crate::util::plural(topics.len()),
                        topics.join(", ")
                    ),
                )],
                vec![Fix::new(
                    Severity::P2,
                    format!(
                        "The current topics do not describe what this project actually is, so searches for it do not find it. {}",
                        suggest(&vocab, topics)
                    ),
                )],
            );
        }

        let note = format!(
            "{} topic{}, {} of which match signals in the repository",
            topics.len(),
            crate::util::plural(topics.len()),
            matched.len()
        );
        match size_advice {
            None => Outcome::perfect(vec![Evidence::new(".", note)]),
            Some(advice) => Outcome::scored(
                base,
                vec![Evidence::new(".", note)],
                vec![Fix::new(
                    Severity::P2,
                    format!("{advice}。{}", suggest(&vocab, topics)),
                )],
            ),
        }
    }
}

/// 本地事实构成的期望词表：主语言 + 依赖名 + 标题/说明里的词 + 项目形态。
fn local_vocabulary(ctx: &RepoContext) -> BTreeSet<String> {
    let mut vocab = BTreeSet::new();

    for (ext, topic) in LANG_TOPICS {
        if ctx.files.content_extension_count(ext) >= 3 {
            vocab.insert((*topic).to_string());
        }
    }

    for m in &ctx.manifests {
        vocab.extend(m.deps.iter().map(|d| normalize(d)).filter(|d| d.len() >= 3));
        if let Some(n) = &m.name {
            vocab.insert(normalize(n));
        }
    }

    if let Some(r) = &ctx.readme {
        let text = format!(
            "{} {}",
            r.title.as_deref().unwrap_or(""),
            r.tagline.as_deref().unwrap_or("")
        );
        vocab.extend(
            text.split(|c: char| !c.is_alphanumeric())
                .map(normalize)
                .filter(|w| w.len() >= 3 && !STOPWORDS.contains(&w.as_str())),
        );
    }

    for (profile, topics) in PROFILE_TOPICS {
        if ctx.profile == *profile {
            vocab.extend(topics.iter().map(|t| (*t).to_string()));
        }
    }

    vocab
}

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "you", "your", "are", "from", "was", "has",
    "not", "但是", "一个", "可以", "使用",
];

/// topic 与词表是否对得上。
///
/// 只认「topic 比词表词更具体」这一个方向：`rust-library` 命中 `rust`。
/// 反方向（topic 是词表词的子串）会让 `cli` 被 `client` 认领，
/// 那样几乎任何 topic 都能蒙混过关，交叉验证也就失去意义了。
fn matches_vocab(topic: &str, vocab: &BTreeSet<String>) -> bool {
    let t = normalize(topic);
    if t.is_empty() {
        return false;
    }
    vocab
        .iter()
        .any(|v| v == &t || (v.len() >= 3 && t.contains(v.as_str())))
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase().replace(['_', '.', ' '], "-")
}

/// 从词表里挑出还没被用上的词作为建议。**建议全部来自本地事实，不经过 LLM。**
fn suggest(vocab: &BTreeSet<String>, existing: &[String]) -> String {
    let used: BTreeSet<String> = existing.iter().map(|t| normalize(t)).collect();
    let picks: Vec<&str> = vocab
        .iter()
        .filter(|v| !used.contains(*v))
        .map(|s| s.as_str())
        .take(MAX_SUGGESTIONS)
        .collect();
    if picks.is_empty() {
        return "Pick one each for the language, the domain, and the shape it ships in".to_string();
    }
    format!("Candidates: {}", picks.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab(words: &[&str]) -> BTreeSet<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn topic_matches_vocabulary_by_containment() {
        let v = vocab(&["rust", "cli"]);
        assert!(matches_vocab("rust", &v));
        assert!(matches_vocab("rust-library", &v));
        assert!(matches_vocab("CLI", &v));
        assert!(!matches_vocab("machine-learning", &v));
        // 反方向不认：否则 `cli` 会被词表里的 `client` 认领
        assert!(!matches_vocab("clu", &vocab(&["cluster"])));
    }

    #[test]
    fn suggestions_exclude_already_used_topics() {
        let v = vocab(&["rust", "cli", "search"]);
        let s = suggest(&v, &["rust".to_string()]);
        assert!(!s.contains("rust"));
        assert!(s.contains("cli"));
    }
}
