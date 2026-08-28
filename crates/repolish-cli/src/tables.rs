//! README 里的表格 → SVG 文件。
//!
//! 抽出来是因为有**两个**调用方，而它们对同一批文件的态度正好相反：
//!
//! - `polish` 负责把 `<img>` 和 `<details>` 插进 README，并第一次写出这些
//!   SVG。它从不覆盖任何东西——那是它最硬的一条不变量。
//! - `card` 负责**重画**。语言构成会随着仓库变，表格内容会随着作者改 README
//!   变；不能重画的话，README 上迟早挂着一张过期的图。
//!
//! 两边共用这里的选表规则与命名规则，否则 `card` 重画出来的文件名会和
//! `polish` 当初插进 README 的那个对不上——那比不重画更糟。

use std::path::PathBuf;

use repolish_core::RepoContext;
use repolish_md::Readme;

/// 生成的表格图放在哪
pub const TABLES_DIR: &str = ".repolish/tables";

/// 表格大到这个行数就不画了。一张十几行的图在手机上要缩到看不清字，
/// 而原表格在 GitHub 上本来就会自己滚动。
pub const MAX_ROWS: usize = 16;
/// 少于这么多行的表画成图没有增益，只是多一次网络请求
pub const MIN_ROWS: usize = 2;

/// 一张待写出的表格图。
pub struct Rendered {
    /// 仓库相对路径，`/` 分隔，可直接写进 README
    pub rel: String,
    pub svg: String,
    /// 图上的小标题，同时也是 `<img alt>` 与 `<summary>` 的来源
    pub title: Option<String>,
    /// 表格在 README 里占的行区间，1-based，含头含尾
    pub start_line: usize,
    pub end_line: usize,
}

impl Rendered {
    pub fn path(&self, root: &std::path::Path) -> PathBuf {
        root.join(&self.rel)
    }
}

/// 主 README 的译本。
///
/// **必须和主 README 同一个目录。** 否则 `docs/README.zh-CN.md` 会被当成译本，
/// 而它是文档索引的中文版，是另一份文档，不是这一份的翻译。判据用目录而不是
/// 「在不在根目录」，是为了让 README 本来就在子目录里的仓库也成立。
pub fn translations(ctx: &RepoContext, main: &Readme) -> Vec<String> {
    let norm = |p: &str| p.replace(std::path::MAIN_SEPARATOR, "/");
    let main_path = norm(&main.path.display().to_string());
    let dir = |p: &str| {
        p.rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default()
    };
    // main.path 可能是绝对路径，files 里是仓库相对路径，比目录名即可
    let main_dir = main_path
        .strip_prefix(&norm(&ctx.root.display().to_string()))
        .map(|p| dir(p.trim_start_matches('/')))
        .unwrap_or_else(|| dir(&main_path));

    ctx.files
        .iter()
        .filter(|p| !main_path.ends_with(*p))
        .filter(|p| dir(p) == main_dir)
        .filter(|p| repolish_md::translation_code(p).is_some())
        .map(str::to_string)
        .collect()
}

/// 挑出值得画的表并画出来。
///
/// `warn` 收到被跳过的表的说明。调用方决定要不要打印——`card` 该说，
/// 而 `polish` 的干跑输出里再多一句会把真正的改动淹掉。
/// 一份 README 的表格图放在哪。
///
/// 主 README 用 `.repolish/tables/`，译本各用一个语言子目录。**必须分开**：
/// slug 是从小节标题来的，而非 ASCII 一律丢掉——中文的「退出码」和
/// 「检查什么」都会 slug 成 `table`，挤在同一个目录里就互相覆盖了。
pub fn dir_for(readme: &Readme) -> String {
    let path = readme.path.display().to_string().replace('\\', "/");
    match repolish_md::translation_code(&path) {
        Some(code) => format!("{TABLES_DIR}/{code}"),
        None => TABLES_DIR.to_string(),
    }
}

