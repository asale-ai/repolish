//! 项目结构树。
//!
//! 这是 `polish` 唯一一把**不由检查结果驱动**的刀：没有任何一项检查要求
//! README 里有目录树。所以它默认关闭，只有配置里显式给了 `tree-depth`
//! 才会生成，而且干跑时会把「由配置要求，不是由发现要求」明写出来。
//!
//! 命名这个例外，比假装它也是一条修复要诚实。
//!
//! readme-ai 这一项常被抱怨会把 README 撑得很长，所以这里有两条克制：
//! 深度默认很浅，且**同一层超过 [`MAX_PER_DIR`] 个条目就折叠成一行计数**——
//! 一个列出 200 个文件的树，没有人读。

use repolish_ingest::FileIndex;

/// 同一层最多列几个，超出的折成 `… and N more`
const MAX_PER_DIR: usize = 12;

/// 生成树的正文（不含围栏）。`depth` 是相对仓库根的层数，1 表示只列根目录。
pub fn render(files: &FileIndex, root_name: &str, depth: usize) -> String {
    let mut out = format!("{root_name}/\n");
    walk(files, "", depth, &mut String::new(), &mut out);
    out
}

/// 某个前缀下的直接子项：目录名（去重）与文件名
fn children(files: &FileIndex, prefix: &str) -> (Vec<String>, Vec<String>) {
    let mut dirs: Vec<String> = Vec::new();
    let mut leaves: Vec<String> = Vec::new();

    for path in files.iter() {
        let Some(rest) = path.strip_prefix(prefix) else {
            continue;
        };
        // strip_prefix("") 会命中一切，这是我们要的；非空前缀必须以 / 结尾
        match rest.split_once('/') {
            Some((dir, _)) => {
                let name = dir.to_string();
                if !dirs.contains(&name) {
                    dirs.push(name);
                }
            }
            None => leaves.push(rest.to_string()),
        }
    }
    dirs.sort();
    leaves.sort();
    (dirs, leaves)
}

fn walk(files: &FileIndex, prefix: &str, depth: usize, indent: &mut String, out: &mut String) {
    if depth == 0 {
        return;
    }
    let (dirs, leaves) = children(files, prefix);

    // 目录在前、文件在后：读的人先要的是结构，不是散落的配置文件
    let mut entries: Vec<(String, bool)> = dirs.into_iter().map(|d| (d, true)).collect();
    entries.extend(leaves.into_iter().map(|f| (f, false)));

    let shown = entries.len().min(MAX_PER_DIR);
    let hidden = entries.len() - shown;

    for (i, (name, is_dir)) in entries.iter().take(shown).enumerate() {
        let last = i + 1 == shown && hidden == 0;
        let elbow = if last { "└── " } else { "├── " };
        let slash = if *is_dir { "/" } else { "" };
        out.push_str(&format!("{indent}{elbow}{name}{slash}\n"));

        if *is_dir {
            let added = if last { "    " } else { "│   " };
            indent.push_str(added);
            walk(files, &format!("{prefix}{name}/"), depth - 1, indent, out);
            indent.truncate(indent.len() - added.len());
        }
    }

    if hidden > 0 {
        out.push_str(&format!("{indent}└── … and {hidden} more\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// 目录名必须按用例区分：测试是并行跑的，共用一个名字会互相清空
    fn index(name: &str, files: &[&str]) -> (FileIndex, PathBuf) {
        let root = std::env::temp_dir().join(format!("repolish-tree-{name}"));
        let _ = fs::remove_dir_all(&root);
        for f in files {
            let full = root.join(f);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, "x\n").unwrap();
        }
        (FileIndex::build(&root).unwrap(), root)
    }

    #[test]
    fn directories_come_before_files_and_both_are_sorted() {
        let (files, root) = index("order", &["z.md", "a.md", "src/main.rs", "docs/guide.md"]);
        let out = render(&files, "thing", 1);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "thing/");
        assert!(lines[1].contains("docs/"), "{out}");
        assert!(lines[2].contains("src/"), "{out}");
        assert!(lines[3].contains("a.md"), "{out}");
        assert!(lines[4].contains("z.md"), "{out}");
        let _ = fs::remove_dir_all(&root);
    }

    /// 深度是硬边界：给 1 就只能看到根目录这一层
    #[test]
    fn depth_one_does_not_descend() {
        let (files, root) = index("depth", &["src/main.rs", "src/deep/mod.rs"]);
        let out = render(&files, "thing", 1);
        assert!(out.contains("src/"), "{out}");
        assert!(!out.contains("main.rs"), "深度 1 不该下钻:\n{out}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn depth_two_shows_one_level_of_children() {
        let (files, root) = index("depth2", &["src/main.rs", "src/deep/mod.rs"]);
        let out = render(&files, "thing", 2);
        assert!(out.contains("main.rs"), "{out}");
        assert!(out.contains("deep/"), "{out}");
        assert!(!out.contains("mod.rs"), "深度 2 不该到第三层:\n{out}");
        let _ = fs::remove_dir_all(&root);
    }

    /// 一个列出 200 个文件的树没有人读
    #[test]
    fn a_crowded_directory_is_folded_into_a_count() {
        let names: Vec<String> = (0..20).map(|i| format!("f{i:02}.md")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let (files, root) = index("crowded", &refs);
        let out = render(&files, "thing", 1);
        assert!(out.contains("… and 8 more"), "{out}");
        assert_eq!(out.lines().count(), 1 + MAX_PER_DIR + 1, "{out}");
        let _ = fs::remove_dir_all(&root);
    }
}
