//! README 解析：AST → 区块模型。
//!
//! 本 crate **只读**。AST 不产出文本——`format_commonmark` 往返有损：
//! 引用式链接会被展平、setext 标题变 ATX、`*` 列表标记变 `-`、制表符变空格。
//! 12 个真实 README 上 0/12 无损，见 `examples/roundtrip.rs`。
//!
//! M4 的 `polish --apply` 因此走文本层：AST 只回答「插在第几行」
//! （`sourcepos`），切开原文拼回去，其余字节不碰。见 `examples/locate.rs`。

use std::path::{Path, PathBuf};

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};

pub mod section;
pub mod title;

pub use section::SectionKind;
pub use title::TitleSource;

#[derive(Debug, Clone)]
pub struct Section {
    pub kind: SectionKind,
    pub title: String,
    pub level: u8,
    /// 标题所在行（1-based）
    pub line: usize,
    /// 区块结束行（不含）。边界是**下一个同级或更高级标题**——
    /// 若按「下一个任意标题」切，`## Getting Started` 会被子标题 `### Installation`
    /// 截断，其中的代码块会被误判为不属于父区块。
    pub end_line: usize,
    /// 该区块正文（含子区块，不含自身标题行）
    pub body: String,
}

impl Section {
    pub fn contains_line(&self, line: usize) -> bool {
        line > self.line && line < self.end_line
    }
}

#[derive(Debug, Clone)]
pub struct CodeBlock {
    /// 围栏语言标记，如 `bash`、`rust`；无标记为空串
    pub info: String,
    pub literal: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct LinkRef {
    pub url: String,
    pub line: usize,
    pub is_image: bool,
}

impl LinkRef {
    /// 是否为指向仓库内文件的链接（排除 http、mailto、锚点、协议相对）
    pub fn is_relative(&self) -> bool {
        let u = self.url.trim();
        if u.is_empty() || u.starts_with('#') || u.starts_with("//") {
            return false;
        }
        !u.contains("://") && !u.starts_with("mailto:") && !u.starts_with("tel:")
    }

    /// 相对仓库根的路径。去掉锚点、查询串与前导斜杠。
    ///
    /// 前导斜杠必须剥掉：`Path::join` 遇到绝对路径会丢弃基路径，
    /// 于是 `/docs/x.md` 会被当成盘符根下的路径，永远判定为不存在。
    pub fn repo_path(&self) -> &str {
        let u = self.url.trim();
        let u = u.split('#').next().unwrap_or(u);
        let u = u.split('?').next().unwrap_or(u);
        u.trim_start_matches("./").trim_start_matches('/')
    }
}

#[derive(Debug, Clone)]
pub struct Readme {
    pub path: PathBuf,
    pub raw: String,
    /// 文档标题。ATX / setext / HTML / logo alt 都算，识别规则见 [`title`]。
    pub title: Option<String>,
    pub title_line: Option<usize>,
    pub title_source: Option<TitleSource>,
    /// 标题之后的首个有实质内容的段落；徽章行与纯图片行不算
    pub tagline: Option<String>,
    pub sections: Vec<Section>,
    pub code_blocks: Vec<CodeBlock>,
    pub links: Vec<LinkRef>,
}

impl Readme {
    pub fn parse(path: impl Into<PathBuf>, raw: impl Into<String>) -> Self {
        let path = path.into();
        let raw = raw.into();

        let arena = Arena::new();
        let opts = options();
        let root = parse_document(&arena, &raw, &opts);
        let lines: Vec<&str> = raw.lines().collect();

        let mut headings: Vec<(u8, String, usize)> = Vec::new();
        let mut code_blocks = Vec::new();
        let mut links = Vec::new();

        for node in root.descendants() {
            let data = node.data.borrow();
            let line = data.sourcepos.start.line;
            match &data.value {
                NodeValue::Heading(h) => {
                    let level = h.level;
                    drop(data);
                    headings.push((level, text_of(node, true), line));
                }
                NodeValue::CodeBlock(cb) => code_blocks.push(CodeBlock {
                    info: cb.info.trim().to_string(),
                    literal: cb.literal.clone(),
                    line,
                }),
                NodeValue::Link(l) => links.push(LinkRef {
                    url: l.url.clone(),
                    line,
                    is_image: false,
                }),
                NodeValue::Image(i) => links.push(LinkRef {
                    url: i.url.clone(),
                    line,
                    is_image: true,
                }),
                // README 首屏大量使用 HTML 排版（居中的 logo、徽章行）。
                // 这些 `<img>` 不是 Image 节点，不认就等于看不见半个 README。
                NodeValue::HtmlBlock(h) => links.extend(html_links(&h.literal, line)),
                NodeValue::HtmlInline(h) => links.extend(html_links(h, line)),
                _ => {}
            }
        }

        let candidate = title::extract(root);
        let title_line = candidate.as_ref().map(|c| c.line);
        let tagline = extract_tagline(root, title_line);
        let sections = build_sections(&headings, &lines);

        Readme {
            path,
            raw,
            title: candidate.as_ref().map(|c| c.text.clone()),
            title_line,
            title_source: candidate.map(|c| c.source),
            tagline,
            sections,
            code_blocks,
            links,
        }
    }

