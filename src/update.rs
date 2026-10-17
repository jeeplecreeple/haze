//! The built-in updater. It asks GitHub for the latest release and, when Haze runs from a folder it
//! can write to (the .tar.gz, the macOS app, the Windows zip), downloads that release and swaps it
//! in. Installs made by a package manager (`/usr/bin/haze` from the .deb or pacman) are only told
//! that an update exists, because replacing their files would go behind the package manager.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const REPO: &str = "jeeplecreeple/haze";
const USER_AGENT: &str = concat!("Haze/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    /// The release's page on GitHub.
    pub page: String,
    /// The download for this system, when the release has one.
    download: Option<String>,
}

impl Release {
    /// A made-up release for HAZE_DEMO=update.
    pub fn demo() -> Self {
        Self { version: "9.9.9".into(), page: format!("https://github.com/{REPO}/releases"), download: None }
    }

    /// Whether Haze can install this release itself.
    pub fn installable(&self) -> bool {
        self.download.is_some() && install_target().is_ok()
    }
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
}

/// The release download that matches this build.
fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("haze-linux-x86_64.tar.gz"),
        ("linux", "aarch64") => Some("haze-linux-arm64.tar.gz"),
        ("macos", "aarch64") => Some("haze-macos-arm64.zip"),
        ("macos", "x86_64") => Some("haze-macos-x86_64.zip"),
        ("windows", "x86_64") => Some("haze-windows-x86_64.zip"),
        _ => None,
    }
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(USER_AGENT).build()?)
}

/// Compares dotted version numbers, so a tag like v0.3.0.1 works as well as v0.3.1.
fn is_newer(tag: &str, current: &str) -> bool {
    let parse = |v: &str| -> Option<Vec<u64>> {
        let mut parts = v.trim().trim_start_matches(['v', 'V']).split('.').map(|p| p.parse().ok()).collect::<Option<Vec<u64>>>()?;
        while parts.len() > 1 && parts.last() == Some(&0) {
            parts.pop();
        }
        Some(parts)
    };
    matches!((parse(tag), parse(current)), (Some(tag), Some(current)) if tag > current)
}

/// The latest release, if it's newer than this build.
pub async fn check() -> Result<Option<Release>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = client()?.get(url).header("Accept", "application/vnd.github+json").send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("GitHub has no public releases of Haze (the repository may be private)");
    }
    let raw: RawRelease = resp.error_for_status()?.json().await?;
    if raw.draft || raw.prerelease || !is_newer(&raw.tag_name, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let download = asset_name()
        .and_then(|name| raw.assets.iter().find(|a| a.name == name))
        .map(|a| a.browser_download_url.clone());
    Ok(Some(Release { version: raw.tag_name.trim_start_matches(['v', 'V']).to_string(), page: raw.html_url, download }))
}

/// What an update replaces: the program file, or on macOS the whole Haze.app.
enum Target {
    Program(PathBuf),
    MacApp(PathBuf),
}

fn install_target() -> Result<Target> {
    let exe = std::env::current_exe()?.canonicalize()?;
    if cfg!(target_os = "linux") && exe.starts_with("/usr") {
        bail!("installed by a package manager");
    }
    let target = if cfg!(target_os = "macos") {
        // .../Haze.app/Contents/MacOS/haze
        match exe.ancestors().nth(3) {
            Some(app) if app.extension().is_some_and(|e| e == "app") => Target::MacApp(app.to_path_buf()),
            _ => Target::Program(exe),
        }
    } else {
        Target::Program(exe)
    };
    let folder = match &target {
        Target::Program(p) | Target::MacApp(p) => p.parent().context("no parent folder")?.to_path_buf(),
    };
    // Only offer to update when Haze can write next to itself.
    let probe = folder.join(".haze-update-probe");
    std::fs::write(&probe, b"").context("the install folder isn't writable")?;
    let _ = std::fs::remove_file(probe);
    Ok(target)
}

