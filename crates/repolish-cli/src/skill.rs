//! `skill` —— 写出 `SKILL.md`，教编码智能体怎么用 repolish。
//!
//! 为什么要有这个文件：一个智能体拿到「把这个仓库的 README 弄好」这样的
//! 任务时，默认做法是**直接开始重写 README**。那正是我们花了整个工具去反对
//! 的做法——它把作者的排版、语气和已有内容一并冲掉，换来一份看着标准、
//! 读着没有人味的说明。
//!
//! 所以这份文件的重点不在「有哪些命令」（`--help` 就写着），而在**顺序和
//! 边界**：先量，再改能机械改的，最后把需要判断的那几条留给人。
//!
//! 内容写死在这里，不从磁盘读：`skill` 阶段要能在任何一个目录下跑出
//! 同一份文件，包括 `cargo install` 之后没有仓库可读的那种情况。
//!
//! 两种用法，落点不同：
//!
//! - `repolish --stages skill` 写进**一个仓库**（`SKILL.md`），跟着代码走，
//!   谁 clone 谁就有。
//! - `repolish --stages skill --target claude` 写进**这台机器上的智能体**
//!   （`~/.claude/skills/repolish/SKILL.md`），装一次，所有项目都用得上。

use std::path::PathBuf;

/// 写进一个仓库时的默认位置
pub const SKILL_PATH: &str = "SKILL.md";
/// 技能在各家智能体目录下的名字
pub const SKILL_NAME: &str = "repolish";

pub fn markdown() -> String {
    include_str!("skill.md").replace("%VERSION%", env!("CARGO_PKG_VERSION"))
}

// ── 装到智能体里 ────────────────────────────────────────────

/// 一家智能体放技能的地方。
///
/// 路径全部相对**用户主目录**，不是相对仓库：技能装一次，此后在哪个项目里
/// 都用得上。装进项目目录的话，每开一个新仓库都要再装一遍。
pub struct Target {
    pub id: &'static str,
    pub label: &'static str,
    /// 相对主目录的技能根目录
    pub skills_dir: &'static str,
    /// 用来判断「这家工具装了没有」的目录。装了才算探测到。
    pub probe_dir: &'static str,
    /// Gemini 要额外一份 `gemini-extension.json` 描述这个扩展
    pub gemini_manifest: bool,
    pub docs: &'static str,
}

/// 已知的落点。
///
/// 这张表**只列我们确认过路径的**。多列一个猜的目录，代价是使用者以为装好了、
/// 实际上那家工具永远读不到——那比不支持它更糟。
pub const TARGETS: &[Target] = &[
    Target {
        id: "claude",
        label: "Claude Code",
        skills_dir: ".claude/skills",
        probe_dir: ".claude",
        gemini_manifest: false,
        docs: "https://docs.claude.com/en/docs/claude-code/skills",
    },
    Target {
        id: "codex",
        label: "OpenAI Codex CLI",
        skills_dir: ".codex/skills",
        probe_dir: ".codex",
        gemini_manifest: false,
        docs: "https://developers.openai.com/codex/",
    },
    Target {
        id: "gemini",
        label: "Gemini CLI",
        skills_dir: ".gemini/extensions/repolish/skills",
        probe_dir: ".gemini",
        gemini_manifest: true,
        docs: "https://google-gemini.github.io/gemini-cli/docs/extensions/",
    },
    Target {
        id: "opencode",
        label: "OpenCode",
        skills_dir: ".config/opencode/skills",
        probe_dir: ".config/opencode",
        gemini_manifest: false,
        docs: "https://opencode.ai/docs/skills/",
    },
    Target {
        id: "agents",
        label: "AGENTS.md-compatible agents",
        skills_dir: ".agents/skills",
        probe_dir: ".agents",
        gemini_manifest: false,
        docs: "https://agents.md",
    },
];

impl Target {
    pub fn find(id: &str) -> Option<&'static Target> {
        TARGETS.iter().find(|t| t.id == id)
    }

    /// 这家工具在这台机器上装了吗
    pub fn detected(&self, home: &std::path::Path) -> bool {
        home.join(self.probe_dir).is_dir()
    }

    /// SKILL.md 的落点
    pub fn skill_path(&self, home: &std::path::Path) -> PathBuf {
        home.join(self.skills_dir).join(SKILL_NAME).join(SKILL_PATH)
    }
}

