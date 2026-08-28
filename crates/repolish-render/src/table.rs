//! Markdown 表格 → SVG。
//!
//! README 里的表格在 GitHub 上渲染成什么样，取决于读者的主题、字号和窗口
//! 宽度，而且**在别处一律不渲染**：crates.io 的包页面、npm 的说明页、
//! 各种 README 聚合站，管道符原样露出来的比比皆是。一张图在哪儿都是同一张图。
//!
//! 这里只负责**画**。原表格由 `polish` 折进 `<details>` 里原样保留——
//! 图片没有文本层，读屏软件、`grep`、翻译工具全都读不到它，
//! 所以原文必须留着。这条不是可选项。
//!
//! 约束与其余 SVG 产物一致：自包含、确定性、色板恒定。

use crate::draw::{self, Anchor, Options};
use crate::theme::Palette;

const PAD: i32 = 28;
/// 单元格内文最多折几行。再多说明这一格本来就不该塞进表格里。
const MAX_LINES: usize = 4;
const ROW_PAD: i32 = 11;
const LINE_H: i32 = 18;
const FONT_SIZE: i32 = 13;
/// 列与列之间的留白
const COL_GAP: i32 = 20;
/// 一列再窄也不能窄过这个值，否则整列都是省略号。
/// 定得低是有意的：状态列只放一个 `✓`，凭什么占和说明列一样的宽度。
const MIN_COL: i32 = 30;
/// 列宽在内容之外多留的一点余量，免得字紧贴着下一列
const COL_SLACK: f32 = 8.0;

/// 列的对齐方式，来自 GFM 分隔行里的冒号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// 一张待渲染的表。
#[derive(Debug, Clone, Default)]
pub struct Table {
    /// 表上方的小标题，通常取自表格所在小节的标题
    pub title: Option<String>,
    pub headers: Vec<String>,
    pub align: Vec<Align>,
    pub rows: Vec<Vec<String>>,
    /// 表下方的一行注脚
    pub caption: Option<String>,
    /// 整图宽度。跟着 README 里其他卡片走，默认与概览卡片同宽。
    pub width: i32,
}

impl Table {
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Table {
        Table {
            align: vec![Align::Left; headers.len()],
            headers,
            rows,
            width: 880,
            ..Default::default()
        }
    }

    fn col_align(&self, i: usize) -> Align {
        self.align.get(i).copied().unwrap_or_default()
    }
}

/// 单元格里的一个「值」。
///
/// 判类型是为了让表看起来像**数据**而不是一段排版过的文字：数字右对齐、
/// 状态画成符号、`code` 有底色。全部按文本画的话，SVG 相对原表格就毫无
/// 增益，那还不如不画。
enum Cell {
    /// ✅ / ✓ —— 成立
    Yes,
    /// ❌ / ✗ —— 不成立
    No,
    /// ⏳ / 🚧 —— 在路上
    Pending,
    /// 纯数字，右对齐
    Number(String),
    /// `反引号` 包住的整格，画成带底的小块
    Code(String),
    Text(String),
}

fn classify(raw: &str) -> Cell {
    let s = raw.trim();
    match s {
        "✅" | "✔" | "✓" | "yes" | "Yes" => return Cell::Yes,
        "❌" | "✖" | "✗" | "no" | "No" => return Cell::No,
        "⏳" | "🚧" | "🔜" => return Cell::Pending,
        _ => {}
    }
    if s.len() >= 2 && s.starts_with('`') && s.ends_with('`') && !s[1..s.len() - 1].contains('`') {
        return Cell::Code(s[1..s.len() - 1].to_string());
    }
    let plain = inline_text(s);
    if !plain.is_empty()
        && plain
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '%' | '+' | '-' | '/'))
    {
        return Cell::Number(plain);
    }
    Cell::Text(plain)
}

