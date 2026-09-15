//! Tiles-grid filter/sort behavior — split out of `tests.rs` (which owns
//! type-to-filter *routing* across views) to keep both files under the
//! repo's 400-line cap. These tests pin the main grid's own filter-matching
//! and cached-order-refresh invariants, independent of which view currently
//! owns keyboard focus.

use super::*;
use crate::app::FAVORITES_TAB;
use crate::state::{AudioFormat, SoundEntry};

fn sound(id: &str, name: &str, duration_ms: Option<u64>, category: &str) -> SoundEntry {
    SoundEntry {
        id: id.into(),
        name: name.into(),
        path: format!("/sounds/{category}/{id}.wav").into(),
        format: AudioFormat::Wav,
        duration_ms,
        category: category.into(),
        modified_ms: None,
    }
}

fn filtered_ids(app: &HonkHonk) -> Vec<&str> {
    app.filtered_sounds()
        .into_iter()
        .map(|sound| sound.id.as_str())
        .collect()
}

#[test]
fn shared_filter_matches_display_name_filename_and_category() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![SoundEntry {
        id: "goose".into(),
        name: "goose_honk".into(),
        path: "/sounds/Animals/goose_honk.WAV".into(),
        format: AudioFormat::Wav,
        duration_ms: None,
        category: "Animals".into(),
        modified_ms: None,
    }];
    app.sound_meta
        .set_display_name("goose", Some("Angry Bird".into()));

    for query in ["angry", ".wav", "animals", "goose_honk"] {
        let _ = app.update(Message::SearchChanged(query.into()));
        assert_eq!(app.filtered_sounds().len(), 1, "query: {query}");
    }
}

#[test]
fn main_grid_filter_results_follow_the_active_sort_state() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        sound("zulu", "Zulu", None, "Other"),
        sound("alpha", "alpha", None, "Other"),
    ];
    app.refresh_filtered_sounds();

    assert_eq!(app.filtered_sounds()[0].name, "alpha");

    let _ = app.update(Message::ToggleSoundSortDirection);

    assert_eq!(app.filtered_sounds()[0].name, "Zulu");
}

#[test]
fn filtered_sounds_reads_cached_order_without_resorting() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        sound("zulu", "Zulu", None, "Other"),
        sound("alpha", "alpha", None, "Other"),
    ];
    app.refresh_filtered_sounds();

    assert_eq!(filtered_ids(&app), vec!["alpha", "zulu"]);
    app.sound_sort.toggle_direction();

    assert_eq!(
        filtered_ids(&app),
        vec!["alpha", "zulu"],
        "reading filtered sounds must not recompute their order"
    );
}

#[test]
fn query_category_and_favorite_updates_refresh_cached_membership() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        sound("alpha", "Alpha", None, "Animals"),
        sound("beta", "Beta", None, "Memes"),
    ];
    app.refresh_filtered_sounds();

    let _ = app.update(Message::SearchChanged("beta".into()));
    assert_eq!(filtered_ids(&app), vec!["beta"]);

    let _ = app.update(Message::SearchChanged(String::new()));
    let _ = app.update(Message::SelectCategory(Some("Animals".into())));
    assert_eq!(filtered_ids(&app), vec!["alpha"]);

    let _ = app.update(Message::SelectCategory(None));
    let _ = app.update(Message::TypeToFilter("beta".into()));
    assert_eq!(filtered_ids(&app), vec!["beta"]);
    let _ = app.update(Message::EscapePressed);
    let _ = app.update(Message::EscapePressed);
    assert_eq!(filtered_ids(&app), vec!["alpha", "beta"]);

    let _ = app.update(Message::ToggleFavorite("beta".into()));
    let _ = app.update(Message::SelectCategory(Some(FAVORITES_TAB.into())));
    assert_eq!(filtered_ids(&app), vec!["beta"]);

    let _ = app.update(Message::ToggleFavorite("beta".into()));
    assert_eq!(filtered_ids(&app), vec!["alpha", "beta"]);
}

#[test]
fn duration_and_display_name_updates_refresh_cached_order() {
    let mut app = HonkHonk::new_for_test();
    app.sounds = vec![
        sound("alpha", "Alpha", Some(200), "Other"),
        sound("zulu", "Zulu", None, "Other"),
    ];
    app.refresh_filtered_sounds();

    let _ = app.update(Message::SelectSoundSort("length"));
    assert_eq!(filtered_ids(&app), vec!["alpha", "zulu"]);

    let durations = std::collections::HashMap::from([("zulu".to_owned(), 100)]);
    let _ = app.update(Message::DurationsLoaded(durations));
    assert_eq!(filtered_ids(&app), vec!["zulu", "alpha"]);

    let _ = app.update(Message::SelectSoundSort("name"));
    let _ = app.update(Message::OpenSoundEditor("zulu".into()));
    let _ = app.update(Message::SoundEditorNameChanged("Aardvark".into()));
    let _ = app.update(Message::SaveSoundMeta("zulu".into()));
    assert_eq!(filtered_ids(&app), vec!["zulu", "alpha"]);

    let _ = app.update(Message::SearchChanged("aardvark".into()));
    assert_eq!(filtered_ids(&app), vec!["zulu"]);
}
