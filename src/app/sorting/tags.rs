use std::collections::BTreeMap;

use super::{HonkHonk, TILES_VIEW_KEY};
use crate::state::SortPref;

impl HonkHonk {
    pub(in crate::app) fn sound_tags_grouped(&self) -> bool {
        self.config
            .sort_prefs
            .get(TILES_VIEW_KEY)
            .is_some_and(SortPref::group_by_tag)
    }

    pub(in crate::app) fn toggle_sound_tag_grouping(&mut self) {
        let enabled = !self.sound_tags_grouped();
        let pref = SortPref::new(self.sound_sort.key().id(), self.sound_sort.direction().id())
            .with_tag_grouping(enabled);
        self.config.sort_prefs.insert(TILES_VIEW_KEY.into(), pref);
        self.sort_menu_anchor = None;
        self.persist_config();
    }

    /// Group the filtered, sorted sequence without changing sound identities.
    /// Case variants share a group; the smallest spelling is its stable label.
    pub(in crate::app) fn sound_tag_groups(&self) -> Vec<(Option<String>, Vec<usize>)> {
        let mut groups: BTreeMap<String, (String, Vec<usize>)> = BTreeMap::new();
        let mut untagged = Vec::new();
        for &index in &self.filtered_sound_indices {
            let Some(sound) = self.sounds.get(index) else {
                continue;
            };
            let meta = self.sound_meta.get(&sound.id);
            if meta.tags.is_empty() {
                untagged.push(index);
            }
            for tag in meta.tags {
                let group = groups
                    .entry(tag.to_lowercase())
                    .or_insert_with(|| (tag.clone(), Vec::new()));
                if tag < group.0 {
                    group.0 = tag;
                }
                group.1.push(index);
            }
        }
        let mut sections: Vec<_> = groups
            .into_values()
            .map(|(label, indices)| (Some(label), indices))
            .collect();
        if !untagged.is_empty() {
            sections.push((None, untagged));
        }
        sections
    }
}
