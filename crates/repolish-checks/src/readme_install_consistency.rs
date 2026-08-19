//! `readme-install-consistency`：README 教人装的，是不是这个仓库发布的东西。
//!
//! 这是最容易出现「复制粘贴残留」的地方——从别的项目抄来 README 模板，
//! 安装命令里的包名忘了改。使用者照着敲，装到的是另一个包。
//!
//! 判定一律偏保守：拿不准就 `Inconclusive`，不猜。

use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};
use repolish_ingest::{normalize_package_name, Ecosystem, Manifest};

use crate::util;

pub struct ReadmeInstallConsistency;

/// 能可靠地从安装命令里抽出包名的生态。
/// Maven / Gradle 的依赖是 XML 或 DSL 片段，形态太多，不参与比对。
const EXTRACTABLE: &[Ecosystem] = &[
    Ecosystem::Cargo,
    Ecosystem::Npm,
    Ecosystem::Pypi,
    Ecosystem::Go,
    Ecosystem::Gem,
    Ecosystem::Composer,
];

impl Check for ReadmeInstallConsistency {
    fn id(&self) -> &'static str {
        "readme-install-consistency"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::High
    }

    fn applies_to(&self, profile: Profile) -> bool {
        !matches!(profile, Profile::Docs | Profile::Collection)
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::inconclusive("没有 README，无安装命令可比对");
        };
        if ctx.manifests.is_empty() {
            // docs/05 明确：未探测到任何包管理器清单时不适用
            return Outcome::NotApplicable {
                profile: ctx.profile,
            };
        }
        let name = util::readme_name(readme);
        let commands = util::command_lines(readme);

        // README 里出现过安装命令的生态，连同命令里给出的包名
        let mentioned: Vec<(Ecosystem, Vec<(usize, String)>)> = [
            Ecosystem::Cargo,
            Ecosystem::Npm,
            Ecosystem::Pypi,
            Ecosystem::Go,
            Ecosystem::Gem,
            Ecosystem::Composer,
            Ecosystem::Maven,
        ]
        .into_iter()
        .filter_map(|eco| {
            let pkgs = packages_for(eco, readme, &commands);
            let seen = pkgs.is_some();
            seen.then(|| (eco, pkgs.unwrap_or_default()))
        })
        .collect();

        if mentioned.is_empty() {
            return Outcome::inconclusive("README 里没有安装命令可比对");
        }

        let declared: Vec<&Manifest> = ctx.manifests.iter().collect();
        let overlap: Vec<&(Ecosystem, Vec<(usize, String)>)> = mentioned
            .iter()
            .filter(|(eco, _)| declared.iter().any(|m| m.ecosystem == *eco))
            .collect();

        if overlap.is_empty() {
            let told = mentioned
                .iter()
                .map(|(e, _)| e.as_str())
                .collect::<Vec<_>>()
                .join(" / ");
            let have = declared
                .iter()
                .map(|m| m.ecosystem.as_str())
                .collect::<Vec<_>>()
                .join(" / ");
            let line = mentioned
                .iter()
                .flat_map(|(_, p)| p.first().map(|(l, _)| *l))
                .next()
                .unwrap_or(1);
            return Outcome::scored(
                3,
                vec![Evidence::at(
                    &name,
                    line,
                    format!("README 教人用 {told} 安装，但仓库里只有 {have} 的清单"),
                )],
                vec![Fix::new(
                    Severity::P1,
                    format!(
                        "安装命令与仓库实际发布方式对不上。要么补上 {told} 的发布配置，\
                         要么把命令改成 {have}"
                    ),
                )],
            );
        }

        compare_names(ctx, &name, &overlap)
    }
}

fn compare_names(
    ctx: &RepoContext,
    readme_name: &str,
    overlap: &[&(Ecosystem, Vec<(usize, String)>)],
) -> Outcome {
    let mut mismatches: Vec<(usize, String, String)> = Vec::new();
    let mut comparable = 0usize;

    for (eco, pkgs) in overlap {
        if !EXTRACTABLE.contains(eco) || pkgs.is_empty() {
            continue;
        }
        let Some(expected) = ctx
            .manifests
            .iter()
            .find(|m| m.ecosystem == *eco)
            .and_then(|m| m.name.as_deref())
        else {
            continue;
        };
        comparable += 1;
        let want = normalize_package_name(expected);
        if pkgs
            .iter()
            .any(|(_, p)| normalize_package_name(p) == want)
        {
            return Outcome::perfect(vec![Evidence::at(
                readme_name,
                pkgs[0].0,
                format!("安装命令装的就是本仓库发布的 `{expected}`"),
            )]);
        }
        let (line, got) = pkgs[0].clone();
        mismatches.push((line, got, expected.to_string()));
    }

    if comparable == 0 {
        return Outcome::inconclusive(
            "README 的安装命令里没有给出包名（如 `pip install -e .`），无从比对",
        );
    }

    let (line, got, expected) = mismatches[0].clone();
    Outcome::scored(
        4,
        mismatches
            .iter()
            .map(|(l, g, e)| {
                Evidence::at(readme_name, *l, format!("命令里装的是 `{g}`，本仓库发布的是 `{e}`"))
            })
            .collect(),
        vec![Fix::new(
            Severity::P1,
            format!(
                "安装命令里的包名是 `{got}`，与仓库发布的 `{expected}` 不一致\
                 （第 {line} 行）。照着敲的人会装到别的包上",
            ),
        )],
    )
}

