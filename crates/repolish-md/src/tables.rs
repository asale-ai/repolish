//! 找出 README 里的 GFM 管道表格，连同它占的行号。
//!
//! 为什么按行扫而不走 AST：调用方要的是**行区间**——`polish` 会在表格前后
//! 各插一段，把原表格原样折进 `<details>` 里。AST 给的是节点，节点还原到
//! 行号这一步比直接扫行更容易出错，而扫行在这里足够可靠：GFM 表格的判据
//! 本来就是逐行的（表头行 + 分隔行 + 若干数据行）。
//!
//! 围栏代码块里的管道符不算表格。一份讲 Markdown 的 README 里，
//! 代码块中演示的表格被当成真表格抠出来，是最难查的那种 bug。

/// 列的对齐方式，来自分隔行里的冒号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// 一张表在原文里的位置与内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// 表头行，1-based
    pub start_line: usize,
    /// 最后一行数据，1-based。含头含尾。
    pub end_line: usize,
    pub headers: Vec<String>,
    pub align: Vec<Align>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// 数据行数。空表（只有表头和分隔行）没有渲染的价值。
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn line_count(&self) -> usize {
        self.end_line - self.start_line + 1
    }
}

/// 扫出全部表格，按出现顺序。
pub fn find(raw: &str) -> Vec<Table> {
    let lines: Vec<&str> = raw.lines().collect();
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        // 围栏进出。``` 与 ~~~ 各算各的，一种围栏里出现另一种不算结束。
        if let Some(c) = fence_char(trimmed) {
            match fence {
                Some(open) if open == c => fence = None,
                None => fence = Some(c),
                _ => {}
            }
            i += 1;
            continue;
        }
        if fence.is_some() {
            i += 1;
            continue;
        }

        // 表头行的下一行必须是分隔行，这是 GFM 表格唯一可靠的判据
        if is_row(line) && lines.get(i + 1).is_some_and(|l| is_separator(l)) {
            let headers = cells(line);
            let align = alignment(lines[i + 1]);
            let mut rows = Vec::new();
            let mut j = i + 2;
            while j < lines.len() && is_row(lines[j]) {
                rows.push(cells(lines[j]));
                j += 1;
            }
            out.push(Table {
                start_line: i + 1,
                end_line: j,
                headers,
                align,
                rows,
            });
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

fn fence_char(trimmed: &str) -> Option<char> {
    ['`', '~']
        .into_iter()
        .find(|c| trimmed.starts_with(&c.to_string().repeat(3)))
}

/// 一行数据行。至少含一个管道符，且去掉两端管道后不为空。
///
/// 不要求以 `|` 开头：GFM 允许省略两端的管道，而真实 README 里两种写法
/// 都常见。
fn is_row(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && t.contains('|') && t.trim_matches('|').trim() != ""
}

/// 分隔行：每一格都只由 `-` 与两端可选的 `:` 组成，且至少三个字符里有一个 `-`。
fn is_separator(line: &str) -> bool {
    let t = line.trim();
    if !t.contains('|') || !t.contains('-') {
        return false;
    }
    split_cells(t).iter().all(|c| {
        let c = c.trim();
        !c.is_empty()
            && c.contains('-')
            && c.chars().all(|ch| ch == '-' || ch == ':')
            // `:-:` 合法，`::` 不合法：冒号只能在两端
            && c.trim_matches(':').chars().all(|ch| ch == '-')
    })
}

fn alignment(sep: &str) -> Vec<Align> {
    split_cells(sep.trim())
        .iter()
        .map(|c| {
            let c = c.trim();
            match (c.starts_with(':'), c.ends_with(':')) {
                (true, true) => Align::Center,
                (false, true) => Align::Right,
                _ => Align::Left,
            }
        })
        .collect()
}

fn cells(line: &str) -> Vec<String> {
    split_cells(line.trim())
        .into_iter()
        .map(|c| c.trim().to_string())
        .collect()
}

/// 按管道切格，两端的管道不算分隔符。
///
/// `\|` 是转义的管道，属于内容——一列里写 `a \| b` 是合法的，
/// 在这里断开会把一格劈成两格，后面每一列都跟着错位。
fn split_cells(line: &str) -> Vec<String> {
    let inner = line.strip_prefix('|').unwrap_or(line);
    let inner = inner.strip_suffix('|').unwrap_or(inner);

    let mut out = vec![String::new()];
    let mut escaped = false;
    for c in inner.chars() {
        if escaped {
            // 反斜杠本身不进内容，它只是那个管道的转义符
            if c != '|' {
                out.last_mut().expect("至少有一格").push('\\');
            }
            out.last_mut().expect("至少有一格").push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '|' => out.push(String::new()),
            _ => out.last_mut().expect("至少有一格").push(c),
        }
    }
    if escaped {
        out.last_mut().expect("至少有一格").push('\\');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_plain_table_with_its_line_range() {
        let md =
            "intro\n\n| Code | Meaning |\n|---|---|\n| 0 | Success |\n| 1 | Too low |\n\nafter\n";
        let tables = find(md);
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.start_line, 3);
        assert_eq!(t.end_line, 6);
        assert_eq!(t.headers, vec!["Code", "Meaning"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1], vec!["1", "Too low"]);
    }

    /// 一份讲 Markdown 的 README 里，代码块中演示的表格不是表格
    #[test]
    fn tables_inside_fenced_code_blocks_are_ignored() {
        let md = "```\n| a | b |\n|---|---|\n| 1 | 2 |\n```\n";
        assert!(find(md).is_empty());
        let md = "~~~markdown\n| a | b |\n|---|---|\n| 1 | 2 |\n~~~\n";
        assert!(find(md).is_empty());
    }

    /// 一种围栏里出现另一种不算结束
    #[test]
    fn a_tilde_fence_is_not_closed_by_a_backtick_fence() {
        let md = "~~~\n```\n| a | b |\n|---|---|\n| 1 | 2 |\n~~~\n";
        assert!(find(md).is_empty());
    }

    #[test]
    fn column_alignment_comes_from_the_separator_row() {
        let md = "| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n";
        assert_eq!(
            find(md)[0].align,
            vec![Align::Left, Align::Center, Align::Right]
        );
    }

    /// GFM 允许省略两端的管道，真实 README 里两种写法都常见
    #[test]
    fn tables_without_leading_and_trailing_pipes_are_found_too() {
        let md = "a | b\n--- | ---\n1 | 2\n";
        let t = &find(md)[0];
        assert_eq!(t.headers, vec!["a", "b"]);
        assert_eq!(t.rows, vec![vec!["1", "2"]]);
    }

    /// `\|` 是内容不是分隔符，断错一次后面每一列都跟着错位
    #[test]
    fn escaped_pipes_stay_inside_their_cell() {
        let md = "| a | b |\n|---|---|\n| x \\| y | z |\n";
        assert_eq!(find(md)[0].rows[0], vec!["x | y", "z"]);
    }

    #[test]
    fn a_row_of_pipes_without_a_separator_is_not_a_table() {
        assert!(find("| just | some | text |\n\nprose\n").is_empty());
        // 分隔行必须含 `-`
        assert!(find("| a | b |\n| : | : |\n| 1 | 2 |\n").is_empty());
    }

    #[test]
    fn several_tables_are_returned_in_document_order() {
        let md = "| a |\n|---|\n| 1 |\n\ntext\n\n| b |\n|---|\n| 2 |\n";
        let tables = find(md);
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].start_line, 1);
        assert_eq!(tables[1].start_line, 7);
        assert_eq!(tables[1].headers, vec!["b"]);
    }

    #[test]
    fn a_header_only_table_is_reported_as_empty() {
        let t = &find("| a | b |\n|---|---|\n\ntext\n")[0];
        assert!(t.is_empty());
        assert_eq!(t.line_count(), 2);
    }

    #[test]
    fn ragged_rows_are_kept_as_written_rather_than_padded() {
        // 补齐是渲染的事。这里少一格就是原文少一格，改了就对不上行号了。
        let t = &find("| a | b | c |\n|---|---|---|\n| 1 |\n")[0];
        assert_eq!(t.rows[0], vec!["1"]);
    }
}
