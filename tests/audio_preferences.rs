use honkhonk::audio::processing::{OutputMode, SoundProcessing};
use honkhonk::state::{SoundMeta, SoundMetaStore};

#[test]
fn fingerprint_preferences_survive_rename_and_restart_without_copying_tags() {
    let mut store = SoundMetaStore::default();
    store.bind_fingerprint("old", "abc");
    store.set(
        "old".into(),
        SoundMeta {
            volume: 1.5,
            tags: vec!["Original".into()],
            display_name: Some("Old name".into()),
            processing: SoundProcessing {
                pan: 0.6,
                output: OutputMode::Stereo,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("meta.json");
    store.save_to(&path).unwrap();
    let mut loaded = SoundMetaStore::load_from(&path);
    loaded.set_tags("new", vec!["New".into()]);
    loaded.bind_fingerprint("new", "abc");
    let moved = loaded.get("new");
    assert_eq!(moved.volume, 1.5);
    assert_eq!(moved.processing.pan, 0.6);
    assert_eq!(moved.tags, ["New"]);
    assert_eq!(moved.display_name, None);
    loaded.set_volume("new", 0.5);
    assert_eq!(loaded.volume_for("old"), 0.5);
    assert_eq!(loaded.get("old").tags, ["Original"]);
    loaded.bind_fingerprint("different", "def");
    assert_eq!(loaded.volume_for("different"), 1.0);
}

#[test]
fn force_mono_stereo_and_pan_have_defined_channel_behavior() {
    use honkhonk::audio::processing::{ChannelLayout, pan};
    let source = [0.6, 0.2, -0.2, 0.6];
    let settings = SoundProcessing {
        output: OutputMode::Mono,
        pan: 1.0,
        ..Default::default()
    };
    let layout = ChannelLayout::new(2, settings);
    let mut panned = [0.0; 4];
    assert_eq!(layout.fill(&source, &mut panned, 1.0), (4, 4));
    pan(&mut panned, layout.output_channels(), settings.pan);
    assert_eq!(panned[0], 0.0);
    assert!((panned[1] - 0.4).abs() < 0.0001);
    assert_eq!(source, [0.6, 0.2, -0.2, 0.6]);
}

#[test]
fn replaced_content_resets_only_audio_and_restores_known_content_preferences() {
    let mut store = SoundMetaStore::default();
    let legacy = SoundMeta {
        volume: 1.5,
        tags: vec!["Keep".into()],
        display_name: Some("Path name".into()),
        color: Some(3),
        favorite: true,
        assigned_graphic: Some(honkhonk::state::GraphicAssetRef::new("tile.png").unwrap()),
        processing: SoundProcessing {
            pan: 0.7,
            ..Default::default()
        },
    };
    store.set("path".into(), legacy.clone());
    store.bind_fingerprint("path", "original");
    assert_eq!(
        store.get("path"),
        legacy,
        "first binding migrates legacy audio"
    );
    store.bind_fingerprint("path", "replacement");
    let replaced = store.get("path");
    assert_eq!(replaced.volume, SoundMeta::default().volume);
    assert_eq!(replaced.processing, SoundProcessing::default());
    assert_eq!(replaced.tags, legacy.tags);
    assert_eq!(replaced.display_name, legacy.display_name);
    assert_eq!(replaced.color, legacy.color);
    assert_eq!(replaced.favorite, legacy.favorite);
    assert_eq!(replaced.assigned_graphic, legacy.assigned_graphic);
    store.bind_fingerprint("path", "original");
    assert_eq!(
        store.get("path"),
        legacy,
        "known content restores its audio"
    );
}
