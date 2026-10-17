//! Things you bring yourself: the app icon, font files, playlist covers. Also the blurred cover
//! behind the lyrics.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use iced::Color;
use iced::widget::image;
use iced::window;

use crate::config;

pub const BUILTIN_ICON: &[u8] = include_bytes!("../../assets/icon.png");

/// The icons Haze comes with. `app_icon` takes one of these names, or the path to an image.
pub const BUILTIN_ICONS: [(&str, &[u8]); 8] = [
    ("haze", BUILTIN_ICON),
    ("sunset", include_bytes!("../../assets/icons/sunset.png")),
    ("ocean", include_bytes!("../../assets/icons/ocean.png")),
    ("mint", include_bytes!("../../assets/icons/mint.png")),
    ("midnight", include_bytes!("../../assets/icons/midnight.png")),
    ("mono", include_bytes!("../../assets/icons/mono.png")),
    ("aurora", include_bytes!("../../assets/icons/aurora.png")),
    ("ember", include_bytes!("../../assets/icons/ember.png")),
];

/// A built-in icon by name. Empty means the default one.
pub fn builtin_icon(name: &str) -> Option<&'static [u8]> {
    let name = name.trim();
    if name.is_empty() {
        return Some(BUILTIN_ICON);
    }
    BUILTIN_ICONS.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, bytes)| *bytes)
}

/// The image for `app_icon`: a built-in icon, an image file, or the default when the file is gone.
pub fn icon_bytes(app_icon: &str) -> std::borrow::Cow<'static, [u8]> {
    if let Some(bytes) = builtin_icon(app_icon) {
        return bytes.into();
    }
    std::fs::read(app_icon.trim()).map_err(|e| log::warn!("app icon {app_icon}: {e}")).map_or(BUILTIN_ICON.into(), Into::into)
}

/// The window icon from `app_icon`.
pub fn window_icon(app_icon: &str) -> Option<window::Icon> {
    let img = ::image::load_from_memory(&icon_bytes(app_icon))
        .or_else(|_| ::image::load_from_memory(BUILTIN_ICON))
        .ok()?
        .thumbnail(256, 256)
        .to_rgba8();
    let (w, h) = img.dimensions();
    window::icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Copies a picked image into the config folder as a PNG no larger than `max` pixels, so it keeps
/// working if the original moves. A fresh file name each time keeps iced from showing a cached copy.
pub fn import_image(from: &Path, folder: &str, stem: &str, max: u32) -> Result<PathBuf> {
    // Read by content, not the file name, so a WebP saved as .png still works.
    let img = ::image::ImageReader::open(from)
        .and_then(|r| r.with_guessed_format())
        .with_context(|| format!("couldn't open {}", from.display()))?
        .decode()
        .with_context(|| format!("{} isn't an image Haze can read", from.display()))?;
    let img = if img.width() > max || img.height() > max { img.thumbnail(max, max) } else { img };
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let safe: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let dir = config::data_dir(folder)?;
    let path = dir.join(format!("{safe}-{stamp}.png"));
    img.save_with_format(&path, ::image::ImageFormat::Png)?;
    Ok(path)
}

/// Deletes an image that `import_image` made, never anything else.
pub fn remove_imported(path: &str, folder: &str) {
    let Ok(dir) = config::data_dir(folder) else { return };
    let path = Path::new(path);
    if path.parent() == Some(dir.as_path()) {
        let _ = std::fs::remove_file(path);
    }
}

pub const IMAGE_TYPES: [&str; 11] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico", "tif", "tiff", "tga", "qoi"];

pub async fn pick_image(title: &str) -> Result<Option<PathBuf>, String> {
    pick_file(title, "Images", &IMAGE_TYPES).await
}

pub async fn pick_font() -> Result<Option<PathBuf>, String> {
    pick_file("Add a font", "Fonts", &["ttf", "otf", "ttc"]).await
}

/// Shows a file picker. `Ok(None)` means you cancelled it.
async fn pick_file(title: &str, kind: &str, types: &[&str]) -> Result<Option<PathBuf>, String> {
    let asked = std::time::Instant::now();
    let picked = rfd::AsyncFileDialog::new().set_title(title).add_filter(kind, types).pick_file().await;
    if let Some(file) = picked {
        return Ok(Some(file.path().to_path_buf()));
    }
    // Cancelling takes a person a moment. A quick "nothing" usually means the desktop has no
    // file-picker portal (common on tiling window managers); rfd then tries zenity by itself.
    if !cfg!(target_os = "linux") || asked.elapsed() > std::time::Duration::from_millis(1500) {
        return Ok(None);
    }
    if on_path("kdialog") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        let patterns = types.iter().map(|t| format!("*.{t}")).collect::<Vec<_>>().join(" ");
        if let Ok(out) = tokio::process::Command::new("kdialog")
            .args(["--title", title, "--getopenfilename", &home, &format!("{patterns}|{kind}")])
            .output()
            .await
        {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return Ok((out.status.success() && !path.is_empty()).then(|| PathBuf::from(path)));
        }
    }
    if on_path("zenity") {
        return Ok(None); // zenity was shown and you closed it
    }
    Err("Haze couldn't open a file picker on this desktop. Drag the file onto the Haze window instead, \
        or install xdg-desktop-portal-gtk (or zenity) to get one."
        .into())
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

const MENU_ENTRY_MARK: &str = "# Written by Haze for your custom icon.";

fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
}

