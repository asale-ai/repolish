//! SVG 基元：卡片、表格、概览共用的一套画笔。
//!
//! 三条约束对所有 SVG 产物一视同仁（详见 [`crate::svg`]）：**自包含**、
//! **确定性**、**深底恒定**。这里只提供画法，约束由各个卡片自己守。
//!
//! 没有布局引擎，也不打算有：每张卡片自己按像素排。一个只画四五种版式的
//! 生成器，引入约束求解器只会让「这行为什么在这儿」变得没人说得清。

use std::fmt::Write as _;

use crate::glyph;
use crate::i18n::Lang;
use crate::theme::{Palette, Rgb, DARK};

/// 一张 SVG 产物的可调项。
///
/// 色板与语言是**每一张**卡片都要的两件事，所以合成一个结构体在各个渲染器
/// 之间传。加第三项配置时，调用点不用跟着改一遍。
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub palette: &'static Palette,
    pub lang: Lang,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            palette: &DARK,
            lang: Lang::En,
        }
    }
}

/// 等宽字栈。CJK 字体排在后面兜底——概览卡片会写进中文 README，
/// 一个渲染不出来的方框比英文标签更糟。
pub const FONT: &str = "ui-monospace, SFMono-Regular, \"SF Mono\", Menlo, Consolas, \
                        \"Liberation Mono\", \"Noto Sans Mono CJK SC\", \"Noto Sans Mono CJK JP\", \
                        \"Microsoft YaHei\", \"Hiragino Sans\", \"Yu Gothic\", \
                        monospace";

/// 等宽字的字符步进比例。
///
/// SVG 里没有字体度量可查（我们也不打算带一个字体解析器进来），但**等宽**
/// 字的步进恒定，这个比例对所有等宽字都成立到足以用来右对齐和截断。
pub const ADVANCE: f32 = 0.6;

pub enum Anchor {
    Start,
    Middle,
    End,
}

impl Anchor {
    fn as_str(&self) -> &'static str {
        match self {
            Anchor::Start => "start",
            Anchor::Middle => "middle",
            Anchor::End => "end",
        }
    }
}

/// 一段文字在给定字号下大约占多少像素。
///
/// CJK 按两倍步进算——中文标签按 ASCII 宽度估会短一半，右对齐的那一列
/// 就会撞进左边的文字里。
pub fn width_px(s: &str, size: f32) -> f32 {
    let cols: f32 = s
        .chars()
        .map(|c| if is_wide(c) { 2.0 } else { 1.0 })
        .sum::<f32>();
    cols * size * ADVANCE
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6)
}

/// 截断到 `max_px` 以内，截了就带一个省略号。
///
/// 卡片是固定宽度的图片，没有换行可用。溢出的文字不会被裁掉，而是画到
/// 边框外面去——那比截断难看得多。
pub fn fit(s: &str, max_px: f32, size: f32) -> String {
    if width_px(s, size) <= max_px {
        return s.to_string();
    }
    let room = max_px - width_px("…", size);
    let mut out = String::new();
    let mut used = 0.0;
    for c in s.chars() {
        let w = if is_wide(c) { 2.0 } else { 1.0 } * size * ADVANCE;
        if used + w > room {
            break;
        }
        out.push(c);
        used += w;
    }
    format!("{}…", out.trim_end())
}

/// XML 转义。仓库名、检查项文案里的 `&` `<` 不转义会让整张卡片解析失败。
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// ── 基元 ────────────────────────────────────────────────────