/// 该生态是否在 README 中出现过；出现则返回命令里给出的包名（可能为空）。
/// 返回 `None` 表示压根没提到这个生态。
fn packages_for(
    eco: Ecosystem,
    readme: &repolish_md::Readme,
    commands: &[(usize, String)],
) -> Option<Vec<(usize, String)>> {
    let mut found = false;
    let mut pkgs = Vec::new();

    for (line, cmd) in commands {
        let lower = cmd.to_lowercase();
        for verb in eco.install_verbs() {
            if !lower.contains(verb) {
                continue;
            }
            found = true;
            pkgs.extend(
                util::args_after(cmd, verb)
                    .into_iter()
                    .filter_map(|a| clean_package(&a))
                    .map(|p| (*line, p)),
            );
        }
    }

    // 依赖声明片段：Rust 生态几乎不写 `cargo add`，而是贴一段
    // `[dependencies]` 让人抄进 Cargo.toml
    if eco == Ecosystem::Cargo {
        for cb in util::blocks_with_info(readme, &["toml", "ini"]) {
            if let Some(keys) = dependency_keys(&cb.literal) {
                found = true;
                pkgs.extend(keys.into_iter().map(|k| (cb.line, k)));
            }
        }
    }
    if eco == Ecosystem::Maven {
        let raw = readme.raw.to_lowercase();
        if raw.contains("<artifactid>") || raw.contains("implementation ") {
            found = true;
        }
    }

    found.then_some(pkgs)
}

/// `[dependencies]` 段下的键名
fn dependency_keys(toml_src: &str) -> Option<Vec<String>> {
    let mut in_deps = false;
    let mut keys = Vec::new();
    for line in toml_src.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t.contains("dependencies");
            continue;
        }
        if !in_deps {
            continue;
        }
        if let Some((k, _)) = t.split_once('=') {
            let k = k.trim().trim_matches('"');
            if !k.is_empty() {
                keys.push(k.to_string());
            }
        }
    }
    (!keys.is_empty()).then_some(keys)
}

/// 把命令参数收拾成包名。不是包名的（本地路径、URL、版本号）返回 None。
fn clean_package(token: &str) -> Option<String> {
    let t = token.trim().trim_matches(['"', '\'', ',', '`']);
    if t.is_empty()
        || t == "."
        || t.starts_with('.')
        || t.starts_with('/')
        || t.starts_with("git+")
        || t.starts_with("http")
        || t.starts_with('$')
    {
        return None;
    }
    // npm 的 scope 名以 @ 开头，不能当成版本分隔符
    let stripped = if let Some(rest) = t.strip_prefix('@') {
        format!("@{}", rest.split('@').next().unwrap_or(rest))
    } else {
        t.split(['@', '=', '>', '<', '~', '^', ':', ';'])
            .next()
            .unwrap_or(t)
            .to_string()
    };
    let s = stripped.trim().to_string();
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_version_specifiers_but_keeps_scopes() {
        assert_eq!(clean_package("requests>=2.0").as_deref(), Some("requests"));
        assert_eq!(clean_package("ripgrep@latest").as_deref(), Some("ripgrep"));
        assert_eq!(clean_package("@scope/tool@1.2.3").as_deref(), Some("@scope/tool"));
    }

    #[test]
    fn local_and_url_targets_are_not_package_names() {
        // `pip install -e .` 是开发安装，不是给使用者的安装命令
        assert!(clean_package(".").is_none());
        assert!(clean_package("./dist/pkg.whl").is_none());
        assert!(clean_package("git+https://github.com/o/r").is_none());
    }

    #[test]
    fn cargo_dependency_snippet_counts_as_an_install_instruction() {
        // serde 这类项目不写 `cargo add`，而是贴一段 [dependencies]
        let keys = dependency_keys("[dependencies]\nserde = { version = \"1.0\" }\n").unwrap();
        assert_eq!(keys, vec!["serde"]);
        // 别的 toml 段不算
        assert!(dependency_keys("[package]\nname = \"x\"\n").is_none());
    }
}
