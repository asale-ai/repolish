//! `release-hygiene`：有没有正经发过版。
//!
//! **浅克隆必须先排除。** `actions/checkout@v4` 默认 `fetch-depth: 1`，不拉 tag。
//! 若不区分「仓库没有 tag」与「tag 没被拉下来」，这一项在 CI 里会给每个项目判 0 分，
//! 而那正是我们希望用户长期挂着的运行环境——一条系统性误判足以让人卸载工具。

use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

pub struct ReleaseHygiene;

const CHANGELOGS: &[&str] = &[
    "changelog.md",
    "changelog",
    "changelog.rst",
    "changes.md",
    "history.md",
    "news.md",
    "releases.md",
    "release-notes.md",
    "更新日志.md",
];

impl Check for ReleaseHygiene {
    fn id(&self) -> &'static str {
        "release-hygiene"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::Medium
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(git) = &ctx.git else {
            return Outcome::inconclusive("not a git repository, so tags cannot be read");
        };

        // 浅克隆里的 tag 一律不可信，有几个都一样。`git clone --depth 1` 会顺带
        // 带上恰好指向 HEAD 的那一个 tag——ripgrep 的克隆里只有 `ignore-0.4.33`
        // 这个子 crate tag，据此会得出「该项目不用语义化版本」的错误结论。
        if git.shallow {
            return Outcome::inconclusive(format!(
                "shallow clone with incomplete tags ({} visible locally), so the release \
                 cadence cannot be judged. In CI, actions/checkout defaults to \
                 fetch-depth: 1 — set fetch-depth: 0 or add fetch-tags: true",
                git.tags.len()
            ));
        }

        if git.tags.is_empty() {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "the repository has no tags at all")],
                vec![Fix::new(
                    Severity::P2,
                    "Cut a first release tag. Without tags nobody can pin a version, and \
                     nobody can tell what changed between one commit and the next",
                )],
            );
        }

        let total = git.tags.len();
        let semver = git.semver_tags().count();
        let changelog = find_changelog(ctx);
        let annotated = git.tags.iter().filter(|t| t.message.is_some()).count();
        let has_notes = changelog.is_some() || annotated > 0;

        let notes_where = match (&changelog, annotated) {
            (Some(c), _) => c.clone(),
            (None, n) if n > 0 => format!(
                "{n} annotated tag{} carrying release notes",
                crate::util::plural(n)
            ),
            _ => String::new(),
        };

        // tag 存在但不是 x.y.z：包管理器、依赖机器人、比较链接都解析不了
        if semver == 0 {
            return Outcome::scored(
                5,
                vec![Evidence::new(
                    ".",
                    // 「1 tag, none of them」在别人的 REPOLISH.md 里读起来
                    // 就是机器写的。plural() 管得了名词后缀，管不了代词。
                    if total == 1 {
                        format!("1 tag, and it is not in x.y.z form (`{}`)", git.tags[0].name)
                    } else {
                        format!(
                            "{total} tags, none of them in x.y.z form (for example `{}`)",
                            git.tags[0].name
                        )
                    },
                )],
                vec![Fix::new(
                    Severity::P2,
                    "Name the tags with semantic versions (`v1.2.3`). Dependency update \
                     bots and compare-version links both depend on that shape",
                )],
            );
        }

        if !has_notes {
            return Outcome::scored(
                7,
                vec![Evidence::new(
                    ".",
                    format!(
                        "{semver} semver tag{}, but no CHANGELOG and no notes on the tags either",
                        crate::util::plural(semver)
                    ),
                )],
                vec![Fix::new(
                    Severity::P2,
                    "Add CHANGELOG.md, or write release notes into annotated tags \
                     (`git tag -a`). Before upgrading, the only thing anyone wants to know \
                     is what changed and whether it breaks them",
                )],
            );
        }

        Outcome::perfect(vec![Evidence::new(
            changelog.as_deref().unwrap_or("."),
            format!(
                "{semver} semver tag{}, with release notes in {notes_where}",
                crate::util::plural(semver)
            ),
        )])
    }
}

fn find_changelog(ctx: &RepoContext) -> Option<String> {
    ctx.files
        .iter()
        .find(|p| {
            let l = p.to_lowercase();
            let name = l.rsplit('/').next().unwrap_or(&l);
            // 只认根目录与 docs/：别处的 changelog 多半是依赖或子模块的
            (!l.contains('/') || l.starts_with("docs/")) && CHANGELOGS.contains(&name)
        })
        .map(str::to_string)
}
