//! 霓虹配色：终端与 SVG 卡片共用的唯一一份色板。
//!
//! 颜色只在这里定义。终端按终端能力降级（truecolor → 256 → 16 → 无色），
//! SVG 直接吐十六进制——两边取的是同一组常量，品牌才不会分叉成两套。
//!
//! 分数分档沿用 `Report::color()` 的阈值（90 / 75 / 60 / 40）。终端里的颜色
//! 和徽章上的颜色必须是同一个判断，否则同一个仓库会有两种说法。

use std::fmt;

// 分档不在这里判 —— 它是分数的属性，住在 core，徽章颜色用的是同一个函数
use repolish_core::band_index;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// 以 `#rrggbb` 输出，SVG 直接用
impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl Rgb {
    /// 朝 `other` 走 `t`（0.0..=1.0）。用于 wordmark 的横向渐变。
    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(f(self.0, other.0), f(self.1, other.1), f(self.2, other.2))
    }
}

// ── 品牌三色 ────────────────────────────────────────────────
pub const PURPLE: Rgb = Rgb(0x7D, 0x56, 0xF4);
pub const PINK: Rgb = Rgb(0xFF, 0x5F, 0xD1);
pub const CYAN: Rgb = Rgb(0x43, 0xE5, 0xD0);

// ── 中性色 ──────────────────────────────────────────────────
pub const INK: Rgb = Rgb(0x1A, 0x1A, 0x24);
pub const INK_SOFT: Rgb = Rgb(0x22, 0x21, 0x30);
pub const TEXT: Rgb = Rgb(0xED, 0xEB, 0xFA);
pub const MUTED: Rgb = Rgb(0x7B, 0x77, 0x93);
/// SVG 上的弱色文字。比终端的 [`MUTED`] 亮一档。
///
/// 两者不能是同一个值：终端的底色不由我们决定，`MUTED` 挑的是一个在深浅
/// 两种终端里都还过得去的中间值；SVG **自己画底**，所以能按真实对比度来
/// 定——`MUTED` 落在 `INK` 上只有 4.0:1，够不着 WCAG AA。卡片上的次要信息
/// （项目简介、页脚、单位）几乎全是这个颜色，读不清就等于没写。
pub const MUTED_ON_INK: Rgb = Rgb(0x8B, 0x87, 0xA3);
pub const LINE: Rgb = Rgb(0x35, 0x33, 0x4A);
/// 条形图未填部分。比分隔线亮一档——分段条的意义就是让缺的那一格看得见，
/// 轨道太暗的话 99 和 100 又变回长得一样了。
pub const TRACK: Rgb = Rgb(0x3E, 0x3B, 0x58);

// ── 判定色 ──────────────────────────────────────────────────
pub const RED: Rgb = Rgb(0xFF, 0x4F, 0x6E);
pub const ORANGE: Rgb = Rgb(0xFF, 0x8F, 0x5F);
pub const AMBER: Rgb = Rgb(0xFF, 0xC5, 0x3D);
pub const LIME: Rgb = Rgb(0xA9, 0xF0, 0x5F);

/// 分数 → 颜色。终端用的那一套，与 [`DARK`] 同源。
pub fn band(score: u8) -> Rgb {
    DARK.bands[band_index(score)]
}

/// wordmark 的横向渐变：紫 → 粉 → 青。终端没有色板可选，走深色那一套。
pub fn sweep(t: f32) -> Rgb {
    DARK.sweep(t)
}

// ── SVG 色板 ────────────────────────────────────────────────

/// 一张卡片的全部用色。
///
/// 终端只有一套配色（终端底色不由我们决定，只能挑一组在深浅底上都立得住的
/// 前景色），SVG 不一样：SVG 自己画底，所以可以有深浅两套。给的是**两套完整
/// 色板**而不是「深色版加几个覆盖」——半套色板迟早会漏掉一个常量，
/// 在浅底上留下一块看不见的深色文字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub name: &'static str,
    /// 卡片底色
    pub bg: Rgb,
    /// 内嵌面板 / 图表底
    pub panel: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub line: Rgb,
    /// 条形图未填部分
    pub track: Rgb,
    /// 多序列图表的取色环，按下标循环
    pub series: [Rgb; 5],
    pub warn: Rgb,
    pub bad: Rgb,
    /// 分数分档的五档色，从 excellent 到 poor
    pub bands: [Rgb; 5],
    /// 品牌标记的三段渐变
    pub brand: [Rgb; 3],
    /// 标记内部的镂空色，必须与 `bg` 对得上
    pub mark_hollow: Rgb,
}

impl Palette {
    /// 分数 → 颜色。分档由 [`band_index`] 决定，与终端是同一个判断。
    pub fn band(&self, score: u8) -> Rgb {
        self.bands[band_index(score)]
    }

