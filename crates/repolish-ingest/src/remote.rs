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

/// star 曲线上的一个点：某个时刻，这个仓库有多少 star。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarPoint {
    /// Unix 秒
    pub at: i64,
    /// 累计 star 数
    pub count: u64,
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
    /// star 增长曲线，按时间升序。空 = 没取（见 [`star_history`]）。
    pub star_history: Vec<StarPoint>,
    /// 曲线为空时的原因，给使用者看。`None` = 压根没要曲线。
    ///
    /// 单独留一个字段，是因为「没有 star」和「不许看 star」画出来是同一张
    /// 空白卡片，而对使用者是两回事——后者他还有办法（换一个有权限的令牌）。
    pub star_note: Option<String>,
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
        // 曲线由 fetch_remote 单独取——它要额外十几次请求，
        // 不该藏在「解析一份 JSON」里
        star_history: Vec::new(),
        star_note: None,
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
/// 曲线上取几个点。
///
/// 每个点是一次请求，所以这个数字直接就是这个功能的 API 开销。12 个点足够
/// 让一条增长曲线看出形状——再多是把速率配额换成读者看不出来的平滑度。
const STAR_SAMPLES: usize = 12;
/// 每页固定 100，这是 GitHub 允许的最大值，也是「第 k 页第一个人 = 第
/// (k-1)*100+1 颗星」这条换算成立的前提。
const PER_PAGE: u64 = 100;
/// GitHub 的分页上限。超过这个页数的部分取不到，曲线的**早期**会缺一段——
/// 缺了就要说出来，不能把一条残缺的线画得像完整的。
const MAX_PAGE: u64 = 400;

/// 取 star 增长曲线。
///
/// **GitHub 没有「历年 star 数」这样的接口。** 但 `/stargazers` 带上
/// `star+json` 之后会按**加星时间升序**返回，每条带 `starred_at`。于是第 k 页
/// 的第一个人，就是这个仓库第 `(k-1)*100+1` 颗星落下的那一刻——这是一个
/// **精确**的数据点，不是估算。抽若干页就得到若干个精确点，点与点之间是直线
/// 插值，那一段才是近似。
///
/// 最后一个点取**最后一页的最后一个人**，不取「现在」：曲线因此完全由远端
/// 数据决定，同一份远端状态跑两次得到同一条线。用「现在」的话，同一个仓库
/// 每次跑都会画出一条略微不同的尾巴。
///
/// **2026 年 7 月起，GitHub 把 stargazer 名单限制给了仓库的 admin 与
/// collaborator。** 别人的公开仓库现在一律 404（未登录则 401）。这不是我们
/// 这边的 bug，也绕不过去——所以取不到时要把原因说出来，而不是让使用者
/// 对着一张没有曲线的卡片猜。
///
/// 对 repolish 来说这条限制不致命：它本来就是给你**自己的**仓库打分的，
/// 而你自己的仓库你是 admin。
///
/// 失败返回原因而不是错误：star 曲线是卡片上的一段装饰，它取不到不该让整个
/// `--remote` 失败——那会把「配额用完了」变成「评分失败」。
pub fn star_history(
    slug: &RepoSlug,
    token: Option<&str>,
    stars: u64,
) -> (Vec<StarPoint>, Option<String>) {
    if stars == 0 {
        return (Vec::new(), None);
    }
    let wanted = sample_pages(stars);
    let last_page = wanted.last().copied().unwrap_or(1);

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();

    let mut points: Vec<StarPoint> = Vec::new();
    let mut note: Option<String> = None;
    for page in wanted {
        let entries = match stargazer_page(&agent, slug, token, page) {
            Ok(e) => e,
            Err(why) => {
                // 中途失败就用已经拿到的点。半条曲线好过没有曲线，
                // 而失败的原因要带出去。
                note = Some(why);
                break;
            }
        };
        let Some(first) = entries.first() else {
            continue;
        };
        points.push(StarPoint {
            at: *first,
            count: (page - 1) * PER_PAGE + 1,
        });
        // 最后一页还给出曲线的终点：最后一颗星落下的时刻
        if page == last_page {
            if let Some(last) = entries.last() {
                points.push(StarPoint {
                    at: *last,
                    count: (page - 1) * PER_PAGE + entries.len() as u64,
                });
            }
        }
    }

    points.sort_by_key(|p| p.at);
    points.dedup_by_key(|p| p.at);
    // 一个点画不出曲线
    if points.len() < 2 {
        return (Vec::new(), note);
    }
    (points, note)
}

/// 要抽哪几页。
///
/// 均匀铺开，且**一定包含第一页与最后一页**——曲线的两端最说明问题：
/// 第一颗星什么时候来的，最后一颗星什么时候来的。
///
/// 单独拆出来是为了能测：真正的抓取要网络，而这段算术才是容易出错的地方
/// （少一页、重复一页、或者最后一页没进去，画出来的曲线都是错的）。
fn sample_pages(stars: u64) -> Vec<u64> {
    if stars == 0 {
        return Vec::new();
    }
    let pages = stars.div_ceil(PER_PAGE).min(MAX_PAGE);
    if pages == 1 {
        return vec![1];
    }
    let n = STAR_SAMPLES.min(pages as usize).max(2);
    let mut wanted: Vec<u64> = Vec::new();
    for i in 0..n {
        let page = 1 + (i as u64 * (pages - 1)) / (n as u64 - 1);
        if !wanted.contains(&page) {
            wanted.push(page);
        }
    }
    if !wanted.contains(&pages) {
        wanted.push(pages);
    }
    wanted
}

