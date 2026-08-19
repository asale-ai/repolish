//! Git 事实。M1 只取 HEAD 提交时间——`activity` 检查项够用了。
//! M2 会补贡献者数、release 节奏。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct GitFacts {
    pub head_id: String,
    /// HEAD 提交时间，Unix 秒
    pub head_time: i64,
}

impl GitFacts {
    /// HEAD 距今天数。时钟异常（提交时间在未来）时返回 0。
    pub fn days_since_head(&self) -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        ((now - self.head_time) / 86_400).max(0)
    }

    pub fn short_id(&self) -> &str {
        let n = self.head_id.len().min(8);
        &self.head_id[..n]
    }
}

pub fn load(root: &Path) -> Option<GitFacts> {
    let repo = gix::discover(root).ok()?;
    let commit = repo.head_commit().ok()?;
    let head_id = commit.id().to_string();
    let head_time = commit.time().ok()?.seconds;
    Some(GitFacts { head_id, head_time })
}
