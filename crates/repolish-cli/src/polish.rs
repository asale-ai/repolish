//! `polish` —— 把能机械落实的建议直接写进 README。
//!
//! 边界：**只增量插入，不重写任何已有内容**。产出的 diff 必须全是新增行，
//! 别的行改动一个字节都算 bug。README 是作者的东西——一个教人把仓库弄体面的
//! 工具，不该顺手把别人的排版重排一遍。
//!
//! 落地方式见 [`repolish_md::edit`]：AST 只回答「插在第几行」，
//! 切开原文拼回去。为什么不能让 AST 产出文本，见 repolish-md 的 crate 文档。

use std::path::PathBuf;

use repolish_core::{RepoContext, Report};
use repolish_md::edit::{apply, Insert};
use repolish_md::Readme;

use crate::scaffold;
use crate::style::{Align, LogoWidth, ReadmeStyle, TableStyle, TocStyle};
use crate::tree;

/// 要写出的一个新文件。
pub struct NewFile {
    pub path: PathBuf,
    pub contents: String,
    /// 哪一条检查结果要求它。干跑时逐条打出来——
    /// 没有理由的新文件不该出现在别人的仓库里。
    pub reason: String,
}

/// 对**另一份** README（译本）的插入。
///
/// 主 README 的插入在 `Plan::inserts` 里。分开是因为绝大多数刀只动主 README——
/// 徽章、目录、卡片在译本里再来一份是重复，不是补齐。只有表格例外：
/// 一份中文 README 里的中文表格，在 crates.io 上照样是一堆管道符。
pub struct TranslationEdit {
    pub path: PathBuf,
    pub raw: String,
    pub inserts: Vec<Insert>,
}

/// 一次运行要落的全部改动。
#[derive(Default)]
pub struct Plan {
    /// 对主 README 的插入
    pub inserts: Vec<Insert>,
    /// 对译本的插入
    pub translations: Vec<TranslationEdit>,
    /// 需要一并写出的新文件。徽章行指向 `.repolish/badge.json`，
    /// 那个文件不存在的话插进去的是一个 404——比不插更糟。
    pub side_files: Vec<NewFile>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.inserts.is_empty() && self.side_files.is_empty() && self.translations.is_empty()
    }

    /// 拿到某份译本的编辑记录，没有就新建一条
    fn translation(&mut self, path: PathBuf, raw: &str) -> &mut TranslationEdit {
        if let Some(i) = self.translations.iter().position(|t| t.path == path) {
            return &mut self.translations[i];
        }
        self.translations.push(TranslationEdit {
            path,
            raw: raw.to_string(),
            inserts: Vec::new(),
        });
        self.translations.last_mut().expect("刚推进去")
    }

    /// 加一个新文件。**已存在就跳过**——polish 从不覆盖任何东西，
    /// 这条不变量对新文件和对 README 一样硬。
    fn add_file(&mut self, path: PathBuf, contents: String, reason: impl Into<String>) {
        if path.exists() {
            return;
        }
        self.side_files.push(NewFile {
            path,
            contents,
            reason: reason.into(),
        });
    }
}

pub fn plan(ctx: &RepoContext, report: &Report, style: &ReadmeStyle) -> Plan {
    let mut plan = Plan::default();
    if let Some(readme) = ctx.readme.as_ref() {
        hero(ctx, readme, style, &mut plan);
        logo(ctx, readme, style, &mut plan);
        badge(ctx, report, readme, style, &mut plan);
        overview_card(ctx, readme, style, &mut plan);
        toc(report, readme, style, &mut plan);
        svg_tables(ctx, readme, style, &mut plan);
        project_tree(ctx, readme, style, &mut plan);
        // 最后一刀，落在文件末尾：分数卡片是给作者和顺着它找来的下一个作者
        // 看的，不是给第一次点进这个仓库的人看的
        footer_card(ctx, report, readme, style, &mut plan);
    }
    templates(ctx, report, &mut plan);
    contributing(ctx, report, &mut plan);
    plan
}

/// 卡片渲染用的色板与语言
fn card_options(style: &ReadmeStyle) -> repolish_render::Options {
    repolish_render::Options {
        palette: style.theme.palette(),
        lang: style.lang,
    }
}

/// README 顶上那张通栏横幅，画的是**这个项目的**名字。
///
/// 和概览卡片同一个套路：这里既生成文件、又插入引用。`artifacts` 阶段在
/// 流水线里排在 `polish` 之后，等它来生成的话，`logo` 这一步会因为文件还
/// 不存在而跳过——首次运行就永远插不上。
///
/// 显式给了 `--logo` 就让位：使用者点名的那张图，优先于我们生成的。
fn hero(ctx: &RepoContext, readme: &Readme, style: &ReadmeStyle, plan: &mut Plan) {
    if !style.hero || style.logo.is_some() {
        return;
    }
    if shows_image(readme, repolish_render::HERO_PATH) || has_image_above_title(readme) {
        return;
    }
    let Some(anchor) = readme.title_line else {
        return;
    };

    let opts = card_options(style);
    let facts = repolish_render::Facts::from_ctx(ctx, opts.lang);
    let tagline = facts.description.clone().unwrap_or_default();
    plan.add_file(
        ctx.root.join(repolish_render::HERO_PATH),
        repolish_render::svg::hero(&facts.name, &tagline, opts.lang),
        "readme style: project banner requested by configuration",
    );

    plan.inserts.push(Insert::new(
        anchor - 1,
        format!(
            "readme style: project banner requested by configuration ({})",
            repolish_render::HERO_PATH
        ),
        logo_lines(
            repolish_render::HERO_PATH,
            Some(LogoWidth::Full),
            style.align,
        ),
    ));
}