pub fn text(s: &str, x: i32, y: i32, size: i32, fill: Rgb, anchor: Anchor, bold: bool) -> String {
    let weight = if bold { r#" font-weight="700""# } else { "" };
    format!(
        "  <text class=\"t\" x=\"{x}\" y=\"{y}\" font-size=\"{size}\" fill=\"{fill}\" \
         text-anchor=\"{}\"{weight}>{}</text>\n",
        anchor.as_str(),
        esc(s)
    )
}

/// 小号字距标签（`LANGUAGES · BY FILE` 那一档）。
///
/// 字距是这套版式唯一的装饰手段：全大写的小字挤在一起读不动，
/// 拉开之后一眼就能认出「这是一个分区标题，不是数据」。
pub fn label(s: &str, x: i32, y: i32, fill: Rgb, anchor: Anchor) -> String {
    format!(
        "  <text class=\"t\" x=\"{x}\" y=\"{y}\" font-size=\"11\" fill=\"{fill}\" \
         text-anchor=\"{}\" letter-spacing=\"1.4\">{}</text>\n",
        anchor.as_str(),
        esc(s)
    )
}

pub fn rect(x: i32, y: i32, w: i32, h: i32, r: f32, fill: Rgb) -> String {
    format!("  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{r:.1}\" fill=\"{fill}\"/>\n")
}

pub fn hline(x: i32, y: i32, w: i32, fill: Rgb) -> String {
    format!("  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"1\" fill=\"{fill}\"/>\n")
}

pub fn circle(cx: i32, cy: i32, r: i32, fill: Rgb) -> String {
    format!("  <circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\" fill=\"{fill}\"/>\n")
}

/// 图例用的小方块
pub fn swatch(x: i32, y: i32, fill: Rgb) -> String {
    rect(x, y, 9, 9, 2.0, fill)
}

// ── 图表 ────────────────────────────────────────────────────

/// 分段条的尺寸。段宽、间距与段数总是一起变，分成三个参数传只会让调用点
/// 出现三个孤零零的数字。
#[derive(Debug, Clone, Copy)]
pub struct Segments {
    pub count: i32,
    pub width: i32,
    pub gap: i32,
}

/// 分段条。段数固定，缺的那一格就是要给人看的——
/// 连续条在高分区没有信息：99 和 100 差几个像素，谁也看不出来。
pub fn segmented(x: i32, y: i32, seg: Segments, filled: i32, on: Rgb, off: Rgb) -> String {
    let mut out = String::new();
    for i in 0..seg.count {
        let fill = if i < filled { on } else { off };
        out.push_str(&rect(
            x + i * (seg.width + seg.gap),
            y,
            seg.width,
            9,
            2.0,
            fill,
        ));
    }
    out
}

/// 连续比例条：一条轨道，左边填一段。
pub fn ratio_bar(x: i32, y: i32, w: i32, h: i32, ratio: f32, on: Rgb, off: Rgb) -> String {
    let filled = (w as f32 * ratio.clamp(0.0, 1.0)).round() as i32;
    let mut out = rect(x, y, w, h, 2.0, off);
    // 比例非零时至少留一像素：0.4% 画成空条，读者会以为是「没有」
    if ratio > 0.0 {
        out.push_str(&rect(x, y, filled.max(1), h, 2.0, on));
    }
    out
}

/// 堆叠条：一条通栏，按份额分段。
///
/// 份额小到画不出一个像素的段直接丢掉——画一条零宽矩形不会出现在图上，
/// 却会出现在文件里，白白撑大卡片体积。
pub fn stacked(x: i32, y: i32, w: i32, h: i32, parts: &[(f32, Rgb)]) -> String {
    let total: f32 = parts.iter().map(|(v, _)| *v).sum();
    if total <= 0.0 {
        return String::new();
    }
    let mut out = String::new();
    let mut cursor = x as f32;
    for (i, (value, color)) in parts.iter().enumerate() {
        let seg = w as f32 * (value / total);
        // 末段吃掉累计误差，右边缘才对得齐
        let end = if i + 1 == parts.len() {
            (x + w) as f32
        } else {
            cursor + seg
        };
        let px = (end - cursor).round() as i32;
        if px > 0 {
            out.push_str(&rect(cursor.round() as i32, y, px, h, 2.0, *color));
        }
        cursor = end;
    }
    out
}

/// 面积图。`values` 是等距采样，`peak` 是纵轴上界，`ink` 同时用作描边与填充
/// （填充降透明度）——两个颜色分开传过，从来没有传成不同的两个。
///
/// 用折线而不是平滑曲线：每一个点都是一周的真实提交数，
/// 插值出来的漂亮弧线会在两周之间凭空造出并不存在的形状。
pub fn area(x: i32, y: i32, w: i32, h: i32, values: &[u32], peak: u32, ink: Rgb) -> String {
    let (stroke, fill) = (ink, ink);
    if values.len() < 2 || peak == 0 {
        return String::new();
    }
    let step = w as f32 / (values.len() - 1) as f32;
    let py = |v: u32| y as f32 + h as f32 * (1.0 - (v as f32 / peak as f32).clamp(0.0, 1.0));

    let mut line = String::new();
    for (i, v) in values.iter().enumerate() {
        let px = x as f32 + i as f32 * step;
        let _ = write!(
            line,
            "{}{px:.1} {:.1}",
            if i == 0 { "M" } else { "L" },
            py(*v)
        );
        line.push(' ');
    }
    let line = line.trim_end().to_string();

    format!(
        "  <path d=\"{line} L{:.1} {} L{x} {} Z\" fill=\"{fill}\" fill-opacity=\"0.35\"/>\n  \
         <path d=\"{line}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"2\" \
         stroke-linejoin=\"round\" stroke-linecap=\"round\"/>\n",
        x as f32 + w as f32,
        y + h,
        y + h,
    )
}

/// 虚线基准线，标一个「上界在这儿」。
pub fn dashed(x: i32, y: i32, w: i32, fill: Rgb) -> String {
    format!(
        "  <line x1=\"{x}\" y1=\"{y}\" x2=\"{}\" y2=\"{y}\" stroke=\"{fill}\" \
         stroke-width=\"1\" stroke-dasharray=\"3 5\"/>\n",
        x + w
    )
}

// ── 品牌 ────────────────────────────────────────────────────

/// `mark()` 引用的渐变定义。凡是画了标记的文档都得先带上这一段。
pub fn brand_defs(p: &Palette) -> String {
    format!(
        "  <defs>\n    <linearGradient id=\"brand\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\n      \
         <stop offset=\"0\" stop-color=\"{}\"/>\n      \
         <stop offset=\"0.55\" stop-color=\"{}\"/>\n      \
         <stop offset=\"1\" stop-color=\"{}\"/>\n    </linearGradient>\n  </defs>\n",
        p.brand[0], p.brand[1], p.brand[2]
    )
}

/// 品牌标记：渐变圆角块 + 一大一小两颗「打磨」的星芒。
/// 纯几何，不依赖字体，16px 下也认得出来。
pub fn mark(x: i32, y: i32, size: i32, p: &Palette) -> String {
    let s = size as f32;
    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"  <rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{:.1}" fill="url(#brand)"/>"#,
        s * 0.26
    );
    let _ = writeln!(
        out,
        r#"  <path d="{}" fill="{}"/>"#,
        sparkle(x as f32 + s * 0.44, y as f32 + s * 0.56, s * 0.34),
        p.mark_hollow
    );
    let _ = writeln!(
        out,
        r#"  <path d="{}" fill="{}"/>"#,
        sparkle(x as f32 + s * 0.76, y as f32 + s * 0.26, s * 0.16),
        p.mark_hollow
    );
    out
}

