//! Plugins: folders in the `plugins` folder, each with a `plugin.toml`. A plugin can bring themes
//! (`themes/*.toml`), layouts (`layouts/*.toml`) and commands that run when something happens in
//! Haze (`[hooks]`). They install from a GitHub repo, a link to a .zip, a .zip file or a folder.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::config;

/// Downloads bigger than this are refused.
const MAX_DOWNLOAD: usize = 50 * 1024 * 1024;

/// What happens in Haze that a plugin's hooks can react to.
pub const EVENTS: [(&str, &str); 5] = [
    ("start", "Haze starts"),
    ("track", "a new song starts"),
    ("play", "playback resumes"),
    ("pause", "playback pauses"),
    ("dj", "the DJ says something"),
];

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub homepage: String,
    /// Event name → shell command, run from the plugin's folder.
    pub hooks: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Plugin {
    /// The plugin's folder name, which is also how the config refers to it.
    pub id: String,
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub themes: usize,
    pub layouts: usize,
}

impl Plugin {
    pub fn name(&self) -> &str {
        if self.manifest.name.trim().is_empty() { &self.id } else { &self.manifest.name }
    }

    /// Plugins with hooks run programs, so they start switched off.
    pub fn runs_commands(&self) -> bool {
        !self.manifest.hooks.is_empty()
    }
}

pub fn dir() -> Result<PathBuf> {
    config::data_dir("plugins")
}

/// Every installed plugin, by name.
pub fn list() -> Vec<Plugin> {
    let Ok(dir) = dir() else { return Vec::new() };
    let mut found: Vec<Plugin> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| load(&e.path()).map_err(|err| log::warn!("plugin {}: {err:#}", e.path().display())).ok())
        .collect();
    found.sort_by_key(|p| p.name().to_lowercase());
    found
}

fn load(dir: &Path) -> Result<Plugin> {
    let text = std::fs::read_to_string(dir.join("plugin.toml")).context("no plugin.toml")?;
    let manifest: Manifest = toml::from_str(&text).context("plugin.toml")?;
    let count = |sub: &str| {
        std::fs::read_dir(dir.join(sub))
            .map(|d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "toml")).count())
            .unwrap_or(0)
    };
    Ok(Plugin {
        id: dir.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string(),
        dir: dir.to_path_buf(),
        themes: count("themes"),
        layouts: count("layouts"),
        manifest,
    })
}

/// The `themes` or `layouts` folders of the plugins that are switched on.
pub fn content_dirs(enabled: &[String], kind: &str) -> Vec<PathBuf> {
    let Ok(root) = dir() else { return Vec::new() };
    enabled.iter().filter(|id| safe_name(id)).map(|id| root.join(id).join(kind)).filter(|d| d.is_dir()).collect()
}

fn safe_name(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\']) && id != "." && id != ".."
}

/// Where a plugin comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// A GitHub repository, optionally a folder inside it and a branch or tag.
    GitHub { owner: String, repo: String, git_ref: Option<String>, path: Option<String> },
    /// A link to a .zip.
    Url(String),
    /// A .zip file or a folder on this computer.
    Local(PathBuf),
}

impl Source {
    /// Understands `owner/repo`, `owner/repo/some/folder`, a github.com link (including
    /// `/tree/<branch>/<folder>`), any other http(s) link to a zip, or a path.
    pub fn parse(input: &str) -> Result<Source> {
        let s = input.trim().trim_end_matches('/');
        if s.is_empty() {
            bail!("type a GitHub repo (owner/name), a link, or a path first");
        }
        let path = PathBuf::from(shellexpand_home(s));
        if path.exists() {
            return Ok(Source::Local(path));
        }
        let rest = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://"));
        let github = match rest {
            Some(r) => match r.strip_prefix("github.com/").or_else(|| r.strip_prefix("www.github.com/")) {
                Some(g) => g.trim_end_matches(".git"),
                None => return Ok(Source::Url(s.to_string())),
            },
            None => s,
        };
        let parts: Vec<&str> = github.split('/').filter(|p| !p.is_empty()).collect();
        if parts.len() < 2 || parts.iter().any(|p| p.contains(char::is_whitespace)) {
            bail!("\"{s}\" isn't a GitHub repo (owner/name), a link, or a file that exists");
        }
        let (owner, repo) = (parts[0].to_string(), parts[1].trim_end_matches(".git").to_string());
        let (git_ref, folder) = match parts.get(2) {
            Some(&"tree") | Some(&"blob") => (parts.get(3).map(|r| r.to_string()), &parts[4.min(parts.len())..]),
            _ => (None, &parts[2..]),
        };
        let path = (!folder.is_empty()).then(|| folder.join("/"));
        Ok(Source::GitHub { owner, repo, git_ref, path })
    }
}

