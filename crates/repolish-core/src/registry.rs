//! 检查项注册表：统一处理适用性、远程模式、用户过滤。
//!
//! 检查项自身只负责「怎么判分」，不负责「要不要跑」。

use std::collections::HashSet;

use crate::check::Check;
use crate::outcome::Outcome;
use crate::score::{CheckResult, Mode, Report};
use repolish_ingest::RepoContext;

#[derive(Default)]
pub struct Registry {
    checks: Vec<Box<dyn Check>>,
}

pub struct RunOptions {
    pub mode: Mode,
    /// 非空时只跑这些 id
    pub only: HashSet<String>,
    pub skip: HashSet<String>,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            mode: Mode::Local,
            only: HashSet::new(),
            skip: HashSet::new(),
        }
    }
}

impl Registry {
    pub fn new() -> Self {
        Registry { checks: Vec::new() }
    }

    pub fn register(&mut self, check: Box<dyn Check>) -> &mut Self {
        self.checks.push(check);
        self
    }

    pub fn len(&self) -> usize {
        self.checks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.checks.iter().map(|c| c.id()).collect()
    }

    pub fn run(&self, ctx: &RepoContext, opts: &RunOptions) -> Report {
        let results = self
            .checks
            .iter()
            .map(|check| {
                let outcome = self.decide(check.as_ref(), ctx, opts);
                CheckResult {
                    id: check.id(),
                    category: check.category(),
                    risk: check.risk(),
                    outcome,
                }
            })
            .collect();

        Report::build(results, ctx.profile, ctx.profile_overridden, opts.mode)
    }

    fn decide(&self, check: &dyn Check, ctx: &RepoContext, opts: &RunOptions) -> Outcome {
        let id = check.id();

        if opts.skip.contains(id) || (!opts.only.is_empty() && !opts.only.contains(id)) {
            return Outcome::skipped("被 --only / --skip 过滤");
        }
        if !check.applies_to(ctx.profile) {
            return Outcome::NotApplicable {
                profile: ctx.profile,
            };
        }
        if check.requires_remote() && opts.mode == Mode::Local {
            return Outcome::skipped("需要 --remote");
        }
        check.run(ctx)
    }
}