/// 四角星芒。凹边用二次贝塞尔，腰越细尖角越利。
fn sparkle(cx: f32, cy: f32, r: f32) -> String {
    let w = r * 0.2;
    format!(
        "M{:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Q{:.1} {:.1} {:.1} {:.1}Z",
        cx, cy - r,
        cx + w, cy - w, cx + r, cy,
        cx + w, cy + w, cx, cy + r,
        cx - w, cy + w, cx - r, cy,
        cx - w, cy - w, cx, cy - r,
    )
}

/// wordmark 的点阵 → 矩形。一个亮点一个方块，圆角一点点，像素味不会太硬。
///
/// **只有 wordmark 走这条路。** 分数、标签、文案一律是普通文本——点阵是
/// **标识**的处理方式，不是数据的：数字换一副等宽字无非是宽了几像素，
/// logo 换一副面孔就不是同一个 logo 了。
///
/// 渐变按**列**上色而不是挂一个 `linearGradient`：objectBoundingBox 的渐变是
/// 相对每个引用它的元素算的，几百个小方块每个都会取满一整条渐变，
/// 结果就是整段 wordmark 一个颜色。
/// 这个名字能不能用点阵字标画出来，在给定格子大小和可用宽度下。
///
/// 点阵字体只有 `A-Z0-9.-`。非拉丁名字整串都画不出来，硬画就是一片空白，
/// 而一片空白的抬头比换成朴素字体糟得多；太长的名字则会压到旁边的内容上。
/// 两种情况都返回 `None`，让调用方退回普通文字。
///
/// `_` 先换成 `-`：仓库名里下划线很常见，而它恰好是这套字体没有的字符之一，
/// 换成连字符比开一个空洞好看。
pub fn as_blocks(name: &str, cell: i32, max_px: i32) -> Option<String> {
    let s: String = name.to_uppercase().replace('_', "-");
    if s.is_empty() || !s.chars().all(glyph::supports) {
        return None;
    }
    (glyph::blocks_width(&s) as i32 * cell <= max_px).then_some(s)
}