fn shellexpand_home(s: &str) -> String {
    match (s.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => s.to_string(),
    }
}

/// Installs (or updates) a plugin and returns it.
pub async fn install(source: Source) -> Result<Plugin> {
    let (zip, folder, inner) = match &source {
        Source::Local(p) if p.is_dir() => (None, Some(p.clone()), None),
        Source::Local(p) => (Some(tokio::fs::read(p).await.with_context(|| format!("reading {}", p.display()))?), None, None),
        Source::Url(url) => (Some(download(url).await?), None, None),
        Source::GitHub { owner, repo, git_ref, path } => {
            let r = git_ref.as_deref().unwrap_or("HEAD");
            let url = format!("https://codeload.github.com/{owner}/{repo}/zip/{r}");
            let bytes = download(&url).await.map_err(|e| {
                e.context(format!("downloading {owner}/{repo} from GitHub (is the repo public, and the name right?)"))
            })?;
            (Some(bytes), None, path.clone())
        }
    };
    tokio::task::spawn_blocking(move || match (zip, folder) {
        (Some(bytes), _) => install_zip(&bytes, inner.as_deref()),
        (None, Some(dir)) => install_folder(&dir),
        _ => unreachable!(),
    })
    .await?
}

async fn download(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("haze/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()?;
    let resp = client.get(url).send().await?.error_for_status()?;
    if resp.content_length().is_some_and(|n| n as usize > MAX_DOWNLOAD) {
        bail!("that download is over 50 MB, which is too big for a plugin");
    }
    let bytes = resp.bytes().await?;
    if bytes.len() > MAX_DOWNLOAD {
        bail!("that download is over 50 MB, which is too big for a plugin");
    }
    Ok(bytes.to_vec())
}

/// Unpacks a zip into a staging folder, finds the plugin in it, and moves it into place.
fn install_zip(bytes: &[u8], inner: Option<&str>) -> Result<Plugin> {
    let root = dir()?;
    let staging = root.join(format!(".installing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let result = (|| {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("that isn't a zip file")?;
        let mut total = 0usize;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            // enclosed_name drops entries that would land outside the folder ("../", absolute paths).
            let Some(path) = file.enclosed_name() else { continue };
            let out = staging.join(path);
            if file.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;
            total += data.len();
            if total > MAX_DOWNLOAD * 2 {
                bail!("the plugin unpacks to over 100 MB");
            }
            std::fs::create_dir_all(out.parent().context("no parent")?)?;
            std::fs::write(&out, data)?;
        }
        let found = find_plugin_root(&staging, inner)?;
        install_folder(&found)
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// The folder holding plugin.toml: `inner` inside the zip's top folder if given, otherwise the
/// shallowest one.
fn find_plugin_root(staging: &Path, inner: Option<&str>) -> Result<PathBuf> {
    if let Some(inner) = inner {
        let candidates = std::iter::once(staging.to_path_buf())
            .chain(std::fs::read_dir(staging)?.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
        for base in candidates {
            let dir = base.join(inner);
            if dir.join("plugin.toml").is_file() {
                return Ok(dir);
            }
        }
        bail!("there's no plugin.toml in the folder {inner}");
    }
    let mut queue = vec![(staging.to_path_buf(), 0)];
    while let Some((dir, depth)) = (!queue.is_empty()).then(|| queue.remove(0)) {
        if dir.join("plugin.toml").is_file() {
            return Ok(dir);
        }
        if depth < 4 {
            let mut subs: Vec<PathBuf> = std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
            subs.sort();
            queue.extend(subs.into_iter().map(|p| (p, depth + 1)));
        }
    }
    bail!("there's no plugin.toml in it, so it isn't a Haze plugin")
}

/// Copies a plugin folder into the plugins folder, replacing an older copy of the same plugin.
fn install_folder(src: &Path) -> Result<Plugin> {
    let manifest = load(src)?.manifest;
    let fallback = src.file_name().and_then(|n| n.to_str()).unwrap_or("plugin").to_string();
    let id = slug(if manifest.name.trim().is_empty() { &fallback } else { &manifest.name });
    let dest = dir()?.join(&id);
    if src.canonicalize().ok() == dest.canonicalize().ok() {
        return load(&dest);
    }
    let fresh = dest.with_extension("new");
    let _ = std::fs::remove_dir_all(&fresh);
    copy_dir(src, &fresh, 0)?;
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&fresh, &dest)?;
    load(&dest)
}

fn copy_dir(from: &Path, to: &Path, depth: usize) -> Result<()> {
    if depth > 8 {
        bail!("the plugin's folders are nested too deeply");
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        if kind.is_dir() {
            copy_dir(&entry.path(), &to.join(&name), depth + 1)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

pub fn uninstall(id: &str) -> Result<()> {
    if !safe_name(id) {
        bail!("bad plugin name");
    }
    std::fs::remove_dir_all(dir()?.join(id)).context("removing the plugin's folder")
}

/// A folder name made from a plugin's name: lowercase letters, digits and dashes.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "plugin".into() } else { out }
}

/// Runs the `event` hooks of the switched-on plugins, without waiting for them.
pub fn run_hooks(enabled: &[String], event: &str, vars: &[(&str, String)]) {
    if enabled.is_empty() {
        return;
    }
    for plugin in list().into_iter().filter(|p| enabled.contains(&p.id)) {
        let Some(command) = plugin.manifest.hooks.get(event) else { continue };
        let mut cmd = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = std::process::Command::new("sh");
            c.args(["-c", command]);
            c
        };
        cmd.current_dir(&plugin.dir).env("HAZE_EVENT", event).env("HAZE_PLUGIN_DIR", &plugin.dir);
        for (k, v) in vars {
            cmd.env(k, v);
        }
        match cmd.stdin(std::process::Stdio::null()).spawn() {
            // Reap it in the background so it doesn't linger as a zombie.
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
            }
            Err(e) => log::warn!("plugin {} ({event}): {e}", plugin.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources() {
        let gh = |owner: &str, repo: &str, r: Option<&str>, p: Option<&str>| Source::GitHub {
            owner: owner.into(),
            repo: repo.into(),
            git_ref: r.map(Into::into),
            path: p.map(Into::into),
        };
        assert_eq!(Source::parse("jeeplecreeple/haze-themes").unwrap(), gh("jeeplecreeple", "haze-themes", None, None));
        assert_eq!(Source::parse("a/b/plugins/x").unwrap(), gh("a", "b", None, Some("plugins/x")));
        assert_eq!(Source::parse("https://github.com/a/b.git").unwrap(), gh("a", "b", None, None));
        assert_eq!(
            Source::parse("https://github.com/a/b/tree/main/plugins/notify/").unwrap(),
            gh("a", "b", Some("main"), Some("plugins/notify"))
        );
        assert_eq!(Source::parse("https://example.com/p.zip").unwrap(), Source::Url("https://example.com/p.zip".into()));
        assert!(Source::parse("just words").is_err());
        assert!(Source::parse("").is_err());
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Now Playing → File!"), "now-playing-file");
        assert_eq!(slug("   "), "plugin");
    }

    #[test]
    fn finds_nested_plugin() {
        let tmp = std::env::temp_dir().join(format!("haze-plugin-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("repo-main/plugins/a")).unwrap();
        std::fs::create_dir_all(tmp.join("repo-main/plugins/b")).unwrap();
        std::fs::write(tmp.join("repo-main/plugins/a/plugin.toml"), "name = \"A\"").unwrap();
        std::fs::write(tmp.join("repo-main/plugins/b/plugin.toml"), "name = \"B\"").unwrap();
        assert_eq!(find_plugin_root(&tmp, Some("plugins/b")).unwrap(), tmp.join("repo-main/plugins/b"));
        assert_eq!(find_plugin_root(&tmp, None).unwrap(), tmp.join("repo-main/plugins/a"));
        assert!(find_plugin_root(&tmp, Some("nope")).is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
