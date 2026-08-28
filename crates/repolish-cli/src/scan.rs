//! `repolish scan <DIR>` —— 一次评分一整个目录下的所有仓库。
//!
//! 为什么不做 `repolish scan <ORG>` 直接从 GitHub 拉：那要求这个二进制会
//! clone，也就要求它带上网络与 git。评分本身是离线优先的（见
//! docs/01-技术架构.md），为了省一条 `git clone` 就把整个工具变成联网工具
//! 不划算。把仓库弄到本地是 `git` 的事，`scripts/clone-org.sh` 一行搞定；
//! 这里只负责评分。
//!
//! `--remote` 下**一个仓库拉不到就单列为失败**，不退回本地分。本地分与远程分
//! 基准不同，把两种分数混在同一张表里排序，是这个工具最不该犯的错。

use std::path::{Path, PathBuf};

use repolish_core::registry::RunOptions;
use repolish_core::{Mode, RepoContext, Report};
use repolish_render::Entry;

use crate::analyze::Common;

/// 扫描一个目录下的直接子目录。
///
/// 返回按目录名排序的条目——渲染时才按分数排。这里保持稳定顺序，
/// 是为了让 `--format json` 的输出在同一批仓库上可复现。
pub fn run(root: &Path, common: &Common, only: &[String], skip: &[String]) -> Vec<Entry> {
    let mut dirs = repositories(root);
    dirs.sort();

    let registry = repolish_checks::registry();
    let token = if common.remote {
        repolish_ingest::remote::token_from_env()
    } else {
        None
    };

    dirs.into_iter()
        .map(|dir| {
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            eprintln!("  scoring {name}");
            Entry {
                report: score(&dir, common, &registry, token.as_deref(), only, skip),
                name,
            }
        })
        .collect()
}

fn score(
    dir: &Path,
    common: &Common,
    registry: &repolish_core::Registry,
    token: Option<&str>,
    only: &[String],
    skip: &[String],
) -> Result<Report, String> {
    let mut ctx = RepoContext::load(dir, None).map_err(|e| format!("{e:#}"))?;

    if common.remote {
        // 拉不到就是失败，不静默退回本地分——两种分数不是同一个基准
        ctx.fetch_remote(token).map_err(|e| e.to_string())?;
    }

    let opts = RunOptions {
        mode: if common.remote {
            Mode::Remote
        } else {
            Mode::Local
        },
        only: only.iter().cloned().collect(),
        skip: skip.iter().cloned().collect(),
    };
    Ok(registry.run(&ctx, &opts))
}

/// 直接子目录中「看起来是个仓库」的那些。
///
/// 三条判据任一成立即算：有 `.git`、有 README、或根下有包清单。
///
/// 清单那一条是必须的：**一个没有 README 的仓库恰恰是最该被报出来的那个**，
/// 只认 README 会让它从表里静默消失——那正好把这个工具的用途反了过来。
/// 三条都不满足的（`node_modules`、`target`、散落的文件）跳过。
fn repositories(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && looks_like_repo(p))
        .collect()
}

/// 根目录出现即说明「这一层发布一个包」，与 `repolish-ingest` 的清单表一致
const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "Gemfile",
    "composer.json",
];

fn looks_like_repo(dir: &Path) -> bool {
    if dir.join(".git").exists() {
        return true;
    }
    if repolish_md::find_readme(dir).is_some() {
        return true;
    }
    MANIFESTS.iter().any(|m| dir.join(m).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("repolish-scan-{name}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn a_directory_with_a_readme_counts_as_a_repository() {
        let root = scratch("readme");
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(root.join("alpha/README.md"), "# alpha\n").unwrap();
        let found = repositories(&root);
        assert_eq!(found.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    /// 构建产物这类目录不该被当成仓库数进去
    #[test]
    fn directories_without_git_readme_or_manifest_are_skipped() {
        let root = scratch("junk");
        fs::create_dir_all(root.join("build/out")).unwrap();
        fs::write(root.join("build/out/app.js"), "\n").unwrap();
        assert!(repositories(&root).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    /// 没有 README 的仓库恰恰是最该被报出来的那个，不能因为没 README 就漏掉
    #[test]
    fn a_repository_without_a_readme_is_still_scanned() {
        let root = scratch("noreadme");
        fs::create_dir_all(root.join("bare/src")).unwrap();
        fs::write(root.join("bare/Cargo.toml"), "[package]\nname = \"bare\"\n").unwrap();
        let found = repositories(&root);
        assert_eq!(found.len(), 1, "{found:?}");
        let _ = fs::remove_dir_all(&root);
    }

    /// 组织的 `.github` 是个以点开头的目录，不能被当成隐藏文件跳过
    #[test]
    fn a_dot_github_repository_is_included() {
        let root = scratch("dotgithub");
        fs::create_dir_all(root.join(".github/profile")).unwrap();
        fs::write(root.join(".github/profile/README.md"), "# acme\n").unwrap();
        fs::create_dir_all(root.join(".github/.git")).unwrap();
        let found = repositories(&root);
        assert_eq!(found.len(), 1, "{found:?}");
        let _ = fs::remove_dir_all(&root);
    }
}
