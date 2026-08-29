//! SVG 产物的文案表。
//!
//! **只有 SVG 走这里。** 终端报告与 `REPOLISH.md` 一律英文：那是给作者自己
//! 看的诊断输出，读它的人正在用一个英文 CLI。卡片不一样——卡片会被贴进
//! **别人的 README**，被这个项目的读者看到。一张中文 README 顶上写着
//! `LANGUAGES · BY FILE` 的卡片，是我们把自己的语言塞进了别人的门面。
//!
//! 用结构体而不是查表函数：少一个字段编译就过不去，翻译漏一条不可能溜进
//! 发布版。新增语言的成本是「把结构体填满」，这个成本正好。

/// 卡片文案的语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    ZhCn,
    Ja,
}

impl Lang {
    pub fn as_str(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::ZhCn => "zh-CN",
            Lang::Ja => "ja",
        }
    }

    /// BCP 47 标签，写进 SVG 根元素的 `lang` 属性——读屏软件据此换发音。
    pub fn tag(self) -> &'static str {
        self.as_str()
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "en" | "en-us" | "en-gb" => Some(Lang::En),
            "zh" | "zh-cn" | "zh-hans" | "zh-sg" => Some(Lang::ZhCn),
            "ja" | "ja-jp" | "jp" => Some(Lang::Ja),
            _ => None,
        }
    }

    /// 卡片要跟着 README 的语言走。CJK 字符过半即判中文——
    /// 一份中文 README 里夹着英文命令名是常态，按「有没有中文」判会
    /// 把几乎所有 README 都判成中文。
    /// 卡片要跟着 README 的语言走。
    ///
    /// **日文与中文共用汉字，所以汉字比例分不开这两者。** 分开它们的是假名：
    /// 平假名和片假名只出现在日文里，一份日文 README 不可能一个都没有。所以
    /// 先看假名，再看汉字比例——顺序不能反，反过来会把所有日文判成中文。
    ///
    /// 汉字那条线是三分之一而不是过半：一个汉字承载的信息量远大于一个拉丁
    /// 字母，等量对比会永远判成英文；而「有没有汉字」又太松，一份英文 README
    /// 里放一个「中文」链接就会被判成中文。
    /// 从译本的**文件名**认语言，例如 `README.zh-CN.md`。
    ///
    /// 比看内容可靠得多:一份短译本可能通篇是代码块和 ASCII 标题，只夹着
    /// 一两句本地语言，[`detect`](Lang::detect) 会把它判成英文。而文件名是
    /// 作者明写的声明。认不出来的语言返回 `None`——我们只有这三种的文案，
    /// 硬塞一种进去就是在一份法语 README 里写中文。
    pub fn from_filename(path: &str) -> Option<Lang> {
        let f = path.rsplit(['/', '\\']).next()?.to_lowercase();
        let stem = f.strip_suffix(".md")?;
        let code = stem.rsplit(['.', '-', '_']).next()?;
        // `README.zh-CN.md` 的最后一段是 `cn`，所以整段也要看一眼
        let full = stem.strip_prefix("readme")?.trim_matches(['.', '-', '_']);
        match (full, code) {
            ("zh-cn" | "zh" | "zh-hans" | "cn", _) => Some(Lang::ZhCn),
            (_, "cn" | "zh") => Some(Lang::ZhCn),
            ("ja" | "jp", _) | (_, "ja" | "jp") => Some(Lang::Ja),
            ("en", _) => Some(Lang::En),
            _ => None,
        }
    }

    pub fn detect(readme: &str) -> Lang {
        let mut cjk = 0usize;
        let mut kana = 0usize;
        let mut letters = 0usize;
        for c in readme.chars() {
            if is_kana(c) {
                kana += 1;
                letters += 1;
            } else if is_cjk(c) {
                cjk += 1;
                letters += 1;
            } else if c.is_alphabetic() {
                letters += 1;
            }
        }
        if letters == 0 {
            return Lang::En;
        }
        // 假名的门槛比汉字低得多：日文里假名本来就密集，出现几个孤零零的
        // 片假名（英文 README 里引用一个日文名字）不该翻盘，但只要成段的
        // 日文出现，这个比例立刻就过了。
        if kana * 20 > letters {
            return Lang::Ja;
        }
        if (cjk + kana) * 3 > letters {
            return Lang::ZhCn;
        }
        Lang::En
    }

    /// 这个语言认哪些 README 文件名后缀（`README.zh-CN.md` 里的 `zh-cn`）。
    ///
    /// 与 `readme-i18n` 检查项用的是同一批约定；那边负责发现译本，
    /// 这边负责在译本里挑对应的一份。
    pub fn matches_code(self, code: &str) -> bool {
        let code = code.to_lowercase();
        match self {
            Lang::En => matches!(code.as_str(), "en" | "en-us" | "en-gb"),
            Lang::ZhCn => matches!(code.as_str(), "zh" | "zh-cn" | "zh-hans" | "cn" | "zh-sg"),
            Lang::Ja => matches!(code.as_str(), "ja" | "ja-jp" | "jp"),
        }
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::En => &EN,
            Lang::ZhCn => &ZH_CN,
            Lang::Ja => &JA,
        }
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

