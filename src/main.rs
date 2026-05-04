// Renderer note: Iced 0.13 selects the renderer at compile time via features.
// The "tiny-skia" feature is enabled for software rendering fallback.
// Iced uses wgpu by default when available, falling back to tiny-skia.
// There is no runtime renderer selection in Iced 0.13.

fn main() -> iced::Result {
    let tray_handle = honkhonk::tray::build_tray().expect("failed to initialize system tray");

    iced::application(
        "HonkHonk",
        honkhonk::app::HonkHonk::update,
        honkhonk::app::HonkHonk::view,
    )
    .subscription(honkhonk::app::HonkHonk::subscription)
    .theme(honkhonk::app::HonkHonk::theme)
    .run_with(move || {
        (
            honkhonk::app::HonkHonk::new(tray_handle.event_rx),
            iced::Task::none(),
        )
    })
}
