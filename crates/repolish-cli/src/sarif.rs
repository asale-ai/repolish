//! SARIF 2.1.0 输出。
//!
//! 每一条扣分**本来就带着文件与行号**——那是这个工具从第一天起的设计原则。
//! SARIF 只是把已有的东西换一种编码，让 GitHub 把它渲染在 PR 的 diff 行内，
//! 而不是留在一段没人展开的 CI 日志里。
//!
//! 两条约束：
//!
//! - **不产生时间戳。** 同一个 commit 必须产出逐字节相同的文件，跟其余所有
//!   产物一个规矩。SARIF 允许 `invocation.startTimeUtc`，我们不写。
//! - **只报扣了分的。** SARIF 的 `results` 是「发现的问题」。把通过的检查也
//!   塞进去，PR 上就会出现 22 条注解，其中 19 条说「一切正常」。

use std::collections::BTreeMap;

use repolish_core::{Evidence, Outcome, Report, Severity};
use serde_json::{json, Value};

const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const INFO_URI: &str = "https://github.com/asale-ai/repolish";
const DOCS: &str = "https://github.com/asale-ai/repolish/blob/main/docs/03-scoring.md";

/// 检查项的一句话描述。取自 docs/03-scoring.md 的「What it looks for」一列。
///
/// 这里重复了一份文案，是有意的：SARIF 的规则元数据会显示在 GitHub 的
/// Security 标签页里，那里读不到我们的 docs。表在这里，改的时候两边一起改——
/// 底下有一个测试盯着「每个注册的检查项都必须有描述」。
const DESCRIPTIONS: &[(&str, &str)] = &[
    (
        "repo-description",
        "The repository description is non-empty and says what this is, not just the name",
    ),
    (
        "repo-topics",
        "A sensible number of topics, cross-checked against signals in the repository",
    ),
    ("repo-homepage", "The homepage field is set"),
    (
        "readme-title-tagline",
        "The first screen has a name and one line saying what this is",
    ),
    (
        "readme-badges",
        "Basic badges are present (build / version / licence)",
    ),
    (
        "readme-quickstart",
        "An install or quick-start section exists",
    ),
    ("readme-usage-example", "A copyable code example exists"),
    (
        "readme-install-consistency",
        "The install command matches the actual package manifest",
    ),
    (
        "readme-link-health",
        "Relative links and images point at files that exist",
    ),
    (
        "readme-length",
        "Neither too thin nor long enough to belong in docs/",
    ),
    ("readme-toc", "A long README offers a table of contents"),
    (
        "docs-presence",
        "A docs/ directory or a link to a documentation site",
    ),
    ("readme-i18n", "A translated README is offered"),
    (
        "license",
        "A LICENSE file exists and is a recognisable standard licence",
    ),
    (
        "claim-consistency",
        "Commands, scripts and APIs the README promises actually exist",
    ),
    ("ci-present", "A CI configuration exists"),
    ("tests-present", "A test directory or test files exist"),
    ("activity", "A commit within the last 90 days"),
    ("contributing", "A CONTRIBUTING file exists"),
    (
        "issue-pr-template",
        "Issue or pull request templates under .github/",
    ),
    ("release-hygiene", "Tags or releases exist, with notes"),
    ("code-of-conduct", "A code of conduct exists"),
];

fn describe(id: &str) -> &'static str {
    DESCRIPTIONS
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| *v)
        // 新加检查项忘了写描述时，规则仍然成立，只是描述空着——
        // 不值得为此让整个 SARIF 输出失败
        .unwrap_or("See the repolish scoring documentation")
}

fn level(s: Severity) -> &'static str {
    match s {
        Severity::P1 => "error",
        Severity::P2 => "warning",
        Severity::P3 => "note",
    }
}

