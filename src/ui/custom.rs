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

pub async fn pick_toml(title: &str) -> Result<Option<PathBuf>, String> {
    pick_file(title, "Haze files", &["toml"]).await
}

pub async fn pick_zip() -> Result<Option<PathBuf>, String> {
    pick_file("Install a plugin", "Plugins", &["zip"]).await
}

pub async fn pick_voice() -> Result<Option<PathBuf>, String> {
    pick_file("Choose a Piper voice", "Piper voices", &["onnx"]).await
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

/// The sizes written into your icon theme, so every dock, menu and taskbar finds one that fits.
const THEME_ICON_SIZES: [u32; 10] = [16, 22, 24, 32, 48, 64, 96, 128, 256, 512];

/// Remembers which icon is in your icon theme, so it's only rewritten when it changes.
const THEME_ICON_STAMP: &str = "haze-icon.stamp";

fn theme_icon_dir() -> Option<PathBuf> {
    Some(data_home()?.join("icons/hicolor"))
}

/// On Linux, docks, menus, notifications and (on Wayland) the taskbar look the icon up by the
/// desktop entry's `Icon=haze`, in your icon theme. So the chosen icon is written as `haze` in
/// every size into `~/.local/share/icons/hicolor`, which comes before the system's icons. That
/// covers the package's own entry too, and launchers you've pinned. When Haze runs without an
/// installed entry (from the .tar.gz or a build folder) it also writes one to the app menu.
pub fn sync_menu_entry(app_icon: &str) -> Result<()> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    let Some(path) = menu_entry_path() else { return Ok(()) };
    let theme = theme_icon_dir().context("no home folder")?;
    let installed = installed_entry_exists();
    let default = is_default_icon(app_icon);
    let stamp_file = theme.join(THEME_ICON_STAMP);
    let source = if default { String::new() } else { app_icon.trim().to_string() };
    // The file's modification time too, so editing your image in place counts as a change.
    let modified = std::fs::metadata(&source).and_then(|m| m.modified()).ok().map(|t| format!("{t:?}")).unwrap_or_default();
    let stamp = format!("{source}\n{modified}\n{installed}\n");
    let mut changed = false;
    if std::fs::read_to_string(&stamp_file).ok().as_deref() != Some(stamp.as_str()) {
        if default && installed {
            // The package's icons are right; take ours away.
            for size in THEME_ICON_SIZES {
                let _ = std::fs::remove_file(theme.join(format!("{size}x{size}/apps/haze.png")));
            }
            let _ = std::fs::remove_file(&stamp_file);
        } else {
            write_theme_icons(&theme, &icon_bytes(app_icon))?;
            std::fs::write(&stamp_file, &stamp)?;
        }
        changed = true;
    }

    // The menu entry. Older versions wrote one pointing at the image file; it isn't needed when
    // a package installed Haze, and otherwise it now uses the themed icon.
    let current = std::fs::read_to_string(&path);
    if current.as_ref().is_ok_and(|s| !s.contains(MENU_ENTRY_MARK)) {
        // You wrote your own entry; leave it alone.
    } else if installed {
        if current.is_ok() {
            std::fs::remove_file(&path)?;
            changed = true;
        }
    } else {
        let entry = format!("{MENU_ENTRY_MARK}\n{}", desktop_entry(&std::env::current_exe()?, "haze"));
        if current.ok().as_deref() != Some(entry.as_str()) {
            std::fs::create_dir_all(path.parent().expect("has parent"))?;
            std::fs::write(&path, entry)?;
            changed = true;
        }
    }
    if changed {
        refresh_desktop_caches(&theme, path.parent().expect("has parent"));
    }
    Ok(())
}

/// `bytes` as `haze.png` in every size of the icon theme, square and centred.
fn write_theme_icons(theme: &Path, bytes: &[u8]) -> Result<()> {
    let img = ::image::load_from_memory(bytes).or_else(|_| ::image::load_from_memory(BUILTIN_ICON))?.to_rgba8();
    // Pad to a square so a wide image isn't stretched.
    let side = img.width().max(img.height());
    let mut square = ::image::RgbaImage::new(side, side);
    ::image::imageops::overlay(&mut square, &img, ((side - img.width()) / 2) as i64, ((side - img.height()) / 2) as i64);
    for size in THEME_ICON_SIZES {
        let file = theme.join(format!("{size}x{size}/apps/haze.png"));
        std::fs::create_dir_all(file.parent().expect("has parent"))?;
        let scaled = ::image::imageops::resize(&square, size, size, ::image::imageops::FilterType::Lanczos3);
        scaled.save_with_format(&file, ::image::ImageFormat::Png).with_context(|| format!("writing {}", file.display()))?;
    }
    Ok(())
}

/// Tells the desktop the icons and menu entries changed. Each of these is optional: they are
/// run only when installed, and in the background.
fn refresh_desktop_caches(theme: &Path, applications: &Path) {
    let run = |program: &str, args: &[&std::ffi::OsStr]| {
        if on_path(program) {
            let _ = std::process::Command::new(program)
                .args(args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    };
    // GTK reads a cache file when there is one; an out-of-date one hides the new icon.
    if theme.join("icon-theme.cache").exists() {
        run("gtk-update-icon-cache", &["-q".as_ref(), "-t".as_ref(), "-f".as_ref(), theme.as_os_str()]);
    }
    run("update-desktop-database", &["-q".as_ref(), applications.as_os_str()]);
    // KDE keeps its own cache of menu entries and icons.
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_lowercase();
    if desktop.contains("kde") {
        for program in ["kbuildsycoca6", "kbuildsycoca5"] {
            if on_path(program) {
                run(program, &[]);
                break;
            }
        }
    }
}

/// A Linux launcher for Haze with the given icon.
pub fn desktop_entry(exe: &Path, icon: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Haze\n\
         Comment=Lightweight Spotify client with an AI DJ\nExec=\"{}\"\nIcon={icon}\n\
         Terminal=false\nCategories=Audio;Music;Player;AudioVideo;\nStartupWMClass=haze\n",
        exe.display(),
    )
}

/// Whether `app_icon` is Haze's own icon.
pub fn is_default_icon(app_icon: &str) -> bool {
    let name = app_icon.trim();
    name.is_empty() || name.eq_ignore_ascii_case("haze")
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

/// The cover cut to the Now Playing outline, see-through outside it.
pub fn shaped_cover(bytes: &[u8], outline: super::shape::Outline) -> Option<image::Handle> {
    let img = ::image::load_from_memory(bytes).ok()?;
    let img = if img.width().min(img.height()) > 600 { img.thumbnail(600, 600) } else { img }.to_rgba8();
    let cut = outline.mask(&img);
    let (w, h) = cut.dimensions();
    Some(image::Handle::from_rgba(w, h, cut.into_raw()))
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
