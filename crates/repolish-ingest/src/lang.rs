//! 语言构成：按扩展名把仓库的文件分门别类。
//!
//! 这是**概览卡片**的数据来源，不进任何一个分数。分数只认检查项，
//! 「这个仓库 74% 是 Rust」不说明它的 README 写得好不好。
//!
//! 只按扩展名数**文件**，不数字节、不做内容嗅探：字节数会被一个压进仓库的
//! 词表或者 lockfile 彻底带偏，而内容嗅探要读全部文件，概览卡片不值这个开销。
//! 卡片上写的是「by file」，说的就是文件数。

use crate::files::{is_content_path, FileIndex};

/// 一门语言（或一类文件）在仓库里占了多少个文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangStat {
    pub name: &'static str,
    pub kind: Kind,
    pub files: usize,
}

/// 文件的用途分类。卡片上的图例就是这四档——
/// 「这个仓库 60% 是代码」和「60% 是配置」是两件完全不同的事。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Code,
    Docs,
    Config,
    Other,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Code => "code",
            Kind::Docs => "docs",
            Kind::Config => "config",
            Kind::Other => "other",
        }
    }
}

/// 扩展名 → 展示名与分类。
///
/// 这张表是有意不全的：认不出来的扩展名一律归到 `Other`，而不是拿扩展名
/// 本身当语言名往卡片上贴。一张写着「XYZ 3 files」的卡片，读者只会觉得
/// 这个工具在瞎猜。
#[rustfmt::skip]
const TABLE: &[(&str, &str, Kind)] = &[
    ("rs",     "Rust",       Kind::Code),
    ("go",     "Go",         Kind::Code),
    ("py",     "Python",     Kind::Code),
    ("ts",     "TypeScript", Kind::Code),
    ("tsx",    "TypeScript", Kind::Code),
    ("js",     "JavaScript", Kind::Code),
    ("jsx",    "JavaScript", Kind::Code),
    ("mjs",    "JavaScript", Kind::Code),
    ("cjs",    "JavaScript", Kind::Code),
    ("vue",    "Vue",        Kind::Code),
    ("svelte", "Svelte",     Kind::Code),
    ("astro",  "Astro",      Kind::Code),
    ("java",   "Java",       Kind::Code),
    ("kt",     "Kotlin",     Kind::Code),
    ("kts",    "Kotlin",     Kind::Code),
    ("swift",  "Swift",      Kind::Code),
    ("c",      "C",          Kind::Code),
    ("h",      "C",          Kind::Code),
    ("cpp",    "C++",        Kind::Code),
    ("cc",     "C++",        Kind::Code),
    ("hpp",    "C++",        Kind::Code),
    ("cs",     "C#",         Kind::Code),
    ("rb",     "Ruby",       Kind::Code),
    ("php",    "PHP",        Kind::Code),
    ("scala",  "Scala",      Kind::Code),
    ("ex",     "Elixir",     Kind::Code),
    ("exs",    "Elixir",     Kind::Code),
    ("dart",   "Dart",       Kind::Code),
    ("lua",    "Lua",        Kind::Code),
    ("zig",    "Zig",        Kind::Code),
    ("sh",     "Shell",      Kind::Code),
    ("bash",   "Shell",      Kind::Code),
    ("zsh",    "Shell",      Kind::Code),
    ("ps1",    "PowerShell", Kind::Code),
    ("sql",    "SQL",        Kind::Code),
    ("css",    "CSS",        Kind::Code),
    ("scss",   "CSS",        Kind::Code),
    ("html",   "HTML",       Kind::Code),
    ("md",     "Markdown",   Kind::Docs),
    ("mdx",    "Markdown",   Kind::Docs),
    ("rst",    "reST",       Kind::Docs),
    ("txt",    "Text",       Kind::Docs),
    ("adoc",   "AsciiDoc",   Kind::Docs),
    ("toml",   "TOML",       Kind::Config),
    ("yml",    "YAML",       Kind::Config),
    ("yaml",   "YAML",       Kind::Config),
    ("json",   "JSON",       Kind::Config),
    ("ini",    "INI",        Kind::Config),
    ("cfg",    "INI",        Kind::Config),
    ("svg",    "SVG",        Kind::Other),
    ("png",    "Images",     Kind::Other),
    ("jpg",    "Images",     Kind::Other),
    ("jpeg",   "Images",     Kind::Other),
    ("gif",    "Images",     Kind::Other),
    ("webp",   "Images",     Kind::Other),
];

/// 仓库的语言构成，按文件数降序。
///
/// 只统计「项目内容」：`.github/`、`.vscode/` 这类工具元数据目录被排除在外，
/// 与 profile 探测用的是同一条界线。一个仓库的语言构成不该由它装了哪些
/// 编辑器插件来决定。
pub fn stats(files: &FileIndex) -> Vec<LangStat> {
    let mut tally: Vec<LangStat> = Vec::new();
    let mut other = 0usize;

    for path in files.iter() {
        if !is_content_path(path) {
            continue;
        }
        let ext = path.rsplit_once('.').map(|(_, e)| e.to_lowercase());
        let entry = ext
            .as_deref()
            .and_then(|e| TABLE.iter().find(|(k, _, _)| *k == e));
        match entry {
            Some((_, name, kind)) => match tally.iter_mut().find(|s| s.name == *name) {
                Some(s) => s.files += 1,
                None => tally.push(LangStat {
                    name,
                    kind: *kind,
                    files: 1,
                }),
            },
            None => other += 1,
        }
    }

    // 名次相同时按名字排，结果才对同一个 commit 稳定——
    // 卡片是要提交进仓库的，排序抖一下就是一次无意义的 diff
    tally.sort_by(|a, b| b.files.cmp(&a.files).then(a.name.cmp(b.name)));
    if other > 0 {
        tally.push(LangStat {
            name: "Other",
            kind: Kind::Other,
            files: other,
        });
    }
    tally
}

/// 按用途归并。图例上的四档就是它。
pub fn by_kind(stats: &[LangStat]) -> Vec<(Kind, usize)> {
    let mut out: Vec<(Kind, usize)> = Vec::new();
    for s in stats {
        match out.iter_mut().find(|(k, _)| *k == s.kind) {
            Some(e) => e.1 += s.files,
            None => out.push((s.kind, s.files)),
        }
    }
    out.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    out
}

/// 主语言。全是 `Other` 时返回 `None`——「主语言：Other」不是一个答案。
pub fn primary(stats: &[LangStat]) -> Option<&LangStat> {
    stats.iter().find(|s| s.kind == Kind::Code)
}
