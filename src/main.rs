fn main() -> iced::Result {
    if let Err(e) = gtk::init() {
        eprintln!("fatal: failed to initialize GTK (required for system tray): {e}");
        std::process::exit(1);
    }

    let tray_handle = match honkhonk::tray::build_tray() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("fatal: failed to initialize system tray: {e}");
            std::process::exit(1);
        }
    };

    let tray_handle = std::sync::Mutex::new(Some(tray_handle));

    iced::application(
        move || {
            let handle = tray_handle
                .lock()
                .expect("tray mutex poisoned")
                .take()
                .expect("boot called more than once");
            honkhonk::app::HonkHonk::new(handle)
        },
        honkhonk::app::HonkHonk::update,
        honkhonk::app::HonkHonk::view,
    )
    .title("HonkHonk")
    .subscription(honkhonk::app::HonkHonk::subscription)
    .theme(honkhonk::app::HonkHonk::theme)
    .run()
}
