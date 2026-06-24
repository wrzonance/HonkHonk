use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};
use std::sync::mpsc::{self, Receiver, Sender};

const TRAY_ID: &str = "honkhonk";
const TRAY_TITLE: &str = "HonkHonk";
const ICON_SIZE: i32 = 64;
const ICON_RGBA: [u8; 4] = [96, 96, 96, 255];

#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    ToggleVisibility,
    Quit,
}

pub struct TrayHandle {
    event_rx: Option<Receiver<TrayEvent>>,
    handle: Handle<HonkTray>,
}

impl TrayHandle {
    pub fn take_rx(&mut self) -> Receiver<TrayEvent> {
        self.event_rx.take().expect("take_rx called more than once")
    }
}

impl Drop for TrayHandle {
    fn drop(&mut self) {
        self.handle.shutdown().wait();
    }
}

pub fn build_tray() -> Result<TrayHandle, Box<dyn std::error::Error>> {
    let (event_tx, event_rx) = mpsc::channel::<TrayEvent>();
    let handle = HonkTray::new(event_tx)
        .disable_dbus_name(ashpd::is_sandboxed())
        .assume_sni_available(true)
        .spawn()?;

    Ok(TrayHandle {
        event_rx: Some(event_rx),
        handle,
    })
}

struct HonkTray {
    event_tx: Sender<TrayEvent>,
}

impl HonkTray {
    fn new(event_tx: Sender<TrayEvent>) -> Self {
        Self { event_tx }
    }

    fn emit(&self, event: TrayEvent) {
        let _ = self.event_tx.send(event);
    }
}

impl ksni::Tray for HonkTray {
    fn id(&self) -> String {
        TRAY_ID.into()
    }

    fn title(&self) -> String {
        TRAY_TITLE.into()
    }

    fn icon_name(&self) -> String {
        TRAY_ID.into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        solid_icon_pixmap(ICON_SIZE, ICON_RGBA)
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: TRAY_TITLE.into(),
            description: "Wayland-native soundboard".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.emit(TrayEvent::ToggleVisibility);
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.emit(TrayEvent::ToggleVisibility);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            menu_item("Show/Hide", TrayEvent::ToggleVisibility),
            MenuItem::Separator,
            menu_item("Quit", TrayEvent::Quit),
        ]
    }
}

fn menu_item(label: &str, event: TrayEvent) -> MenuItem<HonkTray> {
    StandardItem {
        label: label.into(),
        activate: Box::new(move |tray: &mut HonkTray| tray.emit(event.clone())),
        ..Default::default()
    }
    .into()
}

fn solid_icon_pixmap(size: i32, rgba: [u8; 4]) -> Vec<ksni::Icon> {
    let dimension = size.max(1) as usize;
    let pixel_count = dimension * dimension;
    let mut data = Vec::with_capacity(pixel_count * 4);
    for _ in 0..pixel_count {
        data.extend_from_slice(&[rgba[3], rgba[0], rgba[1], rgba[2]]);
    }

    vec![ksni::Icon {
        width: dimension as i32,
        height: dimension as i32,
        data,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::Tray;

    #[test]
    fn icon_pixmap_uses_sni_argb32_channel_order() {
        let icon = solid_icon_pixmap(1, [0x11, 0x22, 0x33, 0x44])
            .pop()
            .expect("icon pixmap should contain one size");

        assert_eq!(icon.width, 1);
        assert_eq!(icon.height, 1);
        assert_eq!(icon.data, vec![0x44, 0x11, 0x22, 0x33]);
    }

    #[test]
    fn tray_menu_actions_emit_app_events() {
        let (tx, rx) = mpsc::channel();
        let mut tray = HonkTray::new(tx);
        let menu = tray.menu();

        activate_menu_item(&menu, "Show/Hide", &mut tray);
        activate_menu_item(&menu, "Quit", &mut tray);

        assert_eq!(
            rx.recv().expect("show/hide event"),
            TrayEvent::ToggleVisibility
        );
        assert_eq!(rx.recv().expect("quit event"), TrayEvent::Quit);
    }

    fn activate_menu_item(menu: &[MenuItem<HonkTray>], label: &str, tray: &mut HonkTray) {
        let item = menu
            .iter()
            .find_map(|item| match item {
                MenuItem::Standard(item) if item.label == label => Some(item),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{label} menu item missing"));

        (item.activate)(tray);
    }
}
