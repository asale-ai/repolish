//! GitHub 仓库元数据。只在 `--remote` 下调用。
//!
//! 用 `ureq` 而非 `octocrab`：整个远程需求就是一个 `GET /repos/{owner}/{repo}`，
//! 为它引入 tokio + hyper 会把这个同步 CLI 拖成异步的，也拖慢 M3 的多平台静态二进制。
//!
//! **失败一律不静默降级。** 拿不到元数据就返回错误由 CLI 以退出码 4 结束——
//! 若悄悄退回本地模式，用户会拿到一个基准不同却没有标注的分数。

use std::fmt;
use std::time::Duration;

const API: &str = "https://api.github.com";
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSlug {
    pub owner: String,
    pub name: String,
}

impl fmt::Display for RepoSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RemoteFacts {
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub topics: Vec<String>,
    /// GitHub 识别出的 SPDX 标识
    pub license: Option<String>,
    pub archived: bool,
    pub stars: u64,
    pub default_branch: Option<String>,
}

#[derive(Debug)]
pub enum RemoteError {
    /// 没有 origin，或 origin 不是 GitHub
    NoGithubRemote,
    NotFound(RepoSlug),
    /// 401/403：多半是未认证时用尽了每小时 60 次匿名配额
    Unauthorized {
        hint: &'static str,
    },
    Http(String),
}

impl fmt::Display for RemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteError::NoGithubRemote => {
                write!(
                    f,
                    "no GitHub remote found (git remote origin), so --remote has nothing to query"
                )
            }
            RemoteError::NotFound(s) => write!(
                f,
                "{s} not found on GitHub (private repositories need a token)"
            ),
            RemoteError::Unauthorized { hint } => {
                write!(f, "GitHub API refused the request: {hint}")
            }
            RemoteError::Http(e) => write!(f, "GitHub API call failed: {e}"),
        }
    }
}

impl std::error::Error for RemoteError {}

/// 从 git remote URL 解析 owner/repo。支持 https、ssh、scp 三种写法。
pub fn parse_slug(url: &str) -> Option<RepoSlug> {
    let u = url.trim().trim_end_matches('/');
    let rest = if let Some(r) = u.strip_prefix("git@github.com:") {
        r
    } else if let Some(r) = u.strip_prefix("ssh://git@github.com/") {
        r
    } else {
        let r = u
            .strip_prefix("https://github.com/")
            .or_else(|| u.strip_prefix("http://github.com/"))
            .or_else(|| u.strip_prefix("git://github.com/"))?;
        // https://user@github.com/... 的写法在 strip 之后不会出现，此处无需处理
        r
    };

    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let owner = parts.next()?.trim();
    let name = parts.next()?.trim();
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some(RepoSlug {
        owner: owner.to_string(),
        name: name.to_string(),
    })
}

pub fn fetch(slug: &RepoSlug, token: Option<&str>) -> Result<RemoteFacts, RemoteError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();

    let url = format!("{API}/repos/{}/{}", slug.owner, slug.name);
    let mut req = agent
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(
            "User-Agent",
            concat!("repolish/", env!("CARGO_PKG_VERSION")),
        );
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }

    let mut res = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(404)) => return Err(RemoteError::NotFound(slug.clone())),
        Err(ureq::Error::StatusCode(401)) => {
            return Err(RemoteError::Unauthorized {
                hint: "the token is invalid or has expired",
            })
        }
        Err(ureq::Error::StatusCode(403)) | Err(ureq::Error::StatusCode(429)) => {
            return Err(RemoteError::Unauthorized {
                hint: "rate limited. Anonymous calls get 60 per hour; setting GITHUB_TOKEN raises that to 5000",
            })
        }
        Err(e) => return Err(RemoteError::Http(e.to_string())),
    };

    let json: serde_json::Value = res
        .body_mut()
        .read_json()
        .map_err(|e| RemoteError::Http(format!("the response was not valid JSON: {e}")))?;

    Ok(from_json(&json))
}

fn from_json(v: &serde_json::Value) -> RemoteFacts {
    RemoteFacts {
        description: non_empty(v.get("description")),
        homepage: non_empty(v.get("homepage")),
        topics: v
            .get("topics")
            .and_then(|t| t.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        license: v
            .get("license")
            .and_then(|l| l.get("spdx_id"))
            .and_then(|s| s.as_str())
            // 认不出许可证时 GitHub 返回字符串 "NOASSERTION"，不是 null
            .filter(|s| *s != "NOASSERTION")
            .map(str::to_string),
        archived: v.get("archived").and_then(|a| a.as_bool()).unwrap_or(false),
        stars: v
            .get("stargazers_count")
            .and_then(|s| s.as_u64())
            .unwrap_or(0),
        default_branch: non_empty(v.get("default_branch")),
    }
}

/// GitHub 把「未设置」表示成 null 或空串两种形态，都要当作没有
fn non_empty(v: Option<&serde_json::Value>) -> Option<String> {
    v.and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// 从环境变量取 token。Action 里 `GITHUB_TOKEN` 免费可得。
pub fn token_from_env() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN", "REPOLISH_GITHUB_TOKEN"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slug(o: &str, n: &str) -> Option<RepoSlug> {
        Some(RepoSlug {
            owner: o.into(),
            name: n.into(),
        })
    }

    #[test]
    fn parses_all_three_remote_url_shapes() {
        assert_eq!(
            parse_slug("https://github.com/BurntSushi/ripgrep.git"),
            slug("BurntSushi", "ripgrep")
        );
        assert_eq!(
            parse_slug("git@github.com:serde-rs/serde.git"),
            slug("serde-rs", "serde")
        );
        assert_eq!(
            parse_slug("ssh://git@github.com/astral-sh/ruff"),
            slug("astral-sh", "ruff")
        );
        assert_eq!(
            parse_slug("https://github.com/koajs/koa/"),
            slug("koajs", "koa")
        );
    }

    #[test]
    fn non_github_remotes_are_rejected() {
        assert!(parse_slug("https://gitlab.com/a/b.git").is_none());
        assert!(parse_slug("https://github.com/onlyowner").is_none());
    }

    #[test]
    fn unset_metadata_is_none_not_empty_string() {
        // GitHub 对未设置的字段返回 null，对清空过的字段返回 ""
        let v: serde_json::Value = serde_json::from_str(
            r#"{"description":null,"homepage":"","license":{"spdx_id":"NOASSERTION"}}"#,
        )
        .unwrap();
        let f = from_json(&v);
        assert!(f.description.is_none());
        assert!(f.homepage.is_none());
        assert!(f.license.is_none());
    }
}
