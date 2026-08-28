//! 霓虹配色：终端与 SVG 卡片共用的唯一一份色板。
//!
//! 颜色只在这里定义。终端按终端能力降级（truecolor → 256 → 16 → 无色），
//! SVG 直接吐十六进制——两边取的是同一组常量，品牌才不会分叉成两套。
//!
//! 分数分档沿用 `Report::color()` 的阈值（90 / 75 / 60 / 40）。终端里的颜色
//! 和徽章上的颜色必须是同一个判断，否则同一个仓库会有两种说法。

use std::fmt;

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
pub const LINE: Rgb = Rgb(0x35, 0x33, 0x4A);
/// 条形图未填部分。比分隔线亮一档——分段条的意义就是让缺的那一格看得见，
/// 轨道太暗的话 99 和 100 又变回长得一样了。
pub const TRACK: Rgb = Rgb(0x3E, 0x3B, 0x58);

// ── 判定色 ──────────────────────────────────────────────────
pub const RED: Rgb = Rgb(0xFF, 0x4F, 0x6E);
pub const ORANGE: Rgb = Rgb(0xFF, 0x8F, 0x5F);
pub const AMBER: Rgb = Rgb(0xFF, 0xC5, 0x3D);
pub const LIME: Rgb = Rgb(0xA9, 0xF0, 0x5F);

/// 分数 → 颜色。阈值与 `Report::color()` 一一对应。
pub fn band(score: u8) -> Rgb {
    match score {
        90..=255 => CYAN,
        75..=89 => LIME,
        60..=74 => AMBER,
        40..=59 => ORANGE,
        _ => RED,
    }
}

/// 分数 → 一个词。放在分数旁边，省得读者自己去查阈值表。
pub fn band_word(score: u8) -> &'static str {
    match score {
        90..=255 => "excellent",
        75..=89 => "good",
        60..=74 => "fair",
        40..=59 => "weak",
        _ => "poor",
    }
}

/// wordmark 的横向渐变：紫 → 粉 → 青。
pub fn sweep(t: f32) -> Rgb {
    if t < 0.5 {
        PURPLE.mix(PINK, t * 2.0)
    } else {
        PINK.mix(CYAN, (t - 0.5) * 2.0)
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
        assert_eq!(band(90), CYAN);
        assert_eq!(band(89), LIME);
        assert_eq!(band(75), LIME);
        assert_eq!(band(74), AMBER);
        assert_eq!(band(60), AMBER);
        assert_eq!(band(59), ORANGE);
        assert_eq!(band(40), ORANGE);
        assert_eq!(band(39), RED);
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
