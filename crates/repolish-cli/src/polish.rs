//! `polish` —— 把能机械落实的建议直接写进 README。
//!
//! 边界：**只增量插入，不重写任何已有内容**。产出的 diff 必须全是新增行，
//! 别的行改动一个字节都算 bug。README 是作者的东西——一个教人把仓库弄体面的
//! 工具，不该顺手把别人的排版重排一遍。
//!
//! 落地方式见 [`repolish_md::edit`]：AST 只回答「插在第几行」，
//! 切开原文拼回去。为什么不能让 AST 产出文本，见 repolish-md 的 crate 文档。

use std::path::PathBuf;

use repolish_core::{RepoContext, Report};
use repolish_md::edit::{apply, Insert};
use repolish_md::Readme;

/// 一次运行要落的全部改动。
#[derive(Default)]
pub struct Plan {
    /// 对 README 的插入
    pub inserts: Vec<Insert>,
    /// 需要一并写出的附带文件。徽章行指向 `.repolish/badge.json`，
    /// 那个文件不存在的话插进去的是一个 404——比不插更糟。
    pub side_files: Vec<(PathBuf, String)>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.inserts.is_empty() && self.side_files.is_empty()
    }
}

pub fn plan(ctx: &RepoContext, report: &Report) -> Plan {
    let mut plan = Plan::default();
    if let Some(readme) = ctx.readme.as_ref() {
        badge(ctx, report, readme, &mut plan);
        toc(report, readme, &mut plan);
    }
    plan
}

/// 某个检查项是否扣了分。
///
/// `polish` 落的每一刀都得对得上一条检查结果——阈值由检查项定义，
/// 这边再写一遍迟早会漂。
fn failing(report: &Report, id: &str) -> bool {
    report.checks.iter().any(|c| {
        c.id == id
            && matches!(c.outcome, repolish_core::Outcome::Scored { score, .. } if score < 10)
    })
}

/// repolish 徽章。
///
/// 三个前提缺一不可：能算出仓库 slug（否则 URL 里的 owner/repo 只能靠猜）、
/// 覆盖率够得上出徽章、README 里还没有。
fn badge(ctx: &RepoContext, report: &Report, readme: &Readme, plan: &mut Plan) {
    let Some(slug) = ctx.slug.as_ref() else {
        return;
    };
    // 覆盖率不足时 badge_json 返回 None。这种情况下连徽章文件都不该写，
    // 更不该往别人 README 里插一个指向不存在文件的链接。
    let Some(json) = repolish_render::badge_json(report) else {
        return;
    };

    let branch = ctx
        .git
        .as_ref()
        .and_then(|g| g.branch.clone())
        .unwrap_or_else(|| "main".to_string());
    let snippet = repolish_render::snippet(&slug.owner, &slug.name, &branch);

    if let Some(insert) = badge_insert(readme, &snippet, repolish_render::BADGE_PATH) {
        plan.inserts.push(insert);
    }

    let path = ctx.root.join(repolish_render::BADGE_PATH);
    if !path.exists() {
        plan.side_files.push((path, json));
    }
}

/// README 里该不该插徽章、插在哪。
///
/// `marker` 是「已经有了」的判据：徽章 URL 里一定含 `.repolish/badge.json`
/// 这个路径。按整段 snippet 比对是不行的——分支名不同、
/// owner 大小写不同都会让同一个徽章看起来像两个。
fn badge_insert(readme: &Readme, snippet: &str, marker: &str) -> Option<Insert> {
    if readme.raw.contains(marker) {
        return None;
    }
    let anchor = readme.badge_anchor()?;
    Some(Insert::new(
        anchor.line(),
        "readme-badges: no repolish badge yet",
        anchor.lines_for(snippet),
    ))
}

/// 少于这个条目数就不插目录——两三行的目录只是噪声。
const MIN_TOC_ITEMS: usize = 4;

/// 目录。
///
/// 每一条都由作者自己的标题生成，一个字都不是编的；锚点按 GitHub 的
/// slugger 算（见 [`repolish_md::toc`]），否则插进去的是一堆跳不到的死链。
fn toc(report: &Report, readme: &Readme, plan: &mut Plan) {
    if !failing(report, "readme-toc") {
        return;
    }
    if let Some(insert) = toc_insert(readme) {
        plan.inserts.push(insert);
    }
}

