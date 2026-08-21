//! 文本层增量编辑：AST 只回答「插在第几行」，切开原文拼回去。
//!
//! 不走 `format_commonmark` 的原因见 crate 文档与 `examples/roundtrip.rs`——
//! 往返有损，12 个真实 README 上 0/12 通过。这里每一次编辑都只在行边界上
//! 拼接，原文其余字节逐字保留：制表符、CRLF、`*` 列表标记、引用式链接定义
//! 全都原样留下。

/// 在第 `after_line` 行之后插入若干行。
///
/// 行号 1-based；`after_line == 0` 表示插到文件最前面。各元素**不带行尾**，
/// 行尾由 [`apply`] 按锚点行的实际写法补——CRLF 的文件插进去的也是 CRLF。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insert {
    pub after_line: usize,
    pub lines: Vec<String>,
    /// 这次插入是为了修哪个检查项，用于 dry run 的输出与测试断言
    pub reason: String,
}

impl Insert {
    pub fn new(after_line: usize, reason: impl Into<String>, lines: Vec<String>) -> Self {
        Insert {
            after_line,
            lines,
            reason: reason.into(),
        }
    }
}

/// 按行切开并**保留各自的行尾**。最后一行可能没有换行符。
fn split_keep_eol(s: &str) -> Vec<&str> {
    s.split_inclusive('\n').collect()
}

/// 整篇文档主用的行尾。只要出现过一个 CRLF 就按 CRLF 走——
/// 混用行尾的文件里，插入哪一种都不算「破坏」，但跟着多数派走
/// 才不会让 `git diff` 出现整行替换。
pub fn dominant_eol(raw: &str) -> &'static str {
    let crlf = raw.matches("\r\n").count();
    if crlf > 0 && crlf * 2 >= raw.matches('\n').count() {
        "\r\n"
    } else {
        "\n"
    }
}

/// 应用所有插入，返回新文本。
///
/// 插入点按行号升序处理，同一行上的多次插入按给定顺序落下。
/// 越界的 `after_line` 夹到文件末尾，而不是 panic——编辑计划由检查结果
/// 推导而来，一个错算的行号不该让整个命令崩掉。
pub fn apply(raw: &str, inserts: &[Insert]) -> String {
    if inserts.is_empty() {
        return raw.to_string();
    }
    let src = split_keep_eol(raw);
    let eol = dominant_eol(raw);

    let mut ordered: Vec<&Insert> = inserts.iter().collect();
    ordered.sort_by_key(|i| i.after_line.min(src.len()));

    let mut out = String::with_capacity(raw.len() + 256);
    let mut next = 0usize;

    // i 表示「已经写出了前 i 行」。i == 0 时处理插到文件最前的情况。
    for i in 0..=src.len() {
        if i > 0 {
            let line = src[i - 1];
            out.push_str(line);
            // 文件末行可能没有换行符。要在它后面接东西，得先补一个。
            if i == src.len() && !line.ends_with('\n') && ordered.iter().any(|x| x.after_line >= i)
            {
                out.push_str(eol);
            }
        }
        while next < ordered.len() && ordered[next].after_line.min(src.len()) == i {
            for l in &ordered[next].lines {
                out.push_str(l);
                out.push_str(eol);
            }
            next += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ins(after: usize, lines: &[&str]) -> Insert {
        Insert::new(after, "test", lines.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn inserts_after_the_named_line() {
        let out = apply("a\nb\nc\n", &[ins(1, &["X"])]);
        assert_eq!(out, "a\nX\nb\nc\n");
    }

    #[test]
    fn line_zero_means_top_of_file() {
        let out = apply("a\nb\n", &[ins(0, &["X"])]);
        assert_eq!(out, "X\na\nb\n");
    }

    #[test]
    fn several_inserts_keep_their_own_anchors() {
        // 后一个插入的锚点不能被前一个插入挤走——按原文行号算，不是按结果行号。
        let out = apply("a\nb\nc\n", &[ins(2, &["Y"]), ins(1, &["X"])]);
        assert_eq!(out, "a\nX\nb\nY\nc\n");
    }

    #[test]
    fn crlf_files_get_crlf_back() {
        let out = apply("a\r\nb\r\n", &[ins(1, &["X"])]);
        assert_eq!(out, "a\r\nX\r\nb\r\n");
    }

    #[test]
    fn last_line_without_a_newline_gets_one_before_the_insert() {
        let out = apply("a\nb", &[ins(2, &["X"])]);
        assert_eq!(out, "a\nb\nX\n");
    }

    #[test]
    fn out_of_range_anchor_clamps_to_the_end() {
        let out = apply("a\n", &[ins(99, &["X"])]);
        assert_eq!(out, "a\nX\n");
    }

    #[test]
    fn no_inserts_is_byte_identical() {
        let raw = "a\r\n\tb\n*  c";
        assert_eq!(apply(raw, &[]), raw);
    }

    #[test]
    fn dominant_eol_follows_the_majority() {
        assert_eq!(dominant_eol("a\r\nb\r\n"), "\r\n");
        assert_eq!(dominant_eol("a\nb\n"), "\n");
        // 一行 CRLF、三行 LF：跟多数派，否则 diff 会变成整行替换
        assert_eq!(dominant_eol("a\r\nb\nc\nd\n"), "\n");
    }
}
