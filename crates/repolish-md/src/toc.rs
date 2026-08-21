//! GitHub 的标题锚点算法。
//!
//! 目录里每一条链接都得真的跳得到，否则 `polish` 插进去的是一堆死链——
//! 比没有目录更糟。GitHub 用的是 github-slugger，规则是：
//!
//! 1. 首尾去空白
//! 2. 转小写
//! 3. **删掉**所有非字母数字、非 `-`、非 `_`、非空格的字符（标点、emoji 都在内）
//! 4. 空格换成 `-`
//!
//! 顺序不能调。`## 🚀 Install` 先删 emoji 再换空格，得到的是 `-install`
//! 而不是 `install`——开头那个连字符是真的，GitHub 上就长这样。
//!
//! CJK 会原样保留：`char::is_alphanumeric` 对汉字为真，GitHub 也不删它们。

/// 单个标题的锚点。重名不在这里处理，见 [`anchors`]。
pub fn anchor(title: &str) -> String {
    title
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// 一篇文档里所有标题的锚点，**按文档顺序**处理重名。
///
/// 重名的第二个起加 `-1`、`-2`。必须拿全文的标题来算：只算目录里要列的那几个，
/// 会漏掉正文里同名的标题，编号就错位了。
pub fn anchors<'a>(titles: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    titles
        .into_iter()
        .map(|t| {
            let base = anchor(t);
            let n = seen.entry(base.clone()).or_insert(0);
            let out = if *n == 0 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            *n += 1;
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_hyphenates() {
        assert_eq!(anchor("Quick start"), "quick-start");
    }

    #[test]
    fn punctuation_is_dropped_leaving_the_spaces_behind() {
        // `&` 被删掉，两侧的空格各自变成连字符——GitHub 上就是两个
        assert_eq!(anchor("Why & how"), "why--how");
        assert_eq!(anchor("What it checks?"), "what-it-checks");
    }

    #[test]
    fn emoji_are_dropped_before_spaces_become_hyphens() {
        // 顺序调过来就会得到 `install`，那个链接在 GitHub 上跳不到
        assert_eq!(anchor("🚀 Install"), "-install");
    }

    #[test]
    fn inline_code_keeps_its_hyphens() {
        // `## `--remote` flag` 的可见文本是 `--remote flag`
        assert_eq!(anchor("--remote flag"), "--remote-flag");
    }

    #[test]
    fn cjk_headings_keep_their_characters() {
        assert_eq!(anchor("快速开始"), "快速开始");
        assert_eq!(anchor("分数怎么来的"), "分数怎么来的");
    }

    #[test]
    fn duplicates_are_numbered_from_the_second_one() {
        let out = anchors(["Usage", "Notes", "Usage", "Usage"]);
        assert_eq!(out, vec!["usage", "notes", "usage-1", "usage-2"]);
    }

    #[test]
    fn underscores_and_digits_survive() {
        assert_eq!(anchor("v1_2 release"), "v1_2-release");
    }
}
