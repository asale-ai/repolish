//! 标题提取。
//!
//! 真实 README 的标题写法有四种：ATX（`# Foo`）、setext（下划线）、
//! HTML（`<h1 align="center">`）、以及纯 logo 图片。四种都得认。
//!
//! 三条硬约束，都是在真实仓库上踩出来的：
//!
//! 1. **按出现位置取最靠前的候选**，不能按「Markdown 优先于 HTML 优先于图片」的
//!    类型优先级取。awesome-list 的真标题是第 2 行的 logo `alt`，第 10 行却有个
//!    赞助位 `<h2>`；按类型优先级会选中广告。
//! 2. **标题必须在文档开头附近**。axios 的 README 开头全是赞助商 HTML，第一个
//!    Markdown 标题是第 453 行的 `## Table of contents`——放宽到「首个任意标题」
//!    会把目录当成项目名。
//! 3. **标题不会是超链接**。项目 logo 不会指向站外；被 `<a href>` 包住的图片
//!    基本都是赞助商或徽章。曾经用「整块含 sponsor 字样就跳过」来挡，结果
//!    fzf 和 awesome 开头块里有个 `alt="Sponsors"` 徽章，真 logo 被一起丢掉。

use comrak::nodes::{AstNode, NodeValue};

/// 超出这个行号的标题不再视为文档标题。
/// 放宽到 40 行是为了容纳开头的大段徽章 / 横幅。
const MAX_TITLE_LINE: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleSource {
    /// Markdown ATX 标题
    Atx,
    /// Markdown setext 标题（下划线式）
    Setext,
    /// HTML h1 / h2。图片包在标题标签里也算——读屏软件仍会播报为标题
    Html,
    /// 裸图片，标题只存在于 alt 文本
    ImageAlt,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub line: usize,
    pub text: String,
    pub source: TitleSource,
}

/// 从整篇文档收集候选并取最靠前的一个。
pub fn extract<'a>(root: &'a AstNode<'a>) -> Option<Candidate> {
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut seen_heading = false;

    for node in root.descendants() {
        let (line, kind) = {
            let data = node.data.borrow();
            let line = data.sourcepos.start.line;
            let kind = match &data.value {
                NodeValue::Heading(h) => Kind::Heading {
                    level: h.level,
                    setext: h.setext,
                },
                NodeValue::HtmlBlock(h) => Kind::Html(h.literal.clone()),
                NodeValue::Image(_) => Kind::Image,
                _ => Kind::Other,
            };
            (line, kind)
        };

        if line > MAX_TITLE_LINE {
            continue;
        }

        match kind {
            Kind::Heading { level, setext } => {
                // 只认文档的首个标题，且必须是 h1/h2。
                // 更深的层级是章节标题，不是项目名。
                if seen_heading || level > 2 {
                    continue;
                }
                seen_heading = true;
                // 标题里常挂一排徽章，跳过图片才不会把 alt 文本拼进项目名
                let text = super::text_of(node, true);
                if !text.is_empty() {
                    candidates.push(Candidate {
                        line,
                        text,
                        source: if setext {
                            TitleSource::Setext
                        } else {
                            TitleSource::Atx
                        },
                    });
                }
            }
            Kind::Html(literal) => {
                if let Some((text, source)) = from_html(&literal) {
                    candidates.push(Candidate { line, text, source });
                }
            }
            Kind::Image => {
                if is_linked(node) {
                    continue;
                }
                let alt = super::text_of(node, false);
                if !alt.is_empty() {
                    candidates.push(Candidate {
                        line,
                        text: alt,
                        source: TitleSource::ImageAlt,
                    });
                }
            }
            Kind::Other => {}
        }
    }

    candidates.into_iter().min_by_key(|c| c.line)
}

enum Kind {
    Heading { level: u8, setext: bool },
    Html(String),
    Image,
    Other,
}

/// 图片是否被链接包裹——徽章与赞助 logo 的共同特征
fn is_linked<'a>(node: &'a AstNode<'a>) -> bool {
    let mut cur = node.parent();
    while let Some(p) = cur {
        if matches!(p.data.borrow().value, NodeValue::Link(_)) {
            return true;
        }
        cur = p.parent();
    }
    false
}

/// 在单个 HTML 块内部，取最先出现的 h1/h2 或 img alt。
fn from_html(html: &str) -> Option<(String, TitleSource)> {
    let lower = html.to_lowercase();
    let heading_at = ["<h1", "<h2"].iter().filter_map(|t| lower.find(t)).min();
    let img_at = lower.find("<img");

    // 标题标签更靠前：图片包在标题里也算标题（读屏软件会播报层级）
    if let Some(h) = heading_at {
        if img_at.is_none_or(|i| h < i) {
            return heading_text(html, &lower, h).map(|t| (t, TitleSource::Html));
        }
    }

    let i = img_at?;
    // 真 logo 不会是超链接。赞助商 logo 一律包在 <a href> 里，
    // 而 <a> 会出现在 <img> 之前。
    if lower[..i].rfind("<a ").is_some_and(|a| lower[a..i].find("</a>").is_none()) {
        return None;
    }
    extract_alt(&html[i..]).map(|t| (t, TitleSource::ImageAlt))
}

