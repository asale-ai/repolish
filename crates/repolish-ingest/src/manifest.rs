//! 包清单解析。
//!
//! 两个检查项靠它吃饭：
//! - `readme-install-consistency`：README 里的安装命令装的是不是这个包
//! - `claim-consistency`：README 里的 `npm run build` 在 scripts 里存不存在
//!
//! 只解析仓库根目录的清单。子包的清单不看——README 讲的是「怎么用这个仓库」，
//! 而 monorepo 里每个子包各有各的说明。

use serde::Serialize;

use crate::files::FileIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Cargo,
    Npm,
    Pypi,
    Go,
    Maven,
    Gem,
    Composer,
}

impl Ecosystem {
    pub fn as_str(self) -> &'static str {
        match self {
            Ecosystem::Cargo => "cargo",
            Ecosystem::Npm => "npm",
            Ecosystem::Pypi => "pypi",
            Ecosystem::Go => "go",
            Ecosystem::Maven => "maven",
            Ecosystem::Gem => "gem",
            Ecosystem::Composer => "composer",
        }
    }

    /// 该生态的安装命令写法。用于把 README 里的命令对回生态。
    pub fn install_verbs(self) -> &'static [&'static str] {
        match self {
            Ecosystem::Cargo => &["cargo add", "cargo install"],
            Ecosystem::Npm => &["npm install", "npm i ", "yarn add", "pnpm add", "bun add"],
            Ecosystem::Pypi => &["pip install", "pip3 install", "uv add", "uv pip install", "poetry add", "pipx install"],
            Ecosystem::Go => &["go get", "go install"],
            Ecosystem::Maven => &["<dependency>", "implementation "],
            Ecosystem::Gem => &["gem install"],
            Ecosystem::Composer => &["composer require"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub ecosystem: Ecosystem,
    pub path: String,
    /// 发布名。workspace 根清单可能没有
    pub name: Option<String>,
    /// 可由使用者直接调用的脚本名（npm scripts / `[project.scripts]`）
    pub scripts: Vec<String>,
    /// 可执行入口名
    pub bins: Vec<String>,
    /// 直接依赖名，用于 `repo-topics` 的本地信号词表
    pub deps: Vec<String>,
    /// 作者自己写的关键词（Cargo `keywords` / package.json `keywords` /
    /// PEP 621 `keywords`）。这是仓库里**唯一由人挑过**的主题词来源，
    /// 比从 README 正文里捞词可靠得多，`repo-topics` 的建议优先用它。
    pub keywords: Vec<String>,
}

impl Manifest {
    fn new(ecosystem: Ecosystem, path: &str) -> Self {
        Manifest {
            ecosystem,
            path: path.to_string(),
            name: None,
            scripts: Vec::new(),
            bins: Vec::new(),
            deps: Vec::new(),
            keywords: Vec::new(),
        }
    }
}

/// 根目录清单。同一仓库可能同时有多个（如 Rust + npm 包装）。
pub fn detect(files: &FileIndex) -> Vec<Manifest> {
    let mut out = Vec::new();

    if files.contains("Cargo.toml") {
        if let Some(mut m) = files.read("Cargo.toml").and_then(|t| parse_cargo(&t)) {
            if m.name.is_none() {
                fill_workspace_name(files, &mut m);
            }
            out.push(m);
        }
    }
    if files.contains("package.json") {
        if let Some(m) = files.read("package.json").and_then(|t| parse_npm(&t)) {
            out.push(m);
        }
    }
    if files.contains("pyproject.toml") {
        if let Some(m) = files.read("pyproject.toml").and_then(|t| parse_pyproject(&t)) {
            out.push(m);
        }
    } else if files.contains("setup.py") || files.contains("setup.cfg") {
        // 旧式 Python 包：拿不到结构化元数据，只登记生态
        out.push(Manifest::new(Ecosystem::Pypi, "setup.py"));
    }
    if files.contains("go.mod") {
        if let Some(m) = files.read("go.mod").and_then(|t| parse_gomod(&t)) {
            out.push(m);
        }
    }
    for (file, eco) in [
        ("pom.xml", Ecosystem::Maven),
        ("build.gradle", Ecosystem::Maven),
        ("build.gradle.kts", Ecosystem::Maven),
        ("Gemfile", Ecosystem::Gem),
        ("composer.json", Ecosystem::Composer),
    ] {
        if files.contains(file) {
            out.push(Manifest::new(eco, file));
        }
    }

    out
}

fn parse_cargo(text: &str) -> Option<Manifest> {
    let v: toml::Table = toml::from_str(text).ok()?;
    let mut m = Manifest::new(Ecosystem::Cargo, "Cargo.toml");

    m.name = v
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string);

    if let Some(bins) = v.get("bin").and_then(|b| b.as_array()) {
        m.bins = bins
            .iter()
            .filter_map(|b| b.get("name").and_then(|n| n.as_str()))
            .map(str::to_string)
            .collect();
    }
    // 有 src/main.rs 而无 [[bin]] 时，二进制名等于包名——由调用方补，这里不猜

    m.deps = table_keys(&v, "dependencies");
    m.keywords = string_array(v.get("package").and_then(|p| p.get("keywords")));
    Some(m)
}

/// workspace 根清单没有 `[package]`，但 README 讲的是其中的主 crate
/// （serde 仓库讲 `serde`，tokio 仓库讲 `tokio`）。
/// 取与仓库目录同名的成员——同名以外的猜测都不可靠，宁可留空。
fn fill_workspace_name(files: &FileIndex, m: &mut Manifest) {
    let Some(repo_dir) = files
        .root()
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
    else {
        return;
    };

    // 按**包名**匹配，不是按目录名：目录叫什么是随意的，
    // `crates/repolish-cli/` 里发布出去的可以是 `repolish`。
    let member = files
        .iter()
        .filter(|p| p.ends_with("/Cargo.toml") && p.matches('/').count() <= 2)
        .take(MAX_MEMBERS)
        .filter_map(|p| files.read(p).and_then(|t| parse_cargo(&t)))
        .find(|sub| {
            sub.name
                .as_deref()
                .is_some_and(|n| n.to_lowercase() == repo_dir)
        });

    if let Some(sub) = member {
        m.name = sub.name;
        m.bins = sub.bins;
    }
}

/// 扫描成员清单的上限。monorepo 可能有上百个子包，全读一遍不值得。
const MAX_MEMBERS: usize = 64;

fn parse_npm(text: &str) -> Option<Manifest> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut m = Manifest::new(Ecosystem::Npm, "package.json");

    m.name = v.get("name").and_then(|n| n.as_str()).map(str::to_string);

    if let Some(scripts) = v.get("scripts").and_then(|s| s.as_object()) {
        m.scripts = scripts.keys().cloned().collect();
    }
    match v.get("bin") {
        Some(serde_json::Value::Object(o)) => m.bins = o.keys().cloned().collect(),
        // `"bin": "./cli.js"` 形式，二进制名等于包名
        Some(serde_json::Value::String(_)) => m.bins = m.name.iter().cloned().collect(),
        _ => {}
    }

    for key in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(o) = v.get(key).and_then(|d| d.as_object()) {
            m.deps.extend(o.keys().cloned());
        }
    }
    if let Some(arr) = v.get("keywords").and_then(|k| k.as_array()) {
        m.keywords = arr
            .iter()
            .filter_map(|k| k.as_str())
            .map(str::to_string)
            .collect();
    }
    Some(m)
}

