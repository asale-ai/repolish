//! `claim-consistency`：README 里承诺的命令，代码里真的存在吗。
//!
//! 这是 repolish 与其他工具的分水岭（docs/05 设计原则 3）——README 检查工具普遍
//! 只看「有没有这个区块」，没人回头去核对区块里写的命令是否还能跑。
//! 脚本被改名、npm script 被删掉，README 却留在原地，是开源项目最常见的腐化方式。
//!
//! **只校验能确定性验证的命令。** 拿不准的一律不算「声明」，
//! 因为误判一条不存在的失效命令，比漏掉十条真失效的代价大得多。

use repolish_core::{Category, Check, Evidence, Fix, Outcome, RepoContext, Risk, Severity};
use repolish_ingest::Ecosystem;

use crate::util;

pub struct ClaimConsistency;

/// 构建产物目录。README 里出现的这些路径在干净检出里本来就不存在，
/// 不算失效声明。
const BUILD_DIRS: &[&str] = &[
    "target/", "dist/", "build/", "out/", "bin/", "node_modules/", "venv/", ".venv/", "vendor/",
    "coverage/", "public/", "_site/",
];

/// 会被当成「脚本路径声明」的扩展名。只认脚本——裸的 `./myapp` 多半是编译产物。
const SCRIPT_EXTS: &[&str] = &[".sh", ".py", ".js", ".mjs", ".ts", ".rb", ".ps1", ".bash"];

const COMPOSE_FILES: &[&str] = &[
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
];

struct Claim {
    line: usize,
    /// 给用户看的声明描述，如 "npm run build"
    what: String,
    ok: bool,
}

impl Check for ClaimConsistency {
    fn id(&self) -> &'static str {
        "claim-consistency"
    }
    fn category(&self) -> Category {
        Category::Credibility
    }
    fn risk(&self) -> Risk {
        Risk::High
    }

    fn run(&self, ctx: &RepoContext) -> Outcome {
        let Some(readme) = &ctx.readme else {
            return Outcome::inconclusive("no README, so there are no claims to verify");
        };
        let name = util::readme_name(readme);
        let commands = util::command_lines(readme);

        let mut claims: Vec<Claim> = Vec::new();
        for (line, cmd) in &commands {
            collect(ctx, *line, cmd, &mut claims);
        }

        if claims.is_empty() {
            return Outcome::inconclusive(
                "no verifiable command claims in the README (npm scripts, make targets, script paths, and the like)",
            );
        }

        let total = claims.len();
        let broken: Vec<&Claim> = claims.iter().filter(|c| !c.ok).collect();

        if broken.is_empty() {
            return Outcome::perfect(vec![Evidence::new(
                &name,
                format!(
                    "all {total} command claim{} resolve to something in the repository",
                    util::plural(total)
                ),
            )]);
        }

        let ratio = (total - broken.len()) as f64 / total as f64;
        let score = (ratio * 10.0).floor() as u8;
        let severity = if broken.len() * 2 > total {
            Severity::P1
        } else {
            Severity::P2
        };

        Outcome::scored(
            score,
            broken
                .iter()
                .take(8)
                .map(|c| Evidence::at(&name, c.line, format!("{} — does not exist in the repository", c.what)))
                .collect(),
            vec![Fix::new(
                severity,
                format!(
                    "{} of the {} verifiable command claims in the README no longer work. \
                     Typing the first command from a README and getting an error is the \
                     fastest way to lose a user",
                    broken.len(),
                    total
                ),
            )],
        )
    }
}

fn collect(ctx: &RepoContext, line: usize, cmd: &str, out: &mut Vec<Claim>) {
    let lower = cmd.to_lowercase();

    // 1. 包管理器脚本：npm run build / pnpm run test / yarn run lint
    for verb in ["npm run ", "npm run-script ", "yarn run ", "pnpm run ", "bun run "] {
        if lower.contains(verb) {
            if let Some(script) = util::first_arg(cmd, verb) {
                if let Some(scripts) = npm_scripts(ctx) {
                    out.push(Claim {
                        line,
                        what: format!("`{} {script}`", verb.trim()),
                        ok: scripts.contains(&script),
                    });
                }
            }
        }
    }

    // 2. make 目标
    if lower.starts_with("make ") {
        if let (Some(target), Some(targets)) = (util::first_arg(cmd, "make "), make_targets(ctx)) {
            out.push(Claim {
                line,
                what: format!("`make {target}`"),
                ok: targets.contains(&target),
            });
        }
    }

    // 3. just 配方
    if lower.starts_with("just ") {
        if let (Some(recipe), Some(recipes)) = (util::first_arg(cmd, "just "), just_recipes(ctx)) {
            out.push(Claim {
                line,
                what: format!("`just {recipe}`"),
                ok: recipes.contains(&recipe),
            });
        }
    }

    // 4. cargo run --bin X
    for verb in ["cargo run --bin ", "cargo run -p "] {
        if lower.contains(verb) {
            if let Some(bin) = util::first_arg(cmd, verb) {
                out.push(Claim {
                    line,
                    what: format!("`{} {bin}`", verb.trim()),
                    ok: cargo_target_exists(ctx, &bin),
                });
            }
        }
    }

    // 5. docker compose
    if lower.contains("docker compose") || lower.contains("docker-compose") {
        out.push(Claim {
            line,
            what: "`docker compose`".to_string(),
            ok: COMPOSE_FILES.iter().any(|f| ctx.files.contains(f)),
        });
    }

    // 6. 脚本路径
    for token in cmd.split_whitespace() {
        let Some((path, anchored)) = script_path(token) else {
            continue;
        };
        // 只有「明确指向本仓库」的路径才算声明。裸文件名在用法示例里
        // 指的是使用者自己的文件：`ruff check file.py` 里的 file.py 不该由我们校验。
        if !anchored && !first_segment_is_repo_dir(ctx, &path) {
            continue;
        }
        out.push(Claim {
            line,
            what: format!("`{path}`"),
            ok: ctx.files.contains_ignore_case(&path),
        });
    }
}

