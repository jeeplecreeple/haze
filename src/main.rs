#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod config;
mod dj;
mod layouts;
mod lyrics;
mod player;
mod plugins;
mod queue;
mod spotify;
mod theme;
mod ui;
mod update;
mod visualizer;

fn main() {
    // glibc gives every thread that allocates its own memory pool, and librespot, tokio and the
    // audio output use many threads, so most of that memory sits unused. Two pools is plenty.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: called first thing, before any other thread exists.
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
    }
    let logger = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("haze=info,librespot=warn")).build();
    log::set_max_level(logger.filter());
    let _ = log::set_boxed_logger(Box::new(player::ProblemLogger(logger)));

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

    update::clean_up();
    if let Err(e) = ui::run(config) {
        eprintln!("Haze: {e}");
        std::process::exit(1);
    }
}