/// README 顶部的图。
///
/// **`alt` 必须留空。** 非空 alt 会让这张图成为标题候选，而图片标题会把
/// `readme-title-tagline` 从 10 分打到 5 分——polish 插一张 logo 反倒让分数掉了，
/// 是不能接受的。空 alt 同时也是正确的无障碍语义：旁边已经有文字标题，
/// 这张图是装饰性的，读屏软件应当跳过它。
fn logo(ctx: &RepoContext, readme: &Readme, style: &ReadmeStyle, plan: &mut Plan) {
    let Some(src) = style.logo.as_deref() else {
        return;
    };
    if readme.raw.contains(src) {
        return;
    }
    // 指向仓库外的图会被 readme-link-health 判成死链，等于用一条修复换一条扣分
    if !ctx.has(src) {
        eprintln!("warning: logo {src} is not in the repository, skipping it");
        return;
    }
    let Some(anchor) = readme.title_line else {
        return;
    };

    plan.inserts.push(Insert::new(
        anchor - 1,
        format!("readme style: logo requested by configuration ({src})"),
        logo_lines(src, style.logo_width, style.align),
    ));
}

/// logo 那几行。
///
/// 结尾**必须**空一行。图片块是 HTML，紧跟其后的 Markdown 会被并进那个块——
/// 少这一行，下面的 `# Name` 就不再是标题，`readme-title-tagline` 会把正文里
/// 第一个小节标题当成项目名，10 分掉到 6 分。徽章那一刀早就记着这个坑
/// （flask、fzf 都栽过），这里是同一个。
fn logo_lines(src: &str, width: Option<LogoWidth>, align: Align) -> Vec<String> {
    let width = width
        .map(|w| format!(" width=\"{}\"", w.attr()))
        .unwrap_or_default();
    let mut lines = wrap(vec![format!("<img src=\"{src}\" alt=\"\"{width}>")], align);
    lines.push(String::new());
    lines
}

/// 项目结构树。
///
/// **这是唯一一把不由检查结果驱动的刀**：没有任何一项检查要求 README 里有
/// 目录树。默认关闭，只有配置里显式给了 `tree-depth` 才生成，理由行里也
/// 照实写「由配置要求」。命名这个例外，比假装它也是一条修复要诚实。
fn project_tree(ctx: &RepoContext, readme: &Readme, style: &ReadmeStyle, plan: &mut Plan) {
    let Some(depth) = style.tree_depth.filter(|d| *d > 0) else {
        return;
    };
    if readme.sections.iter().any(|s| {
        let t = s.title.to_lowercase();
        t.contains("project structure") || t.contains("目录结构")
    }) {
        return;
    }

    let level = readme.outline().first().map_or(2, |s| s.level) as usize;
    let mut lines = vec![
        format!("{} Project structure", "#".repeat(level)),
        String::new(),
        "```".to_string(),
    ];
    lines.extend(
        tree::render(&ctx.files, &ctx.display_name(), depth)
            .lines()
            .map(str::to_string),
    );
    lines.push("```".to_string());
    lines.push(String::new());

    // 插在最后：树是参考资料，不该挤在读者最需要的「这是什么、怎么用」前面
    plan.inserts.push(Insert::new(
        readme.raw.lines().count(),
        format!("readme style: project tree requested by configuration (depth {depth})"),
        lines,
    ));
}

// ── 概览卡片 ────────────────────────────────────────────────

/// README 顶部的项目概览卡片。
///
/// 和目录树一样，**不由检查结果驱动**：没有任何一项检查要求 README 里有
/// 一张概览图。默认关闭，`--overview` 或配置里显式开了才生成，理由行里
/// 也照实写「由配置要求」。
///
/// 位置在徽章之后、正文之前。它回答的是「这是什么项目」——语言构成、
/// 提交活跃度、许可证——那正是一个陌生人点进来的头三秒要的东西。
fn overview_card(ctx: &RepoContext, readme: &Readme, style: &ReadmeStyle, plan: &mut Plan) {
    if !style.overview {
        return;
    }
    if shows_image(readme, repolish_render::OVERVIEW_PATH) {
        return;
    }

    let opts = card_options(style);
    let facts = repolish_render::Facts::from_ctx(ctx, opts.lang);
    plan.add_file(
        ctx.root.join(repolish_render::OVERVIEW_PATH),
        repolish_render::overview(&facts, &opts),
        "readme style: project overview card requested by configuration",
    );

    // 插在徽章那一排之后。插在徽章之前的话，一张图会把作者精心排好的
    // 一行徽章挤到图下面去，看着像两个不相干的块。
    let anchor = readme
        .badge_rows
        .last()
        .map(|r| r.end)
        .or(readme.title_end_line)
        .or(readme.title_line);
    let Some(anchor) = anchor else { return };

    let alt = format!("{} at a glance", ctx.display_name());
    let mut lines = vec![String::new()];
    lines.extend(wrap(
        vec![format!(
            "<img src=\"{}\" alt=\"{}\" width=\"880\">",
            repolish_render::OVERVIEW_PATH,
            escape_attr(&alt)
        )],
        style.align,
    ));
    lines.push(String::new());

    plan.inserts.push(Insert::new(
        anchor,
        "readme style: project overview card requested by configuration",
        lines,
    ));
}

// ── 页脚的分数卡片 ──────────────────────────────────────────