pub fn render(
    readme: &Readme,
    opts: &repolish_render::Options,
    mut warn: impl FnMut(String),
) -> Vec<Rendered> {
    let dir = dir_for(readme);
    let mut out = Vec::new();
    let mut used: Vec<String> = Vec::new();
    for found in repolish_md::tables::find(&readme.raw).iter() {
        if found.rows.len() < MIN_ROWS || found.headers.len() < 2 {
            continue;
        }
        if found.rows.len() > MAX_ROWS {
            warn(format!(
                "the table at {}:{} has {} rows — left as a table, an image that tall is \
                 unreadable on a phone",
                readme.path.display(),
                found.start_line,
                found.rows.len()
            ));
            continue;
        }

        let title = section_title(readme, found.start_line);
        // 文件名只由**标题**决定，不带文档里的序号。
        //
        // 早先用的是下标（`01-exit-codes.svg`）。那是错的：在 README 前面插一张
        // 新表，后面每一张的文件名都跟着往后挪一位，而 README 里已经写好的
        // `<img src=…>` 还指着旧名字——图不会报错，只会永远停在旧内容上，
        // 而重新生成的那几个文件没有任何东西引用。这个坑在这个仓库自己身上
        // 踩过一次：加了一节「一行装好」，三张表全部错位。
        let base = slugify(title.as_deref().unwrap_or(&found.headers.join("-")));
        // 同名小节下有两张表时才需要区分，此时按它们各自的行号排，
        // 而不是按全文下标——同样是为了不被别处的改动带偏。
        let name = match used.iter().filter(|n| *n == &base).count() {
            0 => format!("{base}.svg"),
            n => format!("{base}-{}.svg", n + 1),
        };
        used.push(base);

        let mut table =
            repolish_render::table::Table::new(found.headers.clone(), found.rows.clone());
        table.align = found.align.iter().copied().map(map_align).collect();
        table.title = title.clone();

        out.push(Rendered {
            rel: format!("{dir}/{name}"),
            svg: repolish_render::table(&table, opts),
            title,
            start_line: found.start_line,
            end_line: found.end_line,
        });
    }
    out
}

/// 表格所在小节的标题，用作图上的小标题与文件名
fn section_title(readme: &Readme, line: usize) -> Option<String> {
    readme
        .sections
        .iter()
        .filter(|s| s.line < line)
        .max_by_key(|s| s.line)
        .map(|s| s.title.clone())
}

fn map_align(a: repolish_md::tables::Align) -> repolish_render::table::Align {
    match a {
        repolish_md::tables::Align::Left => repolish_render::table::Align::Left,
        repolish_md::tables::Align::Center => repolish_render::table::Align::Center,
        repolish_md::tables::Align::Right => repolish_render::table::Align::Right,
    }
}

/// 文件名用的 slug。非 ASCII 一律丢掉——生成的路径要写进 README，
/// 而一条带中文的相对路径在某些 CI 与 Windows 检出下会变成乱码。
/// 丢空了就退回一个通用名字，而不是产出一个没有名字的文件。
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(40).collect();
    let out = out.trim_end_matches('-');
    if !out.is_empty() {
        return out.to_string();
    }
    // 非 ASCII 标题（中文小节名）会被丢空。退回一个由**标题内容**决定的短哈希，
    // 而不是「第几张表」——序号是位置的函数，在前面插一张新表就会全体错位，
    // 而 README 里已经写好的引用只认名字。
    format!("t-{:06x}", short_hash(s))
}

/// FNV-1a，取 24 位。这里只需要「同样的标题永远得到同样的名字」，
/// 不需要抗碰撞——真撞上了，下面的去重会补一个 `-2`。
fn short_hash(s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h & 0xff_ffff
}

