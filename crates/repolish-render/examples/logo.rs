//! 生成 `assets/` 下的品牌文件。
//!
//! 为什么不手写这三个 SVG：卡片页眉上的标记和 README 顶上的 logo 必须是
//! 同一个形状。手写两份，改了一处忘了另一处，两个月后就是两个 logo。
//! 这里让它们从 `repolish_render::svg` 的同一段几何里长出来。
//!
//! ```sh
//! cargo run -p repolish-render --example logo
//! ```

use std::fs;
use std::path::Path;

use repolish_render::i18n::{category_label, Lang};

/// 副标题就是三大类的名字，取自与卡片同一张文案表——
/// 横幅上写着一套词、卡片上写着另一套，是同一个品牌两种说法。
fn hero(lang: Lang) -> String {
    use repolish_core::Category;
    let s = lang.strings();
    let tagline: Vec<&str> = Category::ALL
        .iter()
        .map(|c| category_label(*c, s))
        .collect();
    let joined = match lang {
        // 英文小写更像一句副标题；中日文没有大小写，原样即可
        Lang::En => tagline.join(" · ").to_lowercase(),
        Lang::ZhCn | Lang::Ja => tagline.join(" · "),
    };
    // 这个 example 画的是**我们自己的**门面，所以名字在这里是写死的；
    // CLI 给别人画时传的是那个项目的名字。
    repolish_render::svg::hero("repolish", &joined, lang)
}

fn main() -> std::io::Result<()> {
    // 从 crates/repolish-render/ 回到仓库根
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate 应位于 crates/<name>/")
        .to_path_buf();
    let assets = root.join("assets");
    fs::create_dir_all(&assets)?;

    let files = [
        ("logo.svg", repolish_render::svg::logo(128)),
        ("favicon.svg", repolish_render::svg::logo(32)),
        ("wordmark.svg", repolish_render::svg::wordmark(56)),
        // README 顶上那张通栏横幅。与 wordmark 的区别只有 viewBox 的比例，
        // 但那一个差别是关键：以 width="100%" 引用时，一张 450×56 的图会被
        // 拉成横穿页面的巨型字，而这张按比例缩放后仍然是一个居中的标志。
        //
        // **每种语言一张。** 横幅上那句副标题是**我们的**文字，不是仓库的内容，
        // 所以它和卡片上的标签一样要跟着 README 的语言走。中英两份 README 共用
        // 同一张图的话，中文那份顶上会挂着一行英文——而那正是 `--lang` 要解决的
        // 那件事，发生在我们自己的门面上。
        ("hero.svg", hero(Lang::En)),
        ("hero.zh-CN.svg", hero(Lang::ZhCn)),
    ];

    for (name, svg) in files {
        let path = assets.join(name);
        fs::write(&path, svg.as_bytes())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
