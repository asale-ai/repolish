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
    ];

    for (name, svg) in files {
        let path = assets.join(name);
        fs::write(&path, svg.as_bytes())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
