use super::*;
use crate::state::SoundMeta;

#[test]
fn tags_normalize_round_trip_and_prune_without_losing_graphics() {
    let mut store = SoundMetaStore::default();
    let meta: SoundMeta =
        serde_json::from_str(r#"{"tags":["  Air   horn ","air horn"," ","Meme"]}"#).unwrap();
    assert_eq!(meta.tags, ["Air horn", "Meme"]);
    store.set("sound".into(), meta);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("meta.json");
    store.save_to(&path).unwrap();
    let mut loaded = SoundMetaStore::load_from(&path);
    assert_eq!(loaded.get("sound").tags, ["Air horn", "Meme"]);
    loaded.set_tags("sound", vec!["  ".into()]);
    assert!(loaded.get_ref("sound").is_none());
    loaded.set_assigned_graphic(
        "sound",
        crate::state::GraphicAssetRef::new("x.png").unwrap(),
    );
    loaded.set_tags("sound", vec!["Meme".into(), "meme".into()]);
    loaded.set_tags("sound", vec![]);
    assert!(loaded.assigned_graphic("sound").is_some());
    assert!(
        serde_json::from_str::<SoundMeta>("{}")
            .unwrap()
            .tags
            .is_empty()
    );
}

fn tagged_app() -> HonkHonk {
    let mut app = HonkHonk::new_for_test();
    app.sounds = ["Zulu", "Alpha", "Other"]
        .into_iter()
        .map(|name| SoundEntry {
            id: name.into(),
            name: name.into(),
            path: format!("/sounds/{name}.wav").into(),
            category: "Folder".into(),
            format: crate::state::AudioFormat::Wav,
            duration_ms: None,
            modified_ms: None,
        })
        .collect();
    app.sound_meta
        .set_tags("Zulu", vec!["Meme".into(), "Airhorn".into()]);
    app.sound_meta.set_tags("Alpha", vec!["Meme".into()]);
    app.refresh_filtered_sounds();
    app
}

#[test]
fn tag_editor_cancel_and_save_refresh_filter() {
    let mut app = tagged_app();
    let _ = app.update(Message::OpenSoundEditor("Zulu".into()));
    assert_eq!(app.editor_draft_tags, "Meme, Airhorn");
    let _ = app.update(Message::SoundEditorTagsChanged("Bird".into()));
    let _ = app.update(Message::CloseSoundEditor);
    assert_eq!(app.sound_meta.get("Zulu").tags, ["Meme", "Airhorn"]);
    app.replace_filter_query("bird".into());
    assert!(app.filtered_sounds().is_empty());
    let _ = app.update(Message::OpenSoundEditor("Zulu".into()));
    let _ = app.update(Message::SoundEditorTagsChanged(" Bird , bird,  ".into()));
    let _ = app.update(Message::SaveSoundMeta("Zulu".into()));
    assert_eq!(app.filtered_sounds()[0].id, "Zulu");
    assert_eq!(app.sound_meta.get("Zulu").tags, ["Bird"]);
}

#[test]
fn grouping_preserves_sort_and_filter_and_persists_independently() {
    let mut app = tagged_app();
    let _ = app.update(Message::ToggleSoundTagGrouping);
    let groups = app.sound_tag_groups();
    assert_eq!(
        groups,
        vec![
            (Some("Airhorn".into()), vec![0]),
            (Some("Meme".into()), vec![1, 0]),
            (None, vec![2])
        ]
    );
    let _ = app.update(Message::ToggleSoundSortDirection);
    assert_eq!(app.sound_tag_groups()[1].1, vec![0, 1]);
    app.replace_filter_query("AIRHORN".into());
    assert_eq!(app.sound_tag_groups().len(), 2);
    let json = serde_json::to_string(&app.config).unwrap();
    let restored: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(restored.sort_prefs["tiles"].group_by_tag());
    assert_eq!(restored.sort_prefs["tiles"].direction(), "descending");
    assert!(!json.contains("AIRHORN"));
}

#[test]
fn filter_does_not_match_across_distinct_tags() {
    let mut app = tagged_app();
    app.replace_filter_query("meme airhorn".into());
    assert!(app.filtered_sounds().is_empty());
}

#[test]
fn grouping_merges_case_variants_and_distinguishes_untagged() {
    let mut app = tagged_app();
    app.sound_meta
        .set_tags("Zulu", vec!["meme".into(), "Untagged".into()]);
    assert_eq!(
        app.sound_tag_groups(),
        vec![
            (Some("Meme".into()), vec![1, 0]),
            (Some("Untagged".into()), vec![0]),
            (None, vec![2]),
        ]
    );
    let _ = app.update(Message::ToggleSoundSortDirection);
    assert_eq!(app.sound_tag_groups()[0].0.as_deref(), Some("Meme"));
}

#[test]
fn clearing_tags_in_editor_prunes_and_escape_discards() {
    let mut app = tagged_app();
    let _ = app.update(Message::OpenSoundEditor("Zulu".into()));
    let _ = app.update(Message::SoundEditorTagsChanged(String::new()));
    let _ = app.update(Message::EscapePressed);
    assert_eq!(app.sound_meta.get("Zulu").tags.len(), 2);
    let _ = app.update(Message::OpenSoundEditor("Zulu".into()));
    let _ = app.update(Message::SoundEditorTagsChanged(String::new()));
    let _ = app.update(Message::SaveSoundMeta("Zulu".into()));
    assert!(app.sound_meta.get_ref("Zulu").is_none());
    assert_eq!(app.sound_tag_groups().last().unwrap().1, vec![2, 0]);
}

#[test]
fn slots_and_shortcuts_search_tags_without_remapping_assignments() {
    let mut app = tagged_app();
    app.slots.set(7, app.sounds[0].path.clone());
    app.slot_triggers[7] = Some("Meta+8".into());
    for query in ["AIRHORN", "Folder", "Zulu"] {
        app.slot_filter.replace(query.into());
        app.hotkey_filter.replace(query.into());
        assert_eq!(app.slot_render_order(), vec![7]);
        assert_eq!(app.hotkey_rows()[0].slot_index, 7);
        assert_eq!(app.hotkey_rows().len(), 1);
    }
    app.slot_filter.replace("meme airhorn".into());
    app.hotkey_filter.replace("meme airhorn".into());
    assert!(app.slot_render_order().is_empty());
    assert!(app.hotkey_rows().is_empty());
    assert_eq!(app.slots.slot_for(&app.sounds[0].path), Some(7));
}