/// 平假名与片假名。汉字是中日共有的，假名不是——它是区分两者的那条线。
fn is_kana(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF)
}

/// 卡片上出现的每一个字。
///
/// 全部是**标签**，不是句子：卡片宽度固定，一句会随语言长短伸缩的话
/// 迟早在某种语言下溢出边框。
pub struct Strings {
    // ── 概览卡片 ──
    pub overview_title: &'static str,
    pub profile: &'static str,
    pub languages: &'static str,
    pub by_file: &'static str,
    pub composition: &'static str,
    pub activity: &'static str,
    /// star 曲线的分区标题
    pub star_history: &'static str,
    /// 曲线跨度说不清时的兜底说明
    pub stars_since: &'static str,
    pub weeks: &'static str,
    pub files: &'static str,
    pub kinds: &'static str,
    /// 计数用的「语言」，与分区标题 `languages` 分开：
    /// 中文里标题是「语言构成」，而计数处要的是「门语言」
    pub languages_unit: &'static str,
    /// 并起来的那一行。中英的语序不同——英文是 `+3 more`，中文是
    /// 「其他 3 项」——所以拆成前后两截，而不是一个带 `{n}` 的模板：
    /// 模板要么得引一个格式化库，要么得在这里手写替换。
    pub more_prefix: &'static str,
    pub more_suffix: &'static str,
    /// 活跃度图横轴左端：`-52w` / `52 周前`
    pub weeks_ago_prefix: &'static str,
    pub weeks_ago_suffix: &'static str,
    pub commits: &'static str,
    pub tags: &'static str,
    pub stars: &'static str,
    pub topics: &'static str,
    pub license: &'static str,
    pub last_commit: &'static str,
    pub days_ago: &'static str,
    pub today: &'static str,
    pub none: &'static str,
    pub shallow_note: &'static str,
    pub no_history: &'static str,
    pub peak: &'static str,
    pub kind_code: &'static str,
    pub kind_docs: &'static str,
    pub kind_config: &'static str,
    pub kind_other: &'static str,

    // ── 三大类 ──
    // `Category::label()` 在 core 里，返回的是英文——那是给终端报告和 JSON
    // 用的，本来就该是英文。卡片贴进别人的 README，得跟着那份 README 的语言。
    pub cat_discoverability: &'static str,
    pub cat_comprehensibility: &'static str,
    pub cat_credibility: &'static str,

    // ── 分数卡片 ──
    pub score: &'static str,
    pub not_scored: &'static str,
    pub checks: &'static str,
    pub to_fix: &'static str,
    pub scored: &'static str,
    pub not_verified: &'static str,
    pub not_applicable: &'static str,
    pub more_findings: &'static str,
    pub band_excellent: &'static str,
    pub band_good: &'static str,
    pub band_fair: &'static str,
    pub band_weak: &'static str,
    pub band_poor: &'static str,

    // ── 页脚 ──
    pub generated_by: &'static str,
    pub deterministic: &'static str,
}

