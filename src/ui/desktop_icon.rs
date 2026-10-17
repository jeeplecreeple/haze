//! The chosen app icon outside the window: the Haze shortcut on the desktop, and on macOS the
//! Dock and Finder icon of Haze.app. (The Linux app menu entry is kept in step by
//! `custom::sync_menu_entry`.)

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::custom;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// There was nothing to change (no desktop shortcut, or not running from Haze.app).
    Nothing,
    Updated,
    Created,
}

/// Points the desktop shortcut on Linux and Windows (or Haze.app on macOS) at `app_icon`. With
/// `create`, puts a shortcut on the desktop when there isn't one yet. Runs programs and writes
/// files, so call it off the UI thread.
pub fn sync(app_icon: &str, create: bool) -> Result<Outcome> {
    if cfg!(target_os = "linux") {
        linux(app_icon, create)
    } else if cfg!(windows) {
        windows(app_icon, create)
    } else {
        bundle(app_icon)
    }
}

/// Whether there's a desktop shortcut to make. On macOS the app itself carries the icon.
pub const CAN_CREATE: bool = cfg!(any(target_os = "linux", windows));

/// The desktop folder: what `xdg-user-dir` says on Linux, otherwise ~/Desktop.
fn desktop_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?);
    if cfg!(target_os = "linux") {
        let named = std::process::Command::new("xdg-user-dir")
            .arg("DESKTOP")
            .output()
            .ok()
            .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
        // It answers with the home folder when no desktop folder is set up.
        if let Some(dir) = named.filter(|d| d.is_dir() && *d != home) {
            return Some(dir);
        }
    }
    Some(home.join("Desktop"))
}

fn linux(app_icon: &str, create: bool) -> Result<Outcome> {
    let Some(dir) = desktop_dir() else { return Ok(Outcome::Nothing) };
    let path = dir.join("haze.desktop");
    let icon = custom::icon_file(app_icon)?;
    let icon = icon.display().to_string();
    // A link to the installed entry can't be edited, so it's swapped for a file of our own.
    let linked = path.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink());
    let outcome = match std::fs::read_to_string(&path) {
        Ok(current) if !linked => {
            let mut changed = false;
            let text: String = current
                .lines()
                .map(|line| {
                    if line.starts_with("Icon=") && line != format!("Icon={icon}") {
                        changed = true;
                        format!("Icon={icon}\n")
                    } else {
                        format!("{line}\n")
                    }
                })
                .collect();
            if !changed {
                return Ok(Outcome::Nothing);
            }
            std::fs::write(&path, text)?;
            Outcome::Updated
        }
        _ if linked || create => {
            if !dir.is_dir() {
                std::fs::create_dir_all(&dir).with_context(|| format!("making {}", dir.display()))?;
            }
            if linked {
                std::fs::remove_file(&path)?;
            }
            std::fs::write(&path, custom::desktop_entry(&std::env::current_exe()?, &icon))?;
            if linked { Outcome::Updated } else { Outcome::Created }
        }
        _ => return Ok(Outcome::Nothing),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    // GNOME's desktop only starts launchers you've marked as trusted.
    let _ = std::process::Command::new("gio")
        .args(["set", &path.display().to_string(), "metadata::trusted", "true"])
        .stderr(std::process::Stdio::null())
        .status();
    Ok(outcome)
}

fn windows(app_icon: &str, create: bool) -> Result<Outcome> {
    // Shortcuts take .ico files, so the icon is converted once per change.
    let img = ::image::load_from_memory(&custom::icon_bytes(app_icon))?.thumbnail(256, 256);
    let ico = crate::config::data_dir("icons")?.join("desktop.ico");
    img.save_with_format(&ico, ::image::ImageFormat::Ico).context("saving the .ico")?;
    // Paths go through environment variables so they are never parsed as script.
    let script = "$d = [Environment]::GetFolderPath('Desktop'); $p = Join-Path $d 'Haze.lnk'; \
        $had = Test-Path $p; \
        if ($had -or $env:HAZE_CREATE -eq '1') { \
          $s = (New-Object -ComObject WScript.Shell).CreateShortcut($p); \
          $s.TargetPath = $env:HAZE_EXE; $s.WorkingDirectory = Split-Path $env:HAZE_EXE; \
          $s.IconLocation = $env:HAZE_ICO + ',0'; $s.Save(); \
          if ($had) { 'updated' } else { 'created' } \
        }";
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("HAZE_EXE", std::env::current_exe()?)
        .env("HAZE_ICO", &ico)
        .env("HAZE_CREATE", if create { "1" } else { "0" });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up.
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().context("running PowerShell")?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(match String::from_utf8_lossy(&out.stdout).trim() {
        "created" => Outcome::Created,
        "updated" => Outcome::Updated,
        _ => Outcome::Nothing,
    })
}

/// The Haze.app this is running from, if any.
fn app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

/// Finder and the Dock (when Haze isn't running) show the bundle's icon, which a custom icon
/// replaces the same way Get Info does. The default icon removes the custom one.
#[cfg(target_os = "macos")]
fn bundle(app_icon: &str) -> Result<Outcome> {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
    use objc2_foundation::{NSData, NSString};

    let Some(app) = app_bundle() else { return Ok(Outcome::Nothing) };
    let default = custom::is_default_icon(app_icon);
    let image = if default {
        None
    } else {
        let data = NSData::with_bytes(&custom::icon_bytes(app_icon));
        Some(NSImage::initWithData(NSImage::alloc(), &data).context("macOS couldn't read the icon")?)
    };
    let path = NSString::from_str(&app.display().to_string());
    let ok = NSWorkspace::sharedWorkspace().setIcon_forFile_options(image.as_deref(), &path, NSWorkspaceIconCreationOptions(0));
    if !ok {
        bail!("macOS didn't let Haze change the icon of {}", app.display());
    }
    Ok(Outcome::Updated)
}

#[cfg(not(target_os = "macos"))]
fn bundle(_: &str) -> Result<Outcome> {
    let _ = app_bundle;
    Ok(Outcome::Nothing)
}

/// The Dock icon while Haze runs. Has to run on the main thread, which `update` does.
#[cfg(target_os = "macos")]
pub fn set_dock_icon(app_icon: &str) {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let Some(mtm) = MainThreadMarker::new() else { return };
    let image = if custom::is_default_icon(app_icon) {
        None
    } else {
        let data = NSData::with_bytes(&custom::icon_bytes(app_icon));
        NSImage::initWithData(NSImage::alloc(), &data)
    };
    #[allow(unused_unsafe)]
    unsafe {
        NSApplication::sharedApplication(mtm).setApplicationIconImage(image.as_deref());
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon(_: &str) {}
