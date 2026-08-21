//! 探针：`polish` 的徽章锚点落在真实 README 的哪一行，插完是不是只多那几行。
//!
//! 探针一（examples/roundtrip.rs）证明 `format_commonmark` 往返有损，所以
//! `polish --apply` 不能让 AST 产出文本。这里验证替代方案：AST 只回答
//! 「插在第几行」，切开原文拼回去，其余字节不碰。
//!
//!   ./scripts/fetch-fixtures.sh
//!   cargo run -p repolish-md --example locate -- target/fixtures/*/README*.md README.md

use repolish_md::edit::{apply, Insert};
use repolish_md::{BadgeAnchor, Readme};

const BADGE: &str = "[![repolish](https://img.shields.io/endpoint?url=https://example/badge.json)](https://github.com/asale-ai/repolish)";

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    let mut clean = 0usize;

    for path in &files {
        let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path);

        let readme = Readme::parse(path, raw.clone());
        let Some(anchor) = readme.badge_anchor() else {
            println!("  {name:<20} no anchor — skipped");
            continue;
        };

        let added = anchor.lines_for(BADGE);
        let n = added.len();
        let out = apply(&raw, &[Insert::new(anchor.line(), "badge", added)]);

        let before: Vec<&str> = raw.lines().collect();
        let after: Vec<&str> = out.lines().collect();
        let at = anchor.line();
        // 判据：只多了 n 行，锚点两侧逐字节不变，多出来的正是那 n 行。
        let ok = after.len() == before.len() + n
            && before[..at] == after[..at]
            && before[at..] == after[at + n..]
            && after[at + n - 1] == BADGE;

        // 插完还得是一份能解析的文档，且徽章确实成了一个图片节点
        let parsed = Readme::parse(path, out)
            .links
            .iter()
            .any(|l| l.is_image && l.url.contains("img.shields.io/endpoint"));

        let kind = match anchor {
            BadgeAnchor::AppendToRow(_) => "append-to-badge-row",
            BadgeAnchor::AfterHtmlBlock(_) => "after-html-block",
            BadgeAnchor::AfterTitle(_) => "after-title",
        };
        if ok && parsed {
            clean += 1;
        }
        println!(
            "  {name:<20} {kind:<20} @{at:<5} {}",
            if ok && parsed { "clean" } else { "FAILED" }
        );
    }

    println!("\n  {clean}/{} clean", files.len());
}
