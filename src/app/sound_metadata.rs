use super::{FAVORITES_TAB, HonkHonk, Message};
use crate::state::SoundMeta;
use iced::Task;

impl HonkHonk {
    pub(super) fn open_sound_editor(&mut self, sound_id: String) -> Task<Message> {
        let meta = self.sound_meta.get(&sound_id);
        let name_override = meta.display_name.clone().unwrap_or_default();
        let vol = meta.volume;
        // Clear the context menu so the editor overlay surfaces immediately.
        self.context_menu = None;
        self.context_menu_pos = None;
        self.editor_sound_id = Some(sound_id);
        self.editor_draft_name = name_override;
        self.editor_draft_volume = vol;
        Task::none()
    }

    pub(super) fn toggle_sound_favorite(&mut self, sound_id: &str) {
        let favorites_filter_active = self.active_category.as_deref() == Some(FAVORITES_TAB);
        let is_favorite = self.sound_meta.toggle_favorite(sound_id);
        self.persist_sound_metadata();

        // If the user just unstarred the last favorite while on the
        // Favorites tab, the chip disappears from the header. Reset to
        // "All" so the list doesn't show empty under an invisible filter.
        if !is_favorite
            && favorites_filter_active
            && !self
                .sounds
                .iter()
                .any(|sound| self.sound_meta.is_favorite(&sound.id))
        {
            self.active_category = None;
        }
        if favorites_filter_active {
            self.refresh_filtered_sounds();
        }
    }

    pub(super) fn save_sound_metadata(&mut self, sound_id: String) {
        let previous_meta = self.sound_meta.get(&sound_id);
        let display_name = self.editor_display_name();
        let display_name_changed = previous_meta.display_name != display_name;
        self.sound_meta.set(
            sound_id,
            SoundMeta {
                volume: self.editor_draft_volume,
                display_name,
                ..previous_meta
            },
        );
        self.persist_sound_metadata();
        if display_name_changed
            && (!self.filter.query().is_empty() || self.sound_sort.key().uses_display_name())
        {
            self.refresh_filtered_sounds();
        }
        self.editor_sound_id = None;
        self.editor_draft_name.clear();
        self.editor_draft_volume = 1.0;
    }

    fn editor_display_name(&self) -> Option<String> {
        let display_name = self.editor_draft_name.trim();
        (!display_name.is_empty()).then(|| display_name.to_owned())
    }

    fn persist_sound_metadata(&self) {
        if !self.persist {
            return;
        }
        if let Err(error) = self.sound_meta.save() {
            tracing::warn!(error = %error, "sound meta save error");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GraphicAssetRef;

    #[test]
    fn save_sound_metadata_preserves_assigned_graphic() {
        let mut app = HonkHonk::new_for_test();
        let graphic = GraphicAssetRef::new("airhorn.webp").unwrap();
        app.sound_meta.set_assigned_graphic("sound-id", graphic);
        app.editor_draft_name = "Renamed".to_owned();
        app.editor_draft_volume = 1.25;

        app.save_sound_metadata("sound-id".to_owned());

        assert_eq!(
            app.sound_meta
                .assigned_graphic("sound-id")
                .map(GraphicAssetRef::as_str),
            Some("airhorn.webp")
        );
    }
}