    pub fn section(&self, kind: SectionKind) -> Option<&Section> {
        self.sections.iter().find(|s| s.kind == kind)
    }

    pub fn has_section(&self, kind: SectionKind) -> bool {
        self.section(kind).is_some()
    }

    /// 落在该区块范围内的代码块（含其子区块）
    pub fn code_blocks_in<'a>(&'a self, s: &'a Section) -> impl Iterator<Item = &'a CodeBlock> {
        self.code_blocks
            .iter()
            .filter(move |cb| s.contains_line(cb.line))
    }

    /// 词数。中日韩文本不用空格分词，按空白切会把一篇中文 README 数成几十个「词」，
    /// 于是 `readme-length` 会把它判成「信息不足」。
    /// 折中：CJK 字符按 0.6 词计（接近中文双字词的平均密度），其余按空白切分。
    pub fn word_count(&self) -> usize {
        let mut latin = 0usize;
        let mut cjk = 0usize;
        for token in self.raw.split_whitespace() {
            let n = token.chars().filter(|c| is_cjk(*c)).count();
            cjk += n;
            if n == 0 {
                latin += 1;
            }
        }
        latin + cjk * 3 / 5
    }
}

/// 从一段 HTML 里取出 `<img src>` 与 `<a href>`。
///
/// 行号按标签在这段 HTML 中的相对位置换算：HTML 块常跨十几行，
/// 全部记成块首行会让证据指错地方。
fn html_links(html: &str, start_line: usize) -> Vec<LinkRef> {
    let lower = html.to_lowercase();
    let mut out = Vec::new();
    let mut i = 0usize;

    while let Some(rel) = lower[i..].find('<') {
        let start = i + rel;
        let Some(end_rel) = lower[start..].find('>') else {
            break;
        };
        let end = start + end_rel + 1;
        let tag = &html[start..end];
        let is_image = lower[start..end].starts_with("<img");
        let attr_name = if is_image {
            "src"
        } else if lower[start..end].starts_with("<a ") {
            "href"
        } else {
            i = end;
            continue;
        };

        if let Some(url) = title::attr(tag, attr_name) {
            out.push(LinkRef {
                url,
                line: start_line + html[..start].matches('\n').count(),
                is_image,
            });
        }
        i = end;
    }
    out
}

/// 汉字、假名、谚文。标点不算——它们不承载信息量。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF    // CJK 统一表意文字
        | 0x3400..=0x4DBF  // 扩展 A
        | 0x3040..=0x30FF  // 平假名 / 片假名
        | 0xAC00..=0xD7AF  // 谚文音节
    )
}

pub(crate) fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.autolink = true;
    o.extension.tasklist = true;
    o
}

/// 收集节点下的内联文本（含行内代码），软换行折成空格。
///
/// `skip_images` 为真时跳过图片子树。标题行与首段常挂一排徽章，
/// 不跳过就会把 alt 文本拼进项目名与说明里。
pub(crate) fn text_of<'a>(node: &'a AstNode<'a>, skip_images: bool) -> String {
    let mut out = String::new();
    collect_text(node, skip_images, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_text<'a>(node: &'a AstNode<'a>, skip_images: bool, out: &mut String) {
    for child in node.children() {
        let is_image = {
            let v = &child.data.borrow().value;
            match v {
                NodeValue::Text(t) => out.push_str(t),
                NodeValue::Code(c) => out.push_str(&c.literal),
                NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
                _ => {}
            }
            matches!(v, NodeValue::Image(_))
        };
        if is_image && skip_images {
            continue;
        }
        collect_text(child, skip_images, out);
    }
}

/// 标题之后的首个实质说明。
///
/// 跳过图片后仍有内容才算数：徽章行去掉 alt 文本就空了，
/// 而 chalk 这类项目把一句话说明写成引用块，也要认。
fn extract_tagline<'a>(root: &'a AstNode<'a>, title_line: Option<usize>) -> Option<String> {
    const MIN_LEN: usize = 8;
    let after = title_line.unwrap_or(0);
    for node in root.children() {
        let ok = {
            let data = node.data.borrow();
            data.sourcepos.start.line > after
                && matches!(data.value, NodeValue::Paragraph | NodeValue::BlockQuote)
        };
        if !ok {
            continue;
        }
        if is_nav_row(node) {
            continue;
        }
        let text = text_of(node, true);
        if text.chars().count() >= MIN_LEN {
            return Some(text);
        }
    }
    None
}

/// 导航 / 徽章行：整段几乎只有链接与分隔符。
/// ruff 的 README 在标题下先放一排徽章，再放一行 `Docs | Playground`，
/// 两者都不是项目说明。
fn is_nav_row<'a>(node: &'a AstNode<'a>) -> bool {
    const SEPARATORS: &str = "|./,-";
    let mut links = 0usize;
    let mut outside = String::new();
    collect_outside_links(node, &mut links, &mut outside);
    links >= 2
        && outside
            .chars()
            .all(|c| c.is_whitespace() || SEPARATORS.contains(c))
}

