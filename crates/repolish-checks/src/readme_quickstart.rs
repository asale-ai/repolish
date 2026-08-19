use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};
use repolish_md::{Readme, Section, SectionKind};

/// 是否存在能让人跑起来的安装 / 快速开始区块。
///
/// 分档：
/// - 0  完全找不到上手信息
/// - 4  有区块但没有可复制的命令
/// - 6  没有专门的安装区块，但别处能找到安装命令
/// - 8  有命令，未说明前置条件
/// - 10 命令 + 前置条件说明
pub struct ReadmeQuickstart;

const PREREQ_HINTS: &[&str] = &[
    "require", "prerequisite", "depend", "version", "node ", "python ", "rust", "go ",
    "前置", "依赖", "需要", "要求", "环境",
];

/// 安装 / 依赖声明的特征。用于「标题识别不出来但正文确实有安装信息」的兜底，
/// 例如 serde 把 `serde = { version = "1.0" }` 放在「Serde in action」里。
const INSTALL_HINTS: &[&str] = &[
    "npm install", "npm i ", "yarn add", "pnpm add", "pip install", "uv add",
    "cargo add", "go get", "gem install", "composer require", "brew install",
    "apt install", "docker run", "docker pull", "curl -", "= { version",
];

impl Check for ReadmeQuickstart {
    fn id(&self) -> &'static str {
        "readme-quickstart"
    }
    fn category(&self) -> Category {
        Category::Comprehensibility
    }
    fn risk(&self) -> Risk {
        Risk::Critical
    }

    /// 资源集合（awesome-list 类）没有可安装的东西
    fn applies_to(&self, profile: Profile) -> bool {
        profile != Profile::Collection
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "没有 README，无从判断如何上手")],
                vec![Fix::new(Severity::P1, "添加 README 并写「快速开始」区块")],
            );
        };
        let name = file_name(readme);

        // 取所有候选区块中最好的那个。只取第一个会出错：koa 的
        // 「Getting started」是空壳，真正有命令的是靠前的「Installation」。
        let best = readme
            .sections
            .iter()
            .filter(|s| matches!(s.kind, SectionKind::Quickstart | SectionKind::Install))
            .map(|s| (rank(readme, s), s))
            .max_by_key(|(r, _)| *r);

        match best {
            Some((rank, section)) if rank > 0 => score_explicit(readme, section, &name, rank),
            Some((_, section)) => Outcome::scored(
                4,
                vec![Evidence::at(
                    &name,
                    section.line,
                    format!("「{}」区块里没有可复制的命令", section.title),
                )],
                vec![Fix::new(
                    Severity::P2,
                    "在该区块加一个代码块，给出可直接粘贴执行的安装命令",
                )],
            ),
            None => score_fallback(readme, &name),
        }
    }
}

/// 0 = 无命令，1 = 有命令，2 = 有命令且说明了前置条件
fn rank(readme: &Readme, section: &Section) -> u8 {
    if readme.code_blocks_in(section).next().is_none() {
        return 0;
    }
    let body = section.body.to_lowercase();
    if PREREQ_HINTS.iter().any(|h| body.contains(h)) {
        2
    } else {
        1
    }
}

fn score_explicit(_readme: &Readme, section: &Section, name: &str, rank: u8) -> Outcome {
    if rank >= 2 {
        return Outcome::perfect(vec![Evidence::at(
            name,
            section.line,
            format!("「{}」含命令与前置条件说明", section.title),
        )]);
    }
    Outcome::scored(
        8,
        vec![Evidence::at(
            name,
            section.line,
            format!("「{}」有命令，但未说明前置条件", section.title),
        )],
        vec![Fix::new(
            Severity::P3,
            "补一行前置条件（语言/运行时版本、系统依赖），减少「照做却跑不起来」",
        )],
    )
}

/// 没有任何安装 / 快速开始区块时的退路。
///
/// tokio、serde 这类项目把安装信息塞在示例或正文里，判 0 分（等同于连
/// README 结构都没有）过重，但确实不如显式区块好找。
fn score_fallback(readme: &Readme, name: &str) -> Outcome {
    let usage = readme.section(SectionKind::Usage);
    if let Some(section) = usage {
        if readme.code_blocks_in(section).next().is_some() {
            return buried(name, section.line, &section.title);
        }
    }

    // 正文里任何一处像安装命令的代码块
    let found = readme.code_blocks.iter().find(|cb| {
        let lit = cb.literal.to_lowercase();
        INSTALL_HINTS.iter().any(|h| lit.contains(h))
    });
    if let Some(cb) = found {
        return buried(name, cb.line, "正文");
    }

    Outcome::scored(
        0,
        vec![Evidence::new(name, "未找到任何安装或上手说明")],
        vec![Fix::new(
            Severity::P1,
            "加一个「快速开始」区块，写清安装命令和最小可运行示例",
        )],
    )
}

fn buried(name: &str, line: usize, where_: &str) -> Outcome {
    Outcome::scored(
        6,
        vec![Evidence::at(
            name,
            line,
            format!("没有独立的安装 / 快速开始区块，上手信息埋在「{where_}」里"),
        )],
        vec![Fix::new(
            Severity::P2,
            "拆出独立的「快速开始」区块。读者找上手方式时是扫标题，不是通读全文",
        )],
    )
}

fn file_name(readme: &Readme) -> String {
    readme
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
