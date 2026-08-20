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
    "require",
    "prerequisite",
    "depend",
    "version",
    "node ",
    "python ",
    "rust",
    "go ",
    "前置",
    "依赖",
    "需要",
    "要求",
    "环境",
];

/// 安装 / 依赖声明的特征。用于「标题识别不出来但正文确实有安装信息」的兜底，
/// 例如 serde 把 `serde = { version = "1.0" }` 放在「Serde in action」里。
const INSTALL_HINTS: &[&str] = &[
    "npm install",
    "npm i ",
    "yarn add",
    "pnpm add",
    "pip install",
    "uv add",
    "cargo add",
    "go get",
    "gem install",
    "composer require",
    "brew install",
    "apt install",
    "docker run",
    "docker pull",
    "curl -",
    "= { version",
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
                vec![Evidence::new(
                    ".",
                    "no README, so there is no way in at all",
                )],
                vec![Fix::new(
                    Severity::P1,
                    "Add a README with a quick start section",
                )],
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
                    format!("the \"{}\" section contains no command anyone can copy", section.title),
                )],
                vec![Fix::new(
                    Severity::P2,
                    "Put a code block in that section with an install command that can be pasted and run as-is",
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
            format!("\"{}\" has both commands and prerequisites", section.title),
        )]);
    }
    Outcome::scored(
        8,
        vec![Evidence::at(
            name,
            section.line,
            format!("\"{}\" has commands but states no prerequisites", section.title),
        )],
        vec![Fix::new(
            Severity::P3,
            "Add a line of prerequisites (language or runtime version, system dependencies). It is what stands between \"I followed the README\" and \"it works\"",
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
        return buried(name, cb.line, "the body text");
    }

    Outcome::scored(
        0,
        vec![Evidence::new(name, "no installation or getting-started instructions found")],
        vec![Fix::new(
            Severity::P1,
            "Add a quick start section with the install command and one example small enough to run immediately",
        )],
    )
}

fn buried(name: &str, line: usize, where_: &str) -> Outcome {
    Outcome::scored(
        6,
        vec![Evidence::at(
            name,
            line,
            format!("no dedicated install or quick start section; the instructions are buried in \"{where_}\""),
        )],
        vec![Fix::new(
            Severity::P2,
            "Pull the instructions into their own quick start section. Readers looking for a way in scan the headings; they do not read the whole page",
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
