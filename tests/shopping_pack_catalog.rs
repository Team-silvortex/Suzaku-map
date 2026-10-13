//! Optional authored collections, not native/installed-app or model qualification.
#[path = "support/offline_pack_sets.rs"]
mod offline_pack_sets;

use ring::digest::{SHA256, digest};
use serde_json::Value;
use std::{
    fmt::Write,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use suzaku_map::lexicon::{
    self,
    packs::{self, OfflinePack, PackStore},
};

// Captured before adding shopping packs. Keep every original byte and the
// original catalog priority, including all three previously added family packs.
const PREVIOUS: [(&str, &str, &str); 14] = [
    (
        "org.suzaku.en.outdoors",
        include_str!("../data/offline-packs/en-outdoors.json"),
        "e1f798a4237fc24b4bf5a06b34eecfc4d5b089a87a77e3a0a72004616c170276",
    ),
    (
        "org.suzaku.zh-hans.outdoors",
        include_str!("../data/offline-packs/zh-Hans-outdoors.json"),
        "415cfd4a92d019d25451599ddbbef84c785a3b73436acadc694196d0fea72713",
    ),
    (
        "org.suzaku.ja.rail",
        include_str!("../data/offline-packs/ja-rail.json"),
        "842108fbb507076bc00b2ca1a32a8d2e6cbc611225ab4dd6dd1fe409eb350495",
    ),
    (
        "org.suzaku.en.study",
        include_str!("../data/offline-packs/en-study.json"),
        "331f80f23e96953410c328381023bd48033ca130645e3e17db4439989a20605f",
    ),
    (
        "org.suzaku.zh-hans.study",
        include_str!("../data/offline-packs/zh-Hans-study.json"),
        "665573da6842fe6ead5aa9277d1d510748a66db06e604fe592c0ff4c51a8cf52",
    ),
    (
        "org.suzaku.en.cooking",
        include_str!("../data/offline-packs/en-cooking.json"),
        "88dc5ada597e864aec7748a11f7632780fb862f771484dc3dceb07625f3e9490",
    ),
    (
        "org.suzaku.zh-hans.cooking",
        include_str!("../data/offline-packs/zh-Hans-cooking.json"),
        "b9e3f729aba0dcc494b1510d13a4454fafd2e5e88a3648b248f5c01f61935366",
    ),
    (
        "org.suzaku.en.travel",
        include_str!("../data/offline-packs/en-travel.json"),
        "5adb60d402715b9c91a927a2e5c37d2d7515969a59737a1067023d7e5951df49",
    ),
    (
        "org.suzaku.zh-hans.travel",
        include_str!("../data/offline-packs/zh-Hans-travel.json"),
        "e4d4ebaa57e208781edbb482bbbda7e5e5e7a9f95c0c99e21d302f81496aba51",
    ),
    (
        "org.suzaku.en.work",
        include_str!("../data/offline-packs/en-work.json"),
        "ab6479965912877ba78d403679910a0048008bc3ae57db819a44454e9e756a3d",
    ),
    (
        "org.suzaku.zh-hans.work",
        include_str!("../data/offline-packs/zh-Hans-work.json"),
        "d99bcb137ca55852bcf74f4268a99ee86b5fb293bf9df6697fcb14f5185722da",
    ),
    (
        "org.suzaku.en.family",
        include_str!("../data/offline-packs/en-family.json"),
        "219205e3f45c1b82d408a60df5c4fd6e2c16916aa0b99c9fafd1082270942df8",
    ),
    (
        "org.suzaku.zh-hans.family",
        include_str!("../data/offline-packs/zh-Hans-family.json"),
        "766ecb5c438e27ac8d4f9d94caf8306c07b65f3236a70535c0f6cafda817da15",
    ),
    (
        "org.suzaku.ja.family",
        include_str!("../data/offline-packs/ja-family.json"),
        "92c581ace3cba97f202016c61b29cda7ee7173ffa8fefcb93d479097c82382bc",
    ),
];
const SHOPPING: [&str; 2] = ["org.suzaku.en.shopping", "org.suzaku.zh-hans.shopping"];

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-shopping-catalog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }

    fn catalog(&self, args: &[&str]) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"))
            .args(["pack", "catalog"])
            .args(args)
            // Discovery would fail if catalog tried to use the personal store.
            .env("SUZAKU_LEXICON_DIR", "not-an-absolute-registry")
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Creation above was exclusive; cleanup owns only this test directory.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn shopping_append_preserves_all_fourteen_sources_and_catalog_prefix() {
    let catalog = packs::recommended();
    assert_eq!(PREVIOUS.len(), 14);
    assert!(catalog.len() >= 14, "lost an older recommended collection");
    for (index, (id, source, expected)) in PREVIOUS.iter().enumerate() {
        let mut actual = String::new();
        for byte in digest(&SHA256, source.as_bytes()).as_ref() {
            write!(&mut actual, "{byte:02x}").unwrap();
        }
        assert_eq!(actual, *expected, "changed the shipped bytes of {id}");
        assert_eq!(catalog[index].manifest.id, *id, "changed priority/order");
        let source = OfflinePack::from_json(source).unwrap();
        assert_eq!(
            serde_json::to_value(&catalog[index]).unwrap(),
            serde_json::to_value(source).unwrap(),
            "catalog content no longer matches {id}"
        );
    }
}