fn menu_entry_path() -> Option<PathBuf> {
    Some(data_home()?.join("applications/haze.desktop"))
}

/// Whether a package installed Haze's desktop entry (the .deb, pacman, or a system-wide copy).
fn installed_entry_exists() -> bool {
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.split(':').filter(|d| !d.is_empty()).any(|d| Path::new(d).join("applications/haze.desktop").exists())
}

/// On Linux, docks and launchers (and on Wayland, the taskbar) take the icon from the desktop
/// entry named after the app, not from the window. So this keeps a per-user entry in step:
/// one pointing at the chosen icon when it isn't the default, and when Haze runs without an
/// installed entry (from the .tar.gz or a build folder) one that uses the default icon.
pub fn sync_menu_entry(app_icon: &str) -> Result<()> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    let Some(path) = menu_entry_path() else { return Ok(()) };
    let name = app_icon.trim();
    let icon = if name.is_empty() || name.eq_ignore_ascii_case("haze") {
        if installed_entry_exists() {
            // The package's entry already has the right icon; drop any custom one of ours.
            if std::fs::read_to_string(&path).is_ok_and(|s| s.contains(MENU_ENTRY_MARK)) {
                std::fs::remove_file(&path)?;
            }
            return Ok(());
        }
        let file = data_home().context("no home folder")?.join("icons/hicolor/256x256/apps/haze.png");
        if std::fs::read(&file).ok().as_deref() != Some(BUILTIN_ICON) {
            std::fs::create_dir_all(file.parent().expect("has parent"))?;
            std::fs::write(&file, BUILTIN_ICON)?;
        }
        "haze".to_string()
    } else if let Some(bytes) = builtin_icon(name) {
        let file = config::data_dir("icons")?.join(format!("builtin-{}.png", name.to_ascii_lowercase()));
        if !file.exists() {
            std::fs::write(&file, bytes)?;
        }
        file.display().to_string()
    } else {
        name.to_string()
    };
    let current = std::fs::read_to_string(&path);
    if current.as_ref().is_ok_and(|s| !s.contains(MENU_ENTRY_MARK)) {
        // You wrote your own entry; leave it alone.
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let entry = format!(
        "{MENU_ENTRY_MARK}\n[Desktop Entry]\nType=Application\nName=Haze\n\
         Comment=Lightweight Spotify client with an AI DJ\nExec=\"{}\"\nIcon={icon}\n\
         Terminal=false\nCategories=Audio;Music;Player;AudioVideo;\nStartupWMClass=haze\n",
        exe.display(),
    );
    if current.ok().as_deref() != Some(entry.as_str()) {
        std::fs::create_dir_all(path.parent().expect("has parent"))?;
        std::fs::write(&path, entry)?;
    }
    Ok(())
}

/// A custom-icon menu entry written under the app's old name points at the old program, so it is
/// replaced with one under the new name.
pub fn migrate_menu_entry() {
    let Some(new) = menu_entry_path() else { return };
    let old = new.with_file_name(format!("{}.desktop", config::OLD_NAME));
    // Its first line is MENU_ENTRY_MARK with the old name in it.
    let ours = std::fs::read_to_string(&old).is_ok_and(|s| s.contains("for your custom icon."));
    if old == new || !ours {
        return;
    }
    let _ = std::fs::remove_file(&old);
}

