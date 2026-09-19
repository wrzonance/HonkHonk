use std::borrow::Cow;

use super::HonkHonk;
use crate::app::{FAVORITES_TAB, sorting};
use crate::state::SoundEntry;
use crate::ui::list_controls::filter::filter_indices;

impl HonkHonk {
    pub(in crate::app) fn refresh_filtered_sounds(&mut self) {
        let indices = filter_indices(&self.sounds, self.filter.query(), |sound| {
            let display_name = self
                .sound_meta
                .get_ref(&sound.id)
                .and_then(|meta| meta.display_name.as_deref())
                .unwrap_or("");
            let filename = sound
                .path
                .file_name()
                .map(std::ffi::OsStr::to_string_lossy)
                .unwrap_or_default();
            [
                Cow::Borrowed(display_name),
                filename,
                Cow::Borrowed(sound.name.as_str()),
                Cow::Borrowed(sound.category.as_str()),
            ]
            .into_iter()
            .chain(
                self.sound_meta
                    .get_ref(&sound.id)
                    .into_iter()
                    .flat_map(|meta| meta.tags.iter().map(|tag| Cow::Borrowed(tag.as_str()))),
            )
        })
        .into_iter()
        .filter(|&index| self.sound_matches_active_category(index))
        .collect();
        self.filtered_sound_indices =
            sorting::sorted_sound_indices(&self.sounds, indices, self.sound_sort, &self.sound_meta);
    }

    fn sound_matches_active_category(&self, index: usize) -> bool {
        let Some(sound) = self.sounds.get(index) else {
            return false;
        };
        match self.active_category.as_deref() {
            Some(FAVORITES_TAB) => self.sound_meta.is_favorite(&sound.id),
            Some(category) => sound.category == category,
            None => true,
        }
    }

    /// Returns cached sounds matching the shared query and category filters.
    pub fn filtered_sounds(&self) -> Vec<&SoundEntry> {
        self.filtered_sound_indices
            .iter()
            .filter_map(|&index| self.sounds.get(index))
            .collect()
    }
}
