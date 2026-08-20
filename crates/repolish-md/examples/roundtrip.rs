//! 探针：comrak 的 `parse_document` → `format_commonmark` 往返是否无损。
//!
//! M4 的 `polish --apply` 原本打算「在同一份 AST 上做增量改写」。那个前提
//! 成立的条件是往返无损——否则插一行徽章产出的 diff 是整个文件。这里用真实
//! README 把这个前提验掉。
//!
//!   ./scripts/fetch-fixtures.sh
//!   cargo run -p repolish-md --example roundtrip -- target/fixtures/*/README*.md README.md

use std::fs;

use comrak::{format_commonmark, parse_document, Arena, Options};

fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.autolink = true;
    o.extension.tasklist = true;
    o
}

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    let mut lossless = 0usize;

    for path in &files {
        let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let arena = Arena::new();
        let root = parse_document(&arena, &raw, &options());
        let mut out = String::new();
        format_commonmark(root, &options(), &mut out).unwrap();

        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path);
        if out == raw {
            lossless += 1;
            println!("  {name:<20} lossless");
            continue;
        }

        let a: Vec<&str> = raw.lines().collect();
        let b: Vec<&str> = out.lines().collect();
        // 行级差异计数：只数「同位置不同」，不做 LCS 对齐——
        // 行数一旦变了这个数就没意义，所以行数变化单独报。
        let common = a.len().min(b.len());
        let differing = (0..common).filter(|&i| a[i] != b[i]).count();
        println!(
            "  {name:<20} CHANGED  lines {} -> {}  first-{} positions differ: {}",
            a.len(),
            b.len(),
            common,
            differing
        );

        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            if x != y {
                println!("      line {}:", i + 1);
                println!("        - {}", trunc(x));
                println!("        + {}", trunc(y));
                break;
            }
        }
    }

    println!("\n  {lossless}/{} lossless", files.len());
}

fn trunc(s: &str) -> String {
    let s: String = s.chars().take(90).collect();
    s
}