pub const EN: Strings = Strings {
    overview_title: "REPOSITORY",
    profile: "PROFILE",
    languages: "LANGUAGES",
    by_file: "BY FILE",
    composition: "COMPOSITION",
    activity: "COMMIT ACTIVITY",
    star_history: "STARS",
    stars_since: "since",
    weeks: "52w",
    files: "files",
    kinds: "kinds",
    languages_unit: "languages",
    more_prefix: "+",
    more_suffix: " more",
    weeks_ago_prefix: "-",
    weeks_ago_suffix: "w",
    commits: "commits",
    tags: "tags",
    stars: "stars",
    topics: "topics",
    license: "license",
    last_commit: "last commit",
    days_ago: "d ago",
    today: "today",
    none: "none",
    shallow_note: "shallow clone — history is partial",
    no_history: "no commit history available",
    peak: "PEAK",
    kind_code: "code",
    kind_docs: "docs",
    kind_config: "config",
    kind_other: "other",

    cat_discoverability: "Discoverability",
    cat_comprehensibility: "Comprehensibility",
    cat_credibility: "Credibility",

    score: "SCORE",
    not_scored: "not scored",
    checks: "CHECKS",
    to_fix: "TO FIX",
    scored: "scored",
    not_verified: "not verified",
    not_applicable: "not applicable",
    more_findings: "more — run repolish -v",
    band_excellent: "excellent",
    band_good: "good",
    band_fair: "fair",
    band_weak: "weak",
    band_poor: "poor",

    generated_by: "generated by",
    deterministic: "Scoring is deterministic. No model is involved.",
};

pub const ZH_CN: Strings = Strings {
    overview_title: "仓库概览",
    profile: "类型",
    languages: "语言构成",
    by_file: "按文件数",
    composition: "文件用途",
    activity: "提交活跃度",
    star_history: "star 增长",
    stars_since: "自",
    weeks: "52 周",
    files: "个文件",
    kinds: "类",
    languages_unit: "门语言",
    more_prefix: "其他 ",
    more_suffix: " 项",
    weeks_ago_prefix: "",
    weeks_ago_suffix: " 周前",
    commits: "次提交",
    tags: "个标签",
    stars: "星标",
    topics: "个主题",
    license: "许可证",
    last_commit: "最近提交",
    days_ago: " 天前",
    today: "今天",
    none: "无",
    shallow_note: "浅克隆——历史不完整",
    no_history: "读不到提交历史",
    peak: "峰值",
    kind_code: "代码",
    kind_docs: "文档",
    kind_config: "配置",
    kind_other: "其他",

    cat_discoverability: "可发现性",
    cat_comprehensibility: "可理解性",
    cat_credibility: "可信度",

    score: "得分",
    not_scored: "未评分",
    checks: "检查项",
    to_fix: "待修复",
    scored: "项已评分",
    not_verified: "项未验证",
    not_applicable: "项不适用",
    more_findings: "条未列出——运行 repolish -v",
    band_excellent: "优秀",
    band_good: "良好",
    band_fair: "一般",
    band_weak: "偏弱",
    band_poor: "较差",

    generated_by: "生成于",
    deterministic: "评分过程确定，不涉及任何模型。",
};

pub const JA: Strings = Strings {
    overview_title: "リポジトリ概要",
    profile: "種別",
    languages: "言語構成",
    by_file: "ファイル数",
    composition: "ファイル種別",
    activity: "コミット頻度",
    star_history: "スター推移",
    stars_since: "以来",
    weeks: "52週",
    files: "ファイル",
    kinds: "種類",
    languages_unit: "言語",
    // 日本語は数を先に置く：「他 3 件」ではなく「他3件」が自然
    more_prefix: "他",
    more_suffix: "件",
    weeks_ago_prefix: "",
    weeks_ago_suffix: "週前",
    commits: "コミット",
    tags: "タグ",
    stars: "スター",
    topics: "トピック",
    license: "ライセンス",
    last_commit: "最終コミット",
    days_ago: "日前",
    today: "本日",
    none: "なし",
    shallow_note: "浅いクローン — 履歴は不完全です",
    no_history: "コミット履歴を読み取れません",
    peak: "ピーク",
    kind_code: "コード",
    kind_docs: "ドキュメント",
    kind_config: "設定",
    kind_other: "その他",

    cat_discoverability: "見つけやすさ",
    cat_comprehensibility: "分かりやすさ",
    cat_credibility: "信頼性",

    score: "スコア",
    not_scored: "スコアなし",
    checks: "検査項目",
    to_fix: "要修正",
    scored: "項目を評価",
    not_verified: "項目が未検証",
    not_applicable: "項目が対象外",
    more_findings: "件は非表示 — repolish -v を実行",
    band_excellent: "優秀",
    band_good: "良好",
    band_fair: "普通",
    band_weak: "やや弱い",
    band_poor: "不十分",

    generated_by: "生成",
    deterministic: "スコアリングは決定的です。モデルは介在しません。",
};