/// A font file's family name, which is what `font_family` has to match.
pub fn font_family(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let names = face.names();
    let pick = |id| {
        names.into_iter().filter(|n| n.name_id == id && n.is_unicode()).find_map(|n| n.to_string())
    };
    pick(ttf_parser::name_id::TYPOGRAPHIC_FAMILY).or_else(|| pick(ttf_parser::name_id::FAMILY))
}

/// Every font file to load at startup: `font_files` plus whatever is in the `fonts` folder.
pub fn font_paths(extra: &[String]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = extra.iter().map(PathBuf::from).collect();
    if let Ok(dir) = config::data_dir("fonts") {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc"))
            })
            .collect();
        found.sort();
        paths.extend(found);
    }
    paths
}

/// Reads the font files, returning their bytes and the family names they provide.
pub fn load_fonts(extra: &[String]) -> (Vec<Vec<u8>>, Vec<String>) {
    let mut fonts = Vec::new();
    let mut families = Vec::new();
    for path in font_paths(extra) {
        match std::fs::read(&path) {
            Ok(bytes) => {
                if let Some(name) = font_family(&bytes)
                    && !families.contains(&name)
                {
                    families.push(name);
                }
                fonts.push(bytes);
            }
            Err(e) => log::warn!("font {}: {e}", path.display()),
        }
    }
    (fonts, families)
}

/// The font for `font_family`: empty means the bundled Inter, "system" the system's sans-serif.
/// iced wants `'static` font names. Names are leaked once each, so changing fonts in Settings
/// doesn't grow memory beyond a few bytes per distinct name.
pub fn font(name: &str) -> iced::Font {
    use std::collections::HashSet;
    use std::sync::Mutex;
    static NAMES: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
    let name = name.trim();
    if name.is_empty() {
        return iced::Font::with_name("Inter");
    }
    if name.eq_ignore_ascii_case("system") {
        return iced::Font::DEFAULT;
    }
    let mut names = NAMES.lock().expect("font names");
    let set = names.get_or_insert_with(HashSet::new);
    let name: &'static str = match set.get(name) {
        Some(n) => n,
        None => {
            let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
            set.insert(leaked);
            leaked
        }
    };
    iced::Font::with_name(name)
}

/// A tiny, blurred copy of the cover tinted toward the page colour, stretched behind the lyrics.
/// A blurred copy of a cover, faded toward `toward`; `keep` is how much of the cover's own colour stays.
pub fn blurred_cover(bytes: &[u8], toward: Color, keep: f32) -> Option<image::Handle> {
    let mut img = ::image::load_from_memory(bytes).ok()?.thumbnail(40, 40).to_rgba8();
    let tint = [toward.r, toward.g, toward.b].map(|c| c * 255.0);
    // Flatten any transparency onto the page colour first, so see-through corners don't go black.
    for p in img.pixels_mut() {
        let a = p.0[3] as f32 / 255.0;
        for (c, t) in p.0.iter_mut().zip(tint) {
            *c = (*c as f32 * a + t * (1.0 - a)) as u8;
        }
        p.0[3] = 255;
    }
    let mut img = ::image::imageops::blur(&img, 4.0);
    for p in img.pixels_mut() {
        for (c, t) in p.0.iter_mut().zip(tint) {
            *c = (*c as f32 * keep + t * (1.0 - keep)) as u8;
        }
        p.0[3] = 255;
    }
    let (w, h) = img.dimensions();
    Some(image::Handle::from_rgba(w, h, img.into_raw()))
}

/// Opens a web page in the browser.
pub fn open_url(url: &str) {
    open_folder(Path::new(url));
}

/// Opens a folder in the file manager (or a URL in the browser).
pub fn open_folder(path: &Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(program).arg(path).spawn() {
        log::warn!("couldn't open {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_icon_decodes() {
        assert!(window_icon("").is_some());
        assert!(window_icon("/does/not/exist.png").is_some());
    }

    #[test]
    fn blurs_a_cover() {
        let handle = blurred_cover(BUILTIN_ICON, Color::BLACK, 0.4);
        assert!(matches!(handle, Some(image::Handle::Rgba { width: 40, height: 40, .. })));
    }
}
