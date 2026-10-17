#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod config;
mod dj;
mod lyrics;
mod player;
mod queue;
mod spotify;
mod theme;
mod ui;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("haze=info,librespot=warn"))
        .init();

    let config = match config::Config::load_or_create() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Haze: {e:#}\nStarting with default settings.");
            config::Config::default()
        }
    };

    // "cpu" skips the GPU driver entirely, which is the lightest option on most machines.
    if config.appearance.renderer == "cpu" && std::env::var_os("ICED_BACKEND").is_none() {
        // SAFETY: set before any other thread is started.
        unsafe { std::env::set_var("ICED_BACKEND", "tiny-skia") };
    }

    if let Err(e) = ui::run(config) {
        eprintln!("Haze: {e}");
        std::process::exit(1);
    }
}
