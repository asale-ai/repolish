use repolish_core::{
    Category, Check, Evidence, Fix, Outcome, Profile, RepoContext, Risk, Severity,
};

/// 是否存在测试。
///
/// 路径约定（`tests/`、`*_test.go`…）与内联测试（Rust `#[cfg(test)]`）都要认，
/// 否则会把一大批 Rust / Go 项目误判为「没有测试」。
///
/// 分档：0 个 = 0；1-2 个 = 6；3-9 个 = 8；≥10 个 = 10
pub struct TestsPresent;

const TEST_DIRS: &[&str] = &["tests/", "test/", "__tests__/", "spec/", "e2e/"];

/// 内联测试的语言标记。扫描内容有成本，因此只在路径匹配为空时才做，且限量。
const INLINE_MARKERS: &[&str] = &["#[cfg(test)]", "#[test]", "func Test", "@Test"];
const MAX_CONTENT_SCAN: usize = 500;
const SCANNABLE_EXTS: &[&str] = &[".rs", ".go", ".java", ".kt", ".scala"];

impl Check for TestsPresent {
    fn id(&self) -> &'static str {
        "tests-present"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::High
    }

    fn applies_to(&self, profile: Profile) -> bool {
        !matches!(profile, Profile::Docs | Profile::Collection)
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let mut hits: Vec<String> = ctx
            .files
            .iter()
            .filter(|p| is_test_path(p))
            .map(|s| s.to_string())
            .collect();

        // 路径命中与内联测试必须**合并**，不能「路径没命中才扫内联」。
        // Rust 项目的测试绝大多数写在 `#[cfg(test)] mod tests` 里，
        // 只要仓库里有一个 `tests/` 目录，其余几十个内联测试模块就全被忽略了
        // ——repolish 自己就因此被判成「只找到 1 处测试」。
        let known: std::collections::HashSet<&str> = hits.iter().map(|s| s.as_str()).collect();
        let inline: Vec<String> = scan_inline(ctx)
            .into_iter()
            .filter(|p| !known.contains(p.as_str()))
            .collect();
        hits.extend(inline);

        let count = hits.len();
        let sample = hits.first().map(|s| s.as_str()).unwrap_or(".");

        match count {
            0 => Outcome::scored(
                0,
                vec![Evidence::new(".", "no test directory, test files, or inline tests found")],
                vec![Fix::new(
                    Severity::P1,
                    "Add tests. Without them nobody puts this in production, and nobody dares send you a pull request either",
                )],
            ),
            1..=2 => Outcome::scored(
                6,
                vec![Evidence::new(
                    sample,
                    format!("only {count} test location{} found", crate::util::plural(count)),
                )],
                vec![Fix::new(Severity::P2, "Widen the test coverage — at minimum, cover the main paths")],
            ),
            3..=9 => Outcome::scored(
                8,
                vec![Evidence::new(
                    sample,
                    format!("{count} test location{}", crate::util::plural(count)),
                )],
                vec![Fix::new(
                    Severity::P3,
                    "Keep adding tests; if coverage is already good, put a coverage badge in the README to show it",
                )],
            ),
            _ => Outcome::perfect(vec![Evidence::new(
                sample,
                format!("{count} test location{}", crate::util::plural(count)),
            )]),
        }
    }
}

fn is_test_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    if TEST_DIRS.iter().any(|d| lower.starts_with(d)) {
        return true;
    }
    if lower.contains("/tests/") || lower.contains("/test/") || lower.contains("/__tests__/") {
        return true;
    }
    let file = lower.rsplit('/').next().unwrap_or(&lower);
    file.starts_with("test_")
        || file.contains("_test.")
        || file.contains(".test.")
        || file.contains(".spec.")
        || file.contains("_spec.")
}

/// 扫描源码里的内联测试标记。限量执行，避免在大仓库上拖慢诊断。
fn scan_inline(ctx: &RepoContext) -> Vec<String> {
    ctx.files
        .iter()
        .filter(|p| {
            let lower = p.to_lowercase();
            SCANNABLE_EXTS.iter().any(|e| lower.ends_with(e))
        })
        .take(MAX_CONTENT_SCAN)
        .filter(|p| ctx.files.read(p).is_some_and(|src| has_inline_test(&src)))
        .map(|s| s.to_string())
        .collect()
}

/// 标记必须出现在行首（允许缩进）。
/// 否则会命中源码里的字符串字面量——比如本文件的 `INLINE_MARKERS` 自身。
fn has_inline_test(src: &str) -> bool {
    src.lines().any(|line| {
        let t = line.trim_start();
        INLINE_MARKERS.iter().any(|m| t.starts_with(m))
    })
}