/// 报告 → SARIF 文档。
pub fn sarif(report: &Report) -> String {
    // 规则表按注册顺序（也就是报告顺序）建，`ruleIndex` 才对得上
    let mut rule_index: BTreeMap<&str, usize> = BTreeMap::new();
    let mut rules: Vec<Value> = Vec::new();
    for (i, c) in report.checks.iter().enumerate() {
        rule_index.insert(c.id, i);
        rules.push(json!({
            "id": c.id,
            "name": c.id,
            "shortDescription": { "text": describe(c.id) },
            "helpUri": format!("{DOCS}#the-checks"),
            "properties": {
                "category": c.category.label().to_lowercase(),
                "risk": format!("{:?}", c.risk).to_lowercase(),
                "tags": ["repolish", c.category.label().to_lowercase()],
            },
        }));
    }

    let mut results: Vec<Value> = Vec::new();
    for c in &report.checks {
        let Outcome::Scored {
            score,
            evidence,
            fixes,
        } = &c.outcome
        else {
            continue;
        };
        if *score == 10 {
            continue;
        }
        // 每条 Fix 就是一条 result。证据可能有好几条（比如 5 个死链），
        // 每条证据各占一行注解——注解落在哪一行，是这份输出的全部意义。
        for fix in fixes {
            // 扣了分却一条证据都没有,是检查项写得不完整。仍然报出来:
            // 少一条注解,好过让一个真实的扣分从 PR 上消失。
            if evidence.is_empty() {
                results.push(result(
                    c.id,
                    rule_index[c.id],
                    fix.severity,
                    &fix.message,
                    None,
                ));
                continue;
            }
            for ev in evidence {
                let message = format!("{} — {}", fix.message, ev.note);
                results.push(result(
                    c.id,
                    rule_index[c.id],
                    fix.severity,
                    &message,
                    Some(ev),
                ));
            }
        }
    }

    let doc = json!({
        "$schema": SCHEMA,
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "repolish",
                "version": report.repolish_version,
                "informationUri": INFO_URI,
                "rules": rules,
            }},
            "results": results,
        }],
    });

    // 尾随换行：文件末尾没有换行的话，`git diff` 每次都会多出一行噪声
    format!(
        "{}\n",
        serde_json::to_string_pretty(&doc).unwrap_or_default()
    )
}

fn result(
    id: &str,
    index: usize,
    severity: Severity,
    message: &str,
    evidence: Option<&Evidence>,
) -> Value {
    let mut r = json!({
        "ruleId": id,
        "ruleIndex": index,
        "level": level(severity),
        "message": { "text": message },
    });

    let (uri, line) = match evidence {
        Some(ev) => (artifact_uri(ev), ev.line),
        None => (".repolish/".to_string(), None),
    };

    let mut region = json!({ "startLine": line.unwrap_or(1) });
    if line.is_none() {
        // 指向文件整体（或它的缺失）。GitHub 要求 region 有 startLine，
        // 所以落在第 1 行,并明说这一条不是行级的。
        region["properties"] = json!({ "wholeFile": true });
    }

    r["locations"] = json!([{
        "physicalLocation": {
            "artifactLocation": { "uri": uri, "uriBaseId": "%SRCROOT%" },
            "region": region,
        }
    }]);
    // 指纹让 GitHub 在文件被编辑后仍然认得出是同一条发现，
    // 而不是「修好一条、新开一条」
    r["partialFingerprints"] = json!({
        "repolishCheck/v1": format!("{id}:{uri}:{}", line.unwrap_or(0)),
    });
    r
}