/// 一页 stargazer 的 `starred_at`，Unix 秒，升序。
///
/// `Err` 里是一句给人看的原因。403/404 单独说，因为那几乎总是同一件事：
/// GitHub 2026 年 7 月起把这份名单限制给了 admin 与 collaborator。
fn stargazer_page(
    agent: &ureq::Agent,
    slug: &RepoSlug,
    token: Option<&str>,
    page: u64,
) -> Result<Vec<i64>, String> {
    let url = format!(
        "{API}/repos/{}/{}/stargazers?per_page={PER_PAGE}&page={page}",
        slug.owner, slug.name
    );
    let mut req = agent
        .get(&url)
        // 这个 Accept 是关键：没有它，返回的条目里根本没有 `starred_at`
        .header("Accept", "application/vnd.github.star+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(
            "User-Agent",
            concat!("repolish/", env!("CARGO_PKG_VERSION")),
        );
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }

    let mut res = req.call().map_err(|e| e.to_string())?;
    match res.status().as_u16() {
        200 => {}
        401 | 403 | 404 => {
            return Err(format!(
                "GitHub will not list stargazers for {slug}. Since July 2026 that list is \
                 limited to a repository's admins and collaborators, so a curve is only \
                 available for repositories you have access to{}",
                if token.is_none() {
                    " — and it needs a token at all"
                } else {
                    ""
                }
            ))
        }
        429 => return Err("GitHub rate limit reached while reading stargazers".into()),
        other => return Err(format!("GitHub answered {other} for the stargazer list")),
    }

    let json: serde_json::Value = res
        .body_mut()
        .read_json()
        .map_err(|e| format!("the stargazer page was not valid JSON: {e}"))?;
    let mut out: Vec<i64> = json
        .as_array()
        .ok_or("the stargazer page was not a list")?
        .iter()
        .filter_map(|e| e.get("starred_at")?.as_str().and_then(parse_rfc3339))
        .collect();
    out.sort_unstable();
    Ok(out)
}

/// RFC 3339 → Unix 秒。
///
/// 只认 GitHub 实际吐出来的那一种形状：`2024-03-17T08:21:44Z`。不引 chrono——
/// 为一个固定格式的时间戳拉一个日期库，是这个仓库一直在拒绝的那类依赖。
fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' {
        return None;
    }
    let num = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hh, mm, ss) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d) * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// 民用日历 → 距 1970-01-01 的天数。Howard Hinnant 的 `days_from_civil`，
/// 对 1970 之前也成立。闰年规则写在算式里，没有查表。
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

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

#[cfg(test)]
mod star_tests {
    use super::*;

    #[test]
    fn no_stars_means_no_requests() {
        assert!(sample_pages(0).is_empty());
    }

    /// 一页装得下就只取一页——小仓库不该为一张图打十几次 API
    #[test]
    fn a_small_repository_costs_one_request() {
        assert_eq!(sample_pages(1), vec![1]);
        assert_eq!(sample_pages(100), vec![1]);
        assert_eq!(sample_pages(101), vec![1, 2]);
    }

    /// 两端必须在里面：第一颗星和最后一颗星什么时候来的，是曲线的全部意义
    #[test]
    fn both_ends_are_always_sampled() {
        for stars in [150u64, 999, 21_000, 250_000] {
            let pages = sample_pages(stars);
            let last = stars.div_ceil(PER_PAGE).min(MAX_PAGE);
            assert_eq!(pages.first(), Some(&1), "{stars} 没取第一页");
            assert_eq!(pages.last(), Some(&last), "{stars} 没取最后一页");
        }
    }

    #[test]
    fn sampling_is_bounded_sorted_and_free_of_duplicates() {
        for stars in [101u64, 500, 21_000, 10_000_000] {
            let pages = sample_pages(stars);
            assert!(
                pages.len() <= STAR_SAMPLES + 1,
                "{stars} 抽了 {} 页，超出预算",
                pages.len()
            );
            assert!(
                pages.windows(2).all(|w| w[0] < w[1]),
                "{stars}: {pages:?} 没有严格升序"
            );
            assert!(
                pages.iter().all(|p| *p >= 1 && *p <= MAX_PAGE),
                "{stars}: {pages:?} 越界"
            );
        }
    }

    /// GitHub 的分页有上限，超过的部分取不到——不能假装能取到
    #[test]
    fn pagination_is_capped_rather_than_requested_forever() {
        let pages = sample_pages(10_000_000);
        assert_eq!(pages.last(), Some(&MAX_PAGE));
    }

    /// 时间戳解析是自己写的，所以得真的验一遍
    #[test]
    fn github_timestamps_parse_to_unix_seconds() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2024-03-17T08:21:44Z"), Some(1_710_663_704));
        // 闰日
        assert_eq!(parse_rfc3339("2024-02-29T00:00:00Z"), Some(1_709_164_800));
        // 形状不对的一律拒绝，而不是猜
        assert_eq!(parse_rfc3339(""), None);
        assert_eq!(parse_rfc3339("2024-03-17"), None);
        assert_eq!(parse_rfc3339("not a date at all!!"), None);
        assert_eq!(parse_rfc3339("2024-13-01T00:00:00Z"), None);
    }
}
