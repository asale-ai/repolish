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

/// 目录的排版。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TocStyle {
    #[default]
    Bullet,
    Number,
    Roman,
    /// 折进 `<details>`，长 README 里不占屏
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
    pub logo_width: Option<u32>,
    /// 生成项目结构树的深度。`None` = 不生成。
    pub tree_depth: Option<usize>,
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