    /// 序列色环。下标超出就绕回去，条目再多也不会取到一个没定义的颜色。
    pub fn series(&self, i: usize) -> Rgb {
        self.series[i % self.series.len()]
    }

    /// 品牌渐变上 `t`（0.0..=1.0）处的颜色
    pub fn sweep(&self, t: f32) -> Rgb {
        if t < 0.5 {
            self.brand[0].mix(self.brand[1], t * 2.0)
        } else {
            self.brand[1].mix(self.brand[2], (t - 0.5) * 2.0)
        }
    }

    pub fn parse(s: &str) -> Option<&'static Palette> {
        match s.to_lowercase().as_str() {
            "dark" | "neon" => Some(&DARK),
            "porcelain" | "light" | "cream" => Some(&PORCELAIN),
            _ => None,
        }
    }
}

/// 默认色板：与终端同源的霓虹深色。
pub const DARK: Palette = Palette {
    name: "dark",
    bg: INK,
    panel: INK_SOFT,
    text: TEXT,
    muted: MUTED_ON_INK,
    line: LINE,
    track: TRACK,
    series: [PURPLE, PINK, CYAN, LIME, AMBER],
    warn: AMBER,
    bad: RED,
    bands: [CYAN, LIME, AMBER, ORANGE, RED],
    brand: [PURPLE, PINK, CYAN],
    mark_hollow: INK,
};

/// 浅色色板：暖白纸底，深墨字。
///
/// 存在的理由不是「有人喜欢浅色」，是**可读性**：一张深底卡片贴进一份
/// 以浅色为主的 README，在页面上就是一块挖空。让作者能选一张跟着自己
/// 版面走的卡片，比让所有人迁就我们的品牌色重要。
pub const PORCELAIN: Palette = Palette {
    name: "porcelain",
    bg: Rgb(0xF6, 0xF1, 0xE6),
    panel: Rgb(0xEC, 0xE5, 0xD6),
    text: Rgb(0x2B, 0x21, 0x18),
    muted: Rgb(0x71, 0x66, 0x56),
    line: Rgb(0xD8, 0xCE, 0xBB),
    track: Rgb(0xDF, 0xD6, 0xC4),
    // 浅底上霓虹色会糊成一片，改用同一族的深浅阶——
    // 序列之间靠明度区分，打印成黑白也还分得开
    series: [
        Rgb(0x2B, 0x21, 0x18),
        Rgb(0x5A, 0x47, 0x33),
        Rgb(0x8A, 0x74, 0x57),
        Rgb(0xB0, 0x9C, 0x80),
        Rgb(0xCB, 0xBD, 0xA6),
    ],
    warn: Rgb(0xB5, 0x7A, 0x0B),
    bad: Rgb(0xB4, 0x33, 0x2B),
    bands: [
        Rgb(0x1E, 0x6F, 0x63),
        Rgb(0x4B, 0x7A, 0x27),
        Rgb(0xB5, 0x7A, 0x0B),
        Rgb(0xC0, 0x5C, 0x1E),
        Rgb(0xB4, 0x33, 0x2B),
    ],
    brand: [PURPLE, PINK, Rgb(0x1E, 0x9E, 0x8C)],
    mark_hollow: Rgb(0xF6, 0xF1, 0xE6),
};

#[cfg(test)]
mod palette_tests {
    use super::*;

    /// 两套色板必须在同一个阈值上翻面，否则同一个仓库会有两种说法
    #[test]
    fn both_palettes_band_on_the_same_thresholds() {
        for p in [&DARK, &PORCELAIN] {
            assert_eq!(p.band(90), p.bands[0]);
            assert_eq!(p.band(89), p.bands[1]);
            assert_eq!(p.band(74), p.bands[2]);
            assert_eq!(p.band(39), p.bands[4]);
        }
        // 深色板与终端共用同一组常量
        assert_eq!(DARK.band(95), band(95));
        assert_eq!(DARK.band(20), band(20));
    }

