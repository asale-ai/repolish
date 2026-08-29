//! 5×7 点阵字形：wordmark 与大号分数共用的一张表。
//!
//! 为什么自己画字：logo 与 SVG 卡片都要在**别人的机器**上渲染，
//! 那里有没有装某个字体不受我们控制。点阵转成矩形（SVG）或半块字符（终端），
//! 落到哪儿都是同一个形状——一个 logo 不能在半数读者那里换一副面孔。
//!
//! 同一张表喂两个消费者，终端横幅和 README 里的 logo 因此天然对得上。

/// 单字形宽度（列）
pub const W: usize = 5;
/// 单字形高度（行）
pub const H: usize = 7;
/// 字间距（列）
pub const GAP: usize = 1;

/// 每行 5 位，bit4 为最左列。未收录的字符按空格处理。
///
/// 表格保持一行一个字形——rustfmt 会把每个数组拆成三行，
/// 拆完就再也看不出这张表画的是什么了。
#[rustfmt::skip]
fn glyph(c: char) -> [u8; H] {
    match c.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        // 分数里的 0 用不带斜杠的字形：这是数字不是代码，可读优先于区分 O
        '0' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        '/' => [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        _ => [0; H],
    }
}

/// 一串字的点阵。`bits[row][col]`，行数恒为 [`H`]。
pub struct Bitmap {
    pub width: usize,
    pub bits: Vec<Vec<bool>>,
}

/// 这个字符有没有点阵。
///
/// 没有的会画成一列空白——所以整串里只要有一个画不出来，调用方就该换一种
/// 画法。一个中文项目名在这套字体下会变成一片空白，那比用朴素字体糟得多。
pub fn supports(c: char) -> bool {
    glyph(c) != [0; H]
}

pub fn bitmap(text: &str) -> Bitmap {
    let n = text.chars().count();
    let width = if n == 0 { 0 } else { n * (W + GAP) - GAP };
    let mut bits = vec![vec![false; width]; H];
    for (i, ch) in text.chars().enumerate() {
        let g = glyph(ch);
        let x0 = i * (W + GAP);
        for (row, mask) in g.iter().enumerate() {
            for col in 0..W {
                if mask & (1 << (W - 1 - col)) != 0 {
                    bits[row][x0 + col] = true;
                }
            }
        }
    }
    Bitmap { width, bits }
}

/// 点阵 → 半块字符。一个字符盖两行点阵，7 行补齐到 8 行 → 4 行文本。
///
/// 返回的每一行里，第 i 个字符对应点阵第 i 列——调用方按列号取渐变色即可。
pub fn blocks(text: &str) -> Vec<String> {
    let bm = bitmap(text);
    let rows = H.div_ceil(2);
    (0..rows)
        .map(|r| {
            (0..bm.width)
                .map(|c| {
                    let top = bm.bits[r * 2][c];
                    let bottom = bm.bits.get(r * 2 + 1).is_some_and(|row| row[c]);
                    match (top, bottom) {
                        (true, true) => '█',
                        (true, false) => '▀',
                        (false, true) => '▄',
                        (false, false) => ' ',
                    }
                })
                .collect()
        })
        .collect()
}

/// 半块渲染后的显示宽度（列）
pub fn blocks_width(text: &str) -> usize {
    bitmap(text).width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_accounts_for_the_gap_between_glyphs() {
        assert_eq!(blocks_width(""), 0);
        assert_eq!(blocks_width("R"), 5);
        assert_eq!(blocks_width("RE"), 11);
        assert_eq!(blocks_width("REPOLISH"), 47);
    }

    #[test]
    fn seven_rows_render_into_four_lines_of_equal_width() {
        let lines = blocks("REPOLISH");
        assert_eq!(lines.len(), 4);
        for l in &lines {
            assert_eq!(l.chars().count(), 47);
        }
    }

    /// L 的形状最容易验证：左列全亮，底行全亮，右上全暗
    #[test]
    fn glyph_bits_land_where_the_table_says() {
        let bm = bitmap("L");
        assert!(bm.bits.iter().all(|row| row[0]));
        assert!(bm.bits[H - 1].iter().all(|&b| b));
        assert!(!bm.bits[0][4]);
    }

    #[test]
    fn unknown_characters_render_blank_rather_than_panicking() {
        let bm = bitmap("\u{4f60}");
        assert_eq!(bm.width, W);
        assert!(bm.bits.iter().all(|row| row.iter().all(|&b| !b)));
    }
}