/// 主目录。没有 `dirs` 依赖——两个环境变量就够了，为此多拉一个 crate 不划算。
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Gemini 的扩展清单。
///
/// 清单里点名了一个上下文文件，所以那个文件必须一起写出去——
/// 只写清单会让 Gemini CLI 每次启动都指向一个不存在的路径。
pub fn gemini_manifest() -> String {
    format!(
        "{{\n  \"name\": \"repolish\",\n  \"version\": \"{}\",\n  \
         \"description\": \"Score and improve what an open-source repository looks like \
         to a first-time visitor.\",\n  \"contextFileName\": \"GEMINI.md\",\n  \
         \"skills\": [\"{SKILL_NAME}/*\"]\n}}\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// 清单点名的那个上下文文件
pub fn gemini_context() -> String {
    format!(
        "# repolish\n\nThe `{SKILL_NAME}` skill in this extension explains how to \
         diagnose and improve a repository's README with the `repolish` binary.\n\n\
         Read `skills/{SKILL_NAME}/{SKILL_PATH}` before editing any README.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 智能体的技能文件靠 frontmatter 被发现。缺了它，这就是一份没人读的散文。
    #[test]
    fn it_starts_with_frontmatter_carrying_a_name_and_a_description() {
        let md = markdown();
        assert!(md.starts_with("---\n"), "缺少 frontmatter");
        let end = md[4..].find("\n---\n").expect("frontmatter 没有闭合");
        let front = &md[4..4 + end];
        assert!(front.contains("name: repolish"));
        assert!(front.contains("description:"));
    }

    /// 版本号占位符必须被替换掉——一份写着 `%VERSION%` 的文档
    /// 会让智能体照着装一个不存在的版本
    #[test]
    fn the_version_placeholder_is_substituted() {
        let md = markdown();
        assert!(!md.contains("%VERSION%"));
        assert!(md.contains(env!("CARGO_PKG_VERSION")));
    }

    /// 这份文件的全部意义就是那条顺序：先量，再机械修，最后交给人
    /// 落点写错，使用者会以为装好了而那家工具永远读不到——
    /// 比不支持它更糟，所以这张表的每一项都要能自洽
    #[test]
    fn every_target_is_well_formed_and_unique() {
        let mut ids = std::collections::HashSet::new();
        for t in TARGETS {
            assert!(ids.insert(t.id), "重复的 target id: {}", t.id);
            assert!(
                !t.skills_dir.starts_with('/'),
                "{} 的路径不该是绝对路径",
                t.id
            );
            assert!(!t.skills_dir.starts_with('~'), "{} 的路径不该带 ~", t.id);
            assert!(
                t.skills_dir.starts_with(t.probe_dir),
                "{} 的探测目录不在技能目录的路径上",
                t.id
            );
            assert!(t.docs.starts_with("https://"), "{} 缺少文档链接", t.id);
        }
        assert!(Target::find("claude").is_some());
        assert!(Target::find("nope").is_none());
    }

    #[test]
    fn the_skill_lands_under_the_agents_own_directory() {
        let home = std::path::Path::new("/home/x");
        let claude = Target::find("claude").unwrap();
        assert_eq!(
            claude.skill_path(home),
            std::path::Path::new("/home/x/.claude/skills/repolish/SKILL.md")
        );
    }

    /// 清单点名了一个上下文文件，那个文件必须一起写出去，
    /// 否则 Gemini CLI 每次启动都指向一个不存在的路径
    #[test]
    fn the_gemini_manifest_names_a_file_that_is_also_written() {
        let manifest = gemini_manifest();
        assert!(manifest.contains("\"contextFileName\": \"GEMINI.md\""));
        assert!(manifest.contains(env!("CARGO_PKG_VERSION")));
        // 拿 serde_json 解一遍，保证它真的是 JSON 而不是拼错的字符串
        let parsed: serde_json::Value = serde_json::from_str(&manifest).expect("清单不是合法 JSON");
        assert_eq!(parsed["name"], "repolish");
        assert_eq!(parsed["skills"][0], "repolish/*");
        assert!(gemini_context().contains("skills/repolish/SKILL.md"));
    }

    /// 技能文档教的必须是**现在**这套命令面。子命令取消之后，一份还在教
    /// `repolish check .` 的技能，会让智能体照着敲出 command not found。
    #[test]
    fn it_states_the_workflow_and_the_boundary() {
        let md = markdown();
        assert!(md.contains("--stages"));
        assert!(md.contains("--apply"));
        assert!(md.contains("Do not rewrite the README"));
        // 干跑优先是这份技能唯一的安全保证：智能体必须先给人看计划
        assert!(md.contains("without `--apply`"));
        // 每一段都要有名有姓，否则智能体只会跑默认那四段
        for stage in ["check", "polish", "artifacts", "ci", "skill", "demo"] {
            assert!(md.contains(stage), "skill.md never mentions the `{stage}` stage");
        }
        assert!(
            !md.contains("repolish check ."),
            "skill.md still teaches the removed subcommand form"
        );
    }
}
