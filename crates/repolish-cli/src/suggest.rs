//! `polish --suggest`：让模型写那几段**只有人能写**的文字。
//!
//! # 为什么这不违反「评分路径无模型」
//!
//! 那条规则的原话是：同一个 commit 必须永远得到同一个分数，否则徽章没有公信力
//! （docs/01-architecture）。它约束的是**评分**，不是**修复**。
//!
//! 把它顺手延伸到修复上，代价是把这个工具最有价值的一半让了出去：权重最高的
//! 三项——标题与一句话简介（Critical）、快速开始（Critical）、用法示例（High）
//! ——恰好全是「机械方法写不出来」的。`polish` 现在能做的只有插徽章和目录，
//! 而那不是使用者卡住的地方。
//!
//! 所以这里的边界画在别处，画得更死：
//!
//! - **建议永远不落盘。** 连 `--apply` 都不写。它打印文本，由使用者自己贴。
//!   一段模型写的文字进了别人的 README 而他没有逐字看过，是不能接受的。
//! - **只补缺的那一段，绝不重写已有的。** README 里已经有的标题、段落、示例
//!   一个字都不碰。「让 agent 改 README，它第一件事是把整个文件重写一遍」
//!   正是这个工具存在的理由。
//! - **一个分数都不动。** 建议生成前后跑 `check` 得到的数字完全一样。
//!   输出里也照直说。

use std::time::Duration;

use repolish_core::{Outcome, Report};
use repolish_ingest::RepoContext;
use serde::Deserialize;

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-opus-5";
const TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TOKENS: u32 = 4000;

/// 会请模型写的三项。**清单是封死的**——不是「所有扣分项」。
///
/// 其余 19 项要么是机械的（`polish` 已经在做），要么是「去做一件事」而不是
/// 「写一段话」（加 LICENSE、配 CI）。让模型对着它们发挥，产出的是噪声。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Tagline,
    Quickstart,
    UsageExample,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Tagline, Kind::Quickstart, Kind::UsageExample];

    pub fn check_id(self) -> &'static str {
        match self {
            Kind::Tagline => "readme-title-tagline",
            Kind::Quickstart => "readme-quickstart",
            Kind::UsageExample => "readme-usage-example",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Tagline => "title and tagline",
            Kind::Quickstart => "quick start",
            Kind::UsageExample => "usage example",
        }
    }

    fn parse(id: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.check_id() == id)
    }
}