/// 去掉 Markdown 的行内标记，留下人读的那部分。
///
/// 链接只留文字：SVG 里点不动，留一段 URL 只会把列撑宽。原表格折在下面，
/// 需要点链接的人在那里点得到——这也是必须保留原表格的理由之一。
pub fn inline_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            // [text](url) / ![alt](url) → text
            // 紧跟 (...) 或 [...] 的才是链接。不跟的那个方括号是正文的一部分——
            // `[not a link]` 被吃掉括号之后，读者会以为作者本来就那么写的
            '[' => {
                let link = find(&chars, i + 1, ']').and_then(|close| {
                    let end = match chars.get(close + 1) {
                        Some('(') => find(&chars, close + 2, ')'),
                        Some('[') => find(&chars, close + 2, ']'),
                        _ => None,
                    }?;
                    Some((close, end + 1))
                });
                match link {
                    Some((close, next)) => {
                        let text: String = chars[i + 1..close].iter().collect();
                        out.push_str(&inline_text(&text));
                        i = next;
                    }
                    None => {
                        out.push('[');
                        i += 1;
                    }
                }
            }
            '!' if chars.get(i + 1) == Some(&'[') => i += 1,
            '*' | '_' | '`' => i += 1,
            '<' => {
                // 行内 HTML（`<br>`、`<sup>`）整段丢掉，标签名不是内容
                match find(&chars, i + 1, '>') {
                    Some(end) => {
                        // <br> 是作者写的换行，得留下来
                        let tag: String = chars[i + 1..end].iter().collect();
                        if tag.trim_end_matches('/').trim().eq_ignore_ascii_case("br") {
                            out.push(' ');
                        }
                        i = end + 1;
                    }
                    None => {
                        out.push('<');
                        i += 1;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find(chars: &[char], from: usize, target: char) -> Option<usize> {
    (from..chars.len()).find(|&i| chars[i] == target)
}

// ── 布局 ────────────────────────────────────────────────────

/// 列宽。**先给每列它实际需要的宽度，多出来的空间才分给最长的那一列。**
///
/// 按内容长度成比例分配是错的：一个只放 `0`..`5` 的「退出码」列会因为
/// 「说明」列很长而被拉到两百像素宽，数字孤零零地飘在中间。开方分配好一点，
/// 但仍然会把短列撑得过宽。
///
/// 只有在装不下的时候才谈分配——那时从最宽的一列上一点点扣，扣到 `MIN_COL`
/// 为止。窄列永远不会被长列吃掉，因为它压根没参与分配。
fn columns(t: &Table, inner: i32) -> Vec<i32> {
    let n = t
        .headers
        .len()
        .max(t.rows.iter().map(Vec::len).max().unwrap_or(0));
    if n == 0 {
        return Vec::new();
    }

    let gaps = COL_GAP * (n as i32 - 1);
    let usable = (inner - gaps).max(n as i32 * MIN_COL);

    // 每列真正需要多宽
    let natural: Vec<i32> = (0..n)
        .map(|i| {
            let head = t
                .headers
                .get(i)
                .map(|h| draw::width_px(&inline_text(h), 11.0))
                .unwrap_or(0.0);
            let body = t
                .rows
                .iter()
                .filter_map(|r| r.get(i))
                .map(|c| natural_width(c))
                .fold(0.0f32, f32::max);
            ((head.max(body) + COL_SLACK).round() as i32).clamp(MIN_COL, usable)
        })
        .collect();

    let total: i32 = natural.iter().sum();
    let mut widths = natural.clone();

    if total <= usable {
        // 富余全给最宽的那一列。它是承载散文的那一列，多出来的每一像素
        // 都换成少折一行；给短列多留白只是浪费。
        if let Some(i) = widest(&natural) {
            widths[i] += usable - total;
        }
        return widths;
    }

    // 装不下：从最宽的一列上扣，一次扣到与第二宽持平为止
    let mut over = total - usable;
    while over > 0 {
        let Some(i) = widest(&widths) else { break };
        if widths[i] <= MIN_COL {
            break;
        }
        let second = widths
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, w)| *w)
            .max()
            .unwrap_or(MIN_COL);
        let step = (widths[i] - second.max(MIN_COL)).max(1).min(over);
        widths[i] -= step;
        over -= step;
    }
    widths
}

/// 一格「不折行的话要多宽」。
///
/// 符号与数字按它们实际画出来的字号算，不按 13 号正文算——一个 `✓` 撑不出
/// 一列的宽度，也不该撑出。
fn natural_width(raw: &str) -> f32 {
    match classify(raw) {
        Cell::Yes | Cell::No | Cell::Pending => draw::width_px("✓", 15.0),
        Cell::Number(n) => draw::width_px(&n, FONT_SIZE as f32),
        Cell::Code(c) => draw::width_px(&c, 12.0) + 12.0,
        Cell::Text(s) => draw::width_px(&s, FONT_SIZE as f32),
    }
}

/// 整列都是数字或状态符号吗。是的话表头也跟着右对齐 / 居中——
/// 一个左对齐的表头顶着一列右对齐的数字，看着像两件不相干的东西。
fn column_align(t: &Table, i: usize) -> Align {
    let mut seen = false;
    for row in &t.rows {
        let Some(cell) = row.get(i) else { continue };
        if cell.trim().is_empty() {
            continue;
        }
        seen = true;
        match classify(cell) {
            Cell::Number(_) | Cell::Yes | Cell::No | Cell::Pending => {}
            _ => return t.col_align(i),
        }
    }
    if seen {
        // 数字右对齐才能靠位数直接比大小，这是表格相对文字的全部优势
        Align::Right
    } else {
        t.col_align(i)
    }
}

fn widest(widths: &[i32]) -> Option<usize> {
    widths
        .iter()
        .enumerate()
        .max_by_key(|(_, w)| **w)
        .map(|(i, _)| i)
}

/// 按词折行到给定像素宽。单词本身超宽时截断——
/// 一个 80 字符的 URL 不折断就会横穿整张图。
fn wrap(s: &str, max_px: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if draw::width_px(&candidate, FONT_SIZE as f32) <= max_px {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if draw::width_px(word, FONT_SIZE as f32) > max_px {
            lines.push(draw::fit(word, max_px, FONT_SIZE as f32));
        } else {
            line = word.to_string();
        }
        if lines.len() >= MAX_LINES {
            break;
        }
    }
    if !line.is_empty() && lines.len() < MAX_LINES {
        lines.push(line);
    }
    // 折到上限还没完，最后一行加省略号，别让人以为这就是全部
    if lines.len() >= MAX_LINES {
        lines.truncate(MAX_LINES);
        if let Some(last) = lines.last_mut() {
            if !last.ends_with('…') {
                last.push('…');
            }
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ── 渲染 ────────────────────────────────────────────────────

pub fn table(t: &Table, opts: &Options) -> String {
    let p = opts.palette;
    let w = t.width.max(320);
    let inner = w - PAD * 2;
    let widths = columns(t, inner);
    if widths.is_empty() {
        return draw::document("", w, 80, p, opts.lang.tag(), "empty table");
    }

    let mut body = String::new();
    let mut y = PAD;

    if let Some(title) = &t.title {
        body.push_str(&draw::label(
            &draw::fit(&title.to_uppercase(), inner as f32, 11.0),
            PAD,
            y + 10,
            p.muted,
            Anchor::Start,
        ));
        y += 26;
    }

    // 表头
    y += 14;
    // 一排全空的表头（`| | |`）只会留下一条无字的横线和一片空白。
    // 不画，把那段高度还给内容。
    let has_headers = t.headers.iter().any(|h| !h.trim().is_empty());
    if has_headers {
        let mut x = PAD;
        for (i, head) in t.headers.iter().enumerate() {
            let cw = widths.get(i).copied().unwrap_or(MIN_COL);
            let text = draw::fit(&inline_text(head).to_uppercase(), cw as f32, 11.0);
            let (tx, anchor) = anchor_for(column_align(t, i), x, cw);
            body.push_str(&draw::label(&text, tx, y, p.muted, anchor));
            x += cw + COL_GAP;
        }
        y += 10;
    } else {
        y -= 8;
    }
    body.push_str(&draw::hline(PAD, y, inner, p.line));

    // 行
    for (r, row) in t.rows.iter().enumerate() {
        let cells: Vec<(Cell, Vec<String>)> = (0..widths.len())
            .map(|i| {
                let raw = row.get(i).map(String::as_str).unwrap_or("");
                let cell = classify(raw);
                let lines = match &cell {
                    Cell::Text(s) => wrap(s, widths[i] as f32),
                    _ => vec![String::new()],
                };
                (cell, lines)
            })
            .collect();
        let tall = cells.iter().map(|(_, l)| l.len()).max().unwrap_or(1);
        let height = ROW_PAD * 2 + tall as i32 * LINE_H - (LINE_H - FONT_SIZE);

        // 隔行底色。斑马纹是长表里唯一有效的横向引导——
        // 没有它，读到第七行时眼睛已经串行了
        if r % 2 == 1 {
            body.push_str(&draw::rect(PAD - 8, y, inner + 16, height, 5.0, p.panel));
        }

        let mut x = PAD;
        let baseline = y + ROW_PAD + FONT_SIZE;
        for (i, (cell, lines)) in cells.iter().enumerate() {
            let cw = widths[i];
            let align = column_align(t, i);
            body.push_str(&cell_svg(cell, lines, x, baseline, cw, align, p));
            x += cw + COL_GAP;
        }
        y += height;
    }

    body.push_str(&draw::hline(PAD, y, inner, p.line));
    y += 22;

    if let Some(c) = &t.caption {
        body.push_str(&draw::text(
            &draw::fit(&inline_text(c), inner as f32, 12.0),
            PAD,
            y,
            12,
            p.muted,
            Anchor::Start,
            false,
        ));
        y += 16;
    }

    let aria = match &t.title {
        Some(title) => format!("{title} — table"),
        None => "table".to_string(),
    };
    draw::document(&body, w, y + PAD - 12, p, opts.lang.tag(), &aria)
}

fn anchor_for(align: Align, x: i32, cw: i32) -> (i32, Anchor) {
    match align {
        Align::Left => (x, Anchor::Start),
        Align::Center => (x + cw / 2, Anchor::Middle),
        Align::Right => (x + cw, Anchor::End),
    }
}

fn cell_svg(
    cell: &Cell,
    lines: &[String],
    x: i32,
    baseline: i32,
    cw: i32,
    align: Align,
    p: &Palette,
) -> String {
    match cell {
        // 锚点由 `symbol` 自己算一次。这里再算一遍会把偏移叠两次，
        // 符号就会跑到下一列的字上面去。
        Cell::Yes => symbol("✓", x, baseline, align, cw, p.bands[0]),
        Cell::No => symbol("✗", x, baseline, align, cw, p.bad),
        Cell::Pending => symbol("◔", x, baseline, align, cw, p.warn),
        // 数字一律右对齐，不管列的对齐方式怎么写：一列右对齐的数字
        // 才能靠位数直接比大小，这是表格相对文字的全部优势
        Cell::Number(n) => draw::text(n, x + cw, baseline, FONT_SIZE, p.text, Anchor::End, true),
        Cell::Code(code) => {
            let text = draw::fit(code, cw as f32 - 12.0, 12.0);
            let w = draw::width_px(&text, 12.0).round() as i32 + 12;
            let mut out = draw::rect(x, baseline - 13, w.min(cw), 19, 4.0, p.panel);
            out.push_str(&draw::text(
                &text,
                x + 6,
                baseline,
                12,
                p.text,
                Anchor::Start,
                false,
            ));
            out
        }
        Cell::Text(_) => {
            let mut out = String::new();
            for (i, line) in lines.iter().enumerate() {
                let (tx, anchor) = anchor_for(align, x, cw);
                out.push_str(&draw::text(
                    line,
                    tx,
                    baseline + i as i32 * LINE_H,
                    FONT_SIZE,
                    // 折行是同一句话的下半截，不是脚注：颜色必须一样。
                    // 淡下去的第二行读起来像另一条信息。
                    p.text,
                    anchor,
                    false,
                ));
            }
            out
        }
    }
}

/// `x` 是这一格的左边缘，不是已经算好的锚点位置。
fn symbol(
    ch: &str,
    x: i32,
    baseline: i32,
    align: Align,
    cw: i32,
    color: crate::theme::Rgb,
) -> String {
    let (tx, anchor) = anchor_for(align, x, cw);
    draw::text(ch, tx, baseline, 15, color, anchor, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{DARK, PORCELAIN};

    fn sample() -> Table {
        let mut t = Table::new(
            vec!["Code".into(), "Meaning".into()],
            vec![
                vec!["0".into(), "Success".into()],
                vec!["1".into(), "Score below `--min-score`".into()],
                vec![
                    "4".into(),
                    "`--remote` failed (API error, rate limit)".into(),
                ],
            ],
        );
        t.title = Some("Exit codes".into());
        t
    }

    #[test]
    fn is_a_self_contained_svg() {
        let svg = table(&sample(), &Options::default());
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.trim_end().ends_with("</svg>"));
        crate::draw::assert_self_contained(&svg);
    }

    #[test]
    fn the_same_table_renders_byte_identical_svg() {
        assert_eq!(
            table(&sample(), &Options::default()),
            table(&sample(), &Options::default())
        );
    }

    #[test]
    fn every_cell_reaches_the_output() {
        let svg = table(&sample(), &Options::default());
        assert!(svg.contains("Success"));
        assert!(svg.contains("EXIT CODES"));
        assert!(svg.contains("MEANING"));
    }

    /// 链接在 SVG 里点不动，留一段 URL 只会把列撑宽
    #[test]
    fn markdown_inline_syntax_is_reduced_to_its_text() {
        assert_eq!(inline_text("[docs](https://example.com)"), "docs");
        assert_eq!(inline_text("**bold** and `code`"), "bold and code");
        assert_eq!(inline_text("a<br>b"), "a b");
        assert_eq!(inline_text("![alt](x.png)"), "alt");
        assert_eq!(
            inline_text("plain [not a link] text"),
            "plain [not a link] text"
        );
    }

    #[test]
    fn status_marks_become_symbols_in_the_palette_colours() {
        let t = Table::new(
            vec!["".into(), "What".into()],
            vec![
                vec!["✅".into(), "shipped".into()],
                vec!["⏳".into(), "later".into()],
            ],
        );
        let svg = table(&t, &Options::default());
        assert!(svg.contains('✓'));
        assert!(svg.contains('◔'));
        assert!(svg.contains(&DARK.bands[0].to_string()));
        assert!(svg.contains(&DARK.warn.to_string()));
    }

    /// 一列右对齐的数字才能靠位数直接比大小
    #[test]
    fn numeric_cells_are_right_aligned_regardless_of_column_alignment() {
        let svg = table(&sample(), &Options::default());
        // 三行的数字都用 end 锚点
        assert!(svg.matches(r#"text-anchor="end""#).count() >= 3);
    }

    /// 长说明列不能把窄列压没，窄列也不能被富余撑宽
    #[test]
    fn a_short_column_stays_short_and_the_prose_column_takes_the_slack() {
        let t = Table::new(
            vec!["Code".into(), "Meaning".into()],
            vec![
                vec!["0".into(), "Success".into()],
                vec!["1".into(), "a ".repeat(80)],
            ],
        );
        let widths = columns(&t, 808);
        assert!(widths[0] < 70, "只放一位数的列宽到了 {}", widths[0]);
        assert!(widths[1] > 600, "散文列只分到 {}", widths[1]);
        assert_eq!(widths.iter().sum::<i32>(), 808 - COL_GAP);
    }

    /// 全都装不下时按最宽的先扣，窄列不参与
    #[test]
    fn overlong_columns_shrink_from_the_widest_first() {
        let t = Table::new(
            vec!["a".into(), "b".into(), "c".into()],
            vec![vec!["x".repeat(200), "y".repeat(200), "z".into()]],
        );
        let widths = columns(&t, 808);
        assert!(widths.iter().sum::<i32>() <= 808 - COL_GAP * 2);
        assert!(widths[2] >= MIN_COL);
        assert!(
            (widths[0] - widths[1]).abs() <= 2,
            "两根长列该扣得一样多: {widths:?}"
        );
    }

    /// 一排全空的表头只会留下一条无字的横线和一片空白
    #[test]
    fn an_all_blank_header_row_is_not_drawn() {
        let t = Table::new(
            vec!["".into(), "".into()],
            vec![
                vec!["✅".into(), "shipped".into()],
                vec!["⏳".into(), "later".into()],
            ],
        );
        let svg = table(&t, &Options::default());
        assert!(!svg.contains("letter-spacing"), "空表头还是画了标签: {svg}");
        assert!(svg.contains("shipped"));
    }

    /// 左对齐的表头顶着一列右对齐的数字，看着像两件不相干的东西
    #[test]
    fn a_numeric_column_right_aligns_its_header_too() {
        let t = Table::new(
            vec!["Code".into(), "Meaning".into()],
            vec![vec!["0".into(), "ok".into()], vec!["1".into(), "no".into()]],
        );
        assert_eq!(column_align(&t, 0), Align::Right);
        assert_eq!(column_align(&t, 1), Align::Left);
    }

    #[test]
    fn long_cells_wrap_and_are_capped_with_an_ellipsis() {
        let lines = wrap(&"word ".repeat(200), 200.0);
        assert_eq!(lines.len(), MAX_LINES);
        assert!(lines.last().unwrap().ends_with('…'));
    }

    #[test]
    fn an_empty_table_renders_a_valid_document_rather_than_panicking() {
        let svg = table(&Table::new(vec![], vec![]), &Options::default());
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn a_row_with_missing_cells_is_padded_rather_than_panicking() {
        let t = Table::new(
            vec!["a".into(), "b".into(), "c".into()],
            vec![vec!["only one".into()]],
        );
        let svg = table(&t, &Options::default());
        assert!(svg.contains("only one"));
    }

    #[test]
    fn the_palette_reaches_the_table() {
        let light = table(
            &sample(),
            &Options {
                palette: &PORCELAIN,
                lang: crate::i18n::Lang::En,
            },
        );
        assert!(light.contains(&PORCELAIN.bg.to_string()));
        assert!(!light.contains(&DARK.bg.to_string()));
    }

    /// 每一格都必须落在自己那一列里。锚点算两遍会让状态符号
    /// 跑到下一列的字上面去——那是一张画坏了的图。
    #[test]
    fn no_cell_is_drawn_on_top_of_the_next_column() {
        let t = Table::new(
            vec!["".into(), "What".into()],
            vec![
                vec!["✅".into(), "shipped".into()],
                vec!["⏳".into(), "later".into()],
            ],
        );
        let svg = table(&t, &Options::default());
        let widths = columns(&t, t.width - PAD * 2);
        let symbol_right = (PAD + widths[0]) as f32;
        let text_left = (PAD + widths[0] + COL_GAP) as f32;
        assert!(symbol_right <= text_left);

        for (body, left, right) in extents(&svg) {
            assert!(left >= 0.0 && right <= t.width as f32, "{body:?} 越界");
            if body == "✓" || body == "◔" {
                assert!(right <= text_left, "{body:?} 画到了第二列上: {right}");
            }
        }
    }

    #[test]
    fn no_text_is_drawn_outside_the_table() {
        let svg = table(&sample(), &Options::default());
        for (body, left, right) in extents(&svg) {
            assert!(
                left >= 4.0 && right <= sample().width as f32 - 4.0,
                "{body:?} 画到了 {left:.0}..{right:.0}"
            );
        }
    }

    /// 估算每个 `<text>` 的左右边界。等宽字的步进恒定，估得够准。
    fn extents(svg: &str) -> Vec<(String, f32, f32)> {
        let mut out = Vec::new();
        for node in svg.split("<text ").skip(1) {
            let attr = |k: &str| {
                node.split(&format!("{k}=\""))
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .map(str::to_string)
            };
            let (Some(x), Some(size), Some(anchor)) =
                (attr("x"), attr("font-size"), attr("text-anchor"))
            else {
                continue;
            };
            let body: String = node
                .split_once('>')
                .and_then(|(_, r)| r.split_once("</text>"))
                .map(|(t, _)| t.to_string())
                .unwrap_or_default();
            let x: f32 = x.parse().unwrap_or(0.0);
            let w = draw::width_px(&body, size.parse().unwrap_or(12.0));
            let (l, r) = match anchor.as_str() {
                "end" => (x - w, x),
                "middle" => (x - w / 2.0, x + w / 2.0),
                _ => (x, x + w),
            };
            out.push((body, l, r));
        }
        out
    }

    #[test]
    fn xml_special_characters_survive_a_round_trip() {
        let t = Table::new(vec!["a & b".into()], vec![vec!["<script>".into()]]);
        let svg = table(&t, &Options::default());
        assert!(svg.contains("&amp;"));
        assert!(!svg.contains("<script>"));
    }
}