pub fn blocks(s: &str, x: i32, y: i32, cell: i32, p: &Palette) -> String {
    let bm = glyph::bitmap(s);
    let mut out = String::new();
    let rx = (cell / 8).max(1);

    for col in 0..bm.width {
        let rows: Vec<usize> = (0..glyph::H).filter(|&row| bm.bits[row][col]).collect();
        if rows.is_empty() {
            continue;
        }
        let t = col as f32 / (bm.width.saturating_sub(1)).max(1) as f32;
        let _ = write!(out, r#"  <g fill="{}">"#, p.sweep(t));
        for row in rows {
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{cell}" height="{cell}" rx="{rx}"/>"#,
                x + col as i32 * cell,
                y + row as i32 * cell,
            );
        }
        let _ = writeln!(out, "</g>");
    }
    out
}

// ── 文档外壳 ────────────────────────────────────────────────

/// 卡片外壳：底、边框、字体声明、无障碍标签。
///
/// `lang` 落在根元素上，读屏软件据此换发音——一张中文卡片被读成英文
/// 拼读是很难听的。
pub fn document(body: &str, w: i32, h: i32, p: &Palette, lang: &str, aria: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" lang=\"{lang}\" role=\"img\" aria-label=\"{}\">",
        esc(aria)
    );
    s.push_str(&brand_defs(p));
    let _ = writeln!(
        s,
        "  <style>\n    .t {{ font-family: {FONT}; }}\n  </style>"
    );
    s.push_str(&rect(0, 0, w, h, 18.0, p.bg));
    let _ = writeln!(
        s,
        "  <rect x=\"0.5\" y=\"0.5\" width=\"{}\" height=\"{}\" rx=\"17.5\" fill=\"none\" stroke=\"{}\"/>",
        w - 1,
        h - 1,
        p.line
    );
    s.push_str(body);
    let _ = writeln!(s, "</svg>");
    s
}

