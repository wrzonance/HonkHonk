pub mod effects_panel;
pub mod effects_panel_view;
pub mod now_playing;
pub mod search_bar;
pub mod settings;
pub mod slot_manager;
pub mod sound_editor;
pub mod sound_grid;
pub mod theme;
pub mod volume;

pub fn fmt_duration(ms: Option<u64>) -> String {
    match ms {
        Some(ms) => format!("{}:{:02}", ms / 60_000, (ms % 60_000) / 1_000),
        None => "\u{2014}".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_duration_formats_seconds() {
        assert_eq!(fmt_duration(Some(3_500)), "0:03");
    }

    #[test]
    fn fmt_duration_formats_minutes() {
        assert_eq!(fmt_duration(Some(63_000)), "1:03");
    }

    #[test]
    fn fmt_duration_pads_seconds() {
        assert_eq!(fmt_duration(Some(60_000)), "1:00");
    }

    #[test]
    fn fmt_duration_none_returns_dash() {
        assert_eq!(fmt_duration(None), "\u{2014}");
    }
}
