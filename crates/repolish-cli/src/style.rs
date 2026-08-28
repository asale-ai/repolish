//! README 排版选项：`polish` 插入物长什么样。
//!
//! 这些**只影响外观，不影响任何一个分数**。检查项清单与权重在 v1 冻结，
//! 一个仓库不能通过换徽章样式让自己好看一点——那样分数就不可横向比较了。
//!
//! 全部可从 `.repolish.toml` 的 `[readme]` 段或命令行给出，命令行优先。

use std::fmt;

use clap::ValueEnum;
use serde::Deserialize;

/// shields.io 的徽章样式。取值与 shields 的 `?style=` 一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BadgeStyle {
    #[default]
    Flat,
    FlatSquare,
    Plastic,
    ForTheBadge,
    Social,
}

impl BadgeStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeStyle::Flat => "flat",
            BadgeStyle::FlatSquare => "flat-square",
            BadgeStyle::Plastic => "plastic",
            BadgeStyle::ForTheBadge => "for-the-badge",
            BadgeStyle::Social => "social",
        }
    }

    /// 从 README 里已有的徽章 URL 认出作者用的是哪种样式。
    ///
    /// 一排徽章里混进一个样式不同的，比样式统一但不是默认样式更难看。
    /// 没指定时跟着已有的走，是比「用我们的默认值」更合理的默认。
    pub fn detect(readme: &str) -> Option<Self> {
        // 出现最多的那种胜出：偶尔有一个第三方徽章样式不同不该带偏整排
        let mut tally: Vec<(BadgeStyle, usize)> = Vec::new();
        for (i, _) in readme.match_indices("style=") {
            let rest = &readme[i + "style=".len()..];
            let end = rest
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
                .unwrap_or(rest.len());
            let Some(found) = Self::parse(&rest[..end]) else {
                continue;
            };
            match tally.iter_mut().find(|(s, _)| *s == found) {
                Some(entry) => entry.1 += 1,
                None => tally.push((found, 1)),
            }
        }
        tally.into_iter().max_by_key(|(_, n)| *n).map(|(s, _)| s)
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "flat" => Some(BadgeStyle::Flat),
            "flat-square" => Some(BadgeStyle::FlatSquare),
            "plastic" => Some(BadgeStyle::Plastic),
            "for-the-badge" => Some(BadgeStyle::ForTheBadge),
            "social" => Some(BadgeStyle::Social),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    #[default]
    Left,
    Center,
}

/// SVG 产物的色板。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    // 这几条注释是 clap 的 --help 文案，所以是英文——工具吐给使用者的每一个字
    // 都是英文，见 CONTRIBUTING 的第三条。为什么这么设计写在下面的普通注释里。
    /// Neon on near-black, the same palette as the terminal report
    #[default]
    Dark,
    /// Warm paper, dark ink. A dark card in a light README is a hole in the page
    Porcelain,
}

impl Theme {
    pub fn palette(self) -> &'static repolish_render::Palette {
        match self {
            Theme::Dark => &repolish_render::DARK,
            Theme::Porcelain => &repolish_render::PORCELAIN,
        }
    }
}

/// SVG 里那些字用什么语言写。
///
/// 默认跟着 README 走，而不是跟着系统 locale 走：卡片是贴进**别人的
/// README** 的，它该说那份 README 的语言，不是运行这条命令的人的语言。
/// CI 里跑一次 `LANG=C` 就把中文 README 顶上的卡片换成英文，是很荒唐的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CardLang {
    /// Follow the README: kana means Japanese, otherwise a high share of CJK means Chinese
    #[default]
    Auto,
    En,
    #[value(name = "zh-CN")]
    #[serde(rename = "zh-CN")]
    ZhCn,
    Ja,
}

impl CardLang {
    pub fn resolve(self, readme: &str) -> repolish_render::Lang {
        match self {
            CardLang::Auto => repolish_render::Lang::detect(readme),
            CardLang::En => repolish_render::Lang::En,
            CardLang::ZhCn => repolish_render::Lang::ZhCn,
            CardLang::Ja => repolish_render::Lang::Ja,
        }
    }
}

/// README 顶部那张图的宽度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoWidth {
    /// 通栏，输出 `width="100%"`。品牌横幅要的就是这个——固定像素宽的横幅
    /// 在宽屏上会缩在左上角，在手机上又会撑破版心。
    ///
    /// 这个枚举不是 `ValueEnum`（它走 `FromStr`），所以这段注释不会进 --help，
    /// 可以是中文。
    Full,
    Px(u32),
}

impl LogoWidth {
    /// `<img>` 上的 width 属性值
    pub fn attr(self) -> String {
        match self {
            LogoWidth::Full => "100%".to_string(),
            LogoWidth::Px(n) => n.to_string(),
        }
    }
}

impl std::str::FromStr for LogoWidth {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("full") || s == "100%" {
            return Ok(LogoWidth::Full);
        }
        s.trim_end_matches("px")
            .parse::<u32>()
            .map(LogoWidth::Px)
            .map_err(|_| format!("expected a pixel width or `full`, got `{s}`"))
    }
}

impl<'de> Deserialize<'de> for LogoWidth {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // TOML 里 `logo-width = 420` 和 `logo-width = "full"` 都得能写。
        // 只收字符串的话，写数字的人会拿到一个说不清的类型错误。
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Num(u32),
            Text(String),
        }
        match Raw::deserialize(d)? {
            Raw::Num(n) => Ok(LogoWidth::Px(n)),
            Raw::Text(s) => s.parse().map_err(serde::de::Error::custom),
        }
    }
}