fn collect_outside_links<'a>(node: &'a AstNode<'a>, links: &mut usize, out: &mut String) {
    for child in node.children() {
        let is_link = {
            let v = &child.data.borrow().value;
            match v {
                NodeValue::Text(t) => out.push_str(t),
                NodeValue::Code(c) => out.push_str(&c.literal),
                _ => {}
            }
            matches!(v, NodeValue::Link(_))
        };
        if is_link {
            *links += 1;
            continue;
        }
        collect_outside_links(child, links, out);
    }
}

fn build_sections(headings: &[(u8, String, usize)], lines: &[&str]) -> Vec<Section> {
    let total = lines.len() + 1;
    let mut out = Vec::with_capacity(headings.len());

    for (i, (level, title, line)) in headings.iter().enumerate() {
        let end_line = headings[i + 1..]
            .iter()
            .find(|(l, _, _)| l <= level)
            .map(|(_, _, l)| *l)
            .unwrap_or(total);

        let body_start = *line;
        let body_end = end_line.saturating_sub(1).min(lines.len());
        let body = if body_start < body_end {
            lines[body_start..body_end].join("\n")
        } else {
            String::new()
        };

        out.push(Section {
            kind: section::classify(title),
            title: title.clone(),
            level: *level,
            line: *line,
            end_line,
            body,
        });
    }
    out
}

/// 从磁盘读取 README（大小写与扩展名变体）
pub fn find_readme(root: &Path) -> Option<PathBuf> {
    const CANDIDATES: &[&str] = &[
        "README.md",
        "readme.md",
        "Readme.md",
        "README.MD",
        "README.markdown",
        "README.rst",
        "README.txt",
        "README",
    ];
    CANDIDATES
        .iter()
        .map(|c| root.join(c))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(md: &str) -> Readme {
        Readme::parse("README.md", md)
    }

    #[test]
    fn badge_row_is_not_a_tagline() {
        // ruff：标题下先是一排徽章，再是一行导航链接，之后才是真正的说明
        let md = "\
# Ruff

[![v](https://img.shields.io/x.svg)](https://a.example)
[![l](https://img.shields.io/y.svg)](https://b.example)

[**Docs**](https://docs.example) | [**Playground**](https://play.example)

An extremely fast Python linter, written in Rust.
";
        let r = parse(md);
        assert_eq!(r.title.as_deref(), Some("Ruff"));
        assert_eq!(
            r.tagline.as_deref(),
            Some("An extremely fast Python linter, written in Rust.")
        );
    }

    #[test]
    fn blockquote_counts_as_tagline() {
        // chalk 把一句话说明写成引用块
        let md = "<h1 align=\"center\"><img src=\"logo.svg\" alt=\"Chalk\"></h1>\n\n> Terminal string styling done right\n";
        let r = parse(md);
        assert_eq!(r.title.as_deref(), Some("Chalk"));
        assert_eq!(
            r.tagline.as_deref(),
            Some("Terminal string styling done right")
        );
    }

    #[test]
    fn subsection_does_not_truncate_parent_section() {
        // ruff：`## Getting Started` 下有子标题 `### Installation`，
        // 安装命令在子标题里，但仍属于父区块
        let md = "\
# X

## Getting Started

### Installation

```shell
pip install x
```

## Next
";
        let r = parse(md);
        let s = r
            .section(SectionKind::Quickstart)
            .expect("找到快速开始区块");
        assert_eq!(r.code_blocks_in(s).count(), 1);
    }

    #[test]
    fn html_images_and_links_are_collected_with_real_line_numbers() {
        // fzf 的首屏：logo 与整排徽章全是 HTML，不认就等于「没有徽章」
        let md = "<div align=\"center\">\n  <img src=\"logo.png\" alt=\"fzf\">\n  <a href=\"https://ci.example\"><img src=\"https://img.shields.io/x.svg\" alt=\"Build\"></a>\n</div>\n";
        let r = parse(md);
        let imgs: Vec<(&str, usize)> = r
            .links
            .iter()
            .filter(|l| l.is_image)
            .map(|l| (l.url.as_str(), l.line))
            .collect();
        assert_eq!(
            imgs,
            vec![("logo.png", 2), ("https://img.shields.io/x.svg", 3)]
        );
        assert!(r
            .links
            .iter()
            .any(|l| l.url == "https://ci.example" && !l.is_image));
    }

    #[test]
    fn root_absolute_link_resolves_against_repo_root() {
        let r = parse("[a](/docs/x.md) [b](./docs/y.md) [c](https://e.example)\n");
        let rel: Vec<&str> = r
            .links
            .iter()
            .filter(|l| l.is_relative())
            .map(|l| l.repo_path())
            .collect();
        assert_eq!(rel, vec!["docs/x.md", "docs/y.md"]);
    }
}
