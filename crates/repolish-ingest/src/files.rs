//! 文件索引。用 `ignore` 走一遍工作区，尊重 .gitignore 与 .repolishignore。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Default)]
pub struct FileIndex {
    /// 仓库相对路径，统一用 `/` 分隔
    paths: Vec<String>,
    lookup: HashSet<String>,
    lookup_lower: HashSet<String>,
    root: PathBuf,
}

impl FileIndex {
    pub fn build(root: &Path) -> Result<Self> {
        let mut paths = Vec::new();

        let mut builder = ignore::WalkBuilder::new(root);
        builder
            .hidden(false) // .github/ 必须能看见
            .git_ignore(true)
            .git_global(false)
            .parents(false)
            .add_custom_ignore_filename(".repolishignore");

        for entry in builder.build().flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            if let Ok(rel) = entry.path().strip_prefix(root) {
                let s = rel.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
                if s.starts_with(".git/") {
                    continue;
                }
                paths.push(s);
            }
        }

        paths.sort();
        let lookup: HashSet<String> = paths.iter().cloned().collect();
        let lookup_lower: HashSet<String> = paths.iter().map(|p| p.to_lowercase()).collect();

        Ok(FileIndex {
            paths,
            lookup,
            lookup_lower,
            root: root.to_path_buf(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.paths.iter().map(|s| s.as_str())
    }

    pub fn contains(&self, rel: &str) -> bool {
        self.lookup.contains(rel)
    }

    pub fn contains_ignore_case(&self, rel: &str) -> bool {
        self.lookup_lower.contains(&rel.to_lowercase())
    }

    /// 在仓库根目录查找候选文件名之一（大小写不敏感）
    pub fn find_at_root(&self, candidates: &[&str]) -> Option<&str> {
        let wanted: Vec<String> = candidates.iter().map(|c| c.to_lowercase()).collect();
        self.paths
            .iter()
            .find(|p| !p.contains('/') && wanted.contains(&p.to_lowercase()))
            .map(|s| s.as_str())
    }

    /// 前缀匹配（目录）
    pub fn under(&self, prefix: &str) -> impl Iterator<Item = &str> {
        let prefix = prefix.to_string();
        self.paths
            .iter()
            .filter(move |p| p.starts_with(&prefix))
            .map(|s| s.as_str())
    }

    pub fn any_matching<F: Fn(&str) -> bool>(&self, pred: F) -> bool {
        self.paths.iter().any(|p| pred(p))
    }

    pub fn count_matching<F: Fn(&str) -> bool>(&self, pred: F) -> usize {
        self.paths.iter().filter(|p| pred(p)).count()
    }

    pub fn extension_count(&self, ext: &str) -> usize {
        let suffix = format!(".{ext}");
        self.count_matching(|p| p.to_lowercase().ends_with(&suffix))
    }

    /// 只统计「项目内容」：排除 .github / .trae / .vscode / .idea 这类工具元数据目录。
    /// 把它们算进语言统计会让 profile 探测失真（例如把只放了几个 .md 的仓库判成文档站）。
    pub fn content_extension_count(&self, ext: &str) -> usize {
        let suffix = format!(".{ext}");
        self.count_matching(|p| is_content_path(p) && p.to_lowercase().ends_with(&suffix))
    }

    /// 读取仓库内文件内容（仅供检查项取证使用）
    pub fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(rel)).ok()
    }
}

/// 路径的任一层级以点开头即视为工具元数据，不计入项目内容统计。
pub fn is_content_path(path: &str) -> bool {
    !path.split('/').any(|seg| seg.starts_with('.'))
}