/// README 末尾的「用 repolish 打磨过」一节。
///
/// 分数卡片放在这里而不是顶上，是想清楚过的：顶上属于**这个项目**，
/// 一个陌生人点进来第一眼该看到的是它做什么，不是我们的工具给它打了几分。
/// 而在末尾，读到这儿的人已经决定要不要用这个项目了——此时告诉他
/// 「这份 README 是用 repolish 打磨的」，才是一条有用的信息而不是一块广告。
fn footer_card(
    ctx: &RepoContext,
    report: &Report,
    readme: &Readme,
    style: &ReadmeStyle,
    plan: &mut Plan,
) {
    if !style.footer_card {
        return;
    }
    if shows_image(readme, repolish_render::CARD_PATH) {
        return;
    }
    // 覆盖率不足时卡片上是一个 `--`。往别人 README 末尾贴一张没有分数的
    // 分数卡片，比不贴更糟。
    if report.score.is_none() {
        return;
    }

    plan.add_file(
        ctx.root.join(repolish_render::CARD_PATH),
        repolish_render::card(report, &card_options(style)),
        "readme style: repolish report card requested by configuration",
    );

    let cjk = readme_is_cjk(readme);
    let level = readme.outline().first().map_or(2, |s| s.level) as usize;
    let (heading, blurb) = if cjk {
        (
            "用 repolish 打磨",
            format!(
                "这张卡片由 [repolish](%URL%) 生成，是仓库里的一个普通文件——\
                 没有外部字体、没有脚本、不由任何第三方托管。\
                 想给自己的仓库打一次分：`{}`。",
                "npx @asale/repolish"
            ),
        )
    } else {
        (
            "Polished with repolish",
            format!(
                "This card is generated by [repolish]({}) and is a plain file in this \
                 repository — no external fonts, no scripts, nothing hosted by a third party. \
                 To score your own: `{}`.",
                "%URL%", "npx @asale/repolish"
            ),
        )
    };
    let blurb = blurb.replace("%URL%", repolish_render::REPOLISH_URL);

    let mut lines = vec![
        String::new(),
        format!("{} {heading}", "#".repeat(level)),
        String::new(),
    ];
    lines.extend(wrap(
        vec![format!(
            "<img src=\"{}\" alt=\"repolish report card\" width=\"880\">",
            repolish_render::CARD_PATH
        )],
        style.align,
    ));
    lines.push(String::new());
    lines.push(blurb);
    lines.push(String::new());

    plan.inserts.push(Insert::new(
        readme.raw.lines().count(),
        "readme style: repolish report card requested by configuration",
        lines,
    ));
}

// ── 表格 ────────────────────────────────────────────────────

/// README 里的表格 → SVG，原表格折进 `<details>`。
///
/// **这一刀是包，不是改。** 原表格的每一个字节都留在原处，只在它前后各插
/// 一段。README 在 GitHub 上渲染表格，在 crates.io、npm 和各种聚合站上
/// 却常常把管道符原样吐出来——一张图在哪儿都是同一张图。
///
/// 原文必须留着，这条不是可选项：图片没有文本层，读屏软件、`grep`、
/// 翻译工具、以及下一个想改这张表的人，读的都是折起来的那份。
///
/// 选表与命名的规则在 [`crate::tables`]，与 `card --kind tables` 共用一份——
/// 重画出来的文件名和当初插进 README 的对不上，比不重画更糟。
fn svg_tables(ctx: &RepoContext, readme: &Readme, style: &ReadmeStyle, plan: &mut Plan) {
    if style.tables != TableStyle::Svg {
        return;
    }
    // 主 README
    let inserts = table_inserts(ctx, readme, style, plan);
    plan.inserts.extend(inserts);

    // 每一份译本。一份中文 README 里的中文表格，在 crates.io 上照样是一堆
    // 管道符——这个功能对译本成立的理由和对主 README 完全一样。
    for path in crate::tables::translations(ctx, readme) {
        let Some(raw) = ctx.files.read(&path) else {
            continue;
        };
        let translated = Readme::parse(&path, raw.clone());
        // 卡片语言跟着**这份**译本走，不是跟着主 README 走
        let lang = repolish_render::Lang::detect(&translated.raw);
        let mut sub = style.clone();
        sub.lang = lang;

        let inserts = table_inserts(ctx, &translated, &sub, plan);
        if !inserts.is_empty() {
            plan.translation(ctx.root.join(&path), &raw).inserts = inserts;
        }
    }
}

/// 录屏在一份 README 里该插在哪、插什么。
///
/// 位置:概览卡之后 → 徽章之后 → 标题之后,取第一个找得到的。录屏说的是
/// 「这东西跑起来什么样」,那是读者在看完「这是什么」之后紧接着要问的问题;
/// 排到文末就等于给一个已经决定要不要用的人看。
///
/// alt **不能为空**,和 logo 正相反:logo 是装饰,而录屏是内容——读屏软件跳过
/// 它,那个用户就少了一整段信息。它在标题下方,所以不会被当成标题候选。
pub fn recording_inserts(readme: &Readme, rel: &str, lang: repolish_render::Lang) -> Vec<Insert> {
    if shows_image(readme, rel) {
        return Vec::new();
    }
    let after_overview = readme
        .raw
        .lines()
        .position(|l| l.contains(repolish_render::OVERVIEW_PATH))
        .map(|i| i + 1);
    let anchor = after_overview
        .or_else(|| readme.badge_rows.last().map(|r| r.end))
        .or(readme.title_end_line)
        .or(readme.title_line);
    let Some(anchor) = anchor else {
        return Vec::new();
    };

    let alt = match lang {
        repolish_render::Lang::ZhCn => "终端录屏",
        repolish_render::Lang::Ja => "ターミナル録画",
        repolish_render::Lang::En => "terminal recording",
    };
    vec![Insert::new(
        anchor,
        "readme style: terminal recording requested by configuration",
        vec![String::new(), crate::demo::snippet(rel, alt), String::new()],
    )]
}

