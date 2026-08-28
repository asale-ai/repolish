//! Git 事实：HEAD 时间、tag 列表、远端地址。
//!
//! **浅克隆必须能被识别。** `actions/checkout@v4` 默认 `fetch-depth: 1`，
//! 不拉 tag——若不区分「仓库没有 tag」与「tag 没被拉下来」，
//! 在 CI 里跑 `release-hygiene` 会给每个项目判 0 分。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    /// 附注 tag（`git tag -a`）才有说明文本；轻量 tag 只是一个指针
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitFacts {
    pub head_id: String,
    /// HEAD 提交时间，Unix 秒
    pub head_time: i64,
    /// 按引用名排序；数量与顺序对同一 commit 稳定
    pub tags: Vec<Tag>,
    /// 浅克隆：tag / 历史可能不完整，据此判定必须降级为 Inconclusive
    pub shallow: bool,
    /// origin 的 URL，用于推断 owner/repo
    pub remote_url: Option<String>,
    pub branch: Option<String>,
    /// 最近 [`ACTIVITY_WEEKS`] 周的每周提交数，下标 0 是最老的一周。
    /// 浅克隆下只有被拉下来的那部分历史，见 [`GitFacts::shallow`]。
    pub activity: Vec<u32>,
    /// 走图时实际数到的提交数，上限 [`WALK_LIMIT`]。
    /// 卡片上写「N commits」时要能说清这是不是被截断的数
    pub commits_seen: usize,
    /// 是否撞到了 [`WALK_LIMIT`]
    pub commits_truncated: bool,
}

/// 活跃度图的跨度。一年是解读一个仓库「还活着吗」的自然窗口——
/// 再短会把一次正常的假期看成停更，再长则整条曲线都压成一根线。
pub const ACTIVITY_WEEKS: usize = 52;

/// 走图的上限。活跃度图只画一年，但提交不保证按时间序遍历，
/// 所以只能按数量兜底而不能碰到窗口外就停。
///
/// linux 内核这种量级下，无上限地走完整个图要几秒钟——
/// 一张概览卡片不值这个时间。
const WALK_LIMIT: usize = 20_000;

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

    /// 形如 `v1.2.3` / `1.2.3` 的 tag。用于判断是否遵循语义化版本惯例。
    pub fn semver_tags(&self) -> impl Iterator<Item = &Tag> {
        self.tags.iter().filter(|t| is_semver_like(&t.name))
    }
}

/// 只要求「主.次.修订」三段数字，前缀 v 可有可无。
/// 不做完整 semver 校验——预发布后缀（`-rc.1`、`+build`）一律放行。
fn is_semver_like(name: &str) -> bool {
    let s = name.strip_prefix('v').unwrap_or(name);
    let head = s.split(['-', '+']).next().unwrap_or(s);
    let parts: Vec<&str> = head.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

pub fn load(root: &Path) -> Option<GitFacts> {
    let repo = gix::discover(root).ok()?;
    let commit = repo.head_commit().ok()?;
    let head_id = commit.id().to_string();
    let head_time = commit.time().ok()?.seconds;

    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|n| n.shorten().to_string());

    let remote_url = repo
        .config_snapshot()
        .string("remote.origin.url")
        .map(|v| v.to_string());

    let (activity, commits_seen, commits_truncated) = load_activity(&repo, head_time);

    GitFacts {
        head_id,
        head_time,
        tags: load_tags(&repo),
        shallow: repo.is_shallow(),
        remote_url,
        branch,
        activity,
        commits_seen,
        commits_truncated,
    }
    .into()
}

fn load_tags(repo: &gix::Repository) -> Vec<Tag> {
    let Ok(platform) = repo.references() else {
        return Vec::new();
    };
    let Ok(iter) = platform.tags() else {
        return Vec::new();
    };

    let mut out: Vec<Tag> = iter
        .flatten()
        .map(|r| {
            let name = r.name().shorten().to_string();
            // 附注 tag 的引用指向一个 tag 对象；轻量 tag 直接指向 commit。
            let message = r
                .try_id()
                .and_then(|id| id.object().ok())
                .and_then(|obj| obj.try_into_tag().ok())
                .and_then(|t| {
                    t.decode()
                        .ok()
                        .map(|d| d.message.to_string().trim().to_string())
                })
                .filter(|m| !m.is_empty());
            Tag { name, message }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 每周提交数，最近 [`ACTIVITY_WEEKS`] 周。
///
/// 窗口的**终点是 HEAD 的提交时间**，不是「现在」。一个两年没动的仓库若按
/// 当前时间开窗，画出来的是一条整齐的零线——那看着像没数据，而不像停更。
/// 以 HEAD 为终点，同一个 commit 在任何一天跑出来的图都一样，这也是卡片
/// 能逐字节复现的前提。
fn load_activity(repo: &gix::Repository, head_time: i64) -> (Vec<u32>, usize, bool) {
    const WEEK: i64 = 7 * 86_400;
    let mut weeks = vec![0u32; ACTIVITY_WEEKS];
    let start = head_time - (ACTIVITY_WEEKS as i64 - 1) * WEEK;

    let Ok(head) = repo.head_id() else {
        return (weeks, 0, false);
    };
    let Ok(walk) = repo.rev_walk([head]).all() else {
        return (weeks, 0, false);
    };

    let mut seen = 0usize;
    let mut truncated = false;
    for info in walk {
        if seen >= WALK_LIMIT {
            truncated = true;
            break;
        }
        let Ok(info) = info else { continue };
        let Ok(commit) = info.object() else { continue };
        let Ok(time) = commit.time() else { continue };
        seen += 1;
        let t = time.seconds;
        if t < start || t > head_time {
            continue;
        }
        let bucket = ((t - start) / WEEK) as usize;
        if let Some(slot) = weeks.get_mut(bucket) {
            *slot += 1;
        }
    }
    (weeks, seen, truncated)
}

#[cfg(test)]
mod tests {
    use super::is_semver_like;

    #[test]
    fn recognizes_common_tag_shapes() {
        assert!(is_semver_like("v1.2.3"));
        assert!(is_semver_like("1.0.0"));
        assert!(is_semver_like("v0.1.0-rc.1"));
        // 两段式与非版本 tag 不算
        assert!(!is_semver_like("v1.2"));
        assert!(!is_semver_like("nightly"));
        assert!(!is_semver_like("release-2024"));
    }
}