/// README 里的表格怎么处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TableStyle {
    /// Leave tables exactly as they are
    #[default]
    Keep,
    /// Draw each table as an SVG and fold the original into <details>.
    /// The original stays: an image has no text layer, so screen readers,
    /// grep and translation tools all read the folded copy
    Svg,
}

/// 目录的排版。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TocStyle {
    #[default]
    Bullet,
    Number,
    Roman,
    /// Fold the contents into a <details> block, which long READMEs benefit from
    Fold,
}

impl TocStyle {
    /// 第 `i` 条（0-based）前面的标记
    pub fn marker(self, i: usize) -> String {
        match self {
            TocStyle::Bullet | TocStyle::Fold => "-".to_string(),
            TocStyle::Number => format!("{}.", i + 1),
            TocStyle::Roman => format!("{}.", roman(i + 1)),
        }
    }
}

/// 小写罗马数字。目录不会长到需要考虑 4000 以上。
fn roman(mut n: usize) -> String {
    const TABLE: &[(usize, &str)] = &[
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut out = String::new();
    for (value, sym) in TABLE {
        while n >= *value {
            out.push_str(sym);
            n -= value;
        }
    }
    out
}

/// 一次 `polish` 运行的全部排版选项
#[derive(Debug, Clone, Default)]
pub struct ReadmeStyle {
    pub badge: BadgeStyle,
    pub align: Align,
    pub toc: TocStyle,
    /// README 顶部的图。**必须是仓库内的相对路径**——绝对路径在别人的
    /// 机器上打不开，而 `readme-link-health` 会立刻把它判成死链。
    pub logo: Option<String>,
    pub logo_width: Option<LogoWidth>,
    /// 生成项目结构树的深度。`None` = 不生成。
    pub tree_depth: Option<usize>,
    /// SVG 产物的色板与语言
    pub theme: Theme,
    pub lang: repolish_render::Lang,
    /// 插一张项目概览卡片，并把它写进 `.repolish/overview.svg`
    pub overview: bool,
    /// 在 README 末尾插分数卡片与「用 repolish 打磨过」一节
    pub footer_card: bool,
    pub tables: TableStyle,
}

impl fmt::Display for ReadmeStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "badge={} align={:?} toc={:?}",
            self.badge.as_str(),
            self.align,
            self.toc
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 通栏横幅要 `width="100%"`，固定像素在宽屏上会缩成一小块
    #[test]
    fn logo_width_accepts_pixels_and_full() {
        use std::str::FromStr;
        assert_eq!(LogoWidth::from_str("420"), Ok(LogoWidth::Px(420)));
        assert_eq!(LogoWidth::from_str("420px"), Ok(LogoWidth::Px(420)));
        assert_eq!(LogoWidth::from_str("full"), Ok(LogoWidth::Full));
        assert_eq!(LogoWidth::from_str("100%"), Ok(LogoWidth::Full));
        assert!(LogoWidth::from_str("wide").is_err());
        assert_eq!(LogoWidth::Full.attr(), "100%");
        assert_eq!(LogoWidth::Px(300).attr(), "300");
    }

    /// 卡片说的该是那份 README 的语言，不是运行这条命令的人的语言
    #[test]
    fn card_language_follows_the_readme_unless_pinned() {
        let zh = "# 工具\n\n这是一个给开源仓库打分的命令行工具，指出该先改哪里。\n";
        assert_eq!(CardLang::Auto.resolve(zh), repolish_render::Lang::ZhCn);
        assert_eq!(CardLang::En.resolve(zh), repolish_render::Lang::En);
        assert_eq!(
            CardLang::Auto.resolve("# Tool\n\nScore your repository.\n"),
            repolish_render::Lang::En
        );
    }

    #[test]
    fn roman_numerals_are_lowercase_and_correct() {
        let got: Vec<String> = (1..=12).map(roman).collect();
        assert_eq!(
            got,
            vec!["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii"]
        );
        assert_eq!(roman(49), "xlix");
    }

    #[test]
    fn markers_match_the_style() {
        assert_eq!(TocStyle::Bullet.marker(0), "-");
        assert_eq!(TocStyle::Fold.marker(3), "-");
        assert_eq!(TocStyle::Number.marker(0), "1.");
        assert_eq!(TocStyle::Number.marker(9), "10.");
        assert_eq!(TocStyle::Roman.marker(3), "iv.");
    }

    /// 没指定时跟着 README 里已有的徽章走，比用我们的默认值更合理
    #[test]
    fn the_existing_badge_style_is_detected() {
        let md = "[![a](https://img.shields.io/badge/a-b-blue?style=for-the-badge)](x)\n\
                  [![b](https://img.shields.io/badge/c-d-red?style=for-the-badge)](y)\n";
        assert_eq!(BadgeStyle::detect(md), Some(BadgeStyle::ForTheBadge));
    }

    /// 混进一个样式不同的第三方徽章，不该带偏整排
    #[test]
    fn the_most_common_style_wins_over_a_stray_one() {
        let md = "?style=flat-square x ?style=flat-square y ?style=plastic\n";
        assert_eq!(BadgeStyle::detect(md), Some(BadgeStyle::FlatSquare));
    }

    #[test]
    fn a_readme_without_styled_badges_detects_nothing() {
        assert_eq!(BadgeStyle::detect("# thing\n\nNo badges here.\n"), None);
        // 无法识别的样式名不算数
        assert_eq!(BadgeStyle::detect("?style=neon\n"), None);
    }
}
