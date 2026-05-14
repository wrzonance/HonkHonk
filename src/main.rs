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

    // Renderer selection: set ICED_BACKEND before iced::application so the
    // Iced compositor picks up the preference. Both wgpu and tiny-skia are
    // compiled in (see Cargo.toml features). ICED_BACKEND is read by
    // iced_renderer::fallback::Compositor::with_backend during application
    // init. Accepted values: "wgpu", "tiny-skia" (comma-separated for
    // ordered preference list). wgpu is tried first when both are compiled.
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
