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
            return Outcome::inconclusive("不是 git 仓库，无法读取 tag");
        };

        // 浅克隆里的 tag 一律不可信，有几个都一样。`git clone --depth 1` 会顺带
        // 带上恰好指向 HEAD 的那一个 tag——ripgrep 的克隆里只有 `ignore-0.4.33`
        // 这个子 crate tag，据此会得出「该项目不用语义化版本」的错误结论。
        if git.shallow {
            return Outcome::inconclusive(format!(
                "浅克隆，tag 不完整（本地可见 {} 个），无法判断发布节奏。\
                 CI 里 actions/checkout 默认 fetch-depth: 1，需改为 fetch-depth: 0 或加 fetch-tags: true",
                git.tags.len()
            ));
        }

        if git.tags.is_empty() {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "仓库没有任何 tag")],
                vec![Fix::new(
                    Severity::P2,
                    "打第一个版本 tag。没有 tag 的项目，使用者无法锁定版本，\
                     也无从判断两次提交之间发生了什么",
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
            (None, n) if n > 0 => format!("{n} 个附注 tag 带说明"),
            _ => String::new(),
        };

        // tag 存在但不是 x.y.z：包管理器、依赖机器人、比较链接都解析不了
        if semver == 0 {
            return Outcome::scored(
                5,
                vec![Evidence::new(
                    ".",
                    format!("{total} 个 tag，但没有一个是 x.y.z 形态（如 `{}`）", git.tags[0].name),
                )],
                vec![Fix::new(
                    Severity::P2,
                    "改用语义化版本给 tag 命名（`v1.2.3`）。\
                     依赖更新机器人与「版本对比」链接都依赖这个格式",
                )],
            );
        }

        if !has_notes {
            return Outcome::scored(
                7,
                vec![Evidence::new(
                    ".",
                    format!("{semver} 个语义化版本 tag，但没有 CHANGELOG，tag 也没有说明"),
                )],
                vec![Fix::new(
                    Severity::P2,
                    "加 CHANGELOG.md，或改用附注 tag（`git tag -a`）写发布说明。\
                     使用者升级前唯一想知道的是「这个版本改了什么、有没有破坏性变更」",
                )],
            );
        }

        Outcome::perfect(vec![Evidence::new(
            changelog.as_deref().unwrap_or("."),
            format!("{semver} 个语义化版本 tag，发布说明见 {notes_where}"),
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
