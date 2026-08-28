//! `.repolish.toml`。
//!
//! 只收那些**已经有命令行开关**的选项。逐检查项的阈值不开放：检查项清单与
//! 权重在 v1 冻结，允许每个仓库自己调阈值，等于让分数在仓库之间不可比——
//! 那正是这个工具存在的理由。
//!
//! 优先级：命令行 > 配置文件 > 默认值。命令行永远赢，因为 CI 里能改的
//! 只有命令行那一行。
//!
//! 未知键直接报错，不静默忽略。打错一个键名却什么都没发生，比报错更糟：
//! 使用者会以为配置生效了。

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// 配置文件在仓库中的默认位置
pub const CONFIG_PATH: &str = ".repolish.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// 覆盖类型探测，取值同 `--profile`
    pub profile: Option<String>,
    /// 等价于 `--min-score`
    pub min_score: Option<u8>,
    #[serde(default)]
    pub checks: Checks,
    #[serde(default)]
    pub readme: Readme,
}

/// `polish` 插入物的排版。**不影响任何一个分数**——检查项清单与权重在 v1
/// 冻结，一个仓库不能靠换徽章样式让自己好看一点。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Readme {
    pub badge_style: Option<crate::style::BadgeStyle>,
    pub align: Option<crate::style::Align>,
    pub toc_style: Option<crate::style::TocStyle>,
    /// 仓库内的相对路径。绝对路径在别人机器上打不开，
    /// `readme-link-health` 会立刻把它判成死链。
    pub logo: Option<String>,
    pub logo_width: Option<crate::style::LogoWidth>,
    /// 项目结构树的深度。缺省 = 不生成。
    pub tree_depth: Option<usize>,
    /// SVG 产物的色板
    pub theme: Option<crate::style::Theme>,
    /// SVG 里那些字的语言。缺省跟着 README 走。
    pub lang: Option<crate::style::CardLang>,
    /// 插入项目概览卡片
    pub overview: Option<bool>,
    /// 在末尾插分数卡片与「用 repolish 打磨过」一节
    pub footer_card: Option<bool>,
    /// README 里的表格怎么处理
    pub tables: Option<crate::style::TableStyle>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    /// 只跑这些 id
    #[serde(default)]
    pub only: Vec<String>,
    /// 跳过这些 id
    #[serde(default)]
    pub skip: Vec<String>,
}

/// 载入配置。
///
/// `explicit` 来自 `--config`：给了就必须存在，找不到是错误——使用者以为
/// 自己指定了一份配置，静默回退到默认值会让他拿到一个解释不了的分数。
/// 没给就找仓库根下的 [`CONFIG_PATH`]，不存在则一切走默认。
pub fn load(explicit: Option<&Path>, root: &Path) -> Result<Config, String> {
    let path = match explicit {
        Some(p) => {
            if !p.is_file() {
                return Err(format!("no config file at {}", p.display()));
            }
            p.to_path_buf()
        }
        None => {
            let p: PathBuf = root.join(CONFIG_PATH);
            if !p.is_file() {
                return Ok(Config::default());
            }
            p
        }
    };

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    toml::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Config, String> {
        toml::from_str(s).map_err(|e| e.to_string())
    }

    #[test]
    fn an_empty_file_is_valid_and_changes_nothing() {
        let c = parse("").unwrap();
        assert!(c.profile.is_none());
        assert!(c.min_score.is_none());
        assert!(c.checks.skip.is_empty());
    }

    #[test]
    fn the_documented_shape_parses() {
        let c = parse(
            "profile = \"library\"\n\
             min_score = 70\n\n\
             [checks]\n\
             skip = [\"code-of-conduct\"]\n",
        )
        .unwrap();
        assert_eq!(c.profile.as_deref(), Some("library"));
        assert_eq!(c.min_score, Some(70));
        assert_eq!(c.checks.skip, vec!["code-of-conduct"]);
    }

    /// 打错键名却什么都没发生，比报错更糟：使用者会以为配置生效了
    /// TOML 里数字和 `"full"` 都得能写。只收字符串的话，
    /// 写数字的人会拿到一个说不清的类型错误。
    #[test]
    fn logo_width_reads_both_a_number_and_the_word_full() {
        use crate::style::LogoWidth;
        let c = parse("[readme]\nlogo-width = 420\n").unwrap();
        assert_eq!(c.readme.logo_width, Some(LogoWidth::Px(420)));
        let c = parse("[readme]\nlogo-width = \"full\"\n").unwrap();
        assert_eq!(c.readme.logo_width, Some(LogoWidth::Full));
        assert!(parse("[readme]\nlogo-width = \"wide\"\n").is_err());
    }

    #[test]
    fn the_visual_options_parse_from_the_readme_section() {
        let c = parse(
            "[readme]\ntheme = \"porcelain\"\nlang = \"zh-CN\"\noverview = true\n\
             footer-card = true\ntables = \"svg\"\n",
        )
        .unwrap();
        assert_eq!(c.readme.theme, Some(crate::style::Theme::Porcelain));
        assert_eq!(c.readme.lang, Some(crate::style::CardLang::ZhCn));
        assert_eq!(c.readme.overview, Some(true));
        assert_eq!(c.readme.tables, Some(crate::style::TableStyle::Svg));
    }

    #[test]
    fn a_misspelled_key_is_an_error_rather_than_a_silent_no_op() {
        let err = parse("profil = \"library\"\n").unwrap_err();
        assert!(err.contains("profil"), "报错应指出是哪个键: {err}");

        let err = parse("[checks]\nskipp = []\n").unwrap_err();
        assert!(err.contains("skipp"), "{err}");
    }

    /// 逐检查项的阈值不开放——允许每个仓库自己调，分数就不可横向比较了
    #[test]
    fn per_check_thresholds_are_refused() {
        assert!(parse("[checks.readme-length]\nmin_words = 150\n").is_err());
    }

    #[test]
    fn an_explicit_config_that_does_not_exist_is_an_error() {
        let missing = std::env::temp_dir().join("repolish-no-such-config.toml");
        let err = load(Some(&missing), Path::new(".")).unwrap_err();
        assert!(err.contains("no config file"), "{err}");
    }

    /// 仓库根下没有配置文件是常态，不该是错误
    #[test]
    fn a_missing_default_config_falls_back_to_defaults() {
        let empty = std::env::temp_dir().join("repolish-empty-dir-for-config");
        let _ = std::fs::create_dir_all(&empty);
        let c = load(None, &empty).unwrap();
        assert!(c.profile.is_none());
    }
}
