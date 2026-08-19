//! 区块分类：把各种写法的标题归一到语义类型。
//!
//! 匹配顺序即优先级——`Quick Start` 必须在 `Start`/`Install` 之前命中。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionKind {
    Quickstart,
    Install,
    Usage,
    Api,
    Config,
    Contributing,
    License,
    Faq,
    Changelog,
    Other,
}

impl SectionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SectionKind::Quickstart => "quickstart",
            SectionKind::Install => "install",
            SectionKind::Usage => "usage",
            SectionKind::Api => "api",
            SectionKind::Config => "config",
            SectionKind::Contributing => "contributing",
            SectionKind::License => "license",
            SectionKind::Faq => "faq",
            SectionKind::Changelog => "changelog",
            SectionKind::Other => "other",
        }
    }
}

/// 别名表。顺序敏感：先匹配到的胜出。
const TABLE: &[(SectionKind, &[&str])] = &[
    (
        SectionKind::Quickstart,
        &[
            "quick start", "quickstart", "quick-start",
            "getting started", "get started", "getting-started",
            "快速开始", "快速上手", "快速入门", "起步", "上手",
        ],
    ),
    (
        SectionKind::Install,
        &[
            "installation", "install", "setup", "set up", "prerequisite",
            "requirement", "安装", "部署", "环境准备", "依赖",
        ],
    ),
    (
        SectionKind::Usage,
        &[
            "usage", "how to use", "example", "demo", "recipe", "guide",
            "用法", "使用", "示例", "例子", "教程",
        ],
    ),
    (
        SectionKind::Api,
        &["api", "reference", "接口", "参考"],
    ),
    (
        SectionKind::Config,
        &["configuration", "config", "options", "配置", "选项", "参数"],
    ),
    (
        SectionKind::Contributing,
        &["contributing", "contribute", "development", "贡献", "开发"],
    ),
    (
        SectionKind::License,
        &["license", "licence", "许可", "协议", "开源协议"],
    ),
    (
        SectionKind::Faq,
        &["faq", "q&a", "troubleshoot", "常见问题", "问答"],
    ),
    (
        SectionKind::Changelog,
        &["changelog", "release note", "history", "更新日志", "变更"],
    ),
];

pub fn classify(title: &str) -> SectionKind {
    let t = normalize(title);
    for (kind, aliases) in TABLE {
        if aliases.iter().any(|a| t.contains(a)) {
            return *kind;
        }
    }
    SectionKind::Other
}

/// 去掉 emoji、编号、装饰符号，转小写。
fn normalize(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '&' || *c == '-')
        .collect::<String>()
        .to_lowercase()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quickstart_wins_over_install() {
        assert_eq!(classify("Quick Start"), SectionKind::Quickstart);
        assert_eq!(classify("🚀 Getting Started"), SectionKind::Quickstart);
        assert_eq!(classify("快速开始"), SectionKind::Quickstart);
        assert_eq!(classify("## 1. Installation"), SectionKind::Install);
    }

    #[test]
    fn falls_back_to_other() {
        assert_eq!(classify("Acknowledgements"), SectionKind::Other);
        assert_eq!(classify(""), SectionKind::Other);
    }

    #[test]
    fn strips_decoration() {
        assert_eq!(classify("📦 安装"), SectionKind::Install);
        assert_eq!(classify("**Usage**"), SectionKind::Usage);
    }
}
