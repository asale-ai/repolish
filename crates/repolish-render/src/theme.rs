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
/// 前景色），SVG 不一样：SVG 自己画底，所以想有几套就有几套——[`ALL`] 里
/// 现在有十四套。每一套给的都是**完整色板**而不是「默认版加几个覆盖」：
/// 半套色板迟早会漏掉一个常量，在浅底上留下一块看不见的深色文字。
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

    /// 名字 → 色板。别名认的是**别人已经在用的叫法**：一个 Gruvbox 用户
    /// 不会去猜我们把它叫成了 `ember`。
    pub fn parse(s: &str) -> Option<&'static Palette> {
        match s.to_lowercase().as_str() {
            "dark" | "neon" => Some(&DARK),
            "porcelain" | "light" | "cream" => Some(&PORCELAIN),
            "slate" | "github" => Some(&SLATE),
            "nord" => Some(&NORD),
            "ember" | "gruvbox" => Some(&EMBER),
            "solar" | "solarized" => Some(&SOLAR),
            "phosphor" | "crt" => Some(&PHOSPHOR),
            "blueprint" => Some(&BLUEPRINT),
            "okabe" | "okabe-ito" | "colorblind" => Some(&OKABE),
            "newsprint" | "swiss" => Some(&NEWSPRINT),
            "sakura" | "pastel" => Some(&SAKURA),
            "glacier" | "ice" => Some(&GLACIER),
            // `mono` 落在这里而不是 phosphor：绿色的单色仍然是一种颜色，
            // 敲 mono 的人要的是黑白
            "carbon" | "mono" | "bw" => Some(&CARBON),
            "paper" | "print" => Some(&PAPER),
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

/// GitHub 深色主题的蓝灰。
///
/// 存在的理由是**不打眼**：卡片贴进 GitHub 的深色页面时，底色与页面同源，
/// 看上去像页面自带的一块，而不是外挂上去的一张图。霓虹那套是有主张的，
/// 这一套没有主张——大多数仓库要的其实是后者。
pub const SLATE: Palette = Palette {
    name: "slate",
    bg: Rgb(0x0D, 0x11, 0x17),
    panel: Rgb(0x16, 0x1B, 0x22),
    text: Rgb(0xE6, 0xED, 0xF3),
    muted: Rgb(0x91, 0x98, 0xA1),
    line: Rgb(0x30, 0x36, 0x3D),
    track: Rgb(0x21, 0x26, 0x2D),
    series: [
        Rgb(0x58, 0xA6, 0xFF),
        Rgb(0xBC, 0x8C, 0xFF),
        Rgb(0x3F, 0xB9, 0x50),
        Rgb(0xD2, 0x99, 0x22),
        Rgb(0xFF, 0x7B, 0x72),
    ],
    warn: Rgb(0xD2, 0x99, 0x22),
    bad: Rgb(0xF8, 0x51, 0x49),
    bands: [
        Rgb(0x3F, 0xB9, 0x50),
        Rgb(0x56, 0xD3, 0x64),
        Rgb(0xD2, 0x99, 0x22),
        Rgb(0xDB, 0x6D, 0x28),
        Rgb(0xF8, 0x51, 0x49),
    ],
    brand: [
        Rgb(0x58, 0xA6, 0xFF),
        Rgb(0xBC, 0x8C, 0xFF),
        Rgb(0x3F, 0xB9, 0x50),
    ],
    mark_hollow: Rgb(0x0D, 0x11, 0x17),
};

/// 北欧低饱和：同样是深色，但所有色相都被压回中灰。
///
/// 霓虹在一屏代码旁边是会抢戏的。文档站、基础设施、库——这些项目的卡片
/// 该是配角，`nord` 是给它们的。
pub const NORD: Palette = Palette {
    name: "nord",
    bg: Rgb(0x2E, 0x34, 0x40),
    panel: Rgb(0x3B, 0x42, 0x52),
    text: Rgb(0xEC, 0xEF, 0xF4),
    muted: Rgb(0xB0, 0xB8, 0xC6),
    line: Rgb(0x43, 0x4C, 0x5E),
    track: Rgb(0x4C, 0x56, 0x6A),
    series: [
        Rgb(0x88, 0xC0, 0xD0),
        Rgb(0x81, 0xA1, 0xC1),
        Rgb(0xA3, 0xBE, 0x8C),
        Rgb(0xEB, 0xCB, 0x8B),
        Rgb(0xB4, 0x8E, 0xAD),
    ],
    warn: Rgb(0xEB, 0xCB, 0x8B),
    bad: Rgb(0xBF, 0x61, 0x6A),
    bands: [
        Rgb(0x8F, 0xBC, 0xBB),
        Rgb(0xA3, 0xBE, 0x8C),
        Rgb(0xEB, 0xCB, 0x8B),
        Rgb(0xD0, 0x87, 0x70),
        Rgb(0xBF, 0x61, 0x6A),
    ],
    brand: [
        Rgb(0x5E, 0x81, 0xAC),
        Rgb(0x88, 0xC0, 0xD0),
        Rgb(0xA3, 0xBE, 0x8C),
    ],
    mark_hollow: Rgb(0x2E, 0x34, 0x40),
};

/// Gruvbox 的暖棕底。
///
/// 深色不必都是冷的。橙黄绿落在棕黑上是老式终端的颜色，也是 Rust / C /
/// 系统工具这一圈人看惯的颜色——认得出的人会觉得这张卡片是自己人做的。
pub const EMBER: Palette = Palette {
    name: "ember",
    bg: Rgb(0x1D, 0x20, 0x21),
    panel: Rgb(0x28, 0x28, 0x28),
    text: Rgb(0xFB, 0xF1, 0xC7),
    muted: Rgb(0xA8, 0x99, 0x84),
    line: Rgb(0x3C, 0x38, 0x36),
    track: Rgb(0x50, 0x49, 0x45),
    series: [
        Rgb(0xFA, 0xBD, 0x2F),
        Rgb(0xFE, 0x80, 0x19),
        Rgb(0xB8, 0xBB, 0x26),
        Rgb(0x83, 0xA5, 0x98),
        Rgb(0xD3, 0x86, 0x9B),
    ],
    warn: Rgb(0xFA, 0xBD, 0x2F),
    bad: Rgb(0xFB, 0x49, 0x34),
    bands: [
        Rgb(0x8E, 0xC0, 0x7C),
        Rgb(0xB8, 0xBB, 0x26),
        Rgb(0xFA, 0xBD, 0x2F),
        Rgb(0xFE, 0x80, 0x19),
        Rgb(0xFB, 0x49, 0x34),
    ],
    brand: [
        Rgb(0xFE, 0x80, 0x19),
        Rgb(0xFA, 0xBD, 0x2F),
        Rgb(0xB8, 0xBB, 0x26),
    ],
    mark_hollow: Rgb(0x1D, 0x20, 0x21),
};

/// Solarized 深青底。
///
/// 六个低饱和色相配一组固定明度的中性色，是 2011 年就定死的一套方案，
/// 至今还是很多人编辑器里的默认皮肤。我们照抄，不改它的色值——改了就
/// 不是 Solarized，只是一套长得像的绿。
pub const SOLAR: Palette = Palette {
    name: "solar",
    bg: Rgb(0x00, 0x2B, 0x36),
    panel: Rgb(0x07, 0x36, 0x42),
    text: Rgb(0xEE, 0xE8, 0xD5),
    muted: Rgb(0x93, 0xA1, 0xA1),
    line: Rgb(0x0E, 0x45, 0x52),
    track: Rgb(0x14, 0x51, 0x5F),
    series: [
        Rgb(0x26, 0x8B, 0xD2),
        Rgb(0x2A, 0xA1, 0x98),
        Rgb(0x85, 0x99, 0x00),
        Rgb(0xB5, 0x89, 0x00),
        Rgb(0xD3, 0x36, 0x82),
    ],
    warn: Rgb(0xB5, 0x89, 0x00),
    bad: Rgb(0xDC, 0x32, 0x2F),
    bands: [
        Rgb(0x2A, 0xA1, 0x98),
        Rgb(0x85, 0x99, 0x00),
        Rgb(0xB5, 0x89, 0x00),
        Rgb(0xCB, 0x4B, 0x16),
        Rgb(0xDC, 0x32, 0x2F),
    ],
    brand: [
        Rgb(0x26, 0x8B, 0xD2),
        Rgb(0x2A, 0xA1, 0x98),
        Rgb(0x85, 0x99, 0x00),
    ],
    mark_hollow: Rgb(0x00, 0x2B, 0x36),
};

/// 单色 CRT 绿：没有第二个色相。
///
/// 序列色是一条绿色明暗阶而不是五个色相，所以卡片**打印成黑白仍然分得开**——
/// 靠色相区分的图表一旦去色就全糊在一起。判定色是唯一的例外：错误必须是红的，
/// 那是这张卡片上唯一值得打破单色的信息。
pub const PHOSPHOR: Palette = Palette {
    name: "phosphor",
    bg: Rgb(0x04, 0x12, 0x0B),
    panel: Rgb(0x0A, 0x1F, 0x14),
    text: Rgb(0xC8, 0xF5, 0xD2),
    muted: Rgb(0x6F, 0xAE, 0x7F),
    line: Rgb(0x17, 0x39, 0x2A),
    track: Rgb(0x1F, 0x4B, 0x36),
    series: [
        Rgb(0x7E, 0xF7, 0xA2),
        Rgb(0x4F, 0xD9, 0x8A),
        Rgb(0x31, 0xB5, 0x73),
        Rgb(0x2A, 0x8F, 0x5F),
        Rgb(0x25, 0x6F, 0x4D),
    ],
    warn: Rgb(0xB7, 0xF5, 0x6A),
    bad: Rgb(0xFF, 0x6B, 0x6B),
    bands: [
        Rgb(0x7E, 0xF7, 0xA2),
        Rgb(0x4F, 0xD9, 0x8A),
        Rgb(0x31, 0xB5, 0x73),
        Rgb(0x8F, 0xBF, 0x5A),
        Rgb(0xFF, 0x6B, 0x6B),
    ],
    brand: [
        Rgb(0x25, 0x6F, 0x4D),
        Rgb(0x4F, 0xD9, 0x8A),
        Rgb(0xC8, 0xF5, 0xD2),
    ],
    mark_hollow: Rgb(0x04, 0x12, 0x0B),
};

/// 工程制图蓝。
///
/// 深蓝底、冷白线、淡青强调，像一张蓝图。硬件、协议、架构类项目的 README
/// 里通常已经有几张示意图了，卡片跟着它们走比跟着我们的品牌色走更整齐。
pub const BLUEPRINT: Palette = Palette {
    name: "blueprint",
    bg: Rgb(0x0B, 0x25, 0x45),
    panel: Rgb(0x12, 0x32, 0x58),
    text: Rgb(0xEA, 0xF2, 0xFF),
    muted: Rgb(0x9D, 0xB6, 0xD8),
    line: Rgb(0x1D, 0x41, 0x71),
    track: Rgb(0x26, 0x4F, 0x84),
    series: [
        Rgb(0x7F, 0xD1, 0xFF),
        Rgb(0xC9, 0xE4, 0xFF),
        Rgb(0xFF, 0xD4, 0x79),
        Rgb(0x8E, 0xE6, 0xC8),
        Rgb(0xFF, 0x9A, 0xA2),
    ],
    warn: Rgb(0xFF, 0xD4, 0x79),
    bad: Rgb(0xFF, 0x8A, 0x94),
    bands: [
        Rgb(0x7F, 0xD1, 0xFF),
        Rgb(0x8E, 0xE6, 0xC8),
        Rgb(0xFF, 0xD4, 0x79),
        Rgb(0xFF, 0xAB, 0x6B),
        Rgb(0xFF, 0x8A, 0x94),
    ],
    brand: [
        Rgb(0x4C, 0xC3, 0xFF),
        Rgb(0x9A, 0xD7, 0xFF),
        Rgb(0xEA, 0xF2, 0xFF),
    ],
    mark_hollow: Rgb(0x0B, 0x25, 0x45),
};

/// Okabe–Ito 色盲友好方案 + 纯黑底。
///
/// 唯一一套**先解决问题、再谈好看**的色板：五个序列色取自 Okabe–Ito 的八色
/// 方案，红绿色觉异常的人也能把它们区分开——霓虹那套的粉与青在二型色觉下会
/// 靠得很近。底色取纯黑，正文对比度顶到 21:1。
pub const OKABE: Palette = Palette {
    name: "okabe",
    bg: Rgb(0x00, 0x00, 0x00),
    panel: Rgb(0x10, 0x10, 0x10),
    text: Rgb(0xFF, 0xFF, 0xFF),
    muted: Rgb(0xA6, 0xA6, 0xA6),
    line: Rgb(0x2E, 0x2E, 0x2E),
    track: Rgb(0x3D, 0x3D, 0x3D),
    series: [
        Rgb(0x56, 0xB4, 0xE9),
        Rgb(0xE6, 0x9F, 0x00),
        Rgb(0x00, 0x9E, 0x73),
        Rgb(0xCC, 0x79, 0xA7),
        Rgb(0xF0, 0xE4, 0x42),
    ],
    warn: Rgb(0xE6, 0x9F, 0x00),
    bad: Rgb(0xD5, 0x5E, 0x00),
    bands: [
        Rgb(0x56, 0xB4, 0xE9),
        Rgb(0x00, 0x9E, 0x73),
        Rgb(0xF0, 0xE4, 0x42),
        Rgb(0xE6, 0x9F, 0x00),
        Rgb(0xD5, 0x5E, 0x00),
    ],
    brand: [
        Rgb(0x56, 0xB4, 0xE9),
        Rgb(0xCC, 0x79, 0xA7),
        Rgb(0x00, 0x9E, 0x73),
    ],
    mark_hollow: Rgb(0x00, 0x00, 0x00),
};

/// 瑞士排版：灰阶加一点红。
///
/// 浅色。序列色是一条灰阶，只有需要强调的地方是红的——像一页报纸或一份
/// 年报。给那些希望卡片读起来是「文件」而不是「界面」的项目。
pub const NEWSPRINT: Palette = Palette {
    name: "newsprint",
    bg: Rgb(0xFB, 0xFB, 0xF9),
    panel: Rgb(0xF0, 0xF0, 0xEC),
    text: Rgb(0x11, 0x11, 0x11),
    muted: Rgb(0x5C, 0x5C, 0x5C),
    line: Rgb(0xDC, 0xDC, 0xD6),
    track: Rgb(0xE6, 0xE6, 0xE0),
    series: [
        Rgb(0x11, 0x11, 0x11),
        Rgb(0xC8, 0x10, 0x2E),
        Rgb(0x4A, 0x4A, 0x4A),
        Rgb(0x76, 0x76, 0x76),
        Rgb(0x8F, 0x8F, 0x8F),
    ],
    warn: Rgb(0xA3, 0x5A, 0x00),
    bad: Rgb(0xC8, 0x10, 0x2E),
    bands: [
        Rgb(0x0F, 0x6B, 0x57),
        Rgb(0x3F, 0x7A, 0x1F),
        Rgb(0xA3, 0x5A, 0x00),
        Rgb(0xB8, 0x48, 0x0F),
        Rgb(0xC8, 0x10, 0x2E),
    ],
    brand: [
        Rgb(0x11, 0x11, 0x11),
        Rgb(0xC8, 0x10, 0x2E),
        Rgb(0x11, 0x11, 0x11),
    ],
    mark_hollow: Rgb(0xFB, 0xFB, 0xF9),
};

/// 柔和粉彩纸底。
///
/// 浅色里最温和的一套。玫瑰、薰衣草、鼠尾草都压过明度，在粉白纸上不刺眼。
/// 设计工具、内容项目、面向非工程读者的仓库——它们的 README 通常也是这个
/// 温度，卡片不该是那一页上唯一硬的东西。
pub const SAKURA: Palette = Palette {
    name: "sakura",
    bg: Rgb(0xFF, 0xF6, 0xF8),
    panel: Rgb(0xFF, 0xE9, 0xEE),
    text: Rgb(0x3A, 0x21, 0x30),
    muted: Rgb(0x7D, 0x55, 0x66),
    line: Rgb(0xF3, 0xD3, 0xDC),
    track: Rgb(0xF7, 0xDD, 0xE4),
    series: [
        Rgb(0xB3, 0x43, 0x6E),
        Rgb(0x8A, 0x6B, 0xB0),
        Rgb(0x4F, 0x8A, 0x72),
        Rgb(0xB5, 0x79, 0x3A),
        Rgb(0x6B, 0x7F, 0xA8),
    ],
    warn: Rgb(0xA8, 0x6A, 0x12),
    bad: Rgb(0xC0, 0x39, 0x52),
    bands: [
        Rgb(0x2F, 0x7F, 0x6C),
        Rgb(0x4F, 0x8A, 0x3A),
        Rgb(0xA8, 0x6A, 0x12),
        Rgb(0xBD, 0x5C, 0x22),
        Rgb(0xC0, 0x39, 0x52),
    ],
    brand: [
        Rgb(0xB3, 0x43, 0x6E),
        Rgb(0x8A, 0x6B, 0xB0),
        Rgb(0x4F, 0x8A, 0x72),
    ],
    mark_hollow: Rgb(0xFF, 0xF6, 0xF8),
};

/// 冷调近白。
///
/// [`PORCELAIN`] 是暖的，配米色、木色、衬线字的版面；这一套是冷的，配蓝色
/// 系的版面。浅色只有一套时，一半的 README 只能将就。
pub const GLACIER: Palette = Palette {
    name: "glacier",
    bg: Rgb(0xF6, 0xF9, 0xFC),
    panel: Rgb(0xE9, 0xEF, 0xF6),
    text: Rgb(0x0F, 0x1F, 0x2E),
    muted: Rgb(0x54, 0x6A, 0x80),
    line: Rgb(0xD3, 0xDE, 0xE9),
    track: Rgb(0xDD, 0xE6, 0xEF),
    series: [
        Rgb(0x0F, 0x6F, 0xBD),
        Rgb(0x0D, 0x8A, 0x8A),
        Rgb(0x5C, 0x5F, 0xBD),
        Rgb(0x8A, 0x6A, 0x1F),
        Rgb(0xA8, 0x44, 0x3F),
    ],
    warn: Rgb(0x8A, 0x6A, 0x1F),
    bad: Rgb(0xB0, 0x3A, 0x34),
    bands: [
        Rgb(0x0D, 0x7D, 0x7D),
        Rgb(0x2F, 0x7A, 0x3A),
        Rgb(0x8A, 0x6A, 0x1F),
        Rgb(0xB0, 0x56, 0x1C),
        Rgb(0xB0, 0x3A, 0x34),
    ],
    brand: [
        Rgb(0x0F, 0x6F, 0xBD),
        Rgb(0x1B, 0x9A, 0xAA),
        Rgb(0x3F, 0x9F, 0x7A),
    ],
    mark_hollow: Rgb(0xF6, 0xF9, 0xFC),
};

/// 纯黑白，深色。没有一个色相。
///
/// [`PHOSPHOR`] 已经是单色，但那是**绿色的**单色——它仍然在表达一种趣味。
/// 这一套连趣味都不表达：白字黑底，五档分数是五级明度，品牌渐变的三段取值
/// 相同，所以 wordmark 上是一块平色而不是一条渐变。给那些「卡片不该有观点」
/// 的仓库，以及任何需要把图片印在纸上的场合。
pub const CARBON: Palette = Palette {
    name: "carbon",
    bg: Rgb(0x00, 0x00, 0x00),
    panel: Rgb(0x12, 0x12, 0x12),
    text: Rgb(0xE8, 0xE8, 0xE8),
    muted: Rgb(0x9E, 0x9E, 0x9E),
    line: Rgb(0x2E, 0x2E, 0x2E),
    track: Rgb(0x3D, 0x3D, 0x3D),
    // 明度阶而不是色相环：去掉颜色之后，能区分五条数据的只剩深浅
    series: [
        Rgb(0xFF, 0xFF, 0xFF),
        Rgb(0xD4, 0xD4, 0xD4),
        Rgb(0xA8, 0xA8, 0xA8),
        Rgb(0x80, 0x80, 0x80),
        Rgb(0x5C, 0x5C, 0x5C),
    ],
    warn: Rgb(0xBD, 0xBD, 0xBD),
    // 失败取最亮的一档,并且比正文亮 —— 黑白里「显眼」只能靠对比度买
    bad: Rgb(0xFF, 0xFF, 0xFF),
    bands: [
        Rgb(0xFF, 0xFF, 0xFF),
        Rgb(0xD4, 0xD4, 0xD4),
        Rgb(0xA8, 0xA8, 0xA8),
        Rgb(0x80, 0x80, 0x80),
        Rgb(0x5C, 0x5C, 0x5C),
    ],
    // 三段同色 = 没有渐变。这是这套色板的全部主张
    brand: [
        Rgb(0xE8, 0xE8, 0xE8),
        Rgb(0xE8, 0xE8, 0xE8),
        Rgb(0xE8, 0xE8, 0xE8),
    ],
    mark_hollow: Rgb(0x00, 0x00, 0x00),
};

/// 纯黑白，浅色。[`CARBON`] 翻过来的那一面。
///
/// 黑字白纸，没有第二个颜色，也没有渐变。传真、影印、黑白打印、灰度电子墨水——
/// 这些地方任何一套彩色色板都会退化成一堆分不开的灰，而这一套本来就是按灰
/// 设计的：五档分数是从纯黑到 3:1 的五级明度，去色前后长得一模一样。
pub const PAPER: Palette = Palette {
    name: "paper",
    bg: Rgb(0xFF, 0xFF, 0xFF),
    panel: Rgb(0xF2, 0xF2, 0xF2),
    text: Rgb(0x00, 0x00, 0x00),
    muted: Rgb(0x59, 0x59, 0x59),
    line: Rgb(0xD4, 0xD4, 0xD4),
    track: Rgb(0xE6, 0xE6, 0xE6),
    series: [
        Rgb(0x00, 0x00, 0x00),
        Rgb(0x2E, 0x2E, 0x2E),
        Rgb(0x54, 0x54, 0x54),
        Rgb(0x75, 0x75, 0x75),
        Rgb(0x8F, 0x8F, 0x8F),
    ],
    warn: Rgb(0x5C, 0x5C, 0x5C),
    bad: Rgb(0x00, 0x00, 0x00),
    bands: [
        Rgb(0x00, 0x00, 0x00),
        Rgb(0x2E, 0x2E, 0x2E),
        Rgb(0x54, 0x54, 0x54),
        Rgb(0x75, 0x75, 0x75),
        Rgb(0x94, 0x94, 0x94),
    ],
    brand: [
        Rgb(0x00, 0x00, 0x00),
        Rgb(0x00, 0x00, 0x00),
        Rgb(0x00, 0x00, 0x00),
    ],
    mark_hollow: Rgb(0xFF, 0xFF, 0xFF),
};

// ── 全部色板 ────────────────────────────────────────────────

/// 每一套色板，按 `--theme` 的取值顺序。
///
/// 测试遍历的是**这个列表**而不是手写的几个名字：色板会一直加下去，而
/// 「加了一套却忘了把它写进对比度测试」是没人会发现的——那套色板会带着
/// 一行读不清的弱色文字发出去，直到有人截图问为什么。
pub const ALL: &[&Palette] = &[
    &DARK, &PORCELAIN, &SLATE, &NORD, &EMBER, &SOLAR, &PHOSPHOR, &BLUEPRINT, &OKABE, &NEWSPRINT,
    &SAKURA, &GLACIER, &CARBON, &PAPER,
];

#[cfg(test)]
mod palette_tests {
    use super::*;

    /// 每一套色板都必须在同一个阈值上翻面，否则同一个仓库会有两种说法
    #[test]
    fn every_palette_bands_on_the_same_thresholds() {
        for p in ALL {
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
    fn text_contrasts_with_the_background_in_every_palette() {
        for p in ALL {
            let body = contrast(p.text, p.bg);
            assert!(body >= 7.0, "{} 的正文对比度只有 {body:.1}", p.name);
            let muted = contrast(p.muted, p.bg);
            assert!(muted >= 4.5, "{} 的弱色对比度只有 {muted:.1}", p.name);
        }
    }

    /// 分档色是要被读出数值的，不能只靠色相区分
    #[test]
    fn every_band_colour_is_legible_on_its_own_background() {
        for p in ALL {
            for (i, c) in p.bands.iter().enumerate() {
                let ratio = contrast(*c, p.bg);
                assert!(ratio >= 3.0, "{} 第 {i} 档对比度只有 {ratio:.1}", p.name);
            }
        }
    }

    /// 镂空色画在标记上，与卡片底色不一致就会露出一圈边
    #[test]
    fn the_mark_hollow_matches_the_card_background() {
        for p in ALL {
            assert_eq!(p.mark_hollow, p.bg, "{} 的镂空色与底色对不上", p.name);
        }
    }

    /// 每套色板都必须能用名字取回来，且名字与常量里写的一致。
    ///
    /// 漏一条 `parse` 分支的后果不是编译错误，是 `--theme` 里明明列着的
    /// 名字在配置文件里解析失败。
    #[test]
    fn every_palette_is_reachable_by_its_own_name() {
        for p in ALL {
            let got = Palette::parse(p.name).unwrap_or_else(|| panic!("{} 解析不出来", p.name));
            assert_eq!(got.name, p.name);
        }
        // 别名指向的是同一套色板，不是一份拷贝
        assert_eq!(Palette::parse("gruvbox").unwrap().name, "ember");
        assert_eq!(Palette::parse("colorblind").unwrap().name, "okabe");
        assert!(Palette::parse("chartreuse").is_none());
    }

    /// `docs/themes/` 里的色值必须与这里一致。
    ///
    /// 那些页面是 `scripts/render-themes.py` 生成的，而生成物的老毛病是**没人
    /// 记得重跑**：改一个色值，文档就开始说另一套颜色，且不会有任何报错。
    /// 这条测试把「忘了重跑」变成一次红色的 CI。
    ///
    /// 打包发布到 crates.io 的那份源码里没有 `docs/`，所以目录不在就跳过——
    /// 这条测试守的是仓库，不是 crate。
    #[test]
    fn the_generated_theme_pages_agree_with_these_palettes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("docs/themes");
        if !root.is_dir() {
            return;
        }
        for p in ALL {
            let page = root.join(p.name).join("README.md");
            let text = std::fs::read_to_string(&page).unwrap_or_else(|_| {
                panic!(
                    "{} 没有对应的 docs/themes/{}/README.md —— 跑一次 \
                     `cargo build && python3 scripts/render-themes.py`",
                    p.name, p.name
                )
            });
            let mut every: Vec<Rgb> = vec![
                p.bg, p.panel, p.text, p.muted, p.line, p.track, p.warn, p.bad,
            ];
            every.extend_from_slice(&p.series);
            every.extend_from_slice(&p.bands);
            every.extend_from_slice(&p.brand);
            for c in every {
                let hex = c.to_string();
                assert!(
                    text.contains(&hex),
                    "docs/themes/{}/README.md 里没有 {hex} —— 色板改过，文档没重生成",
                    p.name
                );
            }
        }
    }

    /// 黑白色板必须真的是黑白的。
    ///
    /// `carbon` / `paper` 的全部主张就是「没有颜色」，而一个通道差一位是
    /// 看不出来的——`#2e2e2f` 在屏幕上就是灰，直到有人把卡片印成灰度图，
    /// 或者拿去做去色对比时才发现它一直带着一点蓝。
    #[test]
    fn the_monochrome_palettes_have_no_hue_at_all() {
        for p in [&CARBON, &PAPER] {
            let mut every: Vec<Rgb> = vec![
                p.bg, p.panel, p.text, p.muted, p.line, p.track, p.warn, p.bad,
            ];
            every.extend_from_slice(&p.series);
            every.extend_from_slice(&p.bands);
            every.extend_from_slice(&p.brand);
            for c in every {
                assert!(
                    c.0 == c.1 && c.1 == c.2,
                    "{} 里的 {c} 三个通道不相等，它有色相",
                    p.name
                );
            }
        }
    }

    /// 黑白色板不能有渐变：wordmark 的三段取值必须相同。
    ///
    /// 三段同色时 [`Palette::sweep`] 在整个区间上返回同一个颜色，wordmark
    /// 就是一块平色。这条测试盯的是「后来有人手滑把中间那段调亮了一点」。
    #[test]
    fn the_monochrome_palettes_have_no_gradient() {
        for p in [&CARBON, &PAPER] {
            assert_eq!(p.brand[0], p.brand[1], "{} 的品牌渐变有两段不同", p.name);
            assert_eq!(p.brand[1], p.brand[2], "{} 的品牌渐变有两段不同", p.name);
            for step in 0..=10 {
                let t = step as f32 / 10.0;
                assert_eq!(
                    p.sweep(t),
                    p.brand[0],
                    "{} 在 t={t} 处渐出了别的颜色",
                    p.name
                );
            }
        }
    }

    /// 名字不能重复：重名时 `--theme` 说的是哪一套就没法回答了
    #[test]
    fn palette_names_are_unique() {
        let mut seen: Vec<&str> = Vec::new();
        for p in ALL {
            assert!(!seen.contains(&p.name), "色板名 {} 出现了两次", p.name);
            seen.push(p.name);
        }
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
            for p in ALL {
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
