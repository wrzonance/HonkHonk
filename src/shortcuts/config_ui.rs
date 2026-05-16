use std::process::Command;

use crate::shortcuts::PortalCommand;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DesktopEnv {
    Kde,
    Gnome,
    Hyprland,
    Sway,
    #[allow(dead_code)]
    Windows,
    #[allow(dead_code)]
    MacOs,
    Unknown(String),
}

pub(crate) fn parse_desktop_env(raw: &str) -> DesktopEnv {
    let first = raw.split(':').next().unwrap_or("").trim();
    match first.to_lowercase().as_str() {
        "kde" => DesktopEnv::Kde,
        "gnome" => DesktopEnv::Gnome,
        "hyprland" => DesktopEnv::Hyprland,
        "sway" => DesktopEnv::Sway,
        _ => DesktopEnv::Unknown(raw.to_owned()),
    }
}

pub(crate) struct ShortcutConfigService {
    desktop_env: DesktopEnv,
    portal_v2_available: bool,
    portal_cmd_tx: Option<tokio::sync::mpsc::Sender<PortalCommand>>,
}

impl ShortcutConfigService {
    pub(crate) fn new() -> Self {
        #[cfg(target_os = "windows")]
        let desktop_env = DesktopEnv::Windows;

        #[cfg(target_os = "macos")]
        let desktop_env = DesktopEnv::MacOs;

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let desktop_env = std::env::var("XDG_CURRENT_DESKTOP")
            .map(|v| parse_desktop_env(&v))
            .unwrap_or_else(|_| DesktopEnv::Unknown(String::new()));

        Self {
            desktop_env,
            portal_v2_available: false,
            portal_cmd_tx: None,
        }
    }

    pub(crate) fn set_portal_sender(&mut self, tx: tokio::sync::mpsc::Sender<PortalCommand>) {
        self.portal_cmd_tx = Some(tx);
    }

    pub(crate) fn set_portal_v2_available(&mut self, available: bool) {
        self.portal_v2_available = available;
    }

    pub(crate) fn can_open(&self) -> bool {
        self.portal_v2_available || matches!(self.desktop_env, DesktopEnv::Kde | DesktopEnv::Gnome)
    }

    pub(crate) fn open(&self) {
        if self.portal_v2_available {
            if let Some(tx) = &self.portal_cmd_tx {
                if let Err(e) = tx.try_send(PortalCommand::ConfigureShortcuts) {
                    eprintln!("honkhonk: configure_shortcuts command dropped: {e}");
                }
                return;
            }
        }
        match &self.desktop_env {
            DesktopEnv::Kde => {
                if let Err(e) = Command::new("kcmshell6").arg("kcm_keys").spawn() {
                    eprintln!("honkhonk: failed to open KDE shortcuts: {e}");
                }
            }
            DesktopEnv::Gnome => {
                if let Err(e) = Command::new("gnome-control-center").arg("keyboard").spawn() {
                    eprintln!("honkhonk: failed to open GNOME keyboard settings: {e}");
                }
            }
            DesktopEnv::Hyprland => {
                eprintln!(
                    "honkhonk: configure_shortcuts requires portal v2 on Hyprland (not available)"
                );
            }
            DesktopEnv::Sway => {
                eprintln!(
                    "honkhonk: configure_shortcuts requires portal v2 on Sway (not available)"
                );
            }
            DesktopEnv::Windows | DesktopEnv::MacOs => {}
            DesktopEnv::Unknown(de) => {
                eprintln!(
                    "honkhonk: no shortcut config path for DE '{de}'; \
                     install xdg-desktop-portal v2"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kde() {
        assert_eq!(parse_desktop_env("KDE"), DesktopEnv::Kde);
    }

    #[test]
    fn parse_kde_lowercase() {
        assert_eq!(parse_desktop_env("kde"), DesktopEnv::Kde);
    }

    #[test]
    fn parse_gnome_colon_variant() {
        assert_eq!(parse_desktop_env("GNOME:GNOME"), DesktopEnv::Gnome);
    }

    #[test]
    fn parse_hyprland() {
        assert_eq!(parse_desktop_env("Hyprland"), DesktopEnv::Hyprland);
    }

    #[test]
    fn parse_sway() {
        assert_eq!(parse_desktop_env("sway"), DesktopEnv::Sway);
    }

    #[test]
    fn parse_unknown_preserves_value() {
        assert_eq!(
            parse_desktop_env("coolde"),
            DesktopEnv::Unknown("coolde".into())
        );
    }

    #[test]
    fn parse_empty_is_unknown() {
        assert_eq!(parse_desktop_env(""), DesktopEnv::Unknown("".into()));
    }

    #[test]
    fn can_open_kde_no_portal() {
        let svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Kde,
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        assert!(svc.can_open());
    }

    #[test]
    fn can_open_gnome_no_portal() {
        let svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Gnome,
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        assert!(svc.can_open());
    }

    #[test]
    fn can_open_hyprland_no_portal() {
        let svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Hyprland,
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        assert!(!svc.can_open());
    }

    #[test]
    fn can_open_hyprland_with_portal_v2() {
        let svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Hyprland,
            portal_v2_available: true,
            portal_cmd_tx: None,
        };
        assert!(svc.can_open());
    }

    #[test]
    fn set_portal_v2_available_updates_flag() {
        let mut svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Unknown(String::new()),
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        svc.set_portal_v2_available(true);
        assert!(svc.portal_v2_available);
    }

    #[test]
    fn open_sends_via_portal_when_v2_available() {
        use tokio::sync::mpsc;
        let (tx, mut rx) = mpsc::channel(8);
        let mut svc = ShortcutConfigService {
            desktop_env: DesktopEnv::Unknown(String::new()),
            portal_v2_available: false,
            portal_cmd_tx: None,
        };
        svc.set_portal_sender(tx);
        svc.set_portal_v2_available(true);
        svc.open();
        assert!(rx.try_recv().is_ok());
    }
}
