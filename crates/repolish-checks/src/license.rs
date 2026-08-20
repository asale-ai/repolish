use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// LICENSE 是否存在且可识别为标准许可证。
///
/// 分档：无文件 = 0；有文件但认不出 = 6；识别出标准许可证 = 10
pub struct License;

/// 文件名前缀。用前缀而非全名匹配，因为 Rust 生态惯用 `LICENSE-MIT` +
/// `LICENSE-APACHE` 双协议，只认 `LICENSE` 会把这类项目全判成「没有许可证」。
const PREFIXES: &[&str] = &["license", "licence", "copying", "unlicense", "unlicence"];

/// (SPDX 标识, 正文中的特征串)
const SIGNATURES: &[(&str, &str)] = &[
    ("Apache-2.0", "apache license"),
    ("MIT", "permission is hereby granted, free of charge"),
    ("BSD-3-Clause", "neither the name of the copyright holder"),
    ("BSD-2-Clause", "redistribution and use in source and binary forms"),
    ("AGPL-3.0", "gnu affero general public license"),
    ("LGPL", "gnu lesser general public license"),
    ("GPL-3.0", "gnu general public license"),
    ("MPL-2.0", "mozilla public license"),
    ("ISC", "permission to use, copy, modify, and/or distribute"),
    ("Unlicense", "this is free and unencumbered software"),
    ("BSL-1.0", "boost software license"),
    ("WTFPL", "do what the fuck you want to public license"),
    ("Zlib", "this software is provided 'as-is', without any express"),
    // 文档类仓库常用 Creative Commons，顺序上要先于更宽泛的 CC0
    ("CC-BY-SA-4.0", "attribution-sharealike 4.0"),
    ("CC-BY-4.0", "attribution 4.0 international"),
    ("CC0-1.0", "cc0 1.0 universal"),
    ("CC-BY-NC-SA", "attribution-noncommercial-sharealike"),
];

impl Check for License {
    fn id(&self) -> &'static str {
        "license"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::Critical
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let files: Vec<&str> = ctx.files.iter().filter(|p| is_license_file(p)).collect();

        if files.is_empty() {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "no LICENSE file in the repository root")],
                vec![Fix::new(
                    Severity::P1,
                    "Add a LICENSE file. No license means all rights reserved — legally, nobody may use your code",
                )],
            );
        }

        // 必须扫全部候选：ripgrep 的 COPYING 只是一句「双协议」说明，
        // 真正可识别的正文在 LICENSE-MIT 与 UNLICENSE 里。只看第一个会误判。
        let mut identified: Vec<(&str, &str)> = Vec::new();
        let mut unreadable = 0usize;

        for f in &files {
            match ctx.files.read(f) {
                Some(text) => {
                    let lower = text.to_lowercase();
                    if let Some((spdx, _)) = SIGNATURES.iter().find(|(_, sig)| lower.contains(sig)) {
                        identified.push((f, spdx));
                    }
                }
                None => unreadable += 1,
            }
        }

        if !identified.is_empty() {
            let spdx: Vec<&str> = identified.iter().map(|(_, s)| *s).collect();
            return Outcome::perfect(vec![Evidence::new(
                identified[0].0,
                format!("identified as {}", spdx.join(" OR ")),
            )]);
        }

        if unreadable == files.len() {
            return Outcome::inconclusive(format!("{} exists but could not be read", files[0]));
        }

        Outcome::scored(
            6,
            vec![Evidence::new(
                files[0],
                if files.len() == 1 {
                    "the license file does not match any known standard license".to_string()
                } else {
                    format!(
                        "{} license files, none matching a known standard license",
                        files.len()
                    )
                },
            )],
            vec![Fix::new(
                Severity::P2,
                "Use the verbatim text of a standard license (choosealicense.com). Custom terms send a corporate legal review straight to \"no\"",
            )],
        )
    }
}

/// 仅限仓库根目录，且文件名以许可证类前缀开头。
fn is_license_file(path: &str) -> bool {
    if path.contains('/') {
        return false;
    }
    let name = path.to_lowercase();
    let stem = name.split('.').next().unwrap_or(&name);
    PREFIXES.iter().any(|p| stem.starts_with(p))
}