/// 目录本身。门槛判定在 [`toc`]，这里只管「长什么样、插在哪」。
fn toc_insert(readme: &Readme) -> Option<Insert> {
    let outline = readme.outline();
    if outline.len() < MIN_TOC_ITEMS {
        return None;
    }
    let first = outline[0];

    // 锚点要拿**全文**标题一起算：正文里有同名标题时，`-1` / `-2` 的编号
    // 才不会错位。只算目录里列的那几个是不够的。
    let anchors = repolish_md::toc::anchors(readme.sections.iter().map(|s| s.title.as_str()));

    // 目录标题的层级跟着正文走。ripgrep 的小节是 `###`，插一个 `##` 进去
    // 等于凭空多出一层，把它原本的层级结构切断了。
    let hashes = "#".repeat(first.level as usize);
    let mut lines = vec![format!("{hashes} {}", toc_word(&outline)), String::new()];
    for s in &outline {
        let anchor = readme
            .sections
            .iter()
            .position(|x| x.line == s.line)
            .map(|i| anchors[i].clone())
            .unwrap_or_else(|| repolish_md::toc::anchor(&s.title));
        lines.push(format!("- [{}](#{anchor})", s.title));
    }
    lines.push(String::new());

    Some(Insert::new(
        first.line - 1,
        format!(
            "readme-toc: {} sections over {} lines, with no table of contents",
            readme.sections.len(),
            readme.raw.lines().count()
        ),
        lines,
    ))
}

/// 目录该叫「Contents」还是「目录」。
///
/// 这一段是写进**别人的** README 的，跟着人家的语言走。repolish 自己的
/// 报告一律英文，那是另一回事——见 CONTRIBUTING 的第三条规则。
fn toc_word(outline: &[&repolish_md::Section]) -> &'static str {
    let cjk = outline
        .iter()
        .filter(|s| repolish_md::has_cjk(&s.title))
        .count();
    if cjk * 2 > outline.len() {
        "目录"
    } else {
        "Contents"
    }
}

