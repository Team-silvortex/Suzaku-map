//! Source-layer limits and bounded runtime aggregates are different contracts.
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use suzaku_map::lexicon::{
    self, Lexicon,
    packs::{self, Catalog, InstalledPack, OfflinePack, PackStore},
};

fn layers(count: usize) -> String {
    (0..count)
        .map(|n| {
            format!(
                r#"{{"id":"layer-{n}","words":["layerword"],"next_words":[["layer {n} choice",["alpha","beta"]]],"sentences":["layer words stay ordered."]}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn pack_raw(index: usize, layers: &str) -> String {
    format!(
        r#"{{"format_version":1,"manifest":{{"id":"org.suzaku.test.layer-{index}","version":"1.0.0","name":"Layer budget fixture","description":"Synthetic bounded test data only.","language":"en","topics":["test"],"license":"MIT","authors":["Suzaku tests"]}},"lexicon":{{"format_version":1,"language":"en","word_layers":[{layers}]}}}}"#
    )
}

fn pack(index: usize, count: usize) -> OfflinePack {
    // Context keys must be unique across packs, without affecting layer order.
    let layers = layers(count).replace("layer ", &format!("pack {index} layer "));
    OfflinePack::from_json(&pack_raw(index, &layers)).unwrap()
}

fn installed(count: usize, layer_count: usize, enabled: bool) -> Vec<InstalledPack> {
    (0..count)
        .map(|n| InstalledPack {
            enabled,
            package: pack(n, layer_count),
        })
        .collect()
}

fn same_layers(actual: &Lexicon, expected: &Lexicon) {
    assert_eq!(actual.word_layers().len(), expected.word_layers().len());
    for (actual, expected) in actual.word_layers().iter().zip(expected.word_layers()) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.words, expected.words);
        assert_eq!(actual.next_words, expected.next_words);
        assert_eq!(actual.sentences, expected.sentences);
    }
}

fn assert_merged_prefix_and_layers(catalog: &Catalog, entries: &[InstalledPack]) {
    assert!(
        catalog.report.errors.is_empty(),
        "{:?}",
        catalog.report.errors
    );
    assert!(catalog.report.disabled.is_empty());
    assert_eq!(
        catalog.report.loaded,
        entries
            .iter()
            .map(|e| e.package.manifest.id.clone())
            .collect::<Vec<_>>()
    );
    let builtin = lexicon::builtin("en").unwrap();
    let merged = catalog.get("en").unwrap();
    let expected_count = builtin.word_layers().len()
        + entries
            .iter()
            .map(|e| e.package.lexicon.word_layers().len())
            .sum::<usize>();
    assert_eq!(merged.word_layers().len(), expected_count);
    assert!(expected_count > 64);
    assert!(expected_count <= 64 + 16 * 8);
    for (actual, old) in merged.word_layers().iter().zip(builtin.word_layers()) {
        assert_eq!(actual.id, old.id);
        assert_eq!(actual.words, old.words);
        assert_eq!(actual.next_words, old.next_words);
        assert_eq!(actual.sentences, old.sentences);
    }
    assert_eq!(merged.readings(), builtin.readings());
    assert_eq!(
        merged.continuations().collect::<Vec<_>>(),
        builtin.continuations().collect::<Vec<_>>()
    );
    assert_eq!(
        merged.phrase_endings().collect::<Vec<_>>(),
        builtin.phrase_endings().collect::<Vec<_>>()
    );
    let appended = merged.word_layers()[builtin.word_layers().len()..].iter();
    for (actual, (entry, original)) in appended.zip(entries.iter().flat_map(|entry| {
        entry
            .package
            .lexicon
            .word_layers()
            .iter()
            .map(move |layer| (entry, layer))
    })) {
        assert_eq!(
            actual.id,
            format!("{}:{}", entry.package.manifest.id, original.id)
        );
        assert_eq!(actual.words, original.words);
        assert_eq!(actual.next_words, original.next_words);
        assert_eq!(actual.sentences, original.sentences);
    }
    for language in ["zh-Hans", "ja"] {
        assert!(std::ptr::eq(
            catalog.get(language).unwrap(),
            lexicon::builtin(language).unwrap()
        ));
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "suzaku-layer-budgets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }
    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the exclusive private directory created by this fixture is owned.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn standalone_source_64_and_per_pack_eight_layer_limits_remain_strict() {
    for (count, accepted) in [(64, true), (65, false)] {
        let source = format!(
            r#"{{"format_version":1,"language":"en","word_layers":[{}]}}"#,
            layers(count)
        );
        assert_eq!(Lexicon::from_json(&source).is_ok(), accepted);
    }
    assert_eq!(pack(0, 8).lexicon.word_layers().len(), 8);
    assert!(
        OfflinePack::from_json(&pack_raw(0, &layers(9)))
            .unwrap_err()
            .contains("at most 8 layers")
    );
    assert_eq!(
        (
            packs::MAX_PACKS,
            packs::MAX_PACK_UNITS,
            packs::MAX_LANGUAGE_UNITS
        ),
        (16, 2048, 4096)
    );
}

#[test]
fn sixteen_eight_layer_sources_merge_after_the_growing_builtin_in_stable_order() {
    let entries = installed(16, 8, true);
    assert!(entries.iter().map(|e| e.package.units()).sum::<usize>() <= 4096);
    let first = Catalog::from_installed(&entries);
    let second = Catalog::from_installed(&entries);
    assert_merged_prefix_and_layers(&first, &entries);
    assert_merged_prefix_and_layers(&second, &entries);
    same_layers(first.get("en").unwrap(), second.get("en").unwrap());
}

#[test]
fn direct_catalog_counts_disabled_and_invalid_entries_toward_sixteen_installed_slots() {
    for mode in ["disabled", "enabled", "invalid-prefix"] {
        let mut entries = installed(17, 1, false);
        if mode != "disabled" {
            entries[16].enabled = true;
        }
        if mode == "invalid-prefix" {
            for entry in &mut entries[..16] {
                entry.package.manifest.name.clear();
            }
        }
        let catalog = Catalog::from_installed(&entries);
        assert!(catalog.report.loaded.is_empty(), "{mode}");
        assert!(
            catalog.report.errors.iter().any(|e| e.contains("16")),
            "{mode}"
        );
        if mode == "invalid-prefix" {
            assert_eq!(catalog.report.errors.len(), 17);
            assert!(catalog.report.disabled.is_empty());
        } else {
            assert_eq!(catalog.report.errors.len(), 1);
            assert_eq!(catalog.report.disabled.len(), 16);
        }
        assert!(std::ptr::eq(
            catalog.get("en").unwrap(),
            lexicon::builtin("en").unwrap()
        ));
    }
}

#[test]
fn store_pack_layer_count_and_existing_unit_budgets_remain_transactional() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let entries = installed(16, 8, true);
    for entry in &entries {
        store.install(entry.package.clone(), false).unwrap();
    }
    assert_eq!(store.list().unwrap().len(), 16);
    assert_merged_prefix_and_layers(&store.load(), &entries);
    let before = fs::read(store.path()).unwrap();
    assert!(
        store
            .install(pack(16, 8), false)
            .unwrap_err()
            .contains("16")
    );
    assert_eq!(fs::read(store.path()).unwrap(), before);
    store
        .set_enabled(&entries[0].package.manifest.id, false)
        .unwrap();
    let disabled = fs::read(store.path()).unwrap();
    assert!(
        store
            .install(pack(16, 1), false)
            .unwrap_err()
            .contains("16")
    );
    assert_eq!(fs::read(store.path()).unwrap(), disabled);
    let loaded = store.load();
    assert_eq!(loaded.report.loaded.len(), 15);
    assert_eq!(
        loaded.report.disabled,
        [entries[0].package.manifest.id.clone()]
    );
    assert!(loaded.report.errors.is_empty());
    // A separate private store keeps the existing unit ceiling independent of
    // the layer fix; no built-in growth may relax 2048/4096 work budgets.
    let fixture = Fixture::new();
    let store = fixture.store();
    let words = |count: usize| vec!["\"budgetword\""; count].join(",");
    let layer = |count| format!(r#"{{"id":"budget","words":[{}]}}"#, words(count));
    assert!(OfflinePack::from_json(&pack_raw(0, &layer(2049))).is_err());
    for n in 0..2 {
        let package = OfflinePack::from_json(&pack_raw(n, &layer(2048))).unwrap();
        assert_eq!(package.units(), 2048);
        store.install(package, false).unwrap();
    }
    let before = fs::read(store.path()).unwrap();
    assert!(
        store
            .install(
                OfflinePack::from_json(&pack_raw(2, &layer(1))).unwrap(),
                false
            )
            .unwrap_err()
            .contains("4096")
    );
    assert_eq!(fs::read(store.path()).unwrap(), before);
    assert_eq!(store.load().report.loaded.len(), 2);
}
