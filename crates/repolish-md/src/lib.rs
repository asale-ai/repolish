//! README 解析：AST → 区块模型。
//!
//! M1 只做「读」。M4 会在同一份 AST 上做增量改写（`format_commonmark` 往返）。

use std::path::{Path, PathBuf};

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};

pub mod section;
pub use section::SectionKind;

#[derive(Debug, Clone)]
pub struct Section {
    pub kind: SectionKind,
    /// 标题原文
    pub title: String,
    pub level: u8,
    /// 标题所在行（1-based）
    pub line: usize,
    /// 该区块正文（不含标题行）
    pub body: String,
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
    /// 是否为指向仓库内文件的相对链接（排除 http、mailto、锚点、协议相对）
    pub fn is_relative(&self) -> bool {
        let u = self.url.trim();
        if u.is_empty() || u.starts_with('#') || u.starts_with("//") {
            return false;
        }
        !u.contains("://") && !u.starts_with("mailto:") && !u.starts_with("tel:")
    }

    /// 去掉锚点与查询串后的路径部分
    pub fn path_part(&self) -> &str {
        let u = self.url.trim();
        let u = u.split('#').next().unwrap_or(u);
        u.split('?').next().unwrap_or(u)
    }
}

#[derive(Debug, Clone)]
pub struct Readme {
    pub path: PathBuf,
    pub raw: String,
    /// 首个 H1 的文本
    pub title: Option<String>,
    pub title_line: Option<usize>,
    /// H1 之后的首个「有实质内容」的段落——徽章行与纯图片行不算
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
        let mut title: Option<String> = None;
        let mut title_line: Option<usize> = None;

        for node in root.descendants() {
            let data = node.data.borrow();
            let line = data.sourcepos.start.line;
            match &data.value {
                NodeValue::Heading(h) => {
                    let text = inline_text(node);
                    if h.level == 1 && title.is_none() {
                        title = Some(text.clone());
                        title_line = Some(line);
                    }
                    headings.push((h.level, text, line));
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
                _ => {}
            }
        }

        let tagline = extract_tagline(root, title_line);
        let sections = build_sections(&headings, &lines);

        Readme {
            path,
            raw,
            title,
            title_line,
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

    pub fn word_count(&self) -> usize {
        self.raw.split_whitespace().count()
    }
}

fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.autolink = true;
    o.extension.tasklist = true;
    o
}

/// 收集节点下所有内联文本（含行内代码），软换行折成空格。
fn inline_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    for n in node.descendants() {
        match &n.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// H1 之后的首个实质段落。跳过纯徽章 / 纯图片 / 纯链接的段落。
fn extract_tagline<'a>(root: &'a AstNode<'a>, title_line: Option<usize>) -> Option<String> {
    let after = title_line.unwrap_or(0);
    for node in root.children() {
        let data = node.data.borrow();
        if data.sourcepos.start.line <= after {
            continue;
        }
        if !matches!(data.value, NodeValue::Paragraph) {
            continue;
        }
        drop(data);
        let text = inline_text(node);
        if text.len() < 8 {
            continue;
        }
        // 徽章行：段落里几乎全是图片
        let img_count = node
            .descendants()
            .filter(|n| matches!(n.data.borrow().value, NodeValue::Image(_)))
            .count();
        if img_count > 0 && text.len() < 40 {
            continue;
        }
        return Some(text);
    }
    None
}

fn build_sections(headings: &[(u8, String, usize)], lines: &[&str]) -> Vec<Section> {
    let mut out = Vec::with_capacity(headings.len());
    for (i, (level, title, line)) in headings.iter().enumerate() {
        let start = *line; // 标题行之后
        let end = headings
            .get(i + 1)
            .map(|(_, _, l)| l.saturating_sub(1))
            .unwrap_or(lines.len());
        let body = if start < end {
            lines[start..end.min(lines.len())].join("\n")
        } else {
            String::new()
        };
        out.push(Section {
            kind: section::classify(title),
            title: title.clone(),
            level: *level,
            line: *line,
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