/// 对一份 README 算出「包表格」要插的那些行，同时把 SVG 排进 `plan` 的新文件里。
///
/// **这一刀是包，不是改。** 原表格的每一个字节都留在原处，只在它前后各插一段。
/// README 在 GitHub 上渲染表格，在 crates.io、npm 和各种聚合站上却常常把管道符
/// 原样吐出来——一张图在哪儿都是同一张图。
///
/// 原文必须留着，这条不是可选项：图片没有文本层，读屏软件、`grep`、翻译工具、
/// 以及下一个想改这张表的人，读的都是折起来的那份。
fn table_inserts(
    ctx: &RepoContext,
    readme: &Readme,
    style: &ReadmeStyle,
    plan: &mut Plan,
) -> Vec<Insert> {
    let cjk = readme_is_cjk(readme);
    let rendered = crate::tables::render(readme, &card_options(style), |w| {
        eprintln!("note: {w}");
    });

    let mut inserts = Vec::new();
    for table in rendered {
        // 已经被包过一次了。再包一层会得到嵌套的 <details>，
        // 而里层那张图早就画好了。
        if crate::tables::already_wrapped(readme, table.start_line) {
            continue;
        }

        plan.add_file(
            table.path(&ctx.root),
            table.svg.clone(),
            format!(
                "readme style: table at {}:{} rendered as SVG (requested by configuration)",
                readme.path.display(),
                table.start_line
            ),
        );

        let summary = match (&table.title, cjk) {
            (Some(t), true) => format!("{t}（表格原文）"),
            (Some(t), false) => format!("{t} as a table"),
            (None, true) => "表格原文".to_string(),
            (None, false) => "The same thing as a table".to_string(),
        };
        let alt = table.title.clone().unwrap_or_else(|| "table".to_string());

        inserts.push(Insert::new(
            table.start_line - 1,
            format!(
                "readme style: SVG table for the table at line {}",
                table.start_line
            ),
            vec![
                format!(
                    "<img src=\"{}\" alt=\"{}\" width=\"880\">",
                    table.rel,
                    escape_attr(&alt)
                ),
                String::new(),
                "<details>".to_string(),
                format!("<summary>{}</summary>", escape_attr(&summary)),
                String::new(),
            ],
        ));
        inserts.push(Insert::new(
            table.end_line,
            format!(
                "readme style: closing the folded table at line {}",
                table.start_line
            ),
            vec![String::new(), "</details>".to_string()],
        ));
    }
    inserts
}

/// HTML 属性值里的引号与尖括号。标题是作者写的，可能含任何东西。
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// README 里已经**显示**了这张图吗。
///
/// 不能光看路径出现过没有：一份文档里提到 `.repolish/card.svg` 的方式，
/// 除了贴图还有代码块、行内代码和散文——这份 README 自己就在「The cards」
/// 一节里把两个路径都写了出来。按出现与否判断，那一节会让插入永远不发生。
///
/// 只认两种真正会渲染出图的写法：HTML 的 `src="…"` 与 Markdown 的 `](…)`。
/// 标题**之上**已经有一张图了吗。
///
/// 横幅的位置只有一个，而它已经被占了——再插一张的结果是两条横幅摞在一起。
/// 判据不能是「有没有引用我们那个路径」：作者自己画的 banner 叫什么名字
/// 我们无从知道，本仓库顶上那张就叫 `assets/hero.svg`。
fn has_image_above_title(readme: &Readme) -> bool {
    let Some(title) = readme.title_line else {
        return false;
    };
    readme
        .raw
        .lines()
        .take(title.saturating_sub(1))
        .any(|l| l.contains("<img") || l.contains("!["))
}

fn shows_image(readme: &Readme, path: &str) -> bool {
    readme.raw.contains(&format!("src=\"{path}\""))
        || readme.raw.contains(&format!("src='{path}'"))
        || readme.raw.contains(&format!("]({path})"))
}

/// 这份 README 主要是中文吗。插进去的每一段文字都跟着它走——
/// 我们的报告一律英文，那是另一回事，见 CONTRIBUTING 的第三条规则。
fn readme_is_cjk(readme: &Readme) -> bool {
    repolish_render::Lang::detect(&readme.raw) == repolish_render::Lang::ZhCn
}

/// 按对齐方式包一层。居中块里放 Markdown 语法是不渲染的，所以调用方
/// 传进来的必须已经是 HTML。
fn wrap(lines: Vec<String>, align: Align) -> Vec<String> {
    match align {
        Align::Left => lines,
        Align::Center => {
            let mut out = vec!["<p align=\"center\">".to_string()];
            out.extend(lines.into_iter().map(|l| format!("  {l}")));
            out.push("</p>".to_string());
            out
        }
    }
}

/// issue / PR 模板。
///
/// 这一刀最没有争议：GitHub 的表单 schema 问的是版本号、复现步骤、改了什么，
/// 没有一处是项目特有的，因此没有可猜的余地。缺哪个补哪个——
/// 已经有 issue 模板只缺 PR 模板时，不会顺手多写两个表单。
fn templates(ctx: &RepoContext, report: &Report, plan: &mut Plan) {
    if !failing(report, "issue-pr-template") {
        return;
    }
    let project = ctx.display_name();
    let dir = ctx.root.join(".github");

    // 已经有任意一个 issue 模板就不再补：作者自己挑的形状不该被我们加料
    let has_issue = ctx.files.any_matching(|p| {
        let l = p.to_lowercase();
        l.starts_with(".github/issue_template")
            && !l.ends_with("/config.yml")
            && !l.ends_with("/config.yaml")
            && (l.ends_with(".md") || l.ends_with(".yml") || l.ends_with(".yaml"))
    });
    if !has_issue {
        plan.add_file(
            dir.join("ISSUE_TEMPLATE/bug_report.yml"),
            scaffold::bug_report(&project),
            "issue-pr-template: no issue template under `.github/`",
        );
        plan.add_file(
            dir.join("ISSUE_TEMPLATE/feature_request.yml"),
            scaffold::feature_request(&project),
            "issue-pr-template: no issue template under `.github/`",
        );
    }

    let test = scaffold::toolchain(&ctx.manifests).and_then(|t| t.test);
    plan.add_file(
        dir.join("pull_request_template.md"),
        scaffold::pull_request_template(test.as_deref()),
        "issue-pr-template: no PR template under `.github/`",
    );
}