/// 这张表上面是不是已经有一张我们插过的图了。
///
/// 只认真正会渲染出图的 `src="…"`，不认单纯提到这个目录的散文或代码块——
/// 一份介绍这个功能的 README 会在正文里写出这个路径，那不是一张图。
pub fn already_wrapped(readme: &Readme, start_line: usize) -> bool {
    let dir = dir_for(readme);
    let before: Vec<&str> = readme
        .raw
        .lines()
        .take(start_line.saturating_sub(1))
        .collect();
    before
        .iter()
        .rev()
        .take(6)
        .any(|l| l.contains(&format!("src=\"{dir}/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn readme(md: &str) -> Readme {
        Readme::parse("README.md", md)
    }

    const MD: &str = "# T\n\n## Exit codes\n\n| Code | Meaning |\n|---|---|\n| 0 | ok |\n| 1 | no |\n\n## Tiny\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";

    /// 中文小节标题 slug 之后全都变成 `table`，主目录里会互相覆盖。
    /// 译本必须有自己的目录。
    #[test]
    fn each_translation_gets_its_own_directory() {
        assert_eq!(dir_for(&Readme::parse("README.md", "")), ".repolish/tables");
        assert_eq!(
            dir_for(&Readme::parse("README.zh-CN.md", "")),
            ".repolish/tables/zh-cn"
        );
        assert_eq!(
            dir_for(&Readme::parse("docs/README-ja.md", "")),
            ".repolish/tables/ja"
        );
    }

    #[test]
    fn a_translation_renders_into_its_own_directory() {
        let md = "## 退出码\n\n| 码 | 含义 |\n|---|---|\n| 0 | 成功 |\n| 1 | 失败 |\n";
        let r = render(
            &Readme::parse("README.zh-CN.md", md),
            &Default::default(),
            |_| {},
        );
        assert_eq!(r.len(), 1);
        assert!(
            r[0].rel.starts_with(".repolish/tables/zh-cn/"),
            "译本没进自己的目录: {}",
            r[0].rel
        );
    }

    #[test]
    fn only_tables_worth_drawing_are_rendered() {
        let r = render(&readme(MD), &Default::default(), |_| {});
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title.as_deref(), Some("Exit codes"));
        assert_eq!(r[0].rel, ".repolish/tables/exit-codes.svg");
        assert_eq!((r[0].start_line, r[0].end_line), (5, 8));
    }

    /// 文件名只由标题决定。在**前面**插一张新表，后面每一张的名字都不能变——
    /// 变了的话 README 里已经写好的 `<img src=…>` 就全指向了没人再生成的文件。
    /// 这个仓库自己踩过：加了一节「一行装好」，三张表的文件名全部错位。
    #[test]
    fn adding_a_table_earlier_does_not_rename_the_ones_after_it() {
        let exit_codes = |v: &[Rendered]| {
            v.iter()
                .find(|r| r.title.as_deref() == Some("Exit codes"))
                .map(|r| r.rel.clone())
                .expect("Exit codes 那张表应该在")
        };
        let before = render(&readme(MD), &Default::default(), |_| {});
        let with_new = format!("## New\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\n{MD}");
        let after = render(&readme(&with_new), &Default::default(), |_| {});

        assert_eq!(after.len(), 2, "新表没被认出来");
        assert_eq!(exit_codes(&before), exit_codes(&after));
    }

    /// 被跳过的表同样不该影响别人的名字
    #[test]
    fn skipping_a_table_does_not_rename_the_ones_after_it() {
        let md = "## Tiny\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n## Real\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
        let r = render(&readme(md), &Default::default(), |_| {});
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].rel, ".repolish/tables/real.svg");
    }

    /// 同一节下的两张表才需要区分，而且要区分得稳定
    #[test]
    fn two_tables_in_one_section_get_distinct_stable_names() {
        let md = "## Same\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\ntext\n\n| c | d |\n|---|---|\n| 5 | 6 |\n| 7 | 8 |\n";
        let r = render(&readme(md), &Default::default(), |_| {});
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].rel, ".repolish/tables/same.svg");
        assert_eq!(r[1].rel, ".repolish/tables/same-2.svg");
    }

    #[test]
    fn an_oversized_table_is_reported_rather_than_silently_dropped() {
        let mut md = String::from("## Big\n\n| a | b |\n|---|---|\n");
        for i in 0..MAX_ROWS + 1 {
            md.push_str(&format!("| {i} | x |\n"));
        }
        let mut warnings = Vec::new();
        let r = render(&readme(&md), &Default::default(), |w| warnings.push(w));
        assert!(r.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("rows"));
    }

    #[test]
    fn slugs_are_ascii_safe_and_never_empty() {
        assert_eq!(slugify("Exit codes"), "exit-codes");
        assert_eq!(slugify("What it checks!"), "what-it-checks");
        // 非 ASCII 全丢掉（带中文的相对路径在某些 CI 与 Windows 检出下会乱码），
        // 但名字仍由**标题内容**决定，不由位置决定
        let a = slugify("评分维度");
        let b = slugify("退出码");
        assert!(a.starts_with("t-") && b.starts_with("t-"), "{a} {b}");
        assert_ne!(a, b, "不同标题得到了同一个名字");
        assert_eq!(a, slugify("评分维度"), "同一个标题两次结果不同");
        assert!(slugify("").starts_with("t-"));
        let long = slugify(&"ab ".repeat(40));
        assert!(long.len() <= 40 && !long.ends_with('-'), "{long}");
    }

    /// 一份介绍这个功能的 README 会在正文里写出这个路径。
    /// 按「出现过没有」判断的话，那一段会让包装永远不发生。
    #[test]
    fn merely_mentioning_the_directory_in_prose_is_not_a_wrap() {
        let md = format!(
            "## X\n\nRedraw them with `{TABLES_DIR}` after editing:\n\n```bash\nls {TABLES_DIR}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n"
        );
        let r = readme(&md);
        let found = &repolish_md::tables::find(&r.raw)[0];
        assert!(!already_wrapped(&r, found.start_line));
    }

    #[test]
    fn a_table_that_is_already_wrapped_is_recognised() {
        let md = format!(
            "## X\n\n<img src=\"{TABLES_DIR}/01-x.svg\" alt=\"x\">\n\n<details>\n<summary>X</summary>\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\n</details>\n"
        );
        let r = readme(&md);
        let found = &repolish_md::tables::find(&r.raw)[0];
        assert!(already_wrapped(&r, found.start_line));
        assert!(!already_wrapped(&readme(MD), 5));
    }
}
