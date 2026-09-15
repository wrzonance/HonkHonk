//! Sort key shared by the hotkey bindings list (#199) and, later, the slot
//! manager list (#198). Kept as a standalone top-level module — rather than
//! nested inside `hotkeys.rs` — specifically so #198 can reuse it without
//! creating a dependency on the hotkeys module tree.

use crate::ui::list_controls::sort::SortLabel;

/// Sort key for slot-oriented lists (hotkey bindings today, slot manager
/// next). Mirrors `sorting::SoundSortKey`'s id/label/ordering shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SlotSortKey {
    #[default]
    SlotNumber,
    Name,
    Length,
    Tag,
    Modified,
    Added,
}

impl SlotSortKey {
    pub(super) const ALL: [Self; 6] = [
        Self::SlotNumber,
        Self::Name,
        Self::Length,
        Self::Tag,
        Self::Modified,
        Self::Added,
    ];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::SlotNumber => "slot",
            Self::Name => "name",
            Self::Length => "length",
            Self::Tag => "tag",
            Self::Modified => "modified",
            Self::Added => "added",
        }
    }

    /// Kept private (not `pub(super)`) on purpose: each consumer module
    /// (`hotkeys.rs`, `slots.rs`) does its own `ALL.into_iter().find` scan
    /// instead of calling this directly, so outside of its own round-trip
    /// tests this associated function is unreachable from production code.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "round-trip-tested here; production callers do their own ALL-scan"
        )
    )]
    fn from_id(id: &str) -> Option<Self> {
        match id {
            "slot" => Some(Self::SlotNumber),
            "name" => Some(Self::Name),
            "length" => Some(Self::Length),
            "tag" => Some(Self::Tag),
            "modified" => Some(Self::Modified),
            "added" => Some(Self::Added),
            _ => None,
        }
    }
}

impl SortLabel for SlotSortKey {
    fn label(self) -> &'static str {
        match self {
            Self::SlotNumber => "Slot",
            Self::Name => "Name",
            Self::Length => "Length",
            Self::Tag => "Tag",
            Self::Modified => "Modified",
            Self::Added => "Added",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_round_trips_through_its_id() {
        for key in SlotSortKey::ALL {
            assert_eq!(SlotSortKey::from_id(key.id()), Some(key), "key: {key:?}");
        }
    }

    #[test]
    fn unknown_id_resolves_to_none() {
        assert_eq!(SlotSortKey::from_id("bogus"), None);
        assert_eq!(SlotSortKey::from_id(""), None);
    }

    #[test]
    fn default_key_is_slot_number() {
        assert_eq!(SlotSortKey::default(), SlotSortKey::SlotNumber);
        assert_eq!(SlotSortKey::default().id(), "slot");
    }

    #[test]
    fn all_contains_every_variant_exactly_once() {
        let mut ids: Vec<&str> = SlotSortKey::ALL.iter().map(|key| key.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SlotSortKey::ALL.len());
    }

    #[test]
    fn every_key_has_a_non_empty_label() {
        for key in SlotSortKey::ALL {
            assert!(!key.label().is_empty(), "key: {key:?}");
        }
    }
}
