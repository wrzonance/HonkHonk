// Renderer note: Iced 0.13 selects the renderer at compile time via features.
// The "tiny-skia" feature is enabled for software rendering fallback.
// Iced uses wgpu by default when available, falling back to tiny-skia.
// There is no runtime renderer selection in Iced 0.13.

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

    iced::application(
        "HonkHonk",
        honkhonk::app::HonkHonk::update,
        honkhonk::app::HonkHonk::view,
    )
    .subscription(honkhonk::app::HonkHonk::subscription)
    .theme(honkhonk::app::HonkHonk::theme)
    .run_with(move || {
        (
            honkhonk::app::HonkHonk::new(tray_handle),
            iced::Task::none(),
        )
    })
}