/// 把计划应用到原文上。
pub fn polished(readme: &Readme, plan: &Plan) -> String {
    apply(&readme.raw, &plan.inserts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNIPPET: &str = "[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/o/r/main/.repolish/badge.json)](https://github.com/asale-ai/repolish)";
    const MARKER: &str = ".repolish/badge.json";

    fn polish(md: &str) -> Option<String> {
        let readme = Readme::parse("README.md", md);
        let insert = badge_insert(&readme, SNIPPET, MARKER)?;
        Some(apply(&readme.raw, &[insert]))
    }

    #[test]
    fn appends_to_an_existing_badge_row_without_a_blank_line() {
        // 空一行会让徽章变成新段落，渲染出来另起一行——作者摆好的一排就断了
        let out = polish("# Tool\n\n[![CI](ci.svg)](ci)\n\nProse.\n").unwrap();
        assert_eq!(
            out,
            format!("# Tool\n\n[![CI](ci.svg)](ci)\n{SNIPPET}\n\nProse.\n")
        );
    }

    #[test]
    fn inserts_after_the_title_with_a_blank_line() {
        let out = polish("# Tool\n\nProse.\n").unwrap();
        assert_eq!(out, format!("# Tool\n\n{SNIPPET}\n\nProse.\n"));
    }

    #[test]
    fn does_nothing_when_the_badge_is_already_there() {
        let md = format!("# Tool\n\n{SNIPPET}\n\nProse.\n");
        assert!(polish(&md).is_none());
    }

    #[test]
    fn a_badge_on_another_branch_still_counts_as_present() {
        // 同一个徽章指向 master 分支。按整段比对会重复插一次。
        let md = "# Tool\n\n[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/o/r/master/.repolish/badge.json)](https://github.com/asale-ai/repolish)\n";
        assert!(polish(md).is_none());
    }

    #[test]
    fn crlf_readmes_keep_crlf() {
        let out = polish("# Tool\r\n\r\nProse.\r\n").unwrap();
        assert_eq!(out, format!("# Tool\r\n\r\n{SNIPPET}\r\n\r\nProse.\r\n"));
    }

    #[test]
    fn everything_except_the_inserted_lines_is_byte_identical() {
        // 这是这个命令的核心承诺，值得单独立一条：
        // 制表符、`*` 列表标记、引用式链接定义、行尾，一个字节都不能动。
        let md = "Tool\n====\n\n*  item\n\thard tab\n\n[ref]: https://example.com\n";
        let out = polish(md).unwrap();
        let before: Vec<&str> = md.lines().collect();
        let after: Vec<&str> = out.lines().collect();
        assert_eq!(after.len(), before.len() + 2);
        assert_eq!(&after[..2], &before[..2]); // 标题与下划线
        assert_eq!(&after[4..], &before[2..]); // 其余原样后移
        assert_eq!(after[2], "");
        assert_eq!(after[3], SNIPPET);
    }

    #[test]
    fn no_title_means_no_anchor_and_no_edit() {
        // 认不出标题就不猜位置。宁可不改，也不要插到一个说不清的地方。
        assert!(polish("just prose, no heading at all\n").is_none());
    }
}

#[cfg(test)]
mod toc_tests {
    use super::*;
    use repolish_md::edit::apply;

    fn toc(md: &str) -> Option<String> {
        let readme = Readme::parse("README.md", md);
        toc_insert(&readme).map(|i| apply(&readme.raw, &[i]))
    }

    #[test]
    fn lists_the_body_sections_with_github_anchors() {
        let md = "# Tool\n\nTagline.\n\n## Why & how\n\na\n\n## Install\n\nb\n\n## Usage\n\nc\n\n## License\n\nd\n";
        let out = toc(md).unwrap();
        assert!(out.contains("## Contents\n"));
        assert!(out.contains("- [Why & how](#why--how)\n"));
        assert!(out.contains("- [License](#license)\n"));
        // 目录插在第一个正文小节之前，标语之后
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "# Tool");
        assert_eq!(lines[2], "Tagline.");
        assert_eq!(lines[4], "## Contents");
        assert_eq!(lines[11], "## Why & how");
    }

    #[test]
    fn the_toc_heading_matches_the_level_of_the_sections_it_lists() {
        // ripgrep：小节是 `###`。插一个 `##` 进去等于凭空多出一层。
        let md = "rg\n--\n\n### A\n\na\n\n### B\n\nb\n\n### C\n\nc\n\n### D\n\nd\n";
        let out = toc(md).unwrap();
        assert!(out.contains("### Contents\n"), "{out}");
        assert!(out.contains("- [A](#a)\n"));
    }

    #[test]
    fn a_chinese_readme_gets_a_chinese_heading() {
        let md =
            "# 工具\n\n## 为什么做这个\n\na\n\n## 安装\n\nb\n\n## 用法\n\nc\n\n## 许可证\n\nd\n";
        let out = toc(md).unwrap();
        assert!(out.contains("## 目录\n"), "{out}");
        assert!(out.contains("- [安装](#安装)\n"));
    }

    #[test]
    fn duplicate_headings_elsewhere_shift_the_numbering() {
        // 正文里另有一个 `### Usage`。GitHub 按全文顺序编号，
        // 只算目录里那几条会把第二个 Usage 的锚点算成 `usage` 而不是 `usage-1`。
        let md = "# Tool\n\n## Usage\n\na\n\n### Usage\n\nb\n\n## Notes\n\nc\n\n## Usage\n\nd\n\n## End\n\ne\n";
        let out = toc(md).unwrap();
        assert!(out.contains("- [Usage](#usage)\n"), "{out}");
        assert!(out.contains("- [Usage](#usage-2)\n"), "{out}");
    }

    #[test]
    fn a_short_outline_is_left_alone() {
        // 两三行的目录只是噪声
        assert!(toc("# Tool\n\n## A\n\na\n\n## B\n\nb\n").is_none());
    }

    #[test]
    fn everything_outside_the_inserted_block_is_byte_identical() {
        let md =
            "# Tool\n\n*  keep\n\thard tab\n\n## A\n\na\n\n## B\n\nb\n\n## C\n\nc\n\n## D\n\nd\n";
        let out = toc(md).unwrap();
        let before: Vec<&str> = md.lines().collect();
        let after: Vec<&str> = out.lines().collect();
        let added = after.len() - before.len();
        assert_eq!(&after[..5], &before[..5]);
        assert_eq!(&after[5 + added..], &before[5..]);
    }
}
