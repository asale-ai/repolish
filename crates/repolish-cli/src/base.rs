//! `--base`：把同一套检查跑在另一个 commit 上，报出差值。
//!
//! **变化量比绝对值更有行动力。** 「78 分」对一个正在评审 PR 的人没有意义；
//! 「这个 PR 让分数掉了 4 分，因为 README.md:42 的链接失效了」才有。
//!
//! 实现上是真的把基线检出一份再跑一遍，而不是去读基线上那份
//! `.repolish/badge.json`。理由：badge.json 只有一个总分，答不出「哪一项动了」，
//! 而那才是评审时唯一有用的信息。而且它是提交进仓库的产物——一份忘了更新的
//! badge.json 会让差值凭空出现。
//!
//! 用 `git worktree` 而不是 `git stash` / `git checkout`：**使用者的工作区
//! 一个字节都不能动。** 有人正在写 README，而我们为了算个差值把他的文件
//! 换掉了，是不可接受的。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use repolish_core::registry::RunOptions;
use repolish_core::{Delta, Report};
use repolish_ingest::RepoContext;

pub struct Baseline {
    pub delta: Delta,
}

/// 基线跑不起来的原因。全都能给出下一步该做什么——
/// 一句「无法解析 base」在 CI 日志里等于没说。
#[derive(Debug)]
pub enum BaseError {
    NoGit(String),
    UnknownRef { r#ref: String, hint: String },
    Worktree(String),
    Load(String),
}

impl std::fmt::Display for BaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaseError::NoGit(e) => write!(
                f,
                "--base needs the git command on PATH, and it could not run: {e}"
            ),
            BaseError::UnknownRef { r#ref, hint } => {
                write!(f, "cannot resolve {ref_}: {hint}", ref_ = r#ref)
            }
            BaseError::Worktree(e) => write!(f, "could not check out the base commit: {e}"),
            BaseError::Load(e) => write!(f, "could not read the base checkout: {e}"),
        }
    }
}

/// 把 `base_ref` 检出到一个临时工作树，用**同样的选项**跑一遍，返回差值。
///
/// `head` 必须是当前这次运行的报告，`opts` 必须是产出它的那一份选项——
/// 两侧的 mode、`--only`、`--skip` 有任何一处不同，差值就是在拿两把不同的
/// 尺子相减。
pub fn compare(
    root: &Path,
    base_ref: &str,
    head: &RepoContext,
    head_report: &Report,
    opts: &RunOptions,
) -> Result<Baseline, BaseError> {
    let commit = resolve(root, base_ref)?;
    let subpath = subpath_in_repo(root)?;

    let tmp = std::env::temp_dir().join(format!(
        "repolish-base-{}-{}",
        std::process::id(),
        &commit[..commit.len().min(12)]
    ));
    // 上一次跑崩了可能留下残骸。留着它会让 `worktree add` 直接失败
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = git(root, &["worktree", "prune"]);

    git(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            &tmp.to_string_lossy(),
            &commit,
        ],
    )
    .map_err(BaseError::Worktree)?;

    // 被检查的可能是仓库里的一个子目录（`repolish demo/sample`）。
    // 工作树检出的是仓库根，直接对着它跑会去评分**另一个项目**——
    // 一份看起来正常、内容全错的差值。
    let result = run_base(&tmp.join(&subpath), head, opts);

    // 临时工作树无论如何都要收掉,包括中途出错的路径
    let _ = git(
        root,
        &["worktree", "remove", "--force", &tmp.to_string_lossy()],
    );
    let _ = std::fs::remove_dir_all(&tmp);

    let base_report = result?;
    Ok(Baseline {
        delta: repolish_core::diff(&base_report, head_report, base_ref, &commit),
    })
}

