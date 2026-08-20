//! 仓库摄取：一次遍历，产出所有检查项共享的只读事实。
//!
//! 检查项不许自己碰文件系统——全部经由 [`RepoContext`]，
//! 这样才能保证同一 commit 上的结果可复现。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use repolish_md::Readme;

mod files;
mod git;
mod manifest;
mod profile;
pub mod remote;

pub use files::FileIndex;
pub use git::{GitFacts, Tag};
pub use manifest::{normalize_package_name, Ecosystem, Manifest};
pub use profile::Profile;
pub use remote::{RemoteFacts, RepoSlug};

#[derive(Debug)]
pub struct RepoContext {
    pub root: PathBuf,
    pub files: FileIndex,
    pub readme: Option<Readme>,
    pub git: Option<GitFacts>,
    /// 仓库根目录的包清单，可能有多个（如 Rust 库 + npm 包装）
    pub manifests: Vec<Manifest>,
    /// 从 git remote 推断的 owner/repo
    pub slug: Option<RepoSlug>,
    /// 仅 `--remote` 下有值
    pub remote: Option<RemoteFacts>,
    pub profile: Profile,
    /// profile 是否来自用户覆盖而非探测
    pub profile_overridden: bool,
}

impl RepoContext {
    pub fn load(root: impl AsRef<Path>, profile_override: Option<Profile>) -> Result<Self> {
        let root = dunce::canonicalize(root.as_ref())
            .with_context(|| format!("cannot access path: {}", root.as_ref().display()))?;

        let files = FileIndex::build(&root)?;

        let readme = repolish_md::find_readme(&root).and_then(|p| {
            let raw = std::fs::read_to_string(&p).ok()?;
            Some(Readme::parse(p, raw))
        });

        let git = git::load(&root);
        let slug = git
            .as_ref()
            .and_then(|g| g.remote_url.as_deref())
            .and_then(remote::parse_slug);

        let manifests = manifest::detect(&files);

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
            manifests,
            slug,
            remote: None,
            profile,
            profile_overridden,
        })
    }

    /// 拉取 GitHub 元数据。失败不降级——见 [`remote`] 模块说明。
    pub fn fetch_remote(&mut self, token: Option<&str>) -> Result<(), remote::RemoteError> {
        let slug = self
            .slug
            .clone()
            .ok_or(remote::RemoteError::NoGithubRemote)?;
        self.remote = Some(remote::fetch(&slug, token)?);
        Ok(())
    }

    /// 相对仓库根的路径是否存在（走索引，不碰磁盘）
    pub fn has(&self, rel: &str) -> bool {
        self.files.contains(rel)
    }

    /// 该仓库对外发布的包名。多个清单时取第一个有名字的。
    pub fn package_name(&self) -> Option<&str> {
        self.manifests.iter().find_map(|m| m.name.as_deref())
    }

    /// README 首选的展示名：包名优先，其次 README 标题，最后仓库目录名。
    pub fn display_name(&self) -> String {
        if let Some(n) = self.package_name() {
            return n.to_string();
        }
        if let Some(t) = self.readme.as_ref().and_then(|r| r.title.as_deref()) {
            return t.to_string();
        }
        self.root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }
}