#[test]
fn historical_sixteen_are_bilingual_shopping_append_not_a_budget_increase() {
    let catalog = offline_pack_sets::historical_sixteen();
    assert_eq!(catalog.len(), 16);
    assert_eq!(
        catalog[14..]
            .iter()
            .map(|pack| pack.manifest.id.as_str())
            .collect::<Vec<_>>(),
        SHOPPING
    );
    for (language, expected) in [("en", 7), ("zh-Hans", 7), ("ja", 2)] {
        assert_eq!(
            catalog
                .iter()
                .filter(|pack| pack.manifest.language == language)
                .count(),
            expected,
            "{language}"
        );
    }
    for pack in &catalog[14..] {
        assert_eq!(pack.manifest.version, "1.0.0");
        assert_eq!(pack.manifest.topics, ["shopping", "errands"]);
    }
    assert_eq!(packs::MAX_PACKS, 16);
    assert_eq!(packs::MAX_PACK_BYTES, 256 * 1024);
    assert_eq!(packs::MAX_PACK_UNITS, 2048);
    assert_eq!(packs::MAX_LANGUAGE_UNITS, 4096);
}

#[test]
fn shopping_and_errands_filters_are_read_only_and_do_not_enable_collections() {
    let f = Fixture::new();
    assert_eq!(f.catalog(&[]).as_array().unwrap().len(), 25);
    for topic in ["shopping", "errands"] {
        let all = f.catalog(&["--topic", topic]);
        let rows = all.as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter()
                .map(|row| row["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [SHOPPING[0], SHOPPING[1], "org.suzaku.ja.shopping"]
        );
        for (language, expected) in [
            ("en", SHOPPING[0]),
            ("zh-Hans", SHOPPING[1]),
            ("ja", "org.suzaku.ja.shopping"),
        ] {
            let filtered = f.catalog(&["--language", language, "--topic", topic]);
            let detailed = f.catalog(&["--topic", topic, "--details", "--language", language]);
            assert_eq!(filtered.as_array().unwrap().len(), 1);
            assert_eq!(detailed.as_array().unwrap().len(), 1);
            assert_eq!(filtered[0]["id"], expected);
            assert_eq!(filtered[0]["language"], language);
            assert_eq!(detailed[0]["manifest"], filtered[0]);
            assert!(detailed[0]["stats"]["indexing_units"].as_u64().unwrap() > 0);
        }
    }
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
    assert!(!f.store().path().exists());
    for language in ["en", "zh-Hans", "ja"] {
        assert!(std::ptr::eq(
            lexicon::active(language).unwrap(),
            lexicon::builtin(language).unwrap()
        ));
    }
}

#[test]
fn historical_sixteen_install_into_an_owned_store_with_unchanged_language_budgets() {
    let f = Fixture::new();
    let collections = offline_pack_sets::historical_sixteen();
    assert_eq!(collections.len(), 16);
    for language in ["en", "zh-Hans", "ja"] {
        let units: usize = collections
            .iter()
            .filter(|pack| pack.manifest.language == language)
            .map(|pack| pack.units())
            .sum();
        assert!(units > 0 && units <= 4096, "{language}: {units}");
    }
    let expected: Vec<_> = collections
        .iter()
        .map(|pack| pack.manifest.id.clone())
        .collect();
    for collection in collections {
        assert!((1..=2048).contains(&collection.units()));
        f.store().install(collection, false).unwrap();
    }
    let installed = f.store().list().unwrap();
    assert_eq!(installed.len(), 16);
    assert!(installed.iter().all(|pack| pack.enabled));
    let loaded = f.store().load();
    assert!(loaded.report.errors.is_empty(), "{:?}", loaded.report);
    assert!(loaded.report.disabled.is_empty());
    assert_eq!(loaded.report.loaded, expected);
    // Installing into an explicit private store never mutates active runtime.
    for language in ["en", "zh-Hans", "ja"] {
        assert!(std::ptr::eq(
            lexicon::active(language).unwrap(),
            lexicon::builtin(language).unwrap()
        ));
    }
}
