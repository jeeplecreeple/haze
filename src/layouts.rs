//! Saved layouts: where the panels go (`[layout]`) and, optionally, how the player looks
//! (`[player]`), kept as files in the `layouts` folder so you can switch between them and share them.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::{Edge, Layout, PlayerStyle, Side};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutFile {
    pub name: String,
    pub layout: Layout,
    /// Left out, switching to the layout keeps the player as it is.
    pub player: Option<PlayerStyle>,
}

/// The built-in layouts.
pub const BUILT_IN: [&str; 5] = ["Classic", "Minimal", "Queue on the left", "Player bar on top", "Wide queue"];

pub fn built_in(name: &str) -> Option<Layout> {
    let base = Layout::default();
    Some(match name {
        "Classic" => base,
        "Minimal" => Layout { show_side_panel: false, sidebar_width: 190.0, ..base },
        "Queue on the left" => Layout { side_panel: Side::Left, sidebar: Side::Right, ..base },
        "Player bar on top" => Layout { player_bar: Edge::Top, ..base },
        "Wide queue" => Layout { side_panel_width: 420.0, sidebar_width: 200.0, ..base },
        _ => return None,
    })
}

static PLUGIN_DIRS: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// The `layouts` folders of switched-on plugins.
pub fn set_plugin_dirs(dirs: Vec<PathBuf>) {
    if let Ok(mut d) = PLUGIN_DIRS.lock() {
        *d = dirs;
    }
}

fn dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<_> = crate::config::data_dir("layouts").into_iter().collect();
    dirs.extend(PLUGIN_DIRS.lock().map(|d| d.clone()).unwrap_or_default());
    dirs
}

/// Saved layouts (yours, then plugins'), by file name.
pub fn saved() -> Vec<String> {
    let mut names: Vec<String> = dirs()
        .into_iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "toml").then(|| p.file_stem()?.to_str().map(str::to_string)).flatten()
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

pub fn load(name: &str) -> Result<LayoutFile> {
    if name.contains(['/', '\\']) {
        bail!("bad layout name");
    }
    let path = dirs().into_iter().map(|d| d.join(format!("{name}.toml"))).find(|p| p.is_file()).context("layout not found")?;
    parse(&std::fs::read_to_string(path)?)
}

pub fn parse(text: &str) -> Result<LayoutFile> {
    toml::from_str(text).map_err(|e| anyhow::anyhow!("that isn't a layout file: {e}"))
}

pub fn is_yours(name: &str) -> bool {
    !name.contains(['/', '\\']) && crate::config::data_dir("layouts").is_ok_and(|d| d.join(format!("{name}.toml")).is_file())
}

/// Saves the layout (and the player, if `player` is given) and returns its file name.
pub fn save(name: &str, layout: &Layout, player: Option<&PlayerStyle>) -> Result<String> {
    let slug = crate::plugins::slug(name);
    let file = LayoutFile { name: name.trim().to_string(), layout: layout.clone(), player: player.cloned() };
    let text = format!("# A Haze layout. Put it in the layouts folder, or import it in Settings > Layout.\n{}", toml::to_string_pretty(&file)?);
    std::fs::write(crate::config::data_dir("layouts")?.join(format!("{slug}.toml")), text)?;
    Ok(slug)
}

/// Copies a layout file into your `layouts` folder, checking it first.
pub fn import(text: &str, fallback: &str) -> Result<String> {
    let file = parse(text)?;
    let name = if file.name.trim().is_empty() { fallback } else { file.name.trim() };
    save(name, &file.layout, file.player.as_ref())
}

pub fn delete(name: &str) -> Result<()> {
    if !is_yours(name) {
        bail!("only your own layouts can be deleted");
    }
    std::fs::remove_file(crate::config::data_dir("layouts")?.join(format!("{name}.toml")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_file_round_trips() {
        let file = LayoutFile { name: "Mine".into(), layout: built_in("Minimal").unwrap(), player: Some(crate::config::player_preset("Vinyl")) };
        let back = parse(&toml::to_string_pretty(&file).unwrap()).unwrap();
        assert!(!back.layout.show_side_panel);
        assert!(back.player.unwrap().spin);
    }

    #[test]
    fn partial_layout_file_is_fine() {
        let f = parse("[layout]\nplayer_bar = \"top\"\n").unwrap();
        assert_eq!(f.layout.player_bar, Edge::Top);
        assert!(f.player.is_none());
    }
}