/// 三大类的名字，跟着语言走。
///
/// 与 `Category::label()` 分开：那一个服务终端报告和 JSON schema，必须一直是
/// 英文（JSON 的字段值是对外契约）；这一个只服务卡片。
pub fn category_label(cat: repolish_core::Category, s: &'static Strings) -> &'static str {
    use repolish_core::Category::*;
    match cat {
        Discoverability => s.cat_discoverability,
        Comprehensibility => s.cat_comprehensibility,
        Credibility => s.cat_credibility,
    }
}

/// 分数 → 那个词，跟着语言走。
///
/// 分档不在这里判：走 `repolish_core::band_index`，与徽章、配色是同一个判断。
/// 一个卡片上写着 `good` 而条形图是 `fair` 的颜色，读者会以为工具坏了。
pub fn band_word(score: u8, s: &'static Strings) -> &'static str {
    [
        s.band_excellent,
        s.band_good,
        s.band_fair,
        s.band_weak,
        s.band_poor,
    ][repolish_core::band_index(score)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一份短译本可能通篇是代码块和 ASCII 标题，只夹着一两句本地语言——
    /// 内容检测会把它判成英文。文件名是作者明写的声明，所以它排在前面。
    #[test]
    fn the_filename_names_the_language_when_the_content_is_too_thin() {
        assert_eq!(Lang::from_filename("README.zh-CN.md"), Some(Lang::ZhCn));
        assert_eq!(Lang::from_filename("docs/README.ja.md"), Some(Lang::Ja));
        assert_eq!(Lang::from_filename("README.md"), None);
        // 只有这三种语言有文案。给一份法语 README 塞中文，比留着英文更糟。
        assert_eq!(Lang::from_filename("README.fr.md"), None);
        assert_eq!(Lang::from_filename("README.es.md"), None);
    }

    #[test]
    fn language_tags_round_trip() {
        assert_eq!(Lang::parse("zh-CN"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse("zh_cn"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse("EN"), Some(Lang::En));
        assert_eq!(Lang::parse("fr"), None);
    }

    /// 中英混排的 README 是常态：命令名、包名、代码块全是英文
    #[test]
    fn a_chinese_readme_with_english_commands_is_still_chinese() {
        let md = "# 工具\n\n这是一个用来给仓库打分的命令行工具。\n\n```bash\ncargo install repolish\nrepolish check .\n```\n";
        assert_eq!(Lang::detect(md), Lang::ZhCn);
    }

    #[test]
    fn an_english_readme_with_a_chinese_link_stays_english() {
        let md = "# Tool\n\nScore and improve what an open-source repository looks like to a \
                  first-time visitor, from the command line.\n\n[中文](README.zh-CN.md)\n";
        assert_eq!(Lang::detect(md), Lang::En);
    }

    /// 文案表的意义就在于「少一条编译不过」。但常量表本身漏一条，编译器
    /// 是发现不了的——字段填满了，值却是从别的语言抄来的。
    #[test]
    fn nothing_is_left_untranslated() {
        // 只列那些每种语言下必然不同的字段。像 "52週" 里的数字、
        // `more_prefix` 这类符号型的不在此列。
        /// 从一张表里取一个字段。三张表并排比对，靠它遍历字段。
        type Field = (&'static str, fn(&Strings) -> &'static str);
        let fields: &[Field] = &[
            ("overview_title", |s| s.overview_title),
            ("profile", |s| s.profile),
            ("languages", |s| s.languages),
            ("composition", |s| s.composition),
            ("activity", |s| s.activity),
            ("files", |s| s.files),
            ("commits", |s| s.commits),
            ("license", |s| s.license),
            ("score", |s| s.score),
            ("checks", |s| s.checks),
            ("to_fix", |s| s.to_fix),
            ("band_excellent", |s| s.band_excellent),
            ("band_poor", |s| s.band_poor),
            ("generated_by", |s| s.generated_by),
            ("cat_discoverability", |s| s.cat_discoverability),
            ("cat_comprehensibility", |s| s.cat_comprehensibility),
            ("cat_credibility", |s| s.cat_credibility),
        ];
        let tables = [("EN", &EN), ("ZH_CN", &ZH_CN), ("JA", &JA)];
        for (name, get) in fields {
            for (i, (an, a)) in tables.iter().enumerate() {
                assert!(!get(a).is_empty(), "{an} 的 `{name}` 是空的");
                for (bn, b) in tables.iter().skip(i + 1) {
                    assert_ne!(
                        get(a),
                        get(b),
                        "`{name}` 在 {an} 和 {bn} 里一模一样，八成是漏翻了"
                    );
                }
            }
        }
    }

    /// 每种语言都要能从 `Lang` 走到自己的表，漏一个就会静默回退到英文
    #[test]
    fn every_language_reaches_its_own_table() {
        assert_eq!(Lang::En.strings().score, EN.score);
        assert_eq!(Lang::ZhCn.strings().score, ZH_CN.score);
        assert_eq!(Lang::Ja.strings().score, JA.score);
        for lang in [Lang::En, Lang::ZhCn, Lang::Ja] {
            assert_eq!(
                Lang::parse(lang.as_str()),
                Some(lang),
                "{lang:?} 解析不回自己"
            );
            assert!(
                lang.matches_code(lang.as_str()),
                "{lang:?} 认不出自己的语言码"
            );
        }
    }

    /// 三大类的名字曾经直接取 `Category::label()`——那是英文的，
    /// 结果中文卡片上写着 DISCOVERABILITY。
    #[test]
    fn category_names_follow_the_card_language() {
        use repolish_core::Category;
        for cat in Category::ALL {
            let en = category_label(cat, &EN);
            let zh = category_label(cat, &ZH_CN);
            assert_eq!(en, cat.label(), "英文卡片该和 Category::label() 一致");
            assert_ne!(en, zh, "{cat:?} 的中文名没翻");
            assert!(
                zh.chars().any(|c| ('一'..='鿿').contains(&c)),
                "{cat:?}: {zh}"
            );
        }
    }

    /// 挑译本靠的是文件名里的语言码，与 readme-i18n 检查项同一批约定
    #[test]
    fn language_codes_map_to_the_right_readme() {
        assert!(Lang::ZhCn.matches_code("zh-CN"));
        assert!(Lang::ZhCn.matches_code("zh"));
        assert!(Lang::ZhCn.matches_code("zh-hans"));
        assert!(!Lang::ZhCn.matches_code("zh-tw"), "繁体不该当成简体");
        assert!(!Lang::ZhCn.matches_code("ja"));
        assert!(Lang::En.matches_code("en"));
        assert!(!Lang::En.matches_code("zh-cn"));
    }

    /// 日文与中文共用汉字。分开它们的是假名——这几条是这个判据的全部意义。
    #[test]
    fn japanese_is_told_apart_from_chinese_by_the_kana() {
        let ja = "# ツール\n\nリポジトリを診断して、どこを直せばいいかを教えるコマンドラインツールです。\n\n                  ```bash\ncargo install repolish\nrepolish check .\n```\n";
        assert_eq!(Lang::detect(ja), Lang::Ja);

        // 汉字だけの日本語（見出しなど）は中国語と区別できない——それは受け入れる。
        // 実際の README には必ず仮名が出てくる。
        let zh = "# 工具\n\n这是一个用来给仓库打分的命令行工具，指出该先改哪里。\n";
        assert_eq!(Lang::detect(zh), Lang::ZhCn);

        // 英文 README 里引用一个片假名的名字，不该翻盘
        let en = "# Tool\n\nScore and improve what an open-source repository looks like to a \
                  first-time visitor. Originally inspired by a talk at カンファレンス.\n\n\
                  Install it with cargo, then run it against any repository you maintain.\n";
        assert_eq!(Lang::detect(en), Lang::En);
    }

    #[test]
    fn japanese_readme_filenames_are_recognised() {
        assert!(Lang::Ja.matches_code("ja"));
        assert!(Lang::Ja.matches_code("ja-JP"));
        assert!(Lang::Ja.matches_code("jp"));
        assert!(!Lang::Ja.matches_code("zh-cn"));
        assert!(!Lang::ZhCn.matches_code("ja"));
        assert_eq!(Lang::parse("ja"), Some(Lang::Ja));
        assert_eq!(Lang::parse("ja-JP"), Some(Lang::Ja));
    }

    #[test]
    fn an_empty_readme_falls_back_to_english() {
        assert_eq!(Lang::detect(""), Lang::En);
    }
}
