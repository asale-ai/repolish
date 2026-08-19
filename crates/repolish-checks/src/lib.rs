//! M1：8 个纯本地检查项。
//!
//! 全部无需网络，确保 `repolish check .` 零配置可跑。
//! 远程三项（description / topics / homepage）在 M2 加入。

use repolish_core::{Check, Registry};

mod activity;
mod ci;
mod license;
mod readme_links;
mod readme_quickstart;
mod readme_title;
mod readme_usage;
mod tests;

/// M1 注册表。顺序即报告中的展示顺序。
pub fn registry() -> Registry {
    let mut r = Registry::new();
    for c in all() {
        r.register(c);
    }
    r
}

fn all() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(readme_title::ReadmeTitleTagline),
        Box::new(readme_quickstart::ReadmeQuickstart),
        Box::new(readme_usage::ReadmeUsageExample),
        Box::new(readme_links::ReadmeLinkHealth),
        Box::new(license::License),
        Box::new(ci::CiPresent),
        Box::new(tests::TestsPresent),
        Box::new(activity::Activity),
    ]
}
