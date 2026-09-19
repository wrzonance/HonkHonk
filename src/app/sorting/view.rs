use iced::Element;

use super::SoundSortKey;
use crate::app::{HonkHonk, Message};
use crate::ui::list_controls::sort;
use crate::ui::theme;

impl HonkHonk {
    pub(in crate::app) fn view_sound_sort_overlay(
        &self,
        theme: theme::Theme,
    ) -> Option<Element<'_, Message>> {
        let anchor = self.sort_menu_anchor?;
        Some(sort::view_sort_menu_with_grouping(
            sort::SortMenu {
                state: self.sound_sort,
                options: &SoundSortKey::ALL,
                theme,
                anchor,
                window_size: self.window_size,
            },
            |key| Message::SelectSoundSort(key.id()),
            Message::DismissSoundSortMenu,
            Some((self.sound_tags_grouped(), Message::ToggleSoundTagGrouping)),
        ))
    }
}