fn heading_text(html: &str, lower: &str, start: usize) -> Option<String> {
    let tag = &lower[start + 1..start + 3];
    let close = format!("</{tag}>");
    let gt = lower[start..].find('>')? + start + 1;
    let end = lower[gt..].find(&close)? + gt;
    let inner = &html[gt..end];

    let text = strip_tags(inner);
    if !text.is_empty() {
        return Some(text);
    }
    // 标题里只有 logo 图片
    extract_alt(inner)
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in html.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn extract_alt(html: &str) -> Option<String> {
    attr(html, "alt")
}

/// 取 HTML 属性值。属性值可能用单引号或双引号。
pub fn attr(html: &str, name: &str) -> Option<String> {
    const DOUBLE: char = '"';
    const SINGLE: char = '\'';

    let needle = format!("{name}=");
    let lower = html.to_lowercase();
    let i = lower.find(&needle)?;
    let rest = &html[i + needle.len()..];
    let quote = rest.chars().next()?;
    if quote != DOUBLE && quote != SINGLE {
        return None;
    }
    let end = rest[1..].find(quote)? + 1;
    let alt = rest[1..end].trim();
    (!alt.is_empty()).then(|| alt.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title_of(md: &str) -> Option<Candidate> {
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, md, &crate::options());
        extract(root)
    }

    #[test]
    fn atx_heading() {
        let t = title_of("# Ruff\n\nAn extremely fast Python linter.").unwrap();
        assert_eq!(t.text, "Ruff");
        assert_eq!(t.source, TitleSource::Atx);
    }

    #[test]
    fn setext_heading_counts() {
        // ripgrep 用的写法：下划线式标题，级别是 h2 而非 h1
        let t = title_of("ripgrep (rg)\n------------\n\nA search tool.").unwrap();
        assert_eq!(t.text, "ripgrep (rg)");
        assert_eq!(t.source, TitleSource::Setext);
    }

    #[test]
    fn badges_in_heading_are_stripped() {
        // serde 的写法：标题行后面直接跟一排徽章。
        // 引用式链接必须给出定义，否则 comrak 会把它们当普通文本，测不到真实行为。
        let md = concat!(
            "# Serde [![Build Status]][actions] [![Latest Version]][crates]\n",
            "\n",
            "[Build Status]: https://img.shields.io/badge/ci-passing-green.svg\n",
            "[actions]: https://github.com/serde-rs/serde/actions\n",
            "[Latest Version]: https://img.shields.io/crates/v/serde.svg\n",
            "[crates]: https://crates.io/crates/serde\n",
        );
        let t = title_of(md).unwrap();
        assert_eq!(t.text, "Serde");
    }

    #[test]
    fn html_heading_wrapping_logo_is_a_heading() {
        // chalk：logo 包在 h1 里，语义上仍是标题
        let t = title_of("<h1 align=\"center\">\n<img src=\"logo.svg\" alt=\"Chalk\">\n</h1>\n")
            .unwrap();
        assert_eq!(t.text, "Chalk");
        assert_eq!(t.source, TitleSource::Html);
    }

    #[test]
    fn logo_wins_over_later_sponsor_heading() {
        // awesome：真标题是靠前的 logo alt，其后才是赞助位 h2。
        // 整块含 sponsor 字样就跳过的做法会把真 logo 一起丢掉。
        let md = "<div align=\"center\">\n<img src=\"logo.svg\" alt=\"Awesome\">\n<a href=\"https://github.com/sponsors/x\">Sponsors</a>\n<h2><a href=\"https://example.com\">Buy My App</a></h2>\n</div>\n";
        let t = title_of(md).unwrap();
        assert_eq!(t.text, "Awesome");
        assert_eq!(t.source, TitleSource::ImageAlt);
    }

    #[test]
    fn linked_logo_is_not_a_title() {
        // axios：开头是赞助商表格，logo 全包在 <a href> 里
        let md = "<table align=\"center\">\n<tr><td>\n<a href=\"https://sponsor.example\"><img src=\"s.png\" alt=\"Big Sponsor\"></a>\n</td></tr>\n</table>\n";
        assert!(title_of(md).is_none());
    }

    #[test]
    fn linked_markdown_badge_is_not_a_title() {
        assert!(title_of("[![Build Status](badge.svg)](https://ci.example)\n").is_none());
    }

    #[test]
    fn deep_heading_is_not_a_title() {
        let mut md = String::from("<p>banner</p>\n\n");
        md.push_str(&"filler\n\n".repeat(30));
        md.push_str("## Table of contents\n");
        assert!(title_of(&md).is_none());
    }

    #[test]
    fn subsection_heading_is_not_a_title() {
        // fzf：首个 Markdown 标题是 h3
        assert!(title_of("### Using Homebrew\n\nbrew install fzf\n").is_none());
    }
}
