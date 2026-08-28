//! v1 的 22 个检查项。
//!
//! 清单与权重冻结于 docs/03-评分维度.md：**增删检查项会改变分数口径**，
//! 必须走 minor 版本并同步 `schemaVersion` 的说明。
//!
//! 其中 3 项需要 `--remote`（GitHub API），无 `--remote` 时会被标为 `Skipped`
//! 并进入报告的「覆盖限制」——本地分与远程分不可横向比较。

use repolish_core::{Check, Registry};

mod activity;
mod ci;
mod claim_consistency;
mod code_of_conduct;
mod contributing;
mod docs_presence;
mod issue_pr_template;
mod license;
mod readme_badges;
mod readme_i18n;
mod readme_install_consistency;
mod readme_length;
mod readme_links;
mod readme_quickstart;
mod readme_title;
mod readme_toc;
mod readme_usage;
mod release_hygiene;
mod repo_description;
mod repo_homepage;
mod repo_topics;
mod tests;
mod util;

/// 注册表。顺序即报告中的展示顺序：按三大类分组，类内按权重从高到低。
pub fn registry() -> Registry {
    let mut r = Registry::new();
    for c in all() {
        r.register(c);
    }
    r
}

fn all() -> Vec<Box<dyn Check>> {
    vec![
        // 一、可发现性
        Box::new(readme_title::ReadmeTitleTagline),
        Box::new(repo_description::RepoDescription),
        Box::new(repo_topics::RepoTopics),
        Box::new(repo_homepage::RepoHomepage),
        Box::new(readme_badges::ReadmeBadges),
        // 二、可理解性
        Box::new(readme_quickstart::ReadmeQuickstart),
        Box::new(readme_usage::ReadmeUsageExample),
        Box::new(readme_install_consistency::ReadmeInstallConsistency),
        Box::new(readme_links::ReadmeLinkHealth),
        Box::new(readme_length::ReadmeLength),
        Box::new(docs_presence::DocsPresence),
        Box::new(readme_toc::ReadmeToc),
        Box::new(readme_i18n::ReadmeI18n),
        // 三、可信度
        Box::new(license::License),
        Box::new(claim_consistency::ClaimConsistency),
        Box::new(ci::CiPresent),
        Box::new(tests::TestsPresent),
        Box::new(activity::Activity),
        Box::new(contributing::Contributing),
        Box::new(issue_pr_template::IssuePrTemplate),
        Box::new(release_hygiene::ReleaseHygiene),
        Box::new(code_of_conduct::CodeOfConduct),
    ]
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use std::collections::HashSet;

    /// v1 冻结在 22 项。改这个数字意味着分数口径变了，必须同步 docs/03 与版本号。
    #[test]
    fn v1_check_set_is_frozen_at_22() {
        assert_eq!(all().len(), 22);
    }

    #[test]
    fn ids_are_unique() {
        let checks = all();
        let ids: HashSet<&str> = checks.iter().map(|c| c.id()).collect();
        assert_eq!(ids.len(), checks.len(), "存在重复的检查项 id");
    }

    /// 资料仓库上保留哪几项，是一个显式决定，不能因为新增检查项而漂移。
    ///
    /// `Check::applies_to` 的默认值是「对 meta 不适用」，所以新加的检查项
    /// 默认不会对组织名片开火——但反过来，谁要是不小心覆盖成了 `true`，
    /// 这条断言会立刻拦住。
    #[test]
    fn meta_repositories_keep_exactly_the_readme_readability_checks() {
        use repolish_core::Profile;
        let checks = all();
        let kept: Vec<&str> = checks
            .iter()
            .filter(|c| c.applies_to(Profile::Meta))
            .map(|c| c.id())
            .collect();
        assert_eq!(
            kept,
            vec![
                "readme-title-tagline",
                "readme-link-health",
                "readme-length"
            ]
        );
    }

    /// 无 `--remote` 时被剔出分母的权重不能过半，否则本地模式永远出不了总分
    #[test]
    fn local_mode_keeps_enough_weight_to_score() {
        let checks = all();
        let total: f64 = checks.iter().map(|c| c.risk().weight()).sum();
        let remote: f64 = checks
            .iter()
            .filter(|c| c.requires_remote())
            .map(|c| c.risk().weight())
            .sum();
        assert!(
            (total - remote) / total >= repolish_core::MIN_COVERAGE,
            "本地模式覆盖率 {:.2} 低于阈值",
            (total - remote) / total
        );
    }
}