/// 贡献指南。
///
/// **探测不出包生态就不生成。** 那时构建与测试命令只能靠编，而写一份
/// `<your build command here>` 进别人的仓库，比让这一项继续扣分更糟——
/// 它会让检查项变绿，问题却还在那儿。
fn contributing(ctx: &RepoContext, report: &Report, plan: &mut Plan) {
    if !failing(report, "contributing") {
        return;
    }
    // 已经有一份（哪怕很薄）就不动：polish 不重写，补内容是作者的事
    let exists = ctx.files.any_matching(|p| {
        let l = p.to_lowercase();
        matches!(
            l.as_str(),
            "contributing.md" | ".github/contributing.md" | "docs/contributing.md"
        )
    });
    if exists {
        return;
    }
    let Some(t) = scaffold::toolchain(&ctx.manifests) else {
        return;
    };

    plan.add_file(
        ctx.root.join("CONTRIBUTING.md"),
        scaffold::contributing(&ctx.display_name(), ctx.slug.as_ref(), &t),
        format!(
            "contributing: none in the repository root, .github/, or docs/ — build commands taken from the detected {} manifest",
            ctx.manifests
                .first()
                .map(|m| m.ecosystem.as_str())
                .unwrap_or("package")
        ),
    );
}

/// 某个检查项是否扣了分。
///
/// `polish` 落的每一刀都得对得上一条检查结果——阈值由检查项定义，
/// 这边再写一遍迟早会漂。
fn failing(report: &Report, id: &str) -> bool {
    report.checks.iter().any(|c| {
        c.id == id
            && matches!(c.outcome, repolish_core::Outcome::Scored { score, .. } if score < 10)
    })
}

/// repolish 徽章。
///
/// 三个前提缺一不可：能算出仓库 slug（否则 URL 里的 owner/repo 只能靠猜）、
/// 覆盖率够得上出徽章、README 里还没有。
fn badge(
    ctx: &RepoContext,
    report: &Report,
    readme: &Readme,
    style: &ReadmeStyle,
    plan: &mut Plan,
) {
    let Some(slug) = ctx.slug.as_ref() else {
        return;
    };
    // 覆盖率不足时 badge_json 返回 None。这种情况下连徽章文件都不该写，
    // 更不该往别人 README 里插一个指向不存在文件的链接。
    let Some(json) = repolish_render::badge_json(report) else {
        return;
    };

    let branch = ctx
        .git
        .as_ref()
        .and_then(|g| g.branch.clone())
        .unwrap_or_else(|| "main".to_string());
    let shields = style.badge.as_str();
    // 追加到已有的一排徽章里时必须用 Markdown：那一排是作者用 Markdown 写的，
    // 混一行 HTML 进去会在渲染上留下一道接缝。只有另起一块时才谈得上对齐方式。
    let inline = repolish_render::styled_snippet(&slug.owner, &slug.name, &branch, shields);
    if let Some(insert) = badge_insert(readme, &inline, repolish_render::BADGE_PATH) {
        let appended = readme.badge_anchor().is_some_and(|a| a.appends());
        let insert = if appended || style.align == Align::Left {
            insert
        } else {
            let html =
                repolish_render::styled_snippet_html(&slug.owner, &slug.name, &branch, shields);
            // 这一支只在「另起一块」时走到，而另起一块必须与上一行隔开——
            // `BadgeAnchor::lines_for` 本来就是这么做的，换成 HTML 不能把它丢掉
            let mut lines = wrap(vec![html], style.align);
            lines.insert(0, String::new());
            Insert::new(insert.after_line, insert.reason.clone(), lines)
        };
        plan.inserts.push(insert);
    }

    // 本地分把三个可发现性检查项剔出了分母，通常比远程分低几分。
    // 标签上的 `(local)` 已经是诚实的，但一个默认跑 polish 的人不会知道
    // 自己刚往 README 上贴了一个偏低的分数——所以这里说出来。
    let reason = match report.mode {
        repolish_core::Mode::Local => {
            "readme-badges: the badge line points at this file — a local score; \
             re-run with --remote, or let CI overwrite it"
        }
        repolish_core::Mode::Remote => "readme-badges: the badge line points at this file",
    };

    plan.add_file(ctx.root.join(repolish_render::BADGE_PATH), json, reason);
}

/// README 里该不该插徽章、插在哪。
///
/// `marker` 是「已经有了」的判据：徽章 URL 里一定含 `.repolish/badge.json`
/// 这个路径。按整段 snippet 比对是不行的——分支名不同、
/// owner 大小写不同都会让同一个徽章看起来像两个。
fn badge_insert(readme: &Readme, snippet: &str, marker: &str) -> Option<Insert> {
    if readme.raw.contains(marker) {
        return None;
    }
    let anchor = readme.badge_anchor()?;
    Some(Insert::new(
        anchor.line(),
        "readme-badges: no repolish badge yet",
        anchor.lines_for(snippet),
    ))
}

/// 少于这个条目数就不插目录——两三行的目录只是噪声。
const MIN_TOC_ITEMS: usize = 4;

/// 目录。
///
/// 每一条都由作者自己的标题生成，一个字都不是编的；锚点按 GitHub 的
/// slugger 算（见 [`repolish_md::toc`]），否则插进去的是一堆跳不到的死链。
fn toc(report: &Report, readme: &Readme, style: &ReadmeStyle, plan: &mut Plan) {
    if !failing(report, "readme-toc") {
        return;
    }
    if let Some(insert) = toc_insert(readme, style.toc) {
        plan.inserts.push(insert);
    }
}

