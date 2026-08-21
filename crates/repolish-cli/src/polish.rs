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
    }
    plan
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