/// 路径的第一段在仓库里确实是个目录——用来区分 `scripts/build.sh`（本仓库的）
/// 与 `path/to/code/file.py`（示例占位符）
fn first_segment_is_repo_dir(ctx: &RepoContext, path: &str) -> bool {
    let Some((head, _)) = path.split_once('/') else {
        return false;
    };
    let prefix = format!("{head}/");
    ctx.files.any_matching(|p| p.starts_with(&prefix))
}

/// 只有根 package.json 声明了 scripts 时才校验——否则 `npm run x` 无从判断真假
fn npm_scripts(ctx: &RepoContext) -> Option<&[String]> {
    ctx.manifests
        .iter()
        .find(|m| m.ecosystem == Ecosystem::Npm)
        .map(|m| m.scripts.as_slice())
        .filter(|s| !s.is_empty())
}

fn make_targets(ctx: &RepoContext) -> Option<Vec<String>> {
    let path = ctx.files.find_at_root(&["Makefile", "makefile", "GNUmakefile"])?;
    let text = ctx.files.read(path)?;
    let mut targets = Vec::new();
    for raw in text.lines() {
        if raw.starts_with([' ', '\t', '#']) {
            continue;
        }
        let Some((head, _)) = raw.split_once(':') else {
            continue;
        };
        let head = head.trim();
        // `.PHONY: build test` 声明的也是可调用目标
        if head == ".PHONY" {
            let rest = raw.split_once(':').map(|(_, r)| r).unwrap_or("");
            targets.extend(rest.split_whitespace().map(str::to_string));
            continue;
        }
        if head.starts_with('.') || head.contains('=') || head.contains('$') {
            continue;
        }
        targets.extend(head.split_whitespace().map(str::to_string));
    }
    Some(targets)
}

fn just_recipes(ctx: &RepoContext) -> Option<Vec<String>> {
    let path = ctx.files.find_at_root(&["justfile", "Justfile", ".justfile"])?;
    let text = ctx.files.read(path)?;
    let recipes = text
        .lines()
        .filter(|l| !l.starts_with([' ', '\t', '#']))
        .filter_map(|l| l.split_once(':'))
        .map(|(head, _)| head.split_whitespace().next().unwrap_or("").to_string())
        .filter(|r| !r.is_empty() && !r.contains('='))
        .collect();
    Some(recipes)
}

/// workspace 里任一 Cargo.toml 声明了这个名字，或存在 `src/bin/<name>.rs`
fn cargo_target_exists(ctx: &RepoContext, name: &str) -> bool {
    let nested = format!("/src/bin/{name}.rs");
    let at_root = format!("src/bin/{name}.rs");
    if ctx
        .files
        .any_matching(|p| p.ends_with(&nested) || p == at_root)
    {
        return true;
    }
    let needle = format!("name = \"{name}\"");
    ctx.files
        .iter()
        .filter(|p| p.ends_with("Cargo.toml"))
        .take(64)
        .any(|p| ctx.files.read(p).is_some_and(|t| t.contains(&needle)))
}

/// 命令里看起来像「仓库内脚本」的参数，以及它是否用 `./` 显式锚定。
///
/// 只认带脚本扩展名的相对路径，并排除 URL 与构建产物目录——
/// `curl https://x/install.sh` 里的是远端脚本，`./target/release/app`
/// 在干净检出里本来就不存在，把它们算作失效声明都是误报。
fn script_path(token: &str) -> Option<(String, bool)> {
    let t = token.trim().trim_matches(['"', '\'', '`', ',']);
    if t.contains('$')
        || t.contains('*')
        || t.contains('<')
        || t.starts_with('/')
        || t.contains("://")
    {
        return None;
    }
    let lower = t.to_lowercase();
    if !SCRIPT_EXTS.iter().any(|e| lower.ends_with(e)) {
        return None;
    }
    let anchored = t.starts_with("./");
    let path = t.strip_prefix("./").unwrap_or(t);
    if BUILD_DIRS.iter().any(|d| path.to_lowercase().starts_with(d)) {
        return None;
    }
    Some((path.to_string(), anchored))
}

#[cfg(test)]
mod tests {
    use super::script_path;

    #[test]
    fn build_outputs_are_not_broken_claims() {
        // 干净检出里没有 target/，不能算 README 说谎
        assert!(script_path("./target/release/app.sh").is_none());
        assert!(script_path("node_modules/.bin/x.js").is_none());
    }

    #[test]
    fn recognizes_anchored_repo_scripts() {
        assert_eq!(
            script_path("./scripts/build.sh"),
            Some(("scripts/build.sh".to_string(), true))
        );
        // 没有 ./ 的相对路径要另行确认第一段是本仓库的目录
        assert_eq!(
            script_path("scripts/build.sh"),
            Some(("scripts/build.sh".to_string(), false))
        );
    }

    #[test]
    fn remote_scripts_are_not_repo_claims() {
        // ruff：`curl -LsSf https://astral.sh/ruff/install.sh | sh`
        assert!(script_path("https://astral.sh/ruff/install.sh").is_none());
    }

    #[test]
    fn variables_and_globs_are_not_paths() {
        assert!(script_path("$SCRIPT.sh").is_none());
        assert!(script_path("scripts/*.sh").is_none());
        // 编译产物没有扩展名，一律不认
        assert!(script_path("./myapp").is_none());
    }
}