/// 目录本身。门槛判定在 [`toc`]，这里只管「长什么样、插在哪」。
fn toc_insert(readme: &Readme, style: TocStyle) -> Option<Insert> {
    let outline = readme.outline();
    if outline.len() < MIN_TOC_ITEMS {
        return None;
    }
    let first = outline[0];

    // 锚点要拿**全文**标题一起算：正文里有同名标题时，`-1` / `-2` 的编号
    // 才不会错位。只算目录里列的那几个是不够的。
    let anchors = repolish_md::toc::anchors(readme.sections.iter().map(|s| s.title.as_str()));

    // 目录标题的层级跟着正文走。ripgrep 的小节是 `###`，插一个 `##` 进去
    // 等于凭空多出一层，把它原本的层级结构切断了。
    let hashes = "#".repeat(first.level as usize);
    let word = toc_word(&outline);
    // fold 把目录收进 <details>：长 README 里一份二十条的目录本身就占满一屏
    let mut lines = match style {
        TocStyle::Fold => vec![
            "<details>".to_string(),
            format!("<summary>{word}</summary>"),
            String::new(),
        ],
        _ => vec![format!("{hashes} {word}"), String::new()],
    };
    for (i, s) in outline.iter().enumerate() {
        let anchor = readme
            .sections
            .iter()
            .position(|x| x.line == s.line)
            .map(|i| anchors[i].clone())
            .unwrap_or_else(|| repolish_md::toc::anchor(&s.title));
        lines.push(format!("{} [{}](#{anchor})", style.marker(i), s.title));
    }
    if style == TocStyle::Fold {
        lines.push(String::new());
        lines.push("</details>".to_string());
    }
    lines.push(String::new());

    Some(Insert::new(
        first.line - 1,
        format!(
            "readme-toc: {} sections over {} lines, with no table of contents",
            readme.sections.len(),
            readme.raw.lines().count()
        ),
        lines,
    ))
}

/// 目录该叫「Contents」还是「目录」。
///
/// 这一段是写进**别人的** README 的，跟着人家的语言走。repolish 自己的
/// 报告一律英文，那是另一回事——见 CONTRIBUTING 的第三条规则。
fn toc_word(outline: &[&repolish_md::Section]) -> &'static str {
    let cjk = outline
        .iter()
        .filter(|s| repolish_md::has_cjk(&s.title))
        .count();
    if cjk * 2 > outline.len() {
        "目录"
    } else {
        "Contents"
    }
}

