use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use std::sync::mpsc::{self, Receiver};
use tray_icon::{TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    ToggleVisibility,
    Quit,
}

pub struct TrayHandle {
    pub event_rx: Receiver<TrayEvent>,
    _icon: TrayIcon,
    _show_hide_id: muda::MenuId,
    _quit_id: muda::MenuId,
}

pub fn build_tray() -> Result<TrayHandle, Box<dyn std::error::Error>> {
    let (event_tx, event_rx) = mpsc::channel::<TrayEvent>();

    let menu = Menu::new();
    let show_hide = MenuItem::new("Show/Hide", true, None);
    let quit = MenuItem::new("Quit", true, None);

    menu.append(&show_hide)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    let show_hide_id = show_hide.id().clone();
    let quit_id = quit.id().clone();

    let icon = load_icon();
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("HonkHonk")
        .with_icon(icon)
        .build()?;

    let sh_id = show_hide_id.clone();
    let q_id = quit_id.clone();

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id() == &sh_id {
            let _ = event_tx.send(TrayEvent::ToggleVisibility);
        } else if event.id() == &q_id {
            let _ = event_tx.send(TrayEvent::Quit);
        }
    }));

    Ok(TrayHandle {
        event_rx,
        _icon: tray,
        _show_hide_id: show_hide_id,
        _quit_id: quit_id,
    })
}

fn load_icon() -> tray_icon::Icon {
    let rgba = vec![96u8; 64 * 64 * 4];
    tray_icon::Icon::from_rgba(rgba, 64, 64).expect("valid icon dimensions")
}
