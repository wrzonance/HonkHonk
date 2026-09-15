use std::cmp::Ordering;

use super::HonkHonk;
use crate::state::{AppConfig, SortPref, SoundEntry, SoundMetaStore};
use crate::ui::list_controls::sort::{Direction, SortKey, SortLabel, SortState};

mod tags;
#[cfg(test)]
mod tests;
mod view;

const TILES_VIEW_KEY: &str = "tiles";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SoundSortKey {
    #[default]
    Name,
    Length,
    Folder,
    Modified,
    Added,
}

impl SoundSortKey {
    pub(super) const ALL: [Self; 5] = [
        Self::Name,
        Self::Length,
        Self::Folder,
        Self::Modified,
        Self::Added,
    ];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Length => "length",
            Self::Folder => "folder",
            Self::Modified => "modified",
            Self::Added => "added",
        }
    }

    pub(super) const fn uses_duration(self) -> bool {
        matches!(self, Self::Length)
    }

    pub(super) const fn uses_display_name(self) -> bool {
        matches!(self, Self::Name)
    }

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "name" => Some(Self::Name),
            "length" => Some(Self::Length),
            "folder" => Some(Self::Folder),
            "modified" => Some(Self::Modified),
            "added" => Some(Self::Added),
            _ => None,
        }
    }
}

pub(super) type SoundSortState = SortState<SoundSortKey>;

fn default_sound_sort() -> SoundSortState {
    SoundSortState::new(SoundSortKey::Name, Direction::Ascending)
}

pub(super) fn sound_sort_from_config(config: &AppConfig) -> SoundSortState {
    let Some(pref) = config.sort_prefs.get(TILES_VIEW_KEY) else {
        return default_sound_sort();
    };
    let Some(key) = SoundSortKey::from_id(pref.key()) else {
        return default_sound_sort();
    };
    let direction = match pref.direction() {
        "ascending" => Direction::Ascending,
        "descending" => Direction::Descending,
        _ => return default_sound_sort(),
    };
    SoundSortState::new(key, direction)
}

pub(super) fn sorted_sound_indices(
    sounds: &[SoundEntry],
    indices: Vec<usize>,
    state: SoundSortState,
    metadata: &SoundMetaStore,
) -> Vec<usize> {
    let sortable = indices
        .into_iter()
        .filter_map(|index| {
            sounds
                .get(index)
                .map(|sound| SoundSortItem::new(index, sound, state.key(), metadata))
        })
        .collect::<Vec<_>>();
    state
        .sorted(sortable)
        .into_iter()
        .map(|item| item.index)
        .collect()
}

impl HonkHonk {
    pub(super) fn toggle_sound_sort_menu(&mut self) {
        self.sort_menu_anchor = if self.sort_menu_anchor.is_some() {
            None
        } else {
            Some(self.cursor_pos)
        };
    }

    pub(super) fn toggle_sound_sort_direction(&mut self) {
        self.sound_sort.toggle_direction();
        self.refresh_filtered_sounds();
        self.persist_sound_sort();
    }

    pub(super) fn select_sound_sort(&mut self, key_id: &str) {
        let Some(key) = SoundSortKey::from_id(key_id) else {
            self.sort_menu_anchor = None;
            return;
        };
        let changed = self.sound_sort.key() != key;
        self.sound_sort.select(key);
        self.sort_menu_anchor = None;
        if changed {
            self.refresh_filtered_sounds();
        }
        self.persist_sound_sort();
    }

    pub(super) fn dismiss_sound_sort_menu(&mut self) -> bool {
        self.sort_menu_anchor.take().is_some()
    }

    fn persist_sound_sort(&mut self) {
        let grouped = self.sound_tags_grouped();
        self.config.sort_prefs.insert(
            TILES_VIEW_KEY.into(),
            SortPref::new(self.sound_sort.key().id(), self.sound_sort.direction().id())
                .with_tag_grouping(grouped),
        );
        self.persist_config();
    }
}

struct SoundSortItem<'a> {
    index: usize,
    sound: &'a SoundEntry,
    value: SoundSortValue,
}

enum SoundSortValue {
    Text(String),
    Milliseconds(Option<u64>),
}

impl<'a> SoundSortItem<'a> {
    fn new(
        index: usize,
        sound: &'a SoundEntry,
        key: SoundSortKey,
        metadata: &SoundMetaStore,
    ) -> Self {
        let value = match key {
            SoundSortKey::Name => SoundSortValue::Text(
                metadata
                    .get_ref(&sound.id)
                    .and_then(|meta| meta.display_name.as_deref())
                    .unwrap_or(&sound.name)
                    .to_lowercase(),
            ),
            SoundSortKey::Length => SoundSortValue::Milliseconds(sound.duration_ms),
            SoundSortKey::Folder => SoundSortValue::Text(sound.category.to_lowercase()),
            SoundSortKey::Modified => SoundSortValue::Milliseconds(sound.modified_ms),
            SoundSortKey::Added => SoundSortValue::Milliseconds(metadata.added_ms(&sound.id)),
        };
        Self {
            index,
            sound,
            value,
        }
    }

    fn compare_value(&self, other: &Self) -> Ordering {
        match (&self.value, &other.value) {
            (SoundSortValue::Text(left), SoundSortValue::Text(right)) => left.cmp(right),
            (SoundSortValue::Milliseconds(left), SoundSortValue::Milliseconds(right)) => {
                left.cmp(right)
            }
            _ => Ordering::Equal,
        }
    }

    fn value_unknown(&self) -> bool {
        matches!(self.value, SoundSortValue::Milliseconds(None))
    }

    fn tie_break(&self, other: &Self) -> Ordering {
        self.sound
            .path
            .cmp(&other.sound.path)
            .then_with(|| self.sound.id.cmp(&other.sound.id))
    }
}

impl SortLabel for SoundSortKey {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Length => "Length",
            Self::Folder => "Folder",
            Self::Modified => "Modified",
            Self::Added => "Added",
        }
    }
}

impl SortKey<SoundSortItem<'_>> for SoundSortKey {
    fn compare(self, left: &SoundSortItem<'_>, right: &SoundSortItem<'_>) -> Ordering {
        left.compare_value(right)
            .then_with(|| left.tie_break(right))
    }

    fn value_unknown(self, item: &SoundSortItem<'_>) -> bool {
        item.value_unknown()
    }
}
