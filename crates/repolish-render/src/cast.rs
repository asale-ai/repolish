//! 终端录屏 → 会动的 SVG。
//!
//! 这是 [VHS](https://github.com/charmbracelet/vhs) 那件事的另一种做法。VHS 很好，
//! 但它要 ttyd 和 ffmpeg，产出的是 GIF；而这个仓库对自己产出的每一个文件都有
//! 三条硬约束（自包含、确定性、纯文本），GIF 三条全不满足：
//!
//! - **二进制。** 一个 800KB 的 GIF 每次重录都整个换掉，git 历史会被撑肥。
//!   这也正是本仓库的 demo workflow 一直只肯手动触发的原因。
//! - **没有文本层。** 录屏里那行命令，读者复制不走，`grep` 也找不到。
//! - **要装两个外部程序。** 一个「让你的仓库体面起来」的工具，不该先要求
//!   使用者装一条视频工具链。
//!
//! 动画 SVG 三条都满足：它是文本，diff 得动；它自包含，不引字体不引脚本；
//! 同一次录制逐字节一致。而且**命令行里那行字是真的文字**——可以选中、复制、
//! 被读屏软件念出来。
//!
//! 动画只用 CSS `@keyframes`，不用脚本也不用 SMIL：GitHub 把 SVG 当图片经
//! camo 代理渲染，`<img>` 里的脚本一律不执行，而 CSS 动画会跑。
//!
//! **不做完整的终端模拟。** 认 SGR 颜色、`\n` 和 `\r`，认到此为止。会重绘
//! 屏幕的程序（进度条、TUI）录出来是不对的，[`Screen::feed`] 上写明了这一点。
//! 一个画卡片的工具不该顺手长出一个 vt100。

use std::fmt::Write as _;

use crate::draw::{self, Options};
use crate::theme::{Palette, Rgb};

/// 一段同色文本
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub fg: Option<Rgb>,
    pub bold: bool,
}

/// 录到的一行
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line(pub Vec<Span>);

impl Line {
    pub fn width(&self) -> usize {
        self.0.iter().map(|s| s.text.chars().count()).sum()
    }

    pub fn plain(&self) -> String {
        self.0.iter().map(|s| s.text.as_str()).collect()
    }
}

/// 一条命令，以及它吐出来的东西
#[derive(Debug, Clone)]
pub struct Step {
    pub command: String,
    pub output: Vec<Line>,
}

/// 录制的节奏。全部是**固定值**，不来自真实耗时——
/// 真实耗时会让同一次录制在快慢两台机器上产出不同的文件，
/// 确定性就没了。而且没人想看一个 4.2 秒的编译过程原速重放。
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// 每个字符的敲击间隔，毫秒
    pub type_ms: u32,
    /// 敲完到回车之间的停顿
    pub submit_ms: u32,
    /// 输出停留多久
    pub hold_ms: u32,
    /// 整段结束后回到开头前的停顿
    pub loop_ms: u32,
    /// **开场定格**：循环从最后一步的终态停住开始。
    ///
    /// 这一段不是节奏，是**兜底**。任何把这张图冻在 `t=0` 的渲染器——
    /// 有的浏览器就是不给 `<img>` 里的 SVG 跑动画，缩略图和 PDF 导出
    /// 更是一律静态——看到的都是第 0 帧。如果第 0 帧是一个空终端，
    /// 那些读者拿到的就是一张白图。让第 0 帧是**跑完之后的样子**，
    /// 静态渲染下这张图仍然说得清它要说的事。
    pub poster_ms: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            type_ms: 45,
            submit_ms: 650,
            hold_ms: 3200,
            loop_ms: 1200,
            poster_ms: 1800,
        }
    }
}

const FONT_SIZE: f32 = 14.0;
/// 等宽字的步进。与 [`crate::draw::ADVANCE`] 同一个道理。
const CHAR_W: f32 = FONT_SIZE * draw::ADVANCE;
const LINE_H: f32 = FONT_SIZE * 1.5;
const PAD: f32 = 18.0;
/// 窗口标题栏高度
const CHROME_H: f32 = 34.0;
/// 一行最多画这么多列。超出的截断——录屏是给人看的，
/// 一条横向滚动的图没有意义。
const MAX_COLS: usize = 104;
/// 最多画这么多行。命令的输出比这长的话，录屏本来就不是展示它的方式。
const MAX_ROWS: usize = 32;

