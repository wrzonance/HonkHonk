use honkhonk::state::Renderer;

fn effective_renderer(env_val: Option<&str>, config_pref: Renderer) -> Renderer {
    match env_val {
        Some("software") | Some("tiny-skia") => Renderer::TinySkia,
        Some("wgpu") => Renderer::Wgpu,
        _ => config_pref,
    }
}

fn main() -> iced::Result {
    pipewire::init();

    if let Err(e) = gtk::init() {
        eprintln!("fatal: failed to initialize GTK (required for system tray): {e}");
        std::process::exit(1);
    }

    let config = match honkhonk::state::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: failed to load config, using defaults: {e}");
            honkhonk::state::AppConfig::default()
        }
    };

    let sounds = match honkhonk::state::Library::scan(&config.sound_directories) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: failed to scan sound library: {e}");
            Vec::new()
        }
    };

    let slots = honkhonk::state::SlotMap::load();

    // Renderer selection must happen before any threads are spawned.
    // set_var is undefined behavior when other threads are live.
    // At this point: config loaded, sounds scanned, slots loaded — no threads yet.
    let renderer = effective_renderer(
        std::env::var("HONKHONK_RENDERER").ok().as_deref(),
        config.renderer,
    );
    let backend_value = match renderer {
        Renderer::TinySkia => "tiny-skia",
        Renderer::Wgpu => "wgpu",
    };
    // SAFETY: No threads have been spawned yet. pipewire::init() and gtk::init()
    // do not spawn Rust threads visible to std::thread. The audio thread is
    // spawned below (audio::spawn), and build_tray() does not spawn threads.
    // ICED_BACKEND is read by iced_renderer::fallback::Compositor::with_backend
    // during iced::application() init, which runs after this assignment.
    #[allow(unused_unsafe)] // safe on edition 2021, required on edition 2024
    unsafe {
        std::env::set_var("ICED_BACKEND", backend_value);
    }

    let tray_handle = match honkhonk::tray::build_tray() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("fatal: failed to initialize system tray: {e}");
            std::process::exit(1);
        }
    };

    let audio_handle = match honkhonk::audio::spawn(config.mic_passthrough) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("fatal: failed to start audio engine: {e}");
            std::process::exit(1);
        }
    };

    let tray_handle = std::sync::Mutex::new(Some(tray_handle));
    let audio_handle = std::sync::Mutex::new(Some(audio_handle));
    let sounds = std::sync::Mutex::new(Some(sounds));
    let config = std::sync::Mutex::new(Some(config));
    let slots = std::sync::Mutex::new(Some(slots));

    // Renderer selection: HONKHONK_RENDERER (user-facing env var) is translated to
    // ICED_BACKEND before iced::application runs. ICED_BACKEND is read by
    // iced_renderer::fallback::Compositor::with_backend during init. Both wgpu and
    // tiny-skia are compiled in (see Cargo.toml). Accepted ICED_BACKEND values:
    // "wgpu", "tiny-skia" (comma-separated for ordered preference).
    iced::application(
        move || {
            let tray = tray_handle
                .lock()
                .expect("tray mutex poisoned")
                .take()
                .expect("boot called more than once");
            let audio = audio_handle
                .lock()
                .expect("audio mutex poisoned")
                .take()
                .expect("boot called more than once");
            let sounds = sounds
                .lock()
                .expect("sounds mutex poisoned")
                .take()
                .expect("boot called more than once");
            let config = config
                .lock()
                .expect("config mutex poisoned")
                .take()
                .expect("boot called more than once");
            let slots = slots
                .lock()
                .expect("slots mutex poisoned")
                .take()
                .expect("boot called more than once");
            honkhonk::app::HonkHonk::new(tray, audio, sounds, config, slots)
        },
        honkhonk::app::HonkHonk::update,
        honkhonk::app::HonkHonk::view,
    )
    .title("HonkHonk")
    .subscription(honkhonk::app::HonkHonk::subscription)
    .theme(honkhonk::app::HonkHonk::theme)
    .run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use honkhonk::state::Renderer;

    #[test]
    fn effective_renderer_software_env_overrides_wgpu_config() {
        assert_eq!(
            effective_renderer(Some("software"), Renderer::Wgpu),
            Renderer::TinySkia
        );
    }

    #[test]
    fn effective_renderer_tiny_skia_alias_works() {
        assert_eq!(
            effective_renderer(Some("tiny-skia"), Renderer::Wgpu),
            Renderer::TinySkia
        );
    }

    #[test]
    fn effective_renderer_wgpu_env_overrides_tiny_skia_config() {
        assert_eq!(
            effective_renderer(Some("wgpu"), Renderer::TinySkia),
            Renderer::Wgpu
        );
    }

    #[test]
    fn effective_renderer_no_env_uses_config() {
        assert_eq!(
            effective_renderer(None, Renderer::TinySkia),
            Renderer::TinySkia
        );
        assert_eq!(effective_renderer(None, Renderer::Wgpu), Renderer::Wgpu);
    }

    #[test]
    fn effective_renderer_unknown_env_falls_back_to_config() {
        assert_eq!(
            effective_renderer(Some("opengl"), Renderer::TinySkia),
            Renderer::TinySkia
        );
        assert_eq!(effective_renderer(Some(""), Renderer::Wgpu), Renderer::Wgpu);
    }
}