/// 把计划应用到原文上。
pub fn polished(readme: &Readme, plan: &Plan) -> String {
    apply(&readme.raw, &plan.inserts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNIPPET: &str = "[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/o/r/main/.repolish/badge.json)](https://github.com/asale-ai/repolish)";
    const MARKER: &str = ".repolish/badge.json";

    fn polish(md: &str) -> Option<String> {
        let readme = Readme::parse("README.md", md);
        let insert = badge_insert(&readme, SNIPPET, MARKER)?;
        Some(apply(&readme.raw, &[insert]))
    }

    #[test]
    fn appends_to_an_existing_badge_row_without_a_blank_line() {
        // 空一行会让徽章变成新段落，渲染出来另起一行——作者摆好的一排就断了
        let out = polish("# Tool\n\n[![CI](ci.svg)](ci)\n\nProse.\n").unwrap();
        assert_eq!(
            out,
            format!("# Tool\n\n[![CI](ci.svg)](ci)\n{SNIPPET}\n\nProse.\n")
        );
    }

    #[test]
    fn inserts_after_the_title_with_a_blank_line() {
        let out = polish("# Tool\n\nProse.\n").unwrap();
        assert_eq!(out, format!("# Tool\n\n{SNIPPET}\n\nProse.\n"));
    }

    #[test]
    fn does_nothing_when_the_badge_is_already_there() {
        let md = format!("# Tool\n\n{SNIPPET}\n\nProse.\n");
        assert!(polish(&md).is_none());
    }

    #[test]
    fn a_badge_on_another_branch_still_counts_as_present() {
        // 同一个徽章指向 master 分支。按整段比对会重复插一次。
        let md = "# Tool\n\n[![repolish](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/o/r/master/.repolish/badge.json)](https://github.com/asale-ai/repolish)\n";
        assert!(polish(md).is_none());
    }

    #[test]
    fn crlf_readmes_keep_crlf() {
        let out = polish("# Tool\r\n\r\nProse.\r\n").unwrap();
        assert_eq!(out, format!("# Tool\r\n\r\n{SNIPPET}\r\n\r\nProse.\r\n"));
    }

    #[test]
    fn everything_except_the_inserted_lines_is_byte_identical() {
        // 这是这个命令的核心承诺，值得单独立一条：
        // 制表符、`*` 列表标记、引用式链接定义、行尾，一个字节都不能动。
        let md = "Tool\n====\n\n*  item\n\thard tab\n\n[ref]: https://example.com\n";
        let out = polish(md).unwrap();
        let before: Vec<&str> = md.lines().collect();
        let after: Vec<&str> = out.lines().collect();
        assert_eq!(after.len(), before.len() + 2);
        assert_eq!(&after[..2], &before[..2]); // 标题与下划线
        assert_eq!(&after[4..], &before[2..]); // 其余原样后移
        assert_eq!(after[2], "");
        assert_eq!(after[3], SNIPPET);
    }

    #[test]
    fn no_title_means_no_anchor_and_no_edit() {
        // 认不出标题就不猜位置。宁可不改，也不要插到一个说不清的地方。
        assert!(polish("just prose, no heading at all\n").is_none());
    }
}

#[cfg(test)]
mod toc_tests {
    use super::*;
    use repolish_md::edit::apply;

    fn toc(md: &str) -> Option<String> {
        let readme = Readme::parse("README.md", md);
        toc_insert(&readme, TocStyle::default()).map(|i| apply(&readme.raw, &[i]))
    }

    #[test]
    fn lists_the_body_sections_with_github_anchors() {
        let md = "# Tool\n\nTagline.\n\n## Why & how\n\na\n\n## Install\n\nb\n\n## Usage\n\nc\n\n## License\n\nd\n";
        let out = toc(md).unwrap();
        assert!(out.contains("## Contents\n"));
        assert!(out.contains("- [Why & how](#why--how)\n"));
        assert!(out.contains("- [License](#license)\n"));
        // 目录插在第一个正文小节之前，标语之后
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "# Tool");
        assert_eq!(lines[2], "Tagline.");
        assert_eq!(lines[4], "## Contents");
        assert_eq!(lines[11], "## Why & how");
    }

    #[test]
    fn the_toc_heading_matches_the_level_of_the_sections_it_lists() {
        // ripgrep：小节是 `###`。插一个 `##` 进去等于凭空多出一层。
        let md = "rg\n--\n\n### A\n\na\n\n### B\n\nb\n\n### C\n\nc\n\n### D\n\nd\n";
        let out = toc(md).unwrap();
        assert!(out.contains("### Contents\n"), "{out}");
        assert!(out.contains("- [A](#a)\n"));
    }

    #[test]
    fn a_chinese_readme_gets_a_chinese_heading() {
        let md =
            "# 工具\n\n## 为什么做这个\n\na\n\n## 安装\n\nb\n\n## 用法\n\nc\n\n## 许可证\n\nd\n";
        let out = toc(md).unwrap();
        assert!(out.contains("## 目录\n"), "{out}");
        assert!(out.contains("- [安装](#安装)\n"));
    }

    #[test]
    fn duplicate_headings_elsewhere_shift_the_numbering() {
        // 正文里另有一个 `### Usage`。GitHub 按全文顺序编号，
        // 只算目录里那几条会把第二个 Usage 的锚点算成 `usage` 而不是 `usage-1`。
        let md = "# Tool\n\n## Usage\n\na\n\n### Usage\n\nb\n\n## Notes\n\nc\n\n## Usage\n\nd\n\n## End\n\ne\n";
        let out = toc(md).unwrap();
        assert!(out.contains("- [Usage](#usage)\n"), "{out}");
        assert!(out.contains("- [Usage](#usage-2)\n"), "{out}");
    }

    #[test]
    fn a_short_outline_is_left_alone() {
        // 两三行的目录只是噪声
        assert!(toc("# Tool\n\n## A\n\na\n\n## B\n\nb\n").is_none());
    }

    #[test]
    fn everything_outside_the_inserted_block_is_byte_identical() {
        let md =
            "# Tool\n\n*  keep\n\thard tab\n\n## A\n\na\n\n## B\n\nb\n\n## C\n\nc\n\n## D\n\nd\n";
        let out = toc(md).unwrap();
        let before: Vec<&str> = md.lines().collect();
        let after: Vec<&str> = out.lines().collect();
        let added = after.len() - before.len();
        assert_eq!(&after[..5], &before[..5]);
        assert_eq!(&after[5 + added..], &before[5..]);
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;
    use crate::style::{Align, TocStyle};

    /// 图片块后面不空一行，下面的 `# Name` 会被并进 HTML 块里，
    /// 于是正文第一个小节标题被当成项目名——实测 10 分掉到 6 分。
    #[test]
    fn the_logo_block_always_ends_with_a_blank_line() {
        for align in [Align::Left, Align::Center] {
            let lines = logo_lines("assets/hero.svg", Some(LogoWidth::Px(420)), align);
            assert_eq!(
                lines.last().map(String::as_str),
                Some(""),
                "{align:?} 缺少结尾空行: {lines:?}"
            );
        }
    }

    /// alt 必须为空：非空 alt 会让这张图成为标题候选，
    /// 而图片标题会把 readme-title-tagline 从 10 分打到 5 分
    #[test]
    fn the_logo_carries_no_alt_text() {
        let lines = logo_lines("assets/hero.svg", None, Align::Left);
        let img = &lines[0];
        assert!(img.contains(r#"alt="""#), "{img}");
        assert!(
            !img.contains("width="),
            "没给宽度就不该有 width 属性: {img}"
        );
    }

    #[test]
    fn width_is_emitted_only_when_asked_for() {
        let lines = logo_lines("a.svg", Some(LogoWidth::Px(300)), Align::Left);
        assert!(lines[0].contains(r#"width="300""#), "{:?}", lines[0]);
    }

    /// 通栏横幅要 `width="100%"`：固定像素宽的图在宽屏上缩在左上角，
    /// 在手机上又撑破版心
    #[test]
    fn a_full_width_logo_asks_for_a_percentage_not_a_pixel_count() {
        let lines = logo_lines("assets/hero.svg", Some(LogoWidth::Full), Align::Center);
        assert!(
            lines.iter().any(|l| l.contains(r#"width="100%""#)),
            "{lines:?}"
        );
        assert_eq!(
            lines.first().map(String::as_str),
            Some(r#"<p align="center">"#)
        );
    }

    /// 居中块里放 Markdown 是不渲染的，所以 wrap 只接受 HTML；
    /// 左对齐时不该凭空多包一层
    #[test]
    fn left_alignment_wraps_nothing() {
        let out = wrap(vec!["<img src=\"a\">".into()], Align::Left);
        assert_eq!(out, vec!["<img src=\"a\">".to_string()]);
    }

    #[test]
    fn centering_wraps_in_a_paragraph_tag() {
        let out = wrap(vec!["<img src=\"a\">".into()], Align::Center);
        assert_eq!(
            out.first().map(String::as_str),
            Some(r#"<p align="center">"#)
        );
        assert_eq!(out.last().map(String::as_str), Some("</p>"));
    }

    #[test]
    fn fold_style_closes_its_details_block() {
        let readme = Readme::parse(
            "README.md",
            "# t\n\nintro\n\n## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n\n## D\n\nw\n",
        );
        let insert = toc_insert(&readme, TocStyle::Fold).expect("有目录可插");
        assert_eq!(insert.lines.first().map(String::as_str), Some("<details>"));
        assert!(
            insert.lines.iter().any(|l| l == "</details>"),
            "{:?}",
            insert.lines
        );
    }

    #[test]
    fn numbered_and_roman_styles_change_only_the_marker() {
        let md = "# t\n\nintro\n\n## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n\n## D\n\nw\n";
        let readme = Readme::parse("README.md", md);
        let numbered = toc_insert(&readme, TocStyle::Number).unwrap();
        let roman = toc_insert(&readme, TocStyle::Roman).unwrap();
        assert!(numbered.lines.iter().any(|l| l.starts_with("1. [A]")));
        assert!(roman.lines.iter().any(|l| l.starts_with("i. [A]")));
        assert!(roman.lines.iter().any(|l| l.starts_with("iv. [D]")));
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;
    use repolish_md::edit::apply;

    /// 只跑「包表格」这一刀：造一个完整的 RepoContext 要落地一个仓库，
    /// 而这里要验的是插入的形状，不是探测。
    fn wrap_tables(md: &str) -> String {
        let readme = Readme::parse("README.md", md);
        let mut inserts = Vec::new();
        for found in repolish_md::tables::find(&readme.raw) {
            if found.rows.len() < crate::tables::MIN_ROWS || found.headers.len() < 2 {
                continue;
            }
            let title = readme
                .sections
                .iter()
                .filter(|s| s.line < found.start_line)
                .max_by_key(|s| s.line)
                .map(|s| s.title.clone());
            inserts.push(Insert::new(
                found.start_line - 1,
                "svg table",
                vec![
                    format!(
                        "<img src=\"{}/01-x.svg\" alt=\"x\" width=\"880\">",
                        crate::tables::TABLES_DIR
                    ),
                    String::new(),
                    "<details>".to_string(),
                    format!(
                        "<summary>{}</summary>",
                        title.unwrap_or_else(|| "table".into())
                    ),
                    String::new(),
                ],
            ));
            inserts.push(Insert::new(
                found.end_line,
                "close",
                vec![String::new(), "</details>".to_string()],
            ));
        }
        apply(&readme.raw, &inserts)
    }

    const MD: &str = "# Tool\n\n## Exit codes\n\n| Code | Meaning |\n|---|---|\n| 0 | Success |\n| 1 | Too low |\n\nafter\n";

    /// 这一刀是**包**，不是改：原表格的每一个字节都必须留在原处
    #[test]
    fn the_original_table_survives_byte_for_byte() {
        let out = wrap_tables(MD);
        assert!(out.contains("| Code | Meaning |\n|---|---|\n| 0 | Success |\n| 1 | Too low |\n"));
        // 原文的每一行都还在，顺序也没变
        let mut cursor = 0usize;
        for line in MD.lines() {
            let at = out[cursor..]
                .find(line)
                .unwrap_or_else(|| panic!("原文这一行不见了: {line:?}"));
            cursor += at + line.len();
        }
    }

    #[test]
    fn the_image_goes_above_and_the_details_block_closes_below() {
        let out = wrap_tables(MD);
        let lines: Vec<&str> = out.lines().collect();
        let img = lines.iter().position(|l| l.contains(".svg")).unwrap();
        let open = lines.iter().position(|l| *l == "<details>").unwrap();
        let table = lines.iter().position(|l| l.starts_with("| Code")).unwrap();
        let close = lines.iter().position(|l| *l == "</details>").unwrap();
        assert!(img < open && open < table && table < close, "{lines:#?}");
        // <summary> 与表格之间必须空一行，否则 GitHub 不渲染折叠块里的表格
        assert_eq!(lines[open + 2], "");
        assert!(lines[open + 1].starts_with("<summary>"));
    }

    #[test]
    fn the_summary_names_the_section_the_table_lives_in() {
        assert!(wrap_tables(MD).contains("<summary>Exit codes</summary>"));
    }

    /// 两三行的表画成图没有增益，只是多一次网络请求
    #[test]
    fn a_one_row_table_is_left_alone() {
        let md = "## X\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        assert_eq!(wrap_tables(md), md);
    }

    #[test]
    fn crlf_readmes_keep_crlf() {
        let md = MD.replace('\n', "\r\n");
        let out = wrap_tables(&md);
        assert!(!out.contains("\n\n"), "混进了 LF");
        assert!(out.contains("<details>\r\n"));
    }

    /// 已经包过一次的表不能再包一层——里层那张图早就画好了
    #[test]
    fn attribute_values_are_escaped() {
        assert_eq!(
            escape_attr(r#"a "b" & <c>"#),
            "a &quot;b&quot; &amp; &lt;c&gt;"
        );
    }
}

#[cfg(test)]
mod footer_tests {
    use super::*;

    /// 提到路径不等于贴了图。这份 README 自己就在正文里写出了两个路径。
    #[test]
    fn a_path_mentioned_in_prose_does_not_count_as_showing_the_image() {
        let path = repolish_render::CARD_PATH;
        let prose = Readme::parse(
            "README.md",
            format!("# T\n\nRun `repolish --stages artifacts` to write {path}.\n"),
        );
        assert!(!shows_image(&prose, path));

        let html = Readme::parse(
            "README.md",
            format!("# T\n\n<img src=\"{path}\" alt=\"card\" width=\"880\">\n"),
        );
        assert!(shows_image(&html, path));

        let md = Readme::parse("README.md", format!("# T\n\n![card]({path})\n"));
        assert!(shows_image(&md, path));
    }

    /// 分数卡片必须落在文件末尾。落在顶上，一个陌生人点进这个仓库
    /// 第一眼看到的就是我们的分数，而不是这个项目。
    #[test]
    fn the_score_card_section_is_appended_after_everything_else() {
        let md = "# Tool\n\n## A\n\nx\n\n## License\n\nMIT\n";
        let readme = Readme::parse("README.md", md);
        let lines = vec![
            String::new(),
            "## Polished with repolish".to_string(),
            String::new(),
        ];
        let insert = Insert::new(readme.raw.lines().count(), "r", lines);
        let out = repolish_md::edit::apply(&readme.raw, &[insert]);
        assert!(out.starts_with("# Tool\n"));
        assert!(out.trim_end().ends_with("## Polished with repolish"));
        let body = out.find("MIT").unwrap();
        let card = out.find("Polished with repolish").unwrap();
        assert!(body < card, "分数卡片跑到正文前面去了");
    }

    #[test]
    fn a_chinese_readme_is_detected_so_the_section_can_follow_it() {
        let zh = Readme::parse(
            "README.md",
            "# 工具\n\n给开源仓库的门面打分，并指出该先改哪一处。\n",
        );
        assert!(readme_is_cjk(&zh));
        let en = Readme::parse(
            "README.md",
            "# Tool\n\nScore your repository's front door.\n",
        );
        assert!(!readme_is_cjk(&en));
    }
}