/// 画出会动的终端录屏。
pub fn cast(steps: &[Step], timing: &Timing, opts: &Options) -> String {
    let p = opts.palette;
    if steps.is_empty() {
        return draw::document("", 320, 80, p, opts.lang.tag(), "empty recording");
    }

    // 每一步各自 clear 之后重画，所以画布高度取最高的那一步，
    // 不是所有步骤的总和
    let cols = steps
        .iter()
        .flat_map(|s| {
            std::iter::once(s.command.chars().count() + 2).chain(s.output.iter().map(|l| l.width()))
        })
        .max()
        .unwrap_or(40)
        .clamp(40, MAX_COLS);
    let rows = steps
        .iter()
        .map(|s| s.output.len().min(MAX_ROWS) + 1)
        .max()
        .unwrap_or(1);

    let w = (PAD * 2.0 + cols as f32 * CHAR_W).ceil() as i32;
    let h = (CHROME_H + PAD * 2.0 + rows as f32 * LINE_H).ceil() as i32;

    // 先把时间轴排出来，再画：每个元素的 keyframes 百分比都要除以总时长
    let plan = schedule(steps, timing);

    let mut body = String::new();
    let mut css = String::new();
    chrome(&mut body, w, p, steps);

    // 遮板要滑过的距离逐步不同（命令长短不一），所以每一步一个自定义属性。
    // 写成变量而不是直接内联进 keyframes，是为了让整段 CSS 里
    // 「时长」和「距离」这两类数只各出现一次，改的时候不会漏。
    let mut vars = format!("      --d: {:.3}s;\n", plan.total_ms as f32 / 1000.0);
    for (i, step) in steps.iter().enumerate() {
        let _ = writeln!(
            vars,
            "      --w{i}: {:.1}px;",
            step.command.chars().count().min(cols.saturating_sub(2)) as f32 * CHAR_W
        );
    }
    // 静态兜底要把最后一步的遮板推到底，而「最后一步」是第几步由内容决定，
    // 所以给它一个别名，CSS 里不必知道下标
    if let Some(last) = steps.last() {
        let _ = writeln!(
            vars,
            "      --wlast: {:.1}px;",
            last.command.chars().count().min(cols.saturating_sub(2)) as f32 * CHAR_W
        );
    }
    css.push_str(&format!("    svg {{\n{vars}    }}\n"));

    for (i, (step, window)) in steps.iter().zip(plan.windows.iter()).enumerate() {
        let last = i + 1 == steps.len();
        body.push_str(&draw_step(i, step, window, &plan, p, cols, last));
        css.push_str(&step_css(i, window, &plan, step, last));
    }

    document(&body, &css, w, h, p, opts.lang.tag(), steps)
}

// ── ANSI → 屏幕 ─────────────────────────────────────────────

/// 把捕获到的终端字节流变成一屏带颜色的文本。
///
/// **这不是终端模拟器。** 认三样东西：SGR 颜色（`ESC[…m`）、`\n`、`\r`。
/// 别的 CSI 序列一律跳过而不是照着执行——光标移动、清屏、滚动区域，
/// 真要认全就是在写一个 vt100，而这个 crate 的工作是画卡片。
///
/// 后果得说清楚：**会重绘屏幕的程序录出来是不对的。** 进度条、spinner、
/// 全屏 TUI 都属于这一类。`\r` 按「这一行重写」处理，所以单行 spinner
/// 会留下最后一帧，那是巧合下的正确，不是保证。
#[derive(Default)]
pub struct Screen {
    lines: Vec<Line>,
    current: Line,
    style: Style,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    fg: Option<Rgb>,
    bold: bool,
}

impl Screen {
    pub fn new() -> Self {
        Screen::default()
    }

