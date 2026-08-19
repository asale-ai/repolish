use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};

/// LICENSE 是否存在且可识别为标准许可证。
///
/// 分档：无文件 = 0；有文件但认不出 = 6；识别出标准许可证 = 10
pub struct License;

const CANDIDATES: &[&str] = &[
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "LICENCE",
    "LICENCE.md",
    "COPYING",
    "COPYING.md",
    "UNLICENSE",
];

/// (SPDX 标识, 正文中的特征串)
const SIGNATURES: &[(&str, &str)] = &[
    ("Apache-2.0", "apache license"),
    ("MIT", "permission is hereby granted, free of charge"),
    ("BSD-3-Clause", "neither the name of the copyright holder"),
    ("BSD-2-Clause", "redistribution and use in source and binary forms"),
    ("GPL-3.0", "gnu general public license"),
    ("LGPL", "gnu lesser general public license"),
    ("AGPL-3.0", "gnu affero general public license"),
    ("MPL-2.0", "mozilla public license"),
    ("ISC", "permission to use, copy, modify, and/or distribute"),
    ("Unlicense", "this is free and unencumbered software"),
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
        let Some(found) = ctx.files.find_at_root(CANDIDATES) else {
            return Outcome::scored(
                0,
                vec![Evidence::new(".", "仓库根目录没有 LICENSE 文件")],
                vec![Fix::new(
                    Severity::P1,
                    "添加 LICENSE 文件。没有许可证 = 保留所有权利，法律上别人不能用你的代码",
                )],
            );
        };

        let Some(text) = ctx.files.read(found) else {
            return Outcome::inconclusive(format!("{found} 存在但读取失败"));
        };
        let lower = text.to_lowercase();

        match SIGNATURES.iter().find(|(_, sig)| lower.contains(sig)) {
            Some((spdx, _)) => Outcome::perfect(vec![Evidence::new(
                found,
                format!("识别为 {spdx}"),
            )]),
            None => Outcome::scored(
                6,
                vec![Evidence::new(
                    found,
                    "文件存在，但内容不匹配任何已知的标准许可证",
                )],
                vec![Fix::new(
                    Severity::P2,
                    "改用标准许可证原文（choosealicense.com）。自定义条款会让使用者的法务直接放弃",
                )],
            ),
        }
    }
}
