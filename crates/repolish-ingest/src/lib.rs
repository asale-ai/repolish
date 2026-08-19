//! 仓库摄取：一次遍历，产出所有检查项共享的只读事实。
//!
//! 检查项不许自己碰文件系统——全部经由 [`RepoContext`]，
//! 这样才能保证同一 commit 上的结果可复现。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use repolish_md::Readme;

mod files;
mod git;
mod profile;

pub use files::FileIndex;
pub use git::GitFacts;
pub use profile::Profile;

#[derive(Debug)]
pub struct RepoContext {
    pub root: PathBuf,
    pub files: FileIndex,
    pub readme: Option<Readme>,
    pub git: Option<GitFacts>,
    pub profile: Profile,
    /// profile 是否来自用户覆盖而非探测
    pub profile_overridden: bool,
}

impl RepoContext {
    pub fn load(root: impl AsRef<Path>, profile_override: Option<Profile>) -> Result<Self> {
        let root = dunce::canonicalize(root.as_ref())
            .with_context(|| format!("无法访问路径: {}", root.as_ref().display()))?;

        let files = FileIndex::build(&root)?;

        let readme = repolish_md::find_readme(&root).and_then(|p| {
            let raw = std::fs::read_to_string(&p).ok()?;
            Some(Readme::parse(p, raw))
        });

        let git = git::load(&root);

        let detected = profile::detect(&files, readme.as_ref());
        let (profile, profile_overridden) = match profile_override {
            Some(p) => (p, true),
            None => (detected, false),
        };

        Ok(RepoContext {
            root,
            files,
            readme,
            git,
            profile,
            profile_overridden,
        })
    }

    /// 相对仓库根的路径是否存在（走索引，不碰磁盘）
    pub fn has(&self, rel: &str) -> bool {
        self.files.contains(rel)
    }
}