/// 「自包含」的真正判据。
///
/// 早先这里数的是 `http` 出现了几次，理由是「除了 SVG 命名空间不该有别的
/// URL」。那条判据是错的：卡片上会出现仓库简介，表格里会出现链接文字，
/// 录屏更是会把命令打印出的 URL 原样收进去——**内容里带 URL 不是引用**。
/// 一个把自己内容当成违规的测试，迟早被人用一句「哦这个正常」关掉。
///
/// 真正要守的是结构：没有脚本，没有把外部资源拉进来的属性。
#[cfg(test)]
pub fn assert_self_contained(svg: &str) {
    assert!(!svg.contains("<script"), "SVG 里出现了脚本");
    assert!(
        !svg.contains("<foreignObject"),
        "foreignObject 会引入外部渲染上下文"
    );
    assert!(!svg.contains("@import"), "CSS @import 会去拉外部样式表");
    for attr in ["href=", "xlink:href=", "src=", "xlink:show="] {
        assert!(!svg.contains(attr), "SVG 里出现了外部引用属性 {attr}");
    }
    // url(...) 只允许指向文档内部的 defs
    for m in svg.split("url(").skip(1) {
        let target = m.split(')').next().unwrap_or("");
        assert!(target.starts_with('#'), "url() 指向了文档外部: {target}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::DARK;

    #[test]
    fn xml_special_characters_are_escaped_everywhere_text_goes() {
        assert!(text("a<b&c", 0, 0, 12, DARK.text, Anchor::Start, false).contains("a&lt;b&amp;c"));
        assert!(label("a&b", 0, 0, DARK.muted, Anchor::End).contains("a&amp;b"));
    }

    /// CJK 按两倍步进算，否则右对齐的那一列会撞进左边的文字
    #[test]
    fn cjk_counts_as_two_columns_when_measuring() {
        assert_eq!(width_px("仓库", 12.0), width_px("repo", 12.0));
        assert!(width_px("仓库", 12.0) > width_px("ab", 12.0));
        assert_eq!(width_px("ab", 10.0), 12.0);
    }

    #[test]
    fn text_too_wide_for_its_box_is_truncated_with_an_ellipsis() {
        let out = fit("a very long label indeed", 60.0, 12.0);
        assert!(out.ends_with('…'));
        assert!(width_px(&out, 12.0) <= 60.0, "截断后仍然超宽: {out}");
        assert_eq!(fit("short", 600.0, 12.0), "short");
    }

    /// 末段吃掉累计误差，堆叠条的右边缘才对得齐
    #[test]
    fn a_stacked_bar_ends_exactly_where_the_track_ends() {
        let svg = stacked(
            10,
            0,
            300,
            8,
            &[(1.0, DARK.text), (1.0, DARK.muted), (1.0, DARK.line)],
        );
        let last_x: i32 = svg
            .lines()
            .last()
            .unwrap()
            .split("x=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let last_w: i32 = svg
            .lines()
            .last()
            .unwrap()
            .split("width=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(last_x + last_w, 310);
    }

    #[test]
    fn an_empty_stack_draws_nothing_rather_than_dividing_by_zero() {
        assert_eq!(stacked(0, 0, 100, 8, &[]), "");
        assert_eq!(stacked(0, 0, 100, 8, &[(0.0, DARK.text)]), "");
    }

    /// 非零比例至少留一个像素：0.4% 画成空条，读者会以为是「没有」
    #[test]
    fn a_tiny_ratio_still_draws_something() {
        let svg = ratio_bar(0, 0, 200, 8, 0.001, DARK.text, DARK.track);
        assert_eq!(svg.matches("<rect").count(), 2);
        assert_eq!(
            ratio_bar(0, 0, 200, 8, 0.0, DARK.text, DARK.track)
                .matches("<rect")
                .count(),
            1
        );
    }

    #[test]
    fn an_area_chart_needs_at_least_two_points_and_a_peak() {
        assert_eq!(area(0, 0, 100, 40, &[3], 3, DARK.text), "");
        assert_eq!(area(0, 0, 100, 40, &[0, 0], 0, DARK.text), "");
        assert!(area(0, 0, 100, 40, &[1, 2], 2, DARK.text).contains("<path"));
    }
}