/// 报告里这三项哪些没拿满分。满分的不问——给一个已经写好的 tagline
/// 再生成一个「更好的」，是这条功能最容易滑向的地方，也是最没用的。
pub fn wanted(report: &Report) -> Vec<Kind> {
    Kind::ALL
        .into_iter()
        .filter(|k| {
            report
                .checks
                .iter()
                .find(|c| c.id == k.check_id())
                .map(|c| matches!(c.outcome, Outcome::Scored { score, .. } if score < 10))
                .unwrap_or(false)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub kind: Kind,
    /// 建议贴进 README 的文本，原样
    pub text: String,
    /// 为什么这么写。一句话,给使用者判断要不要采纳
    pub why: String,
}

// ── 提问 ────────────────────────────────────────────────────────────────

/// 交给模型的事实。**只放仓库里真实存在的东西**——模型编不出一个
/// 不存在的安装命令，如果我们没给它编造的余地。
pub struct Facts {
    pub name: String,
    pub description: Option<String>,
    pub languages: Vec<String>,
    pub install_commands: Vec<String>,
    pub binaries: Vec<String>,
    pub scripts: Vec<String>,
    /// README 现有的开头,让建议接得上作者的语气
    pub readme_head: String,
    pub has_title: bool,
}

impl Facts {
    pub fn from_ctx(ctx: &RepoContext) -> Self {
        let readme = ctx.readme.as_ref();
        let head = readme
            .map(|r| r.raw.lines().take(40).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();

        let install = ctx
            .manifests
            .iter()
            .filter_map(|m| {
                let name = m.name.as_deref()?;
                Some(match m.ecosystem {
                    repolish_ingest::Ecosystem::Cargo => format!("cargo install {name}"),
                    repolish_ingest::Ecosystem::Npm => format!("npm install {name}"),
                    repolish_ingest::Ecosystem::Pypi => format!("pip install {name}"),
                    repolish_ingest::Ecosystem::Go => format!("go get {name}"),
                    repolish_ingest::Ecosystem::Gem => format!("gem install {name}"),
                    repolish_ingest::Ecosystem::Composer => format!("composer require {name}"),
                    repolish_ingest::Ecosystem::Maven => format!("(maven) {name}"),
                })
            })
            .collect();

        Facts {
            name: ctx.display_name(),
            description: ctx.remote.as_ref().and_then(|r| r.description.clone()),
            languages: repolish_ingest::lang::stats(&ctx.files)
                .into_iter()
                .take(3)
                .map(|s| s.name.to_string())
                .collect(),
            install_commands: install,
            binaries: ctx
                .manifests
                .iter()
                .flat_map(|m| m.bins.clone())
                .take(8)
                .collect(),
            scripts: ctx
                .manifests
                .iter()
                .flat_map(|m| m.scripts.clone())
                .take(12)
                .collect(),
            readme_head: head,
            has_title: readme.map(|r| r.title.is_some()).unwrap_or(false),
        }
    }
}

/// 提示词。纯函数，好测——这段文字决定了产出有没有用，
/// 它值得像代码一样被盯着。
pub fn prompt(facts: &Facts, kinds: &[Kind]) -> String {
    let mut s = String::new();
    s.push_str(
        "You are helping an open-source author fill in the parts of a README that a \
         static tool cannot write for them.\n\n\
         Hard rules:\n\
         - Use ONLY the facts given below. If a fact you need is missing, say so in `why` \
           and leave `text` empty. Never invent an install command, a flag, an API or a URL.\n\
         - Match the voice already in the README excerpt: same person, same register, \
           same amount of punctuation. Do not add marketing adjectives \
           (\"powerful\", \"blazing fast\", \"seamless\", \"revolutionary\").\n\
         - Write the missing piece only. Do not rewrite, restructure or summarise \
           anything that is already there.\n\
         - Plain GitHub-flavoured Markdown. No preamble, no closing remark.\n\n",
    );

    s.push_str("Facts about this repository:\n");
    s.push_str(&format!("- name: {}\n", facts.name));
    if let Some(d) = &facts.description {
        s.push_str(&format!("- repository description: {d}\n"));
    }
    if !facts.languages.is_empty() {
        s.push_str(&format!("- languages: {}\n", facts.languages.join(", ")));
    }
    if !facts.install_commands.is_empty() {
        s.push_str(&format!(
            "- install commands that are real (from the package manifest): {}\n",
            facts.install_commands.join(" | ")
        ));
    }
    if !facts.binaries.is_empty() {
        s.push_str(&format!("- executables: {}\n", facts.binaries.join(", ")));
    }
    if !facts.scripts.is_empty() {
        s.push_str(&format!(
            "- runnable scripts declared in the manifest: {}\n",
            facts.scripts.join(", ")
        ));
    }
    s.push_str(&format!(
        "- the README already has a title: {}\n",
        facts.has_title
    ));

    s.push_str("\nThe first 40 lines of the current README:\n```markdown\n");
    s.push_str(&facts.readme_head);
    s.push_str("\n```\n\n");

    s.push_str("Write these pieces:\n");
    for k in kinds {
        s.push_str(match k {
            Kind::Tagline => {
                "- readme-title-tagline: an H1 with the project name, then ONE line, \
                 under 90 characters, saying what this is and who it is for. \
                 If the README already has a title, give only the tagline line.\n"
            }
            Kind::Quickstart => {
                "- readme-quickstart: an \"## Install\" or \"## Quick start\" section with \
                 a fenced command block. Include prerequisites only if the facts name one. \
                 Every command must come from the facts above.\n"
            }
            Kind::UsageExample => {
                "- readme-usage-example: a \"## Usage\" section with one fenced, copyable \
                 example, the fence tagged with its language. Show the single most common \
                 thing a first-time user does, not an exhaustive tour.\n"
            }
        });
    }

    s.push_str(
        "\nAnswer with a single JSON object and nothing else:\n\
         {\"suggestions\": [{\"check\": \"<the id above>\", \"text\": \"<markdown to paste>\", \
         \"why\": \"<one sentence>\"}]}\n",
    );
    s
}

// ── 解析 ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Envelope {
    suggestions: Vec<Raw>,
}

#[derive(Deserialize)]
struct Raw {
    check: String,
    text: String,
    #[serde(default)]
    why: String,
}

/// 模型的回答 → 建议列表。
///
/// 认得出围栏包着的 JSON：模型被要求只回 JSON，但偶尔会裹一层 ```json。
/// 为这一层多写四行，好过让使用者拿到一句「解析失败」。
pub fn parse(response: &str) -> Result<Vec<Suggestion>, String> {
    let body = unfence(response.trim());
    let env: Envelope = serde_json::from_str(body)
        .map_err(|e| format!("the model did not answer with the JSON we asked for: {e}"))?;

    let out: Vec<Suggestion> = env
        .suggestions
        .into_iter()
        .filter(|r| !r.text.trim().is_empty())
        .filter_map(|r| {
            Kind::parse(&r.check).map(|kind| Suggestion {
                kind,
                text: r.text.trim().to_string(),
                why: r.why.trim().to_string(),
            })
        })
        .collect();
    Ok(out)
}

fn unfence(s: &str) -> &str {
    let Some(rest) = s.strip_prefix("```") else {
        return s;
    };
    // ```json\n…\n```
    let rest = rest.split_once('\n').map(|(_, r)| r).unwrap_or(rest);
    rest.trim_end().strip_suffix("```").unwrap_or(rest).trim()
}

// ── 调用 ────────────────────────────────────────────────────────────────

pub struct Model {
    pub key: String,
    pub model: String,
    pub base_url: String,
}

/// 密钥来源，按优先级。`REPOLISH_LLM_API_KEY` 排第一，
/// 好让人给这个工具单独配一把额度有限的钥匙。
pub fn key_from_env() -> Option<String> {
    ["REPOLISH_LLM_API_KEY", "ANTHROPIC_API_KEY"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .filter(|v| !v.trim().is_empty())
}

impl Model {
    pub fn resolve(cfg: &crate::config::Suggest) -> Result<Model, String> {
        let key = key_from_env().ok_or_else(|| {
            "no API key. --suggest calls a model, so it needs one:\n  \
             export REPOLISH_LLM_API_KEY=…   (or ANTHROPIC_API_KEY)\n\
             Nothing else in repolish talks to a model, and no score depends on this."
                .to_string()
        })?;
        Ok(Model {
            key,
            model: cfg
                .model
                .clone()
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            base_url: cfg.base_url.clone().unwrap_or_else(|| ENDPOINT.to_string()),
        })
    }
}

/// 问一次。失败一律返回错误,不静默降级成「没有建议」——
/// 使用者付了一次 API 调用的钱,至少要知道它去哪了。
pub fn ask(model: &Model, prompt: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();

    let body = serde_json::json!({
        "model": model.model,
        "max_tokens": MAX_TOKENS,
        "messages": [{ "role": "user", "content": prompt }],
    });

    let mut res = agent
        .post(&model.base_url)
        .header("content-type", "application/json")
        .header("x-api-key", &model.key)
        .header("anthropic-version", API_VERSION)
        .header(
            "user-agent",
            concat!("repolish/", env!("CARGO_PKG_VERSION")),
        )
        .send_json(&body)
        .map_err(|e| format!("could not reach the model API: {e}"))?;

    let status = res.status().as_u16();
    let json: serde_json::Value = res
        .body_mut()
        .read_json()
        .map_err(|e| format!("the model API answered with something that is not JSON: {e}"))?;

    if status != 200 {
        let msg = json["error"]["message"]
            .as_str()
            .unwrap_or("no message in the response");
        return Err(format!("the model API answered {status}: {msg}"));
    }
    // 安全分类器可以拒答,返回的仍然是 200。不看这个字段的话,
    // 使用者会拿到一句「模型没有回 JSON」而完全不知道发生了什么。
    if json["stop_reason"] == "refusal" {
        return Err("the model declined to answer this request".to_string());
    }

    Ok(text_of(&json))
}

/// 取回答里的文本块。回答可能是多块，拼起来。
fn text_of(json: &serde_json::Value) -> String {
    json["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

// ── 呈现 ────────────────────────────────────────────────────────────────

use repolish_render::theme::{self, ColorLevel};

/// 打印建议。**一个字都不写进文件**,所以这里就是全部产出。
pub fn render(suggestions: &[Suggestion], model: &str, level: ColorLevel) -> String {
    use std::fmt::Write as _;

    let ink = |t: &str, c: theme::Rgb| format!("{}{t}{}", theme::fg(c, level), theme::reset(level));
    let strong = |t: &str, c: theme::Rgb| {
        format!(
            "{}{}{t}{}",
            theme::bold(level),
            theme::fg(c, level),
            theme::reset(level)
        )
    };
    let dim = |t: &str| ink(t, theme::MUTED);

    let mut s = String::new();
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "  {}  {}",
        strong("SUGGESTED WORDING", theme::PINK),
        dim(&format!("written by {model}"))
    );
    let _ = writeln!(
        s,
        "  {}",
        dim("Not written to any file, and not counted in any score. Paste what you like.")
    );

    for sg in suggestions {
        let _ = writeln!(s);
        let _ = writeln!(
            s,
            "  {} {}",
            strong(sg.kind.label(), theme::CYAN),
            dim(&format!("({})", sg.kind.check_id()))
        );
        if !sg.why.is_empty() {
            let _ = writeln!(s, "  {}", dim(&sg.why));
        }
        let _ = writeln!(s);
        for line in sg.text.lines() {
            let _ = writeln!(s, "    {line}");
        }
    }

    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "  {}",
        dim(&format!(
            "Re-run `{} check .` after pasting: the score moves because the README \
             changed,\n  not because a model was involved.",
            crate::invocation()
        ))
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            name: "widget".into(),
            description: Some("A widget".into()),
            languages: vec!["Rust".into()],
            install_commands: vec!["cargo install widget".into()],
            binaries: vec!["widget".into()],
            scripts: vec![],
            readme_head: "# widget\n".into(),
            has_title: true,
        }
    }

    /// 提示词必须把「只能用给定事实」说死。模型编一条不存在的安装命令,
    /// 恰好是 claim-consistency 存在的理由。
    #[test]
    fn the_prompt_forbids_inventing_commands() {
        let p = prompt(&facts(), &[Kind::Quickstart]);
        assert!(p.contains("Use ONLY the facts"));
        assert!(p.contains("Never invent an install command"));
        assert!(p.contains("cargo install widget"));
    }

    /// 「不要重写已有内容」这句必须在,那是这个工具存在的理由
    #[test]
    fn the_prompt_forbids_rewriting_what_is_already_there() {
        let p = prompt(&facts(), &Kind::ALL);
        assert!(p.contains("Do not rewrite"));
    }

    #[test]
    fn a_plain_json_answer_parses() {
        let r = parse(
            "{\"suggestions\":[{\"check\":\"readme-quickstart\",\
             \"text\":\"## Install\\n\\n`x`\",\"why\":\"none present\"}]}",
        )
        .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, Kind::Quickstart);
        assert!(r[0].text.starts_with("## Install"));
    }

    #[test]
    fn a_fenced_json_answer_parses_too() {
        let r = parse("```json\n{\"suggestions\":[{\"check\":\"readme-usage-example\",\"text\":\"## Usage\",\"why\":\"\"}]}\n```")
            .unwrap();
        assert_eq!(r[0].kind, Kind::UsageExample);
    }

    /// 模型说「我缺一个事实」时会回一段空文本。空建议不该出现在输出里
    #[test]
    fn an_empty_suggestion_is_dropped_rather_than_printed() {
        let r = parse(
            r#"{"suggestions":[{"check":"readme-quickstart","text":"  ","why":"no manifest"}]}"#,
        )
        .unwrap();
        assert!(r.is_empty());
    }

    /// 清单是封死的三项。模型自作主张给别的检查项写建议,一律丢掉
    #[test]
    fn suggestions_for_checks_outside_the_three_are_ignored() {
        let r = parse(r#"{"suggestions":[{"check":"license","text":"MIT","why":"x"}]}"#).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn a_non_json_answer_says_so_rather_than_panicking() {
        assert!(parse("Sure! Here are some ideas:").is_err());
    }

    #[test]
    fn only_the_checks_that_lost_points_are_asked_about() {
        use repolish_core::{
            Category, CheckResult, Fix, Mode, ProfileInfo, Repository, Risk, Severity,
        };
        let mk = |id: &'static str, score: u8| CheckResult {
            id,
            category: Category::Comprehensibility,
            risk: Risk::Critical,
            outcome: if score == 10 {
                Outcome::perfect(vec![])
            } else {
                Outcome::scored(score, vec![], vec![Fix::new(Severity::P1, "x")])
            },
        };
        let report = Report::build(
            vec![
                mk("readme-title-tagline", 10),
                mk("readme-quickstart", 0),
                mk("readme-usage-example", 4),
            ],
            Repository {
                owner: None,
                name: "w".into(),
                commit: None,
            },
            ProfileInfo {
                detected: repolish_ingest::Profile::Cli,
                overridden: false,
            },
            Mode::Local,
        );
        assert_eq!(wanted(&report), vec![Kind::Quickstart, Kind::UsageExample]);
    }
}