    /// 喂一段输出。可以多次调用（stdout 与 stderr 分开捕获时）。
    pub fn feed(&mut self, text: &str) {
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\x1b' => self.escape(&mut chars),
                '\n' => {
                    self.lines.push(std::mem::take(&mut self.current));
                }
                // 回车 = 这一行重来。真正的终端是「光标回到列 0，
                // 后续字符覆盖旧的」；整行丢弃只在「后面会重写满」时等价，
                // 而那正是 spinner 的写法。
                '\r' => self.current = Line::default(),
                '\t' => self.push_text("    "),
                // 其余控制字符丢掉：画到 SVG 里是一个豆腐块
                c if (c as u32) < 0x20 => {}
                c => self.push_text(&c.to_string()),
            }
        }
    }

    /// 收尾，取出所有行。尾部的空行去掉——命令输出通常以换行结束，
    /// 留着会在录屏底下多出一块空白。
    pub fn finish(mut self) -> Vec<Line> {
        if !self.current.0.is_empty() {
            self.lines.push(std::mem::take(&mut self.current));
        }
        while self
            .lines
            .last()
            .is_some_and(|l| l.plain().trim().is_empty())
        {
            self.lines.pop();
        }
        self.lines
    }

    fn push_text(&mut self, s: &str) {
        // 同样式的相邻文本并进一个 span：一个字符一个 <tspan> 会让
        // 文件大出一个数量级，而渲染结果一模一样
        match self.current.0.last_mut() {
            Some(last) if last.fg == self.style.fg && last.bold == self.style.bold => {
                last.text.push_str(s)
            }
            _ => self.current.0.push(Span {
                text: s.to_string(),
                fg: self.style.fg,
                bold: self.style.bold,
            }),
        }
    }

    /// `ESC` 之后。只有 `ESC[…m` 会改样式，其余整段吃掉。
    fn escape(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>) {
        match chars.next() {
            Some('[') => {
                let mut params = String::new();
                for c in chars.by_ref() {
                    // CSI 以 0x40..=0x7E 的字符结束
                    if ('\x40'..='\x7e').contains(&c) {
                        if c == 'm' {
                            self.sgr(&params);
                        }
                        return;
                    }
                    params.push(c);
                }
            }
            // OSC（`ESC]…BEL`，改窗口标题那类）整段吃到 BEL 或 ST
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\x07' {
                        return;
                    }
                    if c == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    fn sgr(&mut self, params: &str) {
        // `ESC[m` 等价于 `ESC[0m`
        let codes: Vec<&str> = if params.is_empty() {
            vec!["0"]
        } else {
            params.split(';').collect()
        };
        let mut i = 0;
        while i < codes.len() {
            let n: u32 = codes[i].parse().unwrap_or(0);
            match n {
                0 => self.style = Style::default(),
                1 => self.style.bold = true,
                22 => self.style.bold = false,
                30..=37 => self.style.fg = Some(ansi16(n - 30, false)),
                90..=97 => self.style.fg = Some(ansi16(n - 90, true)),
                39 => self.style.fg = None,
                // 38;2;r;g;b 与 38;5;n。背景色**不认**：录屏里的反白色块
                // （严重度标签那种）画成前景色也读得懂，而认背景要给每个
                // span 加一个矩形，文件会大一倍。
                38 => {
                    let (color, used) = extended(&codes[i..]);
                    if let Some(c) = color {
                        self.style.fg = Some(c);
                    }
                    i += used;
                    continue;
                }
                48 => {
                    let (_, used) = extended(&codes[i..]);
                    i += used;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// `38;5;n` / `38;2;r;g;b`，返回颜色与吃掉的参数个数
fn extended(codes: &[&str]) -> (Option<Rgb>, usize) {
    match codes.get(1).and_then(|s| s.parse::<u32>().ok()) {
        Some(5) => {
            let n = codes.get(2).and_then(|s| s.parse::<u32>().ok());
            (n.map(ansi256), 3)
        }
        Some(2) => {
            let get = |i: usize| codes.get(i).and_then(|s| s.parse::<u8>().ok());
            match (get(2), get(3), get(4)) {
                (Some(r), Some(g), Some(b)) => (Some(Rgb(r, g, b)), 5),
                _ => (None, 5),
            }
        }
        // 认不出来就只吃掉 38/48 本身，剩下的参数当普通 SGR 继续解析。
        // 猜着多吃几个的话，后面真正的颜色码会被吞掉。
        _ => (None, 1),
    }
}

/// 16 色 → RGB，取的是本项目自己的配色。
///
/// 换句话说，录屏用的是 repolish 的终端主题，而不是读者机器上那套。
/// 这是有意的：录屏是一张要贴进 README 的图，它在谁的屏幕上都该长一样。
fn ansi16(i: u32, bright: bool) -> Rgb {
    use crate::theme::*;
    let dim = [
        INK,
        RED,
        Rgb(0x5C, 0xA8, 0x3A),
        Rgb(0xC9, 0x9A, 0x2E),
        PURPLE,
        PINK,
        Rgb(0x35, 0xB5, 0xA5),
        TEXT,
    ];
    let lit = [
        MUTED_ON_INK,
        RED,
        LIME,
        AMBER,
        PURPLE,
        PINK,
        CYAN,
        Rgb(0xFF, 0xFF, 0xFF),
    ];
    let table = if bright { lit } else { dim };
    table[(i as usize).min(7)]
}

/// 256 色 → RGB。前 16 个走 [`ansi16`]，然后是 6×6×6 色立方，最后 24 级灰阶。
fn ansi256(n: u32) -> Rgb {
    match n {
        0..=7 => ansi16(n, false),
        8..=15 => ansi16(n - 8, true),
        16..=231 => {
            const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
            let i = n - 16;
            Rgb(
                STEPS[(i / 36) as usize],
                STEPS[(i / 6 % 6) as usize],
                STEPS[(i % 6) as usize],
            )
        }
        232..=255 => {
            let v = (8 + (n - 232) * 10) as u8;
            Rgb(v, v, v)
        }
        _ => crate::theme::TEXT,
    }
}

// ── 时间轴 ──────────────────────────────────────────────────

struct Window {
    /// 开始敲这一步
    start_ms: u32,
    /// 敲完
    typed_ms: u32,
    /// 输出出现
    output_ms: u32,
    /// 这一步结束（下一步 clear，或整段结束）
    end_ms: u32,
}

struct Plan {
    windows: Vec<Window>,
    total_ms: u32,
    /// 开场定格结束的时刻，也就是第一步开始敲的时刻
    poster_ms: u32,
}

impl Plan {
    /// 毫秒 → keyframes 的百分比。保留三位小数：
    /// 一段 20 秒的录屏里，1% 是 200 毫秒，四舍五入到整数会让字符抖起来。
    fn pct(&self, ms: u32) -> f32 {
        (ms as f32 / self.total_ms.max(1) as f32 * 100.0).clamp(0.0, 100.0)
    }
}

fn schedule(steps: &[Step], t: &Timing) -> Plan {
    let mut windows = Vec::new();
    // 开场先定格在终态，然后才从第一步开始敲
    let mut cursor = t.poster_ms;
    for step in steps {
        let start = cursor;
        let typed = start + step.command.chars().count() as u32 * t.type_ms;
        let output = typed + t.submit_ms;
        // 输出越长给的时间越多，但有上限：读者不是在读一份文档，
        // 而是在看「这条命令跑出来长这样」
        let hold = t.hold_ms + (step.output.len() as u32).min(24) * 60;
        let end = output + hold;
        windows.push(Window {
            start_ms: start,
            typed_ms: typed,
            output_ms: output,
            end_ms: end,
        });
        cursor = end;
    }
    Plan {
        total_ms: cursor + t.loop_ms,
        windows,
        poster_ms: t.poster_ms,
    }
}

// ── 画 ──────────────────────────────────────────────────────

/// 窗口外壳：标题栏 + 三颗灯。
///
/// 不是装饰：一张没有边框的深色图片，在深色页面上根本看不出边界在哪。
/// 三颗灯是「这是一个终端」最省字的说法。
fn chrome(out: &mut String, w: i32, p: &Palette, steps: &[Step]) {
    out.push_str(&draw::rect(0, 0, w, CHROME_H as i32, 10.0, p.panel));
    // 圆角只留给顶上两个角：下半截要和正文接上
    out.push_str(&draw::rect(
        0,
        (CHROME_H / 2.0) as i32,
        w,
        (CHROME_H / 2.0).ceil() as i32,
        0.0,
        p.panel,
    ));
    for (i, c) in [p.bad, p.warn, p.bands[0]].iter().enumerate() {
        out.push_str(&draw::circle(20 + i as i32 * 18, 17, 5, *c));
    }
    // 标题写第一条命令的第一个词——录的是什么，一眼就知道
    let title = steps
        .first()
        .and_then(|s| s.command.split_whitespace().next())
        .unwrap_or("terminal");
    out.push_str(&draw::text(
        title,
        w / 2,
        22,
        12,
        p.muted,
        draw::Anchor::Middle,
        false,
    ));
}

#[allow(clippy::too_many_arguments)]
fn draw_step(
    i: usize,
    step: &Step,
    win: &Window,
    plan: &Plan,
    p: &Palette,
    cols: usize,
    last: bool,
) -> String {
    let _ = (win, plan);
    let mut out = String::new();
    let x = PAD;
    let top = CHROME_H + PAD;

    // 最后一步额外挂一个 `last`：关掉动画的读者看到的就是它的终态
    let mark = if last { " last" } else { "" };
    let _ = writeln!(out, r#"  <g class="step s{i}{mark}">"#);

    // 提示符 + 命令。命令是**真的文字**，可以选中复制——
    // 这正是动画 SVG 相对 GIF 最实在的一处好处。
    let _ = writeln!(
        out,
        r#"    <text class="t" x="{x:.0}" y="{:.1}" font-size="{FONT_SIZE}" xml:space="preserve"><tspan fill="{}">$</tspan><tspan fill="{}"> {}</tspan></text>"#,
        top + FONT_SIZE,
        p.brand[1],
        p.text,
        draw::esc(&clip_cols(&step.command, cols.saturating_sub(2)))
    );

    // 打字：一块与底色同色的遮板盖住命令，向右滑开。
    // 逐字符生成 <text> 也能做到，但那是几十个元素换同一个效果。
    let cover_w = (cols as f32 + 2.0) * CHAR_W;
    let cover_x = x + 2.0 * CHAR_W;
    let _ = writeln!(
        out,
        r#"    <g class="cover c{i}"><rect x="{cover_x:.1}" y="{:.1}" width="{cover_w:.1}" height="{LINE_H:.1}" fill="{}"/><rect x="{cover_x:.1}" y="{:.1}" width="{CHAR_W:.1}" height="{:.1}" fill="{}" opacity="0.85"/></g>"#,
        top,
        p.bg,
        top + 2.0,
        LINE_H - 4.0,
        p.brand[1],
    );

    // 输出
    let _ = writeln!(out, r#"    <g class="o{i}">"#);
    for (row, line) in step.output.iter().take(MAX_ROWS).enumerate() {
        if line.0.is_empty() {
            continue;
        }
        let y = top + (row as f32 + 2.0) * LINE_H;
        let _ = write!(
            out,
            r#"      <text class="t" x="{x:.0}" y="{y:.1}" font-size="{FONT_SIZE}" xml:space="preserve">"#
        );
        let mut used = 0usize;
        for span in &line.0 {
            if used >= cols {
                break;
            }
            let text = clip_cols(&span.text, cols - used);
            used += text.chars().count();
            let weight = if span.bold {
                r#" font-weight="700""#
            } else {
                ""
            };
            let _ = write!(
                out,
                r#"<tspan fill="{}"{weight}>{}</tspan>"#,
                span.fg.unwrap_or(p.text),
                draw::esc(&text)
            );
        }
        let _ = writeln!(out, "</text>");
    }
    let _ = writeln!(out, "    </g>");
    let _ = writeln!(out, "  </g>");
    out
}

/// 每一步三段动画的 CSS。
///
/// 每个元素的 `animation-duration` 都是**整段总时长**，出现与消失全靠
/// keyframes 的百分比表达。这是能循环的唯一写法：`animation-delay` 只在
/// 第一轮生效，用它来排时间的话第二轮所有东西会一起冒出来。
fn step_css(i: usize, win: &Window, plan: &Plan, step: &Step, last: bool) -> String {
    let chars = step.command.chars().count().max(1);
    let (a, b) = (plan.pct(win.start_ms), plan.pct(win.typed_ms));
    let (c, d) = (plan.pct(win.output_ms), plan.pct(win.end_ms));
    let poster = plan.pct(plan.poster_ms);

    // 切换用 step-end：一步的结束和下一步的开始之间不该有淡入淡出，
    // 终端不会淡出
    if !last {
        return format!(
            "    .s{i} {{ animation: s{i} var(--d) step-end infinite; }}\n\
             \x20   @keyframes s{i} {{ 0%,{a:.3}% {{ opacity: 0 }} {a:.3}%,{d:.3}% {{ opacity: 1 }} {d:.3}%,100% {{ opacity: 0 }} }}\n\
             \x20   .c{i} {{ animation: c{i} var(--d) steps({chars}, end) infinite; }}\n\
             \x20   @keyframes c{i} {{ 0%,{a:.3}% {{ transform: translateX(0) }} {b:.3}%,100% {{ transform: translateX(var(--w{i})) }} }}\n\
             \x20   .o{i} {{ animation: o{i} var(--d) step-end infinite; }}\n\
             \x20   @keyframes o{i} {{ 0%,{c:.3}% {{ opacity: 0 }} {c:.3}%,100% {{ opacity: 1 }} }}\n"
        );
    }

    // 最后一步出现**两次**：开头的定格，以及它自己的那一段。
    // 时间轴因此首尾相接，循环回去时画面不跳。
    //
    // 遮板这一条要在一个 keyframes 里混两种 timing function：定格段是瞬跳
    // （step-end），敲字段才是逐字符（steps）。写在关键帧内部的
    // `animation-timing-function` 管的是**从这一帧开始的那一段**。
    format!(
        "    .s{i} {{ animation: s{i} var(--d) step-end infinite; }}\n\
         \x20   @keyframes s{i} {{ 0%,{poster:.3}% {{ opacity: 1 }} {poster:.3}%,{a:.3}% {{ opacity: 0 }} {a:.3}%,100% {{ opacity: 1 }} }}\n\
         \x20   .c{i} {{ animation: c{i} var(--d) infinite; }}\n\
         \x20   @keyframes c{i} {{\n\
         \x20     0% {{ transform: translateX(var(--w{i})); animation-timing-function: step-end }}\n\
         \x20     {a:.3}% {{ transform: translateX(0); animation-timing-function: steps({chars}, end) }}\n\
         \x20     {b:.3}%,100% {{ transform: translateX(var(--w{i})) }}\n\
         \x20   }}\n\
         \x20   .o{i} {{ animation: o{i} var(--d) step-end infinite; }}\n\
         \x20   @keyframes o{i} {{ 0%,{poster:.3}% {{ opacity: 1 }} {poster:.3}%,{c:.3}% {{ opacity: 0 }} {c:.3}%,100% {{ opacity: 1 }} }}\n"
    )
}

fn document(
    body: &str,
    css: &str,
    w: i32,
    h: i32,
    p: &Palette,
    lang: &str,
    steps: &[Step],
) -> String {
    let mut s = String::new();
    let aria = format!(
        "terminal recording: {}",
        steps
            .iter()
            .map(|s| s.command.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" lang=\"{lang}\" role=\"img\" aria-label=\"{}\">",
        draw::esc(&aria)
    );
    s.push_str(&format!(
        "  <style>\n    .t {{ font-family: {}; white-space: pre; }}\n",
        draw::FONT
    ));
    s.push_str(css);
    // 关掉动画的人看到的必须是**跑完之后的样子**，不是一张空终端。
    // prefers-reduced-motion 不是「少动一点」，是「别动」——
    // 而一张什么都没有的图对他们等于没有这张图。
    //
    // 时间轴上的开场定格（见 `Timing::poster_ms`）管的是另一批读者：
    // 那些把 SVG 冻在第 0 帧的渲染器。两条兜底针对两种情况，都需要。
    s.push_str(
        "    @media (prefers-reduced-motion: reduce) {\n      \
         * { animation: none !important }\n      \
         .step { opacity: 0 }\n      \
         .last { opacity: 1 }\n      \
         .cover { transform: translateX(var(--wlast)) }\n    }\n",
    );
    s.push_str("  </style>\n");
    s.push_str(&draw::rect(0, 0, w, h, 10.0, p.bg));
    let _ = writeln!(
        s,
        "  <rect x=\"0.5\" y=\"0.5\" width=\"{}\" height=\"{}\" rx=\"9.5\" fill=\"none\" stroke=\"{}\"/>",
        w - 1,
        h - 1,
        p.line
    );
    s.push_str(body);
    let _ = writeln!(s, "</svg>");
    s
}

/// 按列截断。超宽的行画出去会盖到窗口外面。
fn clip_cols(s: &str, cols: usize) -> String {
    if s.chars().count() <= cols {
        return s.to_string();
    }
    s.chars().take(cols.saturating_sub(1)).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::DARK;

    fn steps() -> Vec<Step> {
        vec![
            Step {
                command: "repolish".into(),
                output: vec![Line(vec![Span {
                    text: "SCORE 23 / 100".into(),
                    fg: Some(DARK.bad),
                    bold: true,
                }])],
            },
            Step {
                command: "repolish --apply".into(),
                output: vec![Line(vec![Span {
                    text: "Written.".into(),
                    fg: None,
                    bold: false,
                }])],
            },
        ]
    }

    #[test]
    fn is_a_self_contained_svg_with_no_scripts() {
        let svg = cast(&steps(), &Timing::default(), &Options::default());
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.trim_end().ends_with("</svg>"));
        crate::draw::assert_self_contained(&svg);
        assert!(!svg.contains("<animate"), "SMIL 不用，只用 CSS");
    }

    /// 同一次录制必须逐字节一致，否则每次重录都是一个纯噪声的 diff
    #[test]
    fn the_same_recording_renders_byte_identical_svg() {
        let (t, o) = (Timing::default(), Options::default());
        assert_eq!(cast(&steps(), &t, &o), cast(&steps(), &t, &o));
    }

    /// 这是动画 SVG 相对 GIF 最实在的一处好处：命令是真的文字
    #[test]
    fn the_typed_command_is_selectable_text() {
        let svg = cast(&steps(), &Timing::default(), &Options::default());
        assert!(svg.contains("repolish"));
        assert!(svg.contains("repolish --apply"));
        assert!(svg.contains("SCORE 23 / 100"));
    }

    /// 每个元素的 duration 都是整段总时长，出现时机靠 keyframes 百分比表达。
    /// 用 animation-delay 的话第二轮所有东西会一起冒出来。
    #[test]
    fn every_animation_shares_one_looping_duration() {
        let svg = cast(&steps(), &Timing::default(), &Options::default());
        assert_eq!(svg.matches("var(--d)").count(), 6, "两步各三段动画");
        assert!(!svg.contains("animation-delay"));
        assert_eq!(svg.matches("infinite").count(), 6);
    }

    /// 第 0 帧必须是「跑完之后的样子」。任何把这张图冻在 t=0 的渲染器
    /// ——有的浏览器就不给 <img> 里的 SVG 跑动画——否则拿到的是一张空终端。
    #[test]
    fn the_loop_opens_on_the_finished_state_not_an_empty_terminal() {
        let t = Timing::default();
        let plan = schedule(&steps(), &t);
        assert_eq!(plan.poster_ms, t.poster_ms);
        assert_eq!(plan.windows[0].start_ms, t.poster_ms, "第一步不该从 0 开始");

        let svg = cast(&steps(), &t, &Options::default());
        // 最后一步在 0% 处可见，遮板也已经推到底
        let last = svg
            .split("@keyframes s1 {")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert!(last.trim_start().starts_with("0%,"), "{last}");
        assert!(last.contains("opacity: 1"), "{last}");
        assert!(svg.contains(
            "0% { transform: translateX(var(--w1)); animation-timing-function: step-end }"
        ));
    }

    /// 静态兜底靠语义类名，不靠下标——「最后一步」是第几步由内容决定
    #[test]
    fn the_static_fallback_targets_classes_that_actually_exist() {
        let svg = cast(&steps(), &Timing::default(), &Options::default());
        // 元素上真正出现过的 class 词
        let mut present = std::collections::HashSet::new();
        for node in svg.split("class=\"").skip(1) {
            for word in node.split('"').next().unwrap_or("").split_whitespace() {
                present.insert(word.to_string());
            }
        }
        for class in ["step", "last", "cover"] {
            assert!(
                svg.contains(&format!(".{class}")),
                "兜底样式没引用 .{class}"
            );
            assert!(
                present.contains(class),
                ".{class} 只在 CSS 里，没有元素带它"
            );
        }
        assert!(svg.contains("--wlast:"));
    }

    #[test]
    fn steps_do_not_overlap_on_the_timeline() {
        let t = Timing::default();
        let plan = schedule(&steps(), &t);
        for w in plan.windows.windows(2) {
            assert!(w[0].end_ms <= w[1].start_ms, "两步的时间窗重叠了");
        }
        for w in &plan.windows {
            assert!(w.start_ms < w.typed_ms);
            assert!(w.typed_ms < w.output_ms);
            assert!(w.output_ms < w.end_ms);
        }
        assert!(plan.total_ms > plan.windows.last().unwrap().end_ms);
    }

    /// 节奏是固定值，不来自真实耗时——否则同一次录制在快慢两台机器上
    /// 会产出不同的文件
    #[test]
    fn the_timeline_ignores_how_long_the_commands_actually_took() {
        let t = Timing::default();
        let all = steps();
        let plan = schedule(&all, &t);
        let first = &plan.windows[0];
        // 从命令本身的长度算，而不是写死一个数：改一次夹具就得改一次断言，
        // 那种断言迟早会被人顺手改成「当前值」，也就再也守不住任何东西。
        let typed = all[0].command.chars().count() as u32;
        assert_eq!(first.typed_ms - first.start_ms, typed * t.type_ms);
    }

    #[test]
    fn overlong_lines_are_clipped_rather_than_drawn_past_the_window() {
        let long = Step {
            command: "x".repeat(400),
            output: vec![Line(vec![Span {
                text: "y".repeat(400),
                fg: None,
                bold: false,
            }])],
        };
        let svg = cast(&[long], &Timing::default(), &Options::default());
        let w: i32 = svg
            .split(r#"width=""#)
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let cap = (PAD * 2.0 + MAX_COLS as f32 * CHAR_W).ceil() as i32;
        assert_eq!(w, cap, "窗口该正好停在 MAX_COLS 上");
        assert!(svg.contains('…'));
    }

    #[test]
    fn an_empty_recording_renders_a_valid_document() {
        let svg = cast(&[], &Timing::default(), &Options::default());
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn xml_special_characters_in_output_are_escaped() {
        let s = Step {
            command: "echo '<b>&'".into(),
            output: vec![Line(vec![Span {
                text: "<b>&".into(),
                fg: None,
                bold: false,
            }])],
        };
        let svg = cast(&[s], &Timing::default(), &Options::default());
        assert!(svg.contains("&lt;b&gt;&amp;"));
        assert!(!svg.contains("<b>&<"));
    }

    // ── ANSI 解析 ──

    fn parse(s: &str) -> Vec<Line> {
        let mut screen = Screen::new();
        screen.feed(s);
        screen.finish()
    }

    #[test]
    fn plain_text_splits_into_lines() {
        let lines = parse("one\ntwo\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].plain(), "one");
        assert_eq!(lines[1].plain(), "two");
    }

    #[test]
    fn truecolor_and_bold_reach_the_spans() {
        let lines = parse("\x1b[1m\x1b[38;2;255;95;209mpink\x1b[0m plain\n");
        assert_eq!(lines[0].0[0].text, "pink");
        assert_eq!(lines[0].0[0].fg, Some(crate::theme::PINK));
        assert!(lines[0].0[0].bold);
        assert_eq!(lines[0].0[1].text, " plain");
        assert_eq!(lines[0].0[1].fg, None);
        assert!(!lines[0].0[1].bold);
    }

    #[test]
    fn the_256_colour_cube_and_greyscale_ramp_are_decoded() {
        assert_eq!(ansi256(16), Rgb(0, 0, 0));
        assert_eq!(ansi256(231), Rgb(255, 255, 255));
        assert_eq!(ansi256(196), Rgb(255, 0, 0));
        assert_eq!(ansi256(232), Rgb(8, 8, 8));
        let lines = parse("\x1b[38;5;196mred\n");
        assert_eq!(lines[0].0[0].fg, Some(Rgb(255, 0, 0)));
    }

    /// 认不出的转义序列不能把后面的文字吃掉
    #[test]
    fn unknown_escape_sequences_are_skipped_not_printed() {
        let lines = parse("\x1b[2J\x1b[1;1Hafter\n");
        assert_eq!(lines[0].plain(), "after");
        // OSC 改标题那类整段吃掉
        assert_eq!(parse("\x1b]0;title\x07text\n")[0].plain(), "text");
    }

    /// 背景色不认，但它的参数必须被正确吃掉，
    /// 否则后面真正的前景色码会被当成别的东西
    #[test]
    fn a_background_colour_does_not_swallow_the_foreground_that_follows() {
        let lines = parse("\x1b[48;2;0;0;0;38;2;67;229;208mcyan\n");
        assert_eq!(lines[0].0[0].fg, Some(crate::theme::CYAN));
    }

    /// 一个字符一个 span 会让文件大一个数量级，渲染结果却一模一样
    #[test]
    fn runs_of_the_same_style_merge_into_one_span() {
        assert_eq!(parse("hello world\n")[0].0.len(), 1);
    }

    /// spinner 的写法就是「回车之后把这一行重写一遍」
    #[test]
    fn a_carriage_return_restarts_the_line() {
        assert_eq!(parse("50%\r100% done\n")[0].plain(), "100% done");
    }

    #[test]
    fn trailing_blank_lines_are_dropped() {
        // 命令输出几乎总以换行结束，留着会在录屏底下多出一块空白
        assert_eq!(parse("text\n\n\n").len(), 1);
        assert!(parse("\n\n").is_empty());
    }

    #[test]
    fn control_characters_do_not_become_tofu_blocks() {
        assert_eq!(parse("a\x07b\n")[0].plain(), "ab");
        assert_eq!(parse("a\tb\n")[0].plain(), "a    b");
    }

    /// 关掉动画的用户看到的必须是最后一步的最终状态，不是一张空图
    #[test]
    fn reduced_motion_still_shows_something() {
        let svg = cast(&steps(), &Timing::default(), &Options::default());
        assert!(svg.contains("prefers-reduced-motion"));
        assert!(svg.contains(".last"));
    }
}
