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
    /// 401/403。`hint` 是**读出来的**原因，不是猜的——见 [`refuse`]。
    Unauthorized {
        hint: String,
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
    // 不把状态码当错误：ureq 那样会连同响应头和 body 一起丢掉，
    // 而「为什么被拒」正写在那两处
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
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
        Err(e) => return Err(RemoteError::Http(e.to_string())),
    };

    let status = res.status().as_u16();
    if status != 200 {
        return Err(match status {
            404 => RemoteError::NotFound(slug.clone()),
            401 | 403 | 429 => RemoteError::Unauthorized {
                hint: refuse(
                    |name| {
                        res.headers()
                            .get(name)
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string())
                    },
                    token.is_some(),
                ),
            },
            other => RemoteError::Http(format!("GitHub answered {other}")),
        });
    }

    let json: serde_json::Value = res
        .body_mut()
        .read_json()
        .map_err(|e| RemoteError::Http(format!("the response was not valid JSON: {e}")))?;

    Ok(from_json(&json))
}

/// 为什么这次被拒。
///
/// 以前这里把每一个 403 都写成「限流」。那次碰巧是对的，但一个 SSO 拦截、
/// 一个被封的 User-Agent、一次二级限流都会得到同一句话，把使用者送去申请
/// 一个根本帮不上忙的 token。这个项目对检查项的要求是「判不了就说判不了」，
/// 自己的错误路径没有理由例外——所以这里读响应，不猜。
///
/// 三个信号，按可靠度排：
///
/// - `x-ratelimit-remaining: 0` —— 确凿的主配额耗尽，顺带给出恢复时间
/// - `retry-after` —— 二级限流（短时间内请求过密），和主配额是两回事
/// - 都没有 —— 原样转述 GitHub 自己的 `message`
fn refuse(header: impl Fn(&str) -> Option<String>, authenticated: bool) -> String {
    if header("x-ratelimit-remaining").as_deref() == Some("0") {
        let when = header("x-ratelimit-reset")
            .and_then(|v| v.parse::<u64>().ok())
            .map(minutes_until)
            .map(|m| format!(", resets in {m} min"))
            .unwrap_or_default();
        return if authenticated {
            format!("the token's hourly rate limit is used up{when}")
        } else {
            format!(
                "rate limited{when}. Anonymous calls get 60 per hour; setting GITHUB_TOKEN raises that to 5000"
            )
        };
    }

    if let Some(after) = header("retry-after") {
        return format!(
            "GitHub asked us to slow down (secondary rate limit); retry after {after}s. This is not the hourly quota — a token does not lift it"
        );
    }

    match header("x-github-sso") {
        Some(_) => "the token needs SSO authorisation for this organisation".to_string(),
        None if authenticated => "the token is invalid, expired, or lacks access".to_string(),
        None => "GitHub refused the request and gave no reason we can read".to_string(),
    }
}

/// 距离 unix 时间戳 `at` 还有几分钟。时钟不对时返回 0，不做无谓的算术。
fn minutes_until(at: u64) -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    at.saturating_sub(now).div_ceil(60)
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

#[cfg(test)]
mod refusal_tests {
    use super::*;

    /// 用一张固定的表冒充响应头
    fn headers<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn an_exhausted_quota_is_named_as_such_and_says_when_it_comes_back() {
        let msg = refuse(headers(&[("x-ratelimit-remaining", "0")]), false);
        assert!(msg.contains("rate limited"), "{msg}");
        assert!(
            msg.contains("GITHUB_TOKEN"),
            "未认证时应指出 token 能提额: {msg}"
        );
    }

    /// 已经带了 token 还被限流时，再劝人「设置 GITHUB_TOKEN」是没用的建议
    #[test]
    fn an_authenticated_caller_is_not_told_to_set_a_token() {
        let msg = refuse(headers(&[("x-ratelimit-remaining", "0")]), true);
        assert!(!msg.contains("GITHUB_TOKEN"), "{msg}");
        assert!(msg.contains("token"), "{msg}");
    }

    /// 二级限流与每小时配额是两回事，token 提不了它——不能混为一谈
    #[test]
    fn a_secondary_limit_is_not_reported_as_the_hourly_quota() {
        let msg = refuse(headers(&[("retry-after", "60")]), false);
        assert!(msg.contains("secondary"), "{msg}");
        assert!(msg.contains("60s"), "{msg}");
        assert!(!msg.contains("60 per hour"), "不该说成每小时配额: {msg}");
    }

    #[test]
    fn an_sso_protected_organisation_is_called_out() {
        let msg = refuse(headers(&[("x-github-sso", "required")]), true);
        assert!(msg.contains("SSO"), "{msg}");
    }

    /// 读不出原因就说读不出来，不要编一个
    #[test]
    fn an_unexplained_refusal_says_so_rather_than_guessing() {
        let msg = refuse(headers(&[]), false);
        assert!(msg.contains("no reason we can read"), "{msg}");
        assert!(
            !msg.contains("rate limited"),
            "不能把未知原因说成限流: {msg}"
        );
    }
}