/// Downloads `release` and puts it in place of the running build. Returns the program to start
/// for the restart.
pub async fn install(release: Release) -> Result<PathBuf> {
    let url = release.download.context("this release has no download for your system")?;
    let target = install_target()?;
    let bytes = client()?.get(url).send().await?.error_for_status()?.bytes().await?;
    tokio::task::spawn_blocking(move || match target {
        Target::Program(exe) => replace_program(&exe, &bytes).map(|()| exe),
        Target::MacApp(app) => replace_mac_app(&app, &bytes),
    })
    .await?
}

/// The program file inside a release archive.
fn program_from_archive(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    if cfg!(windows) {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        let name = zip.file_names().find(|n| n.ends_with("haze.exe")).context("no haze.exe in the download")?.to_string();
        zip.by_name(&name)?.read_to_end(&mut out)?;
    } else {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
        for entry in tar.entries()? {
            let mut entry = entry?;
            if entry.path()?.as_ref() == Path::new("haze/haze") {
                entry.read_to_end(&mut out)?;
                return Ok(out);
            }
        }
        bail!("no haze program in the download");
    }
    Ok(out)
}

fn replace_program(exe: &Path, archive: &[u8]) -> Result<()> {
    let program = program_from_archive(archive)?;
    let new = exe.with_extension("new");
    std::fs::write(&new, program)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
    }
    if cfg!(windows) {
        // Windows won't overwrite a running program, but it will rename one.
        let old = exe.with_extension("old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(exe, &old)?;
    }
    std::fs::rename(&new, exe)?;
    Ok(())
}

/// Unpacks the new Haze.app beside the old one, then swaps them.
fn replace_mac_app(app: &Path, archive: &[u8]) -> Result<PathBuf> {
    let folder = app.parent().context("no parent folder")?;
    let staging = folder.join(".Haze-update");
    let _ = std::fs::remove_dir_all(&staging);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))?;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let Some(path) = file.enclosed_name() else { continue };
        // The zip holds dist/Haze.app/... plus the docs; keep only the app.
        let Some(rel) = path.iter().position(|c| c == "Haze.app").map(|i| path.iter().skip(i).collect::<PathBuf>()) else {
            continue;
        };
        let out = staging.join(rel);
        if file.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        std::fs::create_dir_all(out.parent().expect("has parent"))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        std::fs::write(&out, data)?;
        #[cfg(unix)]
        if let Some(mode) = file.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))?;
        }
    }
    let new_app = staging.join("Haze.app");
    if !new_app.join("Contents/MacOS/haze").exists() {
        bail!("no Haze.app in the download");
    }
    let old = folder.join(".Haze-old.app");
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(app, &old)?;
    if let Err(e) = std::fs::rename(&new_app, app) {
        let _ = std::fs::rename(&old, app);
        return Err(e.into());
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&staging);
    Ok(app.join("Contents/MacOS/haze"))
}

/// Starts the updated program and quits this one.
pub fn restart(program: &Path) -> ! {
    if let Err(e) = std::process::Command::new(program).spawn() {
        log::warn!("couldn't restart {}: {e}", program.display());
    }
    std::process::exit(0);
}

/// Removes what a Windows update leaves behind (the old program can't delete itself).
pub fn clean_up() {
    if cfg!(windows)
        && let Ok(exe) = std::env::current_exe()
    {
        let _ = std::fs::remove_file(exe.with_extension("old"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.1"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("v0.1.9", "0.2.0"));
        assert!(!is_newer("nightly", "0.2.0"));
        assert!(is_newer("v0.3.0.1", "0.3.0"));
        assert!(is_newer("V0.4.2", "0.4.1"));
        assert!(!is_newer("v0.2.5.0", "0.3.0"));
        assert!(!is_newer("v0.3.0.0", "0.3.0"));
    }

    #[test]
    fn finds_the_program_in_a_linux_archive() {
        if cfg!(windows) {
            return;
        }
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (path, body) in [("haze/README.md", &b"docs"[..]), ("haze/haze", &b"program"[..])] {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, path, body).unwrap();
        }
        let archive = tar.into_inner().unwrap().finish().unwrap();
        assert_eq!(program_from_archive(&archive).unwrap(), b"program");
    }
}
