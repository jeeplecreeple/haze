//! Things you bring yourself: the app icon, font files, playlist covers. Also the blurred cover
//! behind the lyrics.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use iced::Color;
use iced::widget::image;
use iced::window;

use crate::config;

pub const BUILTIN_ICON: &[u8] = include_bytes!("../../assets/icon.png");

/// The window icon from `app_icon`, or the built-in one.
pub fn window_icon(path: &str) -> Option<window::Icon> {
    let bytes = match config::non_empty(path) {
        Some(p) => std::fs::read(p).map_err(|e| log::warn!("app icon {p}: {e}")).ok(),
        None => None,
    };
    let img = ::image::load_from_memory(bytes.as_deref().unwrap_or(BUILTIN_ICON))
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
    let img = ::image::open(from).with_context(|| format!("couldn't read {}", from.display()))?;
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

pub async fn pick_image(title: &str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .add_filter("Images", &["png", "jpg", "jpeg"])
        .pick_file()
        .await
        .map(|f| f.path().to_path_buf())
}

pub async fn pick_font() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Add a font")
        .add_filter("Fonts", &["ttf", "otf", "ttc"])
        .pick_file()
        .await
        .map(|f| f.path().to_path_buf())
}

const MENU_ENTRY_MARK: &str = "# Written by Haze for your custom icon.";

fn menu_entry_path() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("applications/haze.desktop"))
}

/// On Linux the launcher and (on Wayland) the taskbar take the icon from the app's desktop entry,
/// so a custom icon also writes a per-user entry that points at it. `None` removes that entry.
pub fn update_menu_entry(icon: Option<&Path>) -> Result<()> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    let Some(path) = menu_entry_path() else { return Ok(()) };
    let ours = std::fs::read_to_string(&path).map(|s| s.contains(MENU_ENTRY_MARK));
    match icon {
        None => {
            if ours.unwrap_or(false) {
                std::fs::remove_file(&path)?;
            }
        }
        Some(icon) => {
            if ours.as_ref().is_ok_and(|ours| !ours) {
                // The user wrote their own entry; leave it alone.
                return Ok(());
            }
            let exe = std::env::current_exe()?;
            std::fs::create_dir_all(path.parent().expect("has parent"))?;
            std::fs::write(
                &path,
                format!(
                    "{MENU_ENTRY_MARK}\n[Desktop Entry]\nType=Application\nName=Haze\n\
                     Comment=Lightweight Spotify client with an AI DJ\nExec=\"{}\"\nIcon={}\n\
                     Terminal=false\nCategories=Audio;Music;Player;AudioVideo;\nStartupWMClass=haze\n",
                    exe.display(),
                    icon.display()
                ),
            )?;
        }
    }
    Ok(())
}

/// A custom-icon menu entry written under the app's old name points at the old program, so it is
/// replaced with one under the new name.
pub fn migrate_menu_entry(app_icon: &str) {
    let Some(new) = menu_entry_path() else { return };
    let old = new.with_file_name(format!("{}.desktop", config::OLD_NAME));
    // Its first line is MENU_ENTRY_MARK with the old name in it.
    let ours = std::fs::read_to_string(&old).is_ok_and(|s| s.contains("for your custom icon."));
    if old == new || !ours {
        return;
    }
    let _ = std::fs::remove_file(&old);
    if let Some(icon) = config::non_empty(app_icon)
        && let Err(e) = update_menu_entry(Some(Path::new(icon)))
    {
        log::warn!("menu entry: {e}");
    }
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
pub fn blurred_cover(bytes: &[u8], toward: Color) -> Option<image::Handle> {
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
            *c = (*c as f32 * 0.4 + t * 0.6) as u8;
        }
        p.0[3] = 255;
    }
    let (w, h) = img.dimensions();
    Some(image::Handle::from_rgba(w, h, img.into_raw()))
}

/// Opens a folder in the file manager.
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
        let handle = blurred_cover(BUILTIN_ICON, Color::BLACK);
        assert!(matches!(handle, Some(image::Handle::Rgba { width: 40, height: 40, .. })));
    }
}
