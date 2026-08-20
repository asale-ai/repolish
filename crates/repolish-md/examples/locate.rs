//! 探针二：用 `sourcepos` 定位、在文本层插入，验证 diff 是否只有插入的那几行。
//!
//! 探针一（examples/roundtrip.rs）已经证明 `format_commonmark` 往返有损，
//! 所以 M4 的 `polish --apply` 不能让 AST 产出文本。这里验证替代方案：
//! AST 只回答「插在第几行」，切开原文拼回去，其余字节不碰。
//!
//!   ./scripts/fetch-fixtures.sh
//!   cargo run -p repolish-md --example locate -- target/fixtures/*/README*.md README.md

use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena, Options};

const BADGE: &str = "[![repolish](https://img.shields.io/endpoint?url=https://example/badge.json)](https://github.com/asale-ai/repolish)";

fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.autolink = true;
    o.extension.tasklist = true;
    o
}

/// 原文按行切开，**保留各自的行尾**。最后一行可能没有换行符。
fn lines_with_endings(s: &str) -> Vec<&str> {
    s.split_inclusive('\n').collect()
}

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    let mut clean = 0usize;

    for path in &files {
        let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path);

        let arena = Arena::new();
        let root = parse_document(&arena, &raw, &options());

        // 第一个标题，作为「徽章插在它后面」的锚点。
        let mut anchor = None;
        for node in root.children() {
            let d = node.data.borrow();
            if let NodeValue::Heading(h) = d.value {
                anchor = Some((h.level, d.sourcepos.start.line, d.sourcepos.end.line));
                break;
            }
        }
        let Some((level, start, end)) = anchor else {
            println!("  {name:<20} no heading — skipped");
            continue;
        };

        let src = lines_with_endings(&raw);
        // sourcepos 是 1-based。锚点行必须真的存在。
        if end == 0 || end > src.len() {
            println!(
                "  {name:<20} BAD sourcepos: h{level} claims lines {start}..{end}, file has {}",
                src.len()
            );
            continue;
        }

        // 行尾跟着锚点行走，CRLF 的文件插进去的也是 CRLF。
        let eol = if src[end - 1].ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };

        let mut out = String::with_capacity(raw.len() + BADGE.len() + 8);
        for l in &src[..end] {
            out.push_str(l);
        }
        out.push_str(eol);
        out.push_str(BADGE);
        out.push_str(eol);
        for l in &src[end..] {
            out.push_str(l);
        }

        // 判据：除了插入的两行，其余每一行逐字节不变。
        let after = lines_with_endings(&out);
        let ok_len = after.len() == src.len() + 2;
        let ok_before = src[..end] == after[..end];
        let ok_after = src[end..] == after[end + 2..];
        let ok_inserted = after[end].trim_end_matches(['\r', '\n']).is_empty()
            && after[end + 1].trim_end_matches(['\r', '\n']) == BADGE;

        // 插完还得是一份能解析的文档，且徽章确实成了一个链接节点。
        let arena2 = Arena::new();
        let root2 = parse_document(&arena2, &out, &options());
        let mut badge_linked = false;
        for node in root2.descendants() {
            if let NodeValue::Image(ref l) = node.data.borrow().value {
                if l.url.contains("img.shields.io/endpoint") {
                    badge_linked = true;
                }
            }
        }

        let all = ok_len && ok_before && ok_after && ok_inserted && badge_linked;
        if all {
            clean += 1;
        }
        println!(
            "  {name:<20} h{level}@{start}-{end}  eol={:<4} {} {}",
            if eol == "\r\n" { "CRLF" } else { "LF" },
            if all { "clean" } else { "FAILED" },
            if all {
                String::new()
            } else {
                format!(
                    "(len={ok_len} before={ok_before} after={ok_after} inserted={ok_inserted} parsed={badge_linked})"
                )
            }
        );
    }

    println!("\n  {clean}/{} clean", files.len());
}