    /// WCAG 相对亮度。必须做 sRGB 反伽马——按原始通道值直接算，
    /// 深色底的对比度会被系统性低估，一个其实合格的色板会被判不合格。
    #[cfg(test)]
    fn luminance(c: Rgb) -> f32 {
        fn channel(v: u8) -> f32 {
            let s = v as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(c.0) + 0.7152 * channel(c.1) + 0.0722 * channel(c.2)
    }

    #[cfg(test)]
    fn contrast(a: Rgb, b: Rgb) -> f32 {
        let (x, y) = (luminance(a) + 0.05, luminance(b) + 0.05);
        if x > y {
            x / y
        } else {
            y / x
        }
    }

    /// 文字与底色的对比度不够，卡片就是一张糊的图。
    /// 正文按 WCAG AAA（7:1）要求，弱色文字按 AA（4.5:1）——
    /// 弱色本来就是「次要信息」，但次要不等于读不清。
    #[test]
    fn text_contrasts_with_the_background_in_both_palettes() {
        for p in [&DARK, &PORCELAIN] {
            let body = contrast(p.text, p.bg);
            assert!(body >= 7.0, "{} 的正文对比度只有 {body:.1}", p.name);
            let muted = contrast(p.muted, p.bg);
            assert!(muted >= 4.5, "{} 的弱色对比度只有 {muted:.1}", p.name);
        }
    }

    /// 分档色是要被读出数值的，不能只靠色相区分
    #[test]
    fn every_band_colour_is_legible_on_its_own_background() {
        for p in [&DARK, &PORCELAIN] {
            for (i, c) in p.bands.iter().enumerate() {
                let ratio = contrast(*c, p.bg);
                assert!(ratio >= 3.0, "{} 第 {i} 档对比度只有 {ratio:.1}", p.name);
            }
        }
    }

    /// 镂空色画在标记上，与卡片底色不一致就会露出一圈边
    #[test]
    fn the_mark_hollow_matches_the_card_background() {
        assert_eq!(DARK.mark_hollow, DARK.bg);
        assert_eq!(PORCELAIN.mark_hollow, PORCELAIN.bg);
    }

    #[test]
    fn series_colours_wrap_instead_of_running_out() {
        assert_eq!(DARK.series(0), DARK.series(5));
        assert_eq!(DARK.series(7), DARK.series(2));
    }
}

// ── 终端色彩能力 ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorLevel {
    None,
    Ansi16,
    Ansi256,
    TrueColor,
}

impl ColorLevel {
    /// 从环境探测。优先级：显式关闭 > 显式强制 > 终端自述 > 是否 tty。
    ///
    /// `NO_COLOR` 与 `CLICOLOR_FORCE` 都是既成约定，两个都认；
    /// 不是 tty 时默认关色，否则重定向到文件的输出会带一堆转义序列。
    pub fn detect(user_disabled: bool, is_tty: bool) -> ColorLevel {
        if user_disabled || env_set("NO_COLOR") || env_eq("TERM", "dumb") {
            return ColorLevel::None;
        }
        let forced = env_set("CLICOLOR_FORCE") || env_set("FORCE_COLOR");
        if !is_tty && !forced {
            return ColorLevel::None;
        }
        depth()
    }
}

/// 终端自述的色深。Windows Terminal / VS Code 不设 `COLORTERM`，单独认。
fn depth() -> ColorLevel {
    if let Some(v) = var("COLORTERM") {
        if v.contains("truecolor") || v.contains("24bit") {
            return ColorLevel::TrueColor;
        }
    }
    if env_set("WT_SESSION")
        || env_eq("TERM_PROGRAM", "vscode")
        || env_eq("TERM_PROGRAM", "iTerm.app")
    {
        return ColorLevel::TrueColor;
    }
    match var("TERM") {
        Some(t) if t.contains("256") => ColorLevel::Ansi256,
        Some(_) => ColorLevel::Ansi16,
        // Windows 的 conhost / Terminal 都不设 TERM，但 VT 已经开起来了
        None if cfg!(windows) => ColorLevel::TrueColor,
        None => ColorLevel::None,
    }
}

fn var(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

fn env_set(k: &str) -> bool {
    var(k).is_some()
}

fn env_eq(k: &str, v: &str) -> bool {
    var(k).is_some_and(|got| got == v)
}

// ── 转义序列 ────────────────────────────────────────────────

/// 前景色的 SGR 序列。`ColorLevel::None` 返回空串，调用方无需分支。
pub fn fg(c: Rgb, level: ColorLevel) -> String {
    match level {
        ColorLevel::None => String::new(),
        ColorLevel::TrueColor => format!("\x1b[38;2;{};{};{}m", c.0, c.1, c.2),
        ColorLevel::Ansi256 => format!("\x1b[38;5;{}m", to_256(c)),
        ColorLevel::Ansi16 => format!("\x1b[{}m", to_16(c)),
    }
}

/// 背景色的 SGR 序列，给严重度色块用。
pub fn bg(c: Rgb, level: ColorLevel) -> String {
    match level {
        ColorLevel::None => String::new(),
        ColorLevel::TrueColor => format!("\x1b[48;2;{};{};{}m", c.0, c.1, c.2),
        ColorLevel::Ansi256 => format!("\x1b[48;5;{}m", to_256(c)),
        ColorLevel::Ansi16 => format!("\x1b[{}m", to_16(c) + 10),
    }
}

pub fn bold(level: ColorLevel) -> &'static str {
    if level == ColorLevel::None {
        ""
    } else {
        "\x1b[1m"
    }
}

