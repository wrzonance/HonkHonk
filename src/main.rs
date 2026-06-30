use honkhonk::state::Renderer;

fn effective_renderer(env_val: Option<&str>, config_pref: Renderer) -> Renderer {
    match env_val {
        Some("software") | Some("tiny-skia") => Renderer::TinySkia,
        Some("wgpu") => Renderer::Wgpu,
        _ => config_pref,
    }
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "startup sequence keeps renderer env setup before subsystem initialization"
)]
fn main() -> iced::Result {
    honkhonk::logging::init();

    let config = match honkhonk::state::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "failed to load config; using defaults");
            honkhonk::state::AppConfig::default()
        }
    };

    // Set ICED_BACKEND before any subsystem init — pipewire::init() does not
    // document that it avoids spawning background threads, so set_var must run
    // before it to satisfy its SAFETY requirement.
    let renderer = effective_renderer(
        std::env::var("HONKHONK_RENDERER").ok().as_deref(),
        config.renderer,
    );
    let backend_value = match renderer {
        Renderer::TinySkia => "tiny-skia",
        Renderer::Wgpu => "wgpu",
    };
    // SAFETY: No threads exist yet — this is the very start of main(), before
    // any subsystem initialization. ICED_BACKEND is read by
    // iced_renderer::fallback::Compositor::with_backend during iced::application()
    // init, which runs after this point.
    #[allow(unused_unsafe)] // safe on edition 2021, required on edition 2024
    unsafe {
        std::env::set_var("ICED_BACKEND", backend_value);
    }

    pipewire::init();

    let sounds = match honkhonk::state::Library::scan(&config.sound_directories) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to scan sound library");
            Vec::new()
        }
    };

    let slots = honkhonk::state::SlotMap::load();

    let tray_handle = match honkhonk::tray::build_tray() {
        Ok(handle) => handle,
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize system tray; exiting");
            std::process::exit(1);
        }
    };

    let audio_handle = match honkhonk::audio::spawn(
        config.mic_passthrough,
        config.monitor_device.clone(),
        config.input_device.clone(),
    ) {
        Ok(handle) => handle,
        Err(e) => {
            tracing::error!(error = %e, "failed to start audio engine; exiting");
            std::process::exit(1);
        }
    };

    // Restore the saved window size, and disable iced's default auto-close so a
    // window-manager close routes through Message::Quit (audio shutdown + config
    // save) — see the window-event subscription in app::update.
    let window_settings = iced::window::Settings {
        size: iced::Size::new(config.window_width as f32, config.window_height as f32),
        exit_on_close_request: false,
        ..iced::window::Settings::default()
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
    .window(window_settings)
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