/// 证据里的路径 → SARIF 的 artifact URI。
///
/// 仓库级的证据（licence 缺失、topics 太多）用 `.` 作为路径，而 SARIF 的
/// `artifactLocation` 必须指向一个文件。指向仓库根在 GitHub 上不会渲染，
/// 所以退到 README——那是「这个仓库整体有问题」最合理的落点。
fn artifact_uri(ev: &Evidence) -> String {
    let p = ev
        .file
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    if p.is_empty() || p == "." {
        return "README.md".to_string();
    }
    p.trim_start_matches("./").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use repolish_core::{Category, CheckResult, Fix, Mode, ProfileInfo, Repository, Risk};
    use repolish_ingest::Profile;

    fn report(checks: Vec<CheckResult>) -> Report {
        Report::build(
            checks,
            Repository {
                owner: Some("acme".into()),
                name: "widget".into(),
                commit: Some("deadbeef".into()),
            },
            ProfileInfo {
                detected: Profile::Cli,
                overridden: false,
            },
            Mode::Local,
        )
    }

    fn check(id: &'static str, outcome: Outcome) -> CheckResult {
        CheckResult {
            id,
            category: Category::Credibility,
            risk: Risk::Critical,
            outcome,
        }
    }

    fn parse(r: &Report) -> Value {
        serde_json::from_str(&sarif(r)).unwrap()
    }

    #[test]
    fn a_deduction_becomes_a_result_at_its_file_and_line() {
        let r = report(vec![check(
            "claim-consistency",
            Outcome::scored(
                5,
                vec![Evidence::at(
                    "README.md",
                    8,
                    "`scripts/setup.sh` — does not exist",
                )],
                vec![Fix::new(Severity::P1, "one command claim no longer works")],
            ),
        )]);
        let v = parse(&r);
        let res = &v["runs"][0]["results"][0];
        assert_eq!(res["ruleId"], "claim-consistency");
        assert_eq!(res["level"], "error");
        let loc = &res["locations"][0]["physicalLocation"];
        assert_eq!(loc["artifactLocation"]["uri"], "README.md");
        assert_eq!(loc["region"]["startLine"], 8);
    }

    /// SARIF 的 results 是「发现的问题」。把通过项也塞进去，
    /// PR 上就会出现一排说「一切正常」的注解。
    #[test]
    fn passing_checks_produce_no_results() {
        let r = report(vec![check("license", Outcome::perfect(vec![]))]);
        let v = parse(&r);
        assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 0);
        // 但规则本身还是要声明，GitHub 才有元数据可显示
        assert_eq!(v["runs"][0]["tool"]["driver"]["rules"][0]["id"], "license");
    }

    /// 仓库级的发现没有文件可指。落在仓库根上 GitHub 不会渲染，
    /// 必须退到一个真实存在的文件。
    #[test]
    fn a_repo_level_finding_is_anchored_somewhere_github_can_render() {
        let r = report(vec![check(
            "license",
            Outcome::scored(
                0,
                vec![Evidence::new(".", "no LICENSE file in the repository root")],
                vec![Fix::new(Severity::P1, "Add a LICENSE file")],
            ),
        )]);
        let v = parse(&r);
        let loc = &v["runs"][0]["results"][0]["locations"][0]["physicalLocation"];
        assert_eq!(loc["artifactLocation"]["uri"], "README.md");
        assert_eq!(loc["region"]["startLine"], 1);
        assert_eq!(loc["region"]["properties"]["wholeFile"], true);
    }

    #[test]
    fn every_evidence_line_gets_its_own_annotation() {
        let r = report(vec![check(
            "readme-link-health",
            Outcome::scored(
                4,
                vec![
                    Evidence::at("README.md", 12, "docs/a.md"),
                    Evidence::at("README.md", 40, "docs/b.md"),
                ],
                vec![Fix::new(Severity::P2, "2 relative links do not resolve")],
            ),
        )]);
        let v = parse(&r);
        let results = v["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[1]["locations"][0]["physicalLocation"]["region"]["startLine"],
            40
        );
    }

    /// 同一个 commit 必须产出逐字节相同的文件，跟其余所有产物一个规矩
    #[test]
    fn the_output_carries_no_timestamp_and_is_byte_stable() {
        let r = report(vec![check("license", Outcome::perfect(vec![]))]);
        assert_eq!(sarif(&r), sarif(&r));
        assert!(!sarif(&r).contains("TimeUtc"));
    }

    /// 描述表是给 GitHub 的 Security 标签页看的，那里读不到我们的 docs。
    /// 新加检查项时忘了补一行，这个测试会说出来。
    #[test]
    fn every_registered_check_has_a_description() {
        let missing: Vec<&str> = repolish_checks::registry()
            .ids()
            .into_iter()
            .filter(|id| !DESCRIPTIONS.iter().any(|(k, _)| k == id))
            .collect();
        assert!(missing.is_empty(), "no SARIF description for: {missing:?}");
    }
}
