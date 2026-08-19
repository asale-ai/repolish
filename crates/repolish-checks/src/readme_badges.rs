use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// README 首屏是否有基础徽章。
///
/// 看的是**种类**而不是数量：十个下载量徽章不如「构建 + 版本 + 许可证」三个，
/// 后者恰好回答了读者最先问的三件事——能用吗、什么版本、我能不能用。
///
/// 分档：0 个 = 0；有徽章但只覆盖 1 类 = 6；2 类 = 8；≥3 类 = 10
pub struct ReadmeBadges;

/// 徽章图床。GitHub Actions 的 `.../badge.svg` 不走图床，单独判。
const BADGE_HOSTS: &[&str] = &[
    "img.shields.io",
    "shields.io",
    "badgen.net",
    "badge.fury.io",
    "codecov.io",
    "coveralls.io",
    "circleci.com",
    "travis-ci",
    "appveyor.com",
    "api.netlify.com",
    "pkg.go.dev/badge",
    "docs.rs/",
    "isitmaintained.com",
    // 不能收 opencollective.com：赞助商 logo 也托管在那里
    // （axios 首屏的 `images.opencollective.com/...` 是赞助位，不是徽章）。
    // 走 shields 的 `img.shields.io/opencollective/...` 仍会被认成「社区」类。
];

/// (类别, URL 或 alt 文本中的特征串)。顺序即匹配优先级。
const KINDS: &[(&str, &[&str])] = &[
    ("构建", &["workflow", "actions", "/ci", "build", "travis", "appveyor", "circleci"]),
    ("覆盖率", &["codecov", "coveralls", "coverage"]),
    ("版本", &["crates/v", "npm/v", "pypi/v", "version", "release", "badge.fury", "gem/v", "packagist"]),
    ("许可证", &["license", "licence"]),
    ("文档", &["docs.rs", "readthedocs", "docs-"]),
    ("下载量", &["downloads", "dm/", "dt/", "crates/d"]),
    ("社区", &["discord", "slack", "gitter", "twitter", "opencollective", "contributors"]),
];

/// 徽章只在首屏才起作用，往下埋没人看
const HEAD_LINES: usize = 40;

impl Check for ReadmeBadges {
    fn id(&self) -> &'static str {
        "readme-badges"
    }
    fn category(&self) -> Category {
        Category::Discoverability
    }
    fn risk(&self) -> Risk {
        Risk::Low
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::inconclusive("没有 README");
        };
        let name = crate::util::readme_name(readme);

        let all: Vec<&repolish_md::LinkRef> = readme
            .links
            .iter()
            .filter(|l| l.is_image && is_badge(&l.url))
            .collect();
        let badges: Vec<&repolish_md::LinkRef> =
            all.iter().copied().filter(|l| l.line <= HEAD_LINES).collect();

        if badges.is_empty() {
            // 有徽章但在折叠线以下，与「一个徽章都没有」不是一回事：
            // 前者是排版问题，后者是缺失。说成后者会让作者去加已经有的东西。
            if let Some(first) = all.first() {
                return Outcome::scored(
                    5,
                    vec![Evidence::at(
                        &name,
                        first.line,
                        format!("{} 个徽章，但都在第 {} 行之后，首屏看不到", all.len(), HEAD_LINES),
                    )],
                    vec![Fix::new(
                        Severity::P3,
                        format!(
                            "徽章埋在第 {} 行。把它们移到标题正下方——\
                             读者判断项目是否可用只看首屏",
                            first.line
                        ),
                    )],
                );
            }
            return Outcome::scored(
                0,
                vec![Evidence::new(&name, "没有徽章")],
                vec![Fix::new(
                    Severity::P3,
                    "加上构建状态、最新版本、许可证三个徽章。\
                     它们回答的是读者最先问的三件事：能用吗、什么版本、我能不能用",
                )],
            );
        }

        let mut kinds: Vec<&str> = badges.iter().filter_map(|b| classify(&b.url)).collect();
        kinds.sort_unstable();
        kinds.dedup();

        let listed = if kinds.is_empty() {
            "未识别类别".to_string()
        } else {
            kinds.join(" / ")
        };
        let note = format!("{} 个徽章（{}）", badges.len(), listed);

        match kinds.len() {
            0 | 1 => Outcome::scored(
                6,
                vec![Evidence::at(&name, badges[0].line, note)],
                vec![Fix::new(
                    Severity::P3,
                    "徽章种类太单一。补齐构建状态、最新版本、许可证三类",
                )],
            ),
            2 => Outcome::scored(
                8,
                vec![Evidence::at(&name, badges[0].line, note)],
                vec![Fix::new(Severity::P3, "再补一类徽章（构建 / 版本 / 许可证中缺的那个）")],
            ),
            _ => Outcome::perfect(vec![Evidence::at(&name, badges[0].line, note)]),
        }
    }
}

fn is_badge(url: &str) -> bool {
    let u = url.to_lowercase();
    BADGE_HOSTS.iter().any(|h| u.contains(h))
        // GitHub Actions 自带的徽章：.../actions/workflows/ci.yml/badge.svg
        || (u.contains("github.com") && u.ends_with("badge.svg"))
}

fn classify(url: &str) -> Option<&'static str> {
    let u = url.to_lowercase();
    KINDS
        .iter()
        .find(|(_, keys)| keys.iter().any(|k| u.contains(k)))
        .map(|(kind, _)| *kind)
}

#[cfg(test)]
mod tests {
    use super::{classify, is_badge};

    #[test]
    fn recognizes_badge_urls() {
        assert!(is_badge("https://img.shields.io/crates/v/serde.svg"));
        assert!(is_badge("https://github.com/o/r/actions/workflows/ci.yml/badge.svg"));
        // 项目自己的截图不是徽章
        assert!(!is_badge("https://raw.githubusercontent.com/o/r/main/screenshot.png"));
    }

    #[test]
    fn classifies_by_purpose() {
        assert_eq!(classify("https://img.shields.io/crates/v/serde.svg"), Some("版本"));
        assert_eq!(classify("https://img.shields.io/badge/license-MIT-blue"), Some("许可证"));
        assert_eq!(classify("https://codecov.io/gh/o/r/badge.svg"), Some("覆盖率"));
    }
}