fn parse_pyproject(text: &str) -> Option<Manifest> {
    let v: toml::Table = toml::from_str(text).ok()?;
    let mut m = Manifest::new(Ecosystem::Pypi, "pyproject.toml");

    let project = v.get("project");
    let poetry = v.get("tool").and_then(|t| t.get("poetry"));

    m.name = project
        .or(poetry)
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string);

    for src in [project, poetry] {
        if let Some(s) = src.and_then(|p| p.get("scripts")).and_then(|s| s.as_table()) {
            m.scripts.extend(s.keys().cloned());
            m.bins.extend(s.keys().cloned());
        }
    }

    // PEP 621 的 dependencies 是需求字符串数组：`requests >= 2.0`
    if let Some(arr) = project
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        m.deps = arr
            .iter()
            .filter_map(|d| d.as_str())
            .map(requirement_name)
            .collect();
    }
    if let Some(t) = poetry.and_then(|p| p.get("dependencies")).and_then(|d| d.as_table()) {
        m.deps.extend(t.keys().cloned());
    }
    for src in [project, poetry] {
        m.keywords
            .extend(string_array(src.and_then(|p| p.get("keywords"))));
    }
    Some(m)
}

/// TOML 里的字符串数组。非数组或元素不是字符串时返回空，不猜。
fn string_array(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(|k| k.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|k| k.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_gomod(text: &str) -> Option<Manifest> {
    let module = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("module "))?
        .trim()
        .trim_matches('"');
    let mut m = Manifest::new(Ecosystem::Go, "go.mod");
    // 发布名是完整模块路径，但使用者感知的是最后一段
    m.name = module.rsplit('/').next().map(str::to_string);
    m.deps = text
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            let path = t.strip_prefix("require ").unwrap_or(t);
            let first = path.split_whitespace().next()?;
            first.contains('/').then(|| first.rsplit('/').next().unwrap_or(first).to_string())
        })
        .collect();
    Some(m)
}

