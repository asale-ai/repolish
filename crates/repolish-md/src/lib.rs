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

pub mod edit;
pub mod section;
pub mod title;
pub mod toc;

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
    /// 标题**结束**行。setext 标题跨两行，插入锚点得落在下划线之后
    pub title_end_line: Option<usize>,
    pub title_source: Option<TitleSource>,
    /// 只由图片（含被链接包住的图片）构成、且至少有一张看着像徽章的段落。
    /// `polish` 往这里追加徽章，而不是另起一行——一排徽章分成两段
    /// 在渲染出来就是两行，作者摆好的版被破坏了。
    pub badge_rows: Vec<BadgeRow>,
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
        let title_end_line = candidate.as_ref().map(|c| c.end_line);
        let tagline = extract_tagline(root, title_line);
        let sections = build_sections(&headings, &lines);
        let badge_rows = badge_rows(root);

        Readme {
            path,
            raw,
            title: candidate.as_ref().map(|c| c.text.clone()),
            title_line,
            title_end_line,
            title_source: candidate.map(|c| c.source),
            badge_rows,
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

    /// 正文里最浅一层的标题，也就是目录该列的那些。
    ///
    /// 不能写死「h2」：ripgrep 的标题是 setext h2，正文小节全是 `###`，
    /// 按 h2 取会得到一个空目录。取正文里实际出现的最浅层级才通用。
    ///
    /// 文档标题本身排掉——它是项目名，不是章节。
    pub fn outline(&self) -> Vec<&Section> {
        let after = self.title_end_line.unwrap_or(0);
        let body: Vec<&Section> = self.sections.iter().filter(|s| s.line > after).collect();
        let Some(min) = body.iter().map(|s| s.level).min() else {
            return Vec::new();
        };
        body.into_iter().filter(|s| s.level == min).collect()
    }

    /// 往哪一行之后插徽章。
    ///
    /// 优先追加到开头附近**已有的**徽章行——一排徽章被拆成两个段落，
    /// 渲染出来就是两行。没有徽章行时退回标题之后。
    ///
    /// 「开头附近」用的是标题识别那套 40 行窗口：axios 的 README 正文中段
    /// 也有徽章段落，追加到那里等于把徽章插进了正文。
    pub fn badge_anchor(&self) -> Option<BadgeAnchor> {
        self.badge_rows
            .iter()
            .filter(|r| r.start <= title::MAX_TITLE_LINE)
            // 徽章最多的那一行才是「那排徽章」。开头常有一张独立的 logo，
            // 它也是个只含图片的段落；挂在 logo 后面会跟 logo 挤在同一行。
            // 并列时取靠前的——离标题越近越像首屏的那一排。
            .max_by_key(|r| (r.images, std::cmp::Reverse(r.start)))
            .map(|r| {
                if r.html {
                    BadgeAnchor::AfterHtmlBlock(r.end)
                } else {
                    BadgeAnchor::AppendToRow(r.end)
                }
            })
            .or(self.title_end_line.map(BadgeAnchor::AfterTitle))
    }
}

/// 徽章插到哪儿，以及**怎么**插。两种情形的空行处理不同：
/// 追加到已有徽章行必须紧贴上一行，否则会被解析成新段落、渲染成新的一行；
/// 插在标题之后则必须空一行，否则会粘进标题里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeAnchor {
    /// 追加到一排 Markdown 徽章后面，紧贴上一行
    AppendToRow(usize),
    /// 上一块是 HTML（居中的徽章 `<div>`）。**必须空一行**——紧跟在 HTML 块
    /// 后面的 Markdown 会被并进那个块，徽章根本不会被解析成图片。
    /// flask 和 fzf 就是这样把徽章吃掉的。
    AfterHtmlBlock(usize),
    AfterTitle(usize),
}

impl BadgeAnchor {
    pub fn line(self) -> usize {
        match self {
            BadgeAnchor::AppendToRow(l)
            | BadgeAnchor::AfterHtmlBlock(l)
            | BadgeAnchor::AfterTitle(l) => l,
        }
    }

