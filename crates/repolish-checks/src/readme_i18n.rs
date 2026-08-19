use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// 是否提供多语言 README。
///
/// 没有译本不算缺陷，只是错过了受众——所以起评分是 5 而不是 0，权重也定为 Low。
///
/// 分档：无译本 = 5；有译本但主 README 没有语言入口 = 7；1 个 = 8；≥2 个 = 10
pub struct ReadmeI18n;

/// 常见语言代码。只认白名单，否则 `README.old.md`、`README.dev.md`
/// 会被当成译本。
const LANGS: &[&str] = &[
    "zh", "zh-cn", "zh-tw", "zh-hans", "zh-hant", "cn", "tw", "ja", "jp", "ko", "kr", "es", "fr",
    "de", "pt", "pt-br", "ru", "it", "tr", "ar", "hi", "id", "vi", "pl", "nl", "uk", "fa", "th",
];

impl Check for ReadmeI18n {
    fn id(&self) -> &'static str {
        "readme-i18n"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::Low
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::inconclusive("没有 README，无从判断多语言");
        };
        let main = crate::util::readme_name(readme);

        let translations: Vec<&str> = ctx
            .files
            .iter()
            .filter(|p| p.to_lowercase() != main.to_lowercase())
            .filter(|p| translation_lang(p).is_some())
            .collect();

        if translations.is_empty() {
            return Outcome::scored(
                5,
                vec![Evidence::new(&main, "只有一种语言的 README")],
                vec![Fix::new(
                    Severity::P3,
                    "若目标用户里有非英语使用者，加一份 `README.zh-CN.md` 之类的译本，\
                     并在主 README 顶部放语言切换链接",
                )],
            );
        }

        // 有译本却没有入口等于没有：读者从仓库首页只看得到主 README
        let linked = translations
            .iter()
            .any(|t| readme.links.iter().any(|l| l.repo_path().eq_ignore_ascii_case(t)));

        if !linked {
            return Outcome::scored(
                7,
                vec![Evidence::new(
                    translations[0],
                    format!("有 {} 份译本，但主 README 没有链接过去", translations.len()),
                )],
                vec![Fix::new(
                    Severity::P3,
                    "在主 README 顶部加一行语言切换链接。读者只会看到仓库首页展示的那一份",
                )],
            );
        }

        let n = translations.len();
        let evidence = vec![Evidence::new(
            translations[0],
            format!("{n} 份译本，主 README 已链接"),
        )];
        if n >= 2 {
            return Outcome::perfect(evidence);
        }
        Outcome::scored(
            8,
            evidence,
            vec![Fix::new(Severity::P3, "如有余力可再补一种语言")],
        )
    }
}

/// `README.zh-CN.md` / `README_ja.md` / `docs/README-ko.md` → 语言代码
fn translation_lang(path: &str) -> Option<&'static str> {
    let file = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let stem = file.strip_suffix(".md").or_else(|| file.strip_suffix(".rst"))?;
    let rest = stem.strip_prefix("readme")?;
    let code = rest.trim_start_matches(['.', '_', '-']);
    LANGS.iter().find(|l| **l == code).copied()
}

#[cfg(test)]
mod tests {
    use super::translation_lang;

    #[test]
    fn recognizes_translation_filenames() {
        assert_eq!(translation_lang("README.zh-CN.md"), Some("zh-cn"));
        assert_eq!(translation_lang("README_ja.md"), Some("ja"));
        assert_eq!(translation_lang("docs/README-ko.md"), Some("ko"));
    }

    #[test]
    fn non_language_suffixes_are_not_translations() {
        // 只认语言白名单，否则这些都会被当成译本
        assert_eq!(translation_lang("README.old.md"), None);
        assert_eq!(translation_lang("README.dev.md"), None);
        assert_eq!(translation_lang("READMORE.md"), None);
    }
}