fn table_keys(v: &toml::Table, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|d| d.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default()
}

/// `requests[security] >= 2.0` → `requests`
fn requirement_name(req: &str) -> String {
    req.trim()
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == '.'))
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}

/// 包名归一化：PyPI 把 `-`、`_`、`.` 视为等价，npm 的 scope 前缀不参与比对。
pub fn normalize_package_name(name: &str) -> String {
    let n = name.trim().to_lowercase();
    let n = n.rsplit('/').next().unwrap_or(&n);
    n.replace(['_', '.'], "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_manifest_yields_name_and_bins() {
        let m = parse_cargo(
            "[package]\nname = \"repolish\"\n\n[[bin]]\nname = \"repolish\"\n\n[dependencies]\nclap = \"4\"\n",
        )
        .unwrap();
        assert_eq!(m.name.as_deref(), Some("repolish"));
        assert_eq!(m.bins, vec!["repolish"]);
        assert_eq!(m.deps, vec!["clap"]);
    }

    #[test]
    fn workspace_root_has_no_package_name() {
        let m = parse_cargo("[workspace]\nmembers = [\"crates/*\"]\n").unwrap();
        assert!(m.name.is_none());
    }

    #[test]
    fn npm_scripts_and_string_bin() {
        let m = parse_npm(
            r#"{"name":"@scope/tool","bin":"./cli.js","scripts":{"build":"tsc","test":"jest"}}"#,
        )
        .unwrap();
        assert_eq!(m.name.as_deref(), Some("@scope/tool"));
        assert_eq!(m.bins, vec!["@scope/tool"]);
        let mut s = m.scripts.clone();
        s.sort();
        assert_eq!(s, vec!["build", "test"]);
    }

    #[test]
    fn pep621_dependencies_drop_version_specifiers() {
        let m = parse_pyproject(
            "[project]\nname = \"my_pkg\"\ndependencies = [\"requests>=2.0\", \"click[all] ~= 8.0\"]\n\n[project.scripts]\nmy-cli = \"my_pkg:main\"\n",
        )
        .unwrap();
        assert_eq!(m.deps, vec!["requests", "click"]);
        assert_eq!(m.bins, vec!["my-cli"]);
    }

    #[test]
    fn package_names_compare_across_separator_conventions() {
        // PyPI 上 my_pkg 与 my-pkg 是同一个包；npm 的 scope 不参与比对
        assert_eq!(normalize_package_name("my_pkg"), normalize_package_name("My-Pkg"));
        assert_eq!(normalize_package_name("@scope/tool"), "tool");
    }
}