    /// 是否追加到已有的那排 Markdown 徽章里。
    ///
    /// 调用方据此决定用 Markdown 还是 HTML：那一排是作者用 Markdown 写的，
    /// 混一行 HTML 进去会在渲染上留下一道接缝。只有另起一块时才谈得上对齐。
    pub fn appends(self) -> bool {
        matches!(self, BadgeAnchor::AppendToRow(_))
    }

    /// 插入的行，含必要的空行
    pub fn lines_for(self, badge: &str) -> Vec<String> {
        match self {
            BadgeAnchor::AppendToRow(_) => vec![badge.to_string()],
            BadgeAnchor::AfterHtmlBlock(_) | BadgeAnchor::AfterTitle(_) => {
                vec![String::new(), badge.to_string()]
            }
        }
    }
}

/// 一段只有图片的段落，且至少有一张看着像徽章。起止行 1-based。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeRow {
    pub start: usize,
    pub end: usize,
    /// 这一段里的图片张数。用来把「一排徽章」和「一张 logo」区分开。
    pub images: usize,
    /// 这一段是 HTML 块。决定插入时要不要先空一行。
    pub html: bool,
}

/// 只由图片构成、且至少有一张看着像徽章的段落（含 HTML 徽章块）。
fn badge_rows<'a>(root: &'a AstNode<'a>) -> Vec<BadgeRow> {
    let mut out = Vec::new();
    for node in root.children() {
        let (start, end, is_html) = {
            let d = node.data.borrow();
            let html = match &d.value {
                NodeValue::Paragraph => None,
                NodeValue::HtmlBlock(h) => Some(h.literal.clone()),
                _ => continue,
            };
            (d.sourcepos.start.line, d.sourcepos.end.line, html)
        };
        let html = is_html.is_some();
        let images = match is_html {
            // HTML 块：去掉标签后没有文字，就是一排居中的图片
            Some(literal) => {
                if !title::strip_tags(&literal).trim().is_empty() {
                    continue;
                }
                html_images(&literal)
            }
            None => {
                let mut images = Vec::new();
                if !only_images(node, false, &mut images) {
                    continue;
                }
                images
            }
        };
        // 必须至少有一张**被链接包住的**徽章图。裸图是 logo 或截图：
        // flask 开头那个 `<div>` 里只有 `flask-name.svg`，把徽章追加到它后面
        // 就跑到 `# Flask` 前面去了。判据和 title.rs 用的是同一条——
        // 真 logo 不会是超链接。
        if images
            .iter()
            .any(|(url, linked)| *linked && looks_like_badge(url))
        {
            out.push(BadgeRow {
                start,
                end,
                images: images.len(),
                html,
            });
        }
    }
    out
}

/// HTML 里的图片，以及它是不是被 `<a>` 包着。
fn html_images(html: &str) -> Vec<(String, bool)> {
    let lower = html.to_lowercase();
    html_links(html, 0)
        .into_iter()
        .filter(|l| l.is_image)
        .map(|l| {
            // 这张图之前最近的 `<a ` 后面还没有 `</a>`，说明它在链接里
            let at = lower.find(&l.url.to_lowercase()).unwrap_or(0);
            let linked = lower[..at]
                .rfind("<a ")
                .is_some_and(|a| !lower[a..at].contains("</a>"));
            (l.url, linked)
        })
        .collect()
}

/// 图片 URL 像不像徽章。
///
/// 光凭「这一段只有图片」是不够的：ripgrep 的截图、项目 logo 都是独立成段的
/// 单张图片。把 repolish 徽章追加到截图后面，就成了正文中间凭空多一个徽章。
/// 徽章几乎一律是 SVG，且来自 shields / badgen 这类服务；截图一律是位图。
fn looks_like_badge(url: &str) -> bool {
    let u = url.to_lowercase();
    let path = u.split(['?', '#']).next().unwrap_or(&u);
    u.contains("shields.io")
        || u.contains("badgen.net")
        || u.contains("badge")
        || path.ends_with(".svg")
}

