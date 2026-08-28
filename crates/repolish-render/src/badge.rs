//! `.repolish/badge.json` 与可粘贴的徽章 snippet。
//!
//! 遵循 [shields.io endpoint 协议]。整个分发回路就架在这上面：用户仓库里的
//! 这个 JSON 由他自己的 `raw.githubusercontent.com` 提供，shields.io 去读它，
//! 徽章本身链回 repolish。我们不托管任何东西。
//!
//! [shields.io endpoint 协议]: https://shields.io/badges/endpoint-badge

use repolish_core::{Mode, Report};
use serde::Serialize;

/// 徽章文件在用户仓库中的位置。改动它等于让所有已存在的徽章失效。
pub const BADGE_PATH: &str = ".repolish/badge.json";

/// 徽章指回的地址——这是整个 CLI-only 分发策略的落点
pub const REPOLISH_URL: &str = "https://github.com/asale-ai/repolish";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Badge<'a> {
    /// shields.io **endpoint 协议**的版本号，恒为 1。
    /// 与我们自己 JSON 报告的 `schemaVersion` 无关，只是同名。
    schema_version: u8,
    label: String,
    message: String,
    color: &'a str,
    /// 以下两个是非标准字段，shields.io 会忽略，留给人和工具看
    repolish_version: &'a str,
    mode: &'a str,
}

/// 生成 badge.json。覆盖不足以致没有总分时返回 `None`——
/// 与其挂一个「N/A」的徽章，不如不挂。
pub fn badge_json(report: &Report) -> Option<String> {
    let score = report.score?;
    let badge = Badge {
        schema_version: 1,
        label: label_for(report.mode),
        message: format!("{score}/100"),
        color: report.color(),
        repolish_version: report.repolish_version,
        mode: report.mode.as_str(),
    };
    // 末尾补换行：这个文件会被 CI 提交，没有换行的文件在 diff 里很难看
    serde_json::to_string_pretty(&badge).ok().map(|s| s + "\n")
}

/// 本地分与远程分基准不同，标签必须让读者一眼看出是哪一种。
fn label_for(mode: Mode) -> String {
    match mode {
        Mode::Remote => "repolish".to_string(),
        Mode::Local => "repolish (local)".to_string(),
    }
}

/// 可直接粘进 README 的 markdown。
///
/// `branch` 是徽章 JSON 所在的分支——raw.githubusercontent.com 的 URL 里必须写死
/// 一个 ref，写默认分支即可。
pub fn snippet(owner: &str, repo: &str, branch: &str) -> String {
    styled_snippet(owner, repo, branch, "flat")
}

/// 带 shields 样式的版本。`style` 取 shields 的 `?style=` 取值。
///
/// `flat` 是 shields 自己的默认值，因此**不写进 URL**：一排徽章里只有我们这个
/// 拖着一个多余的参数，看起来像是有什么特别之处，而它并没有。
pub fn styled_snippet(owner: &str, repo: &str, branch: &str, style: &str) -> String {
    format!(
        "[![repolish]({})]({REPOLISH_URL})",
        shield_url(owner, repo, branch, style)
    )
}

/// HTML 形式，给需要 `<p align="center">` 包裹的场合——
/// 居中块里放 Markdown 图片语法是不渲染的。
pub fn styled_snippet_html(owner: &str, repo: &str, branch: &str, style: &str) -> String {
    format!(
        r#"<a href="{REPOLISH_URL}"><img src="{}" alt="repolish"></a>"#,
        shield_url(owner, repo, branch, style)
    )
}

fn shield_url(owner: &str, repo: &str, branch: &str, style: &str) -> String {
    let suffix = if style == "flat" {
        String::new()
    } else {
        format!("&style={style}")
    };
    format!(
        "https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{BADGE_PATH}{suffix}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use repolish_core::{Category, CheckResult, Outcome, Profile, ProfileInfo, Repository, Risk};

    fn report(score: u8, mode: Mode) -> Report {
        // 用一个 Critical 项凑出想要的分数：score×10 即为百分制总分
        let checks = vec![CheckResult {
            id: "x",
            category: Category::Credibility,
            risk: Risk::Critical,
            outcome: Outcome::Scored {
                score: score / 10,
                evidence: vec![],
                fixes: vec![],
            },
        }];
        Report::build(
            checks,
            Repository {
                owner: Some("acme".into()),
                name: "widget".into(),
                commit: None,
            },
            ProfileInfo {
                detected: Profile::Cli,
                overridden: false,
            },
            mode,
        )
    }

    #[test]
    fn remote_badge_matches_the_shields_endpoint_contract() {
        let json = badge_json(&report(90, Mode::Remote)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["schemaVersion"], 1);
        assert_eq!(v["label"], "repolish");
        assert_eq!(v["message"], "90/100");
        assert_eq!(v["color"], "brightgreen");
        assert_eq!(v["mode"], "remote");
    }

    #[test]
    fn local_badge_is_labelled_so_it_cannot_be_mistaken_for_a_full_score() {
        let json = badge_json(&report(90, Mode::Local)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["label"], "repolish (local)");
    }

    #[test]
    fn colors_follow_the_documented_thresholds() {
        for (score, want) in [
            (100, "brightgreen"),
            (80, "green"),
            (60, "yellow"),
            (40, "orange"),
            (30, "red"),
        ] {
            let json = badge_json(&report(score, Mode::Remote)).unwrap();
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(v["color"], want, "score {score}");
        }
    }

    #[test]
    fn snippet_points_at_the_users_own_raw_url_and_links_back_to_repolish() {
        let s = snippet("acme", "widget", "main");
        assert!(s.contains("raw.githubusercontent.com/acme/widget/main/.repolish/badge.json"));
        assert!(s.ends_with(&format!("]({REPOLISH_URL})")));
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;

    /// flat 是 shields 的默认值，写进 URL 只是噪声
    #[test]
    fn the_default_style_adds_no_query_parameter() {
        assert!(!styled_snippet("o", "r", "main", "flat").contains("style="));
    }

    #[test]
    fn a_non_default_style_reaches_shields() {
        let s = styled_snippet("o", "r", "main", "for-the-badge");
        assert!(s.contains("&style=for-the-badge"), "{s}");
        assert!(s.contains(BADGE_PATH), "{s}");
    }

    /// 居中块里放 Markdown 图片语法是不渲染的，必须给 HTML
    #[test]
    fn the_html_form_is_a_real_anchor_and_image() {
        let s = styled_snippet_html("o", "r", "main", "flat-square");
        assert!(s.starts_with("<a href="), "{s}");
        assert!(s.contains("<img src="), "{s}");
        assert!(s.contains("alt=\"repolish\""), "{s}");
        assert!(!s.contains("!["), "不该混入 Markdown 语法: {s}");
    }

    #[test]
    fn both_forms_point_at_the_same_file() {
        let md = styled_snippet("acme", "widget", "main", "plastic");
        let html = styled_snippet_html("acme", "widget", "main", "plastic");
        let path = "raw.githubusercontent.com/acme/widget/main/.repolish/badge.json";
        assert!(md.contains(path) && html.contains(path));
    }
}