fn run_base(tmp: &Path, head: &RepoContext, opts: &RunOptions) -> Result<Report, BaseError> {
    let mut ctx = RepoContext::load(tmp, head.profile_overridden.then_some(head.profile))
        .map_err(|e| BaseError::Load(format!("{e:#}")))?;

    // 描述、topics、homepage 是**仓库**的属性，不是 commit 的属性——
    // 基线那一侧再打一遍 GitHub API 拿到的是同样的答案,只是多烧一次配额。
    // 更要紧的是：不复制过去的话，基线会退化成 local 模式，两侧分母不同,
    // 差值就没有意义了。
    ctx.remote = head.remote.clone();

    Ok(repolish_checks::registry().run(&ctx, opts))
}

/// 被检查的目录相对仓库根的位置。仓库根本身返回空路径。
fn subpath_in_repo(root: &Path) -> Result<PathBuf, BaseError> {
    let top = git(root, &["rev-parse", "--show-toplevel"]).map_err(BaseError::NoGit)?;
    let top = dunce::canonicalize(top.trim()).map_err(|e| BaseError::NoGit(e.to_string()))?;
    Ok(root
        .strip_prefix(&top)
        .map(|p| p.to_path_buf())
        .unwrap_or_default())
}

/// ref → 完整 commit id。
///
/// 浅克隆里基线常常不存在——CI 上 `actions/checkout` 默认只取一层。
/// 这是最常见的失败，所以它自己一条错误分支,并直接给出修法。
fn resolve(root: &Path, base_ref: &str) -> Result<String, BaseError> {
    let spec = format!("{base_ref}^{{commit}}");
    match git(root, &["rev-parse", "--verify", "--quiet", &spec]) {
        Ok(out) => {
            let id = out.trim().to_string();
            if id.is_empty() {
                Err(unknown(root, base_ref))
            } else {
                Ok(id)
            }
        }
        Err(e) if e.contains("not a git repository") => Err(BaseError::NoGit(e)),
        Err(_) => Err(unknown(root, base_ref)),
    }
}

fn unknown(root: &Path, base_ref: &str) -> BaseError {
    let shallow = root.join(".git/shallow").exists();
    let hint = if shallow {
        "this is a shallow clone, so the base commit was never fetched. \
         In GitHub Actions: actions/checkout with `fetch-depth: 0`"
            .to_string()
    } else {
        format!("no such commit, branch or tag. Try `git fetch origin {base_ref}` first")
    };
    BaseError::UnknownRef {
        r#ref: base_ref.to_string(),
        hint,
    }
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("git {} exited {}", args[0], out.status)
        } else {
            err
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 浅克隆是 CI 上最常见的失败,错误信息必须直接给出修法,
    /// 而不是让人去搜「repolish base 解析失败」
    #[test]
    fn a_shallow_clone_is_named_as_the_cause() {
        let dir = std::env::temp_dir().join("repolish-shallow-test/.git");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("shallow"), "").unwrap();
        let e = unknown(dir.parent().unwrap(), "origin/main");
        let msg = e.to_string();
        assert!(msg.contains("shallow"), "{msg}");
        assert!(msg.contains("fetch-depth"), "{msg}");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    /// `repolish demo/sample --base …` 检出的是仓库根,
    /// 不把子路径接回去就会去评分另一个项目——一份看起来正常、内容全错的差值
    #[test]
    fn a_subdirectory_is_located_again_inside_the_baseline_checkout() {
        let here = dunce::canonicalize(env!("CARGO_MANIFEST_DIR")).unwrap();
        let sub = subpath_in_repo(&here).unwrap();
        assert_eq!(sub, PathBuf::from("crates").join("repolish-cli"));

        let top = here.parent().unwrap().parent().unwrap();
        assert_eq!(subpath_in_repo(top).unwrap(), PathBuf::new());
    }

    #[test]
    fn an_ordinary_missing_ref_suggests_fetching_it() {
        let e = unknown(
            &std::env::temp_dir().join("repolish-no-such-repo"),
            "v9.9.9",
        );
        let msg = e.to_string();
        assert!(msg.contains("git fetch origin v9.9.9"), "{msg}");
    }
}