/// 段落里除了图片、包着图片的链接和空白，还有没有别的东西。
/// 是的话收走全部图片 URL。
///
/// 图片子树整个跳过——alt 文本是图片的一部分，不是段落里的散文。
fn only_images<'a>(node: &'a AstNode<'a>, in_link: bool, out: &mut Vec<(String, bool)>) -> bool {
    for child in node.children() {
        let kind = { child.data.borrow().value.clone() };
        match kind {
            NodeValue::Image(i) => out.push((i.url.clone(), in_link)),
            NodeValue::Text(t) => {
                if !t.trim().is_empty() {
                    return false;
                }
            }
            NodeValue::SoftBreak | NodeValue::LineBreak => {}
            NodeValue::HtmlInline(h) => out.extend(html_images(&h)),
            NodeValue::Link(_) => {
                if !only_images(child, true, out) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
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

/// 字符串里有没有中日韩文字。`polish` 用它决定插进去的章节标题该用哪种语言。
pub fn has_cjk(s: &str) -> bool {
    s.chars().any(is_cjk)
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

#[cfg(test)]
mod anchor_tests {
    use super::*;

    fn anchor(md: &str) -> Option<usize> {
        Readme::parse("README.md", md)
            .badge_anchor()
            .map(|a| a.line())
    }

    #[test]
    fn appends_to_an_existing_badge_row() {
        let md = "# Tool\n\n[![CI](ci.svg)](ci)\n[![npm](npm.svg)](npm)\n\nProse.\n";
        assert_eq!(anchor(md), Some(4));
    }

    #[test]
    fn falls_back_to_the_title_when_there_are_no_badges() {
        assert_eq!(anchor("# Tool\n\nProse.\n"), Some(1));
    }

    #[test]
    fn setext_titles_anchor_below_the_underline() {
        // ripgrep 的写法。锚点若取标题起始行，徽章会插进标题和下划线之间，
        // 标题就变成了普通段落。
        assert_eq!(anchor("ripgrep (rg)\n------------\n\nProse.\n"), Some(2));
    }

    #[test]
    fn prose_paragraphs_are_not_badge_rows() {
        // 语言切换行全是链接、没有图片，不是徽章行
        let md = "# Tool\n\n[English](README.md) · [中文](README.zh-CN.md)\n\nProse.\n";
        assert_eq!(anchor(md), Some(1));
    }

    #[test]
    fn badge_rows_far_down_the_page_are_ignored() {
        // axios 正文中段也有徽章段落；追加到那里等于把徽章插进正文
        let mut md = String::from("# Tool\n\nProse.\n");
        for _ in 0..50 {
            md.push_str("filler\n\n");
        }
        md.push_str("[![x](x.svg)](x)\n");
        assert_eq!(
            Readme::parse("README.md", &md)
                .badge_anchor()
                .map(|a| a.line()),
            Some(1)
        );
    }

    #[test]
    fn centred_html_badge_blocks_count() {
        // 徽章图必须是超链接。fzf 开头那个 `<div>` 里第一张是裸 logo，
        // 后面几张才是包在 `<a>` 里的徽章；只认后者才不会挂到 logo 上。
        let md =
            "# Tool\n\n<p align=\"center\">\n  <a href=\"ci\"><img src=\"a.svg\"></a>\n</p>\n\nProse.\n";
        assert_eq!(anchor(md), Some(5));
    }
}

#[cfg(test)]
mod badge_row_tests {
    use super::*;

    fn anchor(md: &str) -> Option<usize> {
        Readme::parse("README.md", md)
            .badge_anchor()
            .map(|a| a.line())
    }

    #[test]
    fn a_screenshot_paragraph_is_not_a_badge_row() {
        // ripgrep 第 36 行：正文中间一张独立的截图，也是「只含图片的段落」。
        // 不把它排掉，徽章就会被插进「Screenshot of search results」小节里。
        let md = "# rg\n\n[![Build](https://img.shields.io/x.svg)](ci)\n\nProse.\n\n\
                  ### Screenshot\n\n[![a shot](https://example.com/shot.png)](https://example.com/shot.png)\n";
        assert_eq!(anchor(md), Some(3));
    }

    #[test]
    fn the_row_with_the_most_badges_wins_over_a_lone_logo() {
        // 开头一张 logo 单独成段，下面才是那排徽章。挂在 logo 后面会跟 logo 挤成一行。
        let md = "# Tool\n\n![logo](logo.svg)\n\n\
                  [![a](https://img.shields.io/a.svg)](a)\n[![b](https://img.shields.io/b.svg)](b)\n\nProse.\n";
        assert_eq!(anchor(md), Some(6));
    }

    #[test]
    fn ties_go_to_the_row_nearer_the_title() {
        let md = "# Tool\n\n[![a](https://img.shields.io/a.svg)](a)\n\nProse.\n\n\
                  [![b](https://img.shields.io/b.svg)](b)\n";
        assert_eq!(anchor(md), Some(3));
    }
}

#[cfg(test)]
mod html_anchor_tests {
    use super::*;

    fn a(md: &str) -> Option<BadgeAnchor> {
        Readme::parse("README.md", md).badge_anchor()
    }

    #[test]
    fn a_lone_logo_is_not_a_badge_row() {
        // flask 的开头。挂到这个 div 后面，徽章就跑到 `# Flask` 前面去了。
        let md =
            "<div align=\"center\"><img src=\"logo.svg\" alt=\"\"></div>\n\n# Flask\n\nProse.\n";
        assert_eq!(a(md), Some(BadgeAnchor::AfterTitle(3)));
    }

    #[test]
    fn html_badge_blocks_need_a_blank_line_after_them() {
        // 紧跟在 HTML 块后面的 Markdown 会被并进那个块，徽章不会被解析成图片。
        let md = "<div>\n  <a href=\"ci\"><img src=\"https://img.shields.io/a.svg\"></a>\n</div>\n\n# Tool\n";
        let anchor = a(md).unwrap();
        assert_eq!(anchor, BadgeAnchor::AfterHtmlBlock(3));
        assert_eq!(
            anchor.lines_for("BADGE"),
            vec!["".to_string(), "BADGE".to_string()]
        );

        // 真插一遍：徽章必须被解析成图片节点，而不是被 HTML 块吃掉
        let out = edit::apply(
            md,
            &[edit::Insert::new(
                3,
                "badge",
                anchor.lines_for("![b](https://img.shields.io/b.svg)"),
            )],
        );
        assert!(Readme::parse("README.md", out)
            .links
            .iter()
            .any(|l| l.is_image && l.url.contains("/b.svg")));
    }
}

#[cfg(test)]
mod outline_tests {
    use super::*;

    fn titles(md: &str) -> Vec<String> {
        Readme::parse("README.md", md)
            .outline()
            .iter()
            .map(|s| s.title.clone())
            .collect()
    }

    #[test]
    fn takes_the_shallowest_body_level() {
        let md = "# Tool\n\n## A\n\n### A1\n\n## B\n";
        assert_eq!(titles(md), vec!["A", "B"]);
    }

    #[test]
    fn a_setext_h2_title_does_not_swallow_the_h3_sections() {
        // ripgrep：标题是 setext h2，正文小节全是 `###`。
        // 写死 h2 会既把标题算进目录，又漏掉全部正文小节。
        let md = "ripgrep (rg)\n------------\n\n### CHANGELOG\n\ntext\n\n### Installation\n";
        assert_eq!(titles(md), vec!["CHANGELOG", "Installation"]);
    }

    #[test]
    fn a_readme_with_no_body_headings_has_no_outline() {
        assert!(titles("# Tool\n\nJust prose.\n").is_empty());
    }
}