pub fn reset(level: ColorLevel) -> &'static str {
    if level == ColorLevel::None {
        ""
    } else {
        "\x1b[0m"
    }
}

/// 6×6×6 色立方 + 24 级灰阶。灰色单独走灰阶，否则中性色会被染上色偏。
fn to_256(c: Rgb) -> u8 {
    let (r, g, b) = (c.0 as i32, c.1 as i32, c.2 as i32);
    if (r - g).abs() < 10 && (g - b).abs() < 10 && (r - b).abs() < 10 {
        let level = (r * 23 / 255) as u8;
        return 232 + level;
    }
    let q = |v: i32| (v * 5 / 255) as u8;
    16 + 36 * q(r) + 6 * q(g) + q(b)
}

/// 16 色下只保留色相，取最近的一个亮色/暗色。
fn to_16(c: Rgb) -> u8 {
    let (r, g, b) = (c.0 as u32, c.1 as u32, c.2 as u32);
    let bright = r.max(g).max(b) > 160;
    let base = 30
        + match (r > 110, g > 110, b > 110) {
            (false, false, false) => 0, // black
            (true, false, false) => 1,  // red
            (false, true, false) => 2,  // green
            (true, true, false) => 3,   // yellow
            (false, false, true) => 4,  // blue
            (true, false, true) => 5,   // magenta
            (false, true, true) => 6,   // cyan
            (true, true, true) => 7,    // white
        };
    if bright {
        base + 60
    } else {
        base
    }
}

/// Windows 的控制台默认不解释 ANSI，得先把 VT 模式打开。
/// 不引入 windows-sys：这里只要三个函数，自己声明比多一个依赖划算。
#[cfg(windows)]
pub fn enable_ansi() {
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    extern "system" {
        fn GetStdHandle(which: u32) -> isize;
        fn GetConsoleMode(handle: isize, mode: *mut u32) -> i32;
        fn SetConsoleMode(handle: isize, mode: u32) -> i32;
    }
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle == 0 || handle == -1 {
            return;
        }
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return;
        }
        SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
    }
}

#[cfg(not(windows))]
pub fn enable_ansi() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_lowercase_six_digits() {
        assert_eq!(PURPLE.to_string(), "#7d56f4");
        assert_eq!(INK.to_string(), "#1a1a24");
    }

    /// 终端配色与徽章颜色必须在同一个阈值上翻面
    #[test]
    fn bands_line_up_with_the_badge_thresholds() {
        // 颜色、词、徽章现在都从 `repolish_core::band_index` 派生，所以这条
        // 测试盯的是**那一个函数**在正确的分数上翻面。此前它只拿 band() 和
        // 自己比，名字里的 "badge" 从来没有被验证过。
        for (score, expect) in [
            (100, 0),
            (90, 0),
            (89, 1),
            (75, 1),
            (74, 2),
            (60, 2),
            (59, 3),
            (40, 3),
            (39, 4),
            (0, 4),
        ] {
            assert_eq!(band_index(score), expect, "分数 {score} 落错了档");
        }
        assert_eq!(band(90), CYAN);
        assert_eq!(band(39), RED);
        // 深浅两套色板与终端在同一个分数上翻面
        for score in [0u8, 39, 40, 59, 60, 74, 75, 89, 90, 100] {
            for p in [&DARK, &PORCELAIN] {
                assert_eq!(p.band(score), p.bands[band_index(score)]);
            }
        }
        // 分数旁边那个词也是同一个判断
        assert_eq!(crate::i18n::band_word(89, &crate::i18n::EN), "good");
        assert_eq!(crate::i18n::band_word(90, &crate::i18n::EN), "excellent");
    }

    #[test]
    fn no_color_emits_no_escapes() {
        assert_eq!(fg(PINK, ColorLevel::None), "");
        assert_eq!(bold(ColorLevel::None), "");
        assert_eq!(reset(ColorLevel::None), "");
    }

    #[test]
    fn truecolor_escape_carries_the_exact_channels() {
        assert_eq!(fg(PINK, ColorLevel::TrueColor), "\x1b[38;2;255;95;209m");
    }

    #[test]
    fn sweep_starts_purple_and_ends_cyan() {
        assert_eq!(sweep(0.0), PURPLE);
        assert_eq!(sweep(0.5), PINK);
        assert_eq!(sweep(1.0), CYAN);
    }

    /// 中性色走灰阶，不能被染上色偏
    #[test]
    fn greys_map_into_the_greyscale_ramp() {
        assert!(to_256(Rgb(0x80, 0x80, 0x80)) >= 232);
        assert!(to_256(PINK) < 232);
    }
}
