//! Catalog availability is not simultaneous installation or native acceptance.
#[path = "support/offline_pack_sets.rs"]
mod offline_pack_sets;

use ring::digest::{SHA256, digest};
use serde_json::Value;
use std::{
    collections::BTreeSet,
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

// Captured before either internet pack was added. These sixteen source hashes
// and their catalog positions are independent of the new batching helpers.
const PREVIOUS: [(&str, &str, &str); 16] = [
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
    (
        "org.suzaku.en.shopping",
        include_str!("../data/offline-packs/en-shopping.json"),
        "d13716a0ab1b3e7b7cecd70562282790b20562845a6aebfb930bf91077ce156b",
    ),
    (
        "org.suzaku.zh-hans.shopping",
        include_str!("../data/offline-packs/zh-Hans-shopping.json"),
        "74ecabdb7ff9101805c73221bc4167ae5cf934940617a7c4f3927d7e2be30499",
    ),
];
const INTERNET: [&str; 2] = ["org.suzaku.en.internet", "org.suzaku.zh-hans.internet"];
// Original fourteen plus internet. Shopping remains in the historical batch;
// no test pretends all eighteen can share one sixteen-record registry.
const WITH_INTERNET: [&str; 16] = [
    "org.suzaku.en.outdoors",
    "org.suzaku.zh-hans.outdoors",
    "org.suzaku.ja.rail",
    "org.suzaku.en.study",
    "org.suzaku.zh-hans.study",
    "org.suzaku.en.cooking",
    "org.suzaku.zh-hans.cooking",
    "org.suzaku.en.travel",
    "org.suzaku.zh-hans.travel",
    "org.suzaku.en.work",
    "org.suzaku.zh-hans.work",
    "org.suzaku.en.family",
    "org.suzaku.zh-hans.family",
    "org.suzaku.ja.family",
    "org.suzaku.en.internet",
    "org.suzaku.zh-hans.internet",
];

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-internet-catalog-{}-{}",
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
            // Catalog discovery must not resolve even an invalid registry path.
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
        // Exclusive creation above establishes cleanup ownership.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn find<'a>(catalog: &'a [OfflinePack], id: &str) -> &'a OfflinePack {
    let mut matches = catalog.iter().filter(|pack| pack.manifest.id == id);
    let found = matches.next().unwrap_or_else(|| panic!("missing {id}"));
    assert!(matches.next().is_none(), "duplicate catalog ID {id}");
    found
}

#[test]
fn internet_append_preserves_all_sixteen_sources_and_catalog_prefix() {
    let catalog = packs::recommended();
    assert_eq!(PREVIOUS.len(), 16);
    assert!(catalog.len() >= 16, "lost an existing collection");
    for (index, (id, source, expected)) in PREVIOUS.iter().enumerate() {
        let mut actual = String::new();
        for byte in digest(&SHA256, source.as_bytes()).as_ref() {
            write!(&mut actual, "{byte:02x}").unwrap();
        }
        assert_eq!(actual, *expected, "changed source bytes for {id}");
        assert_eq!(catalog[index].manifest.id, *id, "changed catalog priority");
        assert_eq!(
            serde_json::to_value(&catalog[index]).unwrap(),
            serde_json::to_value(OfflinePack::from_json(source).unwrap()).unwrap(),
            "catalog content differs from the frozen source of {id}"
        );
    }
}

#[test]
fn historical_eighteen_collections_keep_every_existing_installation_limit() {
    let catalog = offline_pack_sets::historical_eighteen();
    assert_eq!(catalog.len(), 18);
    assert_eq!(
        catalog[16..]
            .iter()
            .map(|pack| pack.manifest.id.as_str())
            .collect::<Vec<_>>(),
        INTERNET
    );
    assert_eq!(
        catalog
            .iter()
            .map(|pack| &pack.manifest.id)
            .collect::<BTreeSet<_>>()
            .len(),
        18
    );
    for (language, expected) in [("en", 8), ("zh-Hans", 8), ("ja", 2)] {
        let language_packs: Vec<_> = catalog
            .iter()
            .filter(|pack| pack.manifest.language == language)
            .collect();
        assert_eq!(language_packs.len(), expected, "{language}");
        let units: usize = language_packs.iter().map(|pack| pack.units()).sum();
        assert!(units > 0 && units < 4096, "{language}: {units}");
    }
    for pack in &catalog[16..] {
        assert_eq!(pack.manifest.version, "1.0.0");
        assert_eq!(pack.manifest.topics, ["internet", "digital"]);
        assert!((1..=2048).contains(&pack.units()));
    }
    assert_eq!(packs::MAX_PACKS, 16);
    assert_eq!(packs::MAX_PACK_BYTES, 256 * 1024);
    assert_eq!(packs::MAX_PACK_UNITS, 2048);
    assert_eq!(packs::MAX_LANGUAGE_UNITS, 4096);
}

#[test]
fn catalog_and_details_show_all_twenty_five_without_resolving_or_creating_a_registry() {
    let f = Fixture::new();
    let catalog = f.catalog(&[]);
    let details = f.catalog(&["--details"]);
    assert_eq!(catalog.as_array().unwrap().len(), 25);
    assert_eq!(details.as_array().unwrap().len(), 25);
    for (plain, detailed) in catalog
        .as_array()
        .unwrap()
        .iter()
        .zip(details.as_array().unwrap())
    {
        assert_eq!(detailed["manifest"], *plain);
        assert!(detailed["stats"]["indexing_units"].as_u64().unwrap() > 0);
    }
    for topic in ["internet", "digital"] {
        let all = f.catalog(&["--topic", topic]);
        let rows = all.as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[..2]
                .iter()
                .map(|row| row["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            INTERNET
        );
        assert_eq!(rows[2]["id"], "org.suzaku.ja.internet");
        for (language, id) in [
            ("en", INTERNET[0]),
            ("zh-Hans", INTERNET[1]),
            ("ja", "org.suzaku.ja.internet"),
        ] {
            let plain = f.catalog(&["--language", language, "--topic", topic]);
            let detailed = f.catalog(&["--topic", topic, "--details", "--language", language]);
            assert_eq!(plain.as_array().unwrap().len(), 1);
            assert_eq!(detailed.as_array().unwrap().len(), 1);
            assert_eq!(plain[0]["id"], id);
            assert_eq!(plain[0]["language"], language);
            assert_eq!(detailed[0]["manifest"], plain[0]);
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
fn two_explicit_sixteen_pack_batches_cover_eighteen_and_refuse_a_seventeenth() {
    let catalog = offline_pack_sets::historical_eighteen();
    assert_eq!(catalog.len(), 18);
    let historical = PREVIOUS.map(|(id, _, _)| id);
    assert_eq!(historical.len(), 16);
    assert_eq!(WITH_INTERNET.len(), 16);
    let mut covered = BTreeSet::new();
    for (batch_index, batch) in [historical, WITH_INTERNET].into_iter().enumerate() {
        let f = Fixture::new();
        assert_eq!(batch.into_iter().collect::<BTreeSet<_>>().len(), 16);
        for id in batch {
            f.store()
                .install(find(&catalog, id).clone(), false)
                .unwrap();
            covered.insert(id);
        }
        let entries = f.store().list().unwrap();
        assert_eq!(entries.len(), 16);
        assert!(entries.iter().all(|entry| entry.enabled));
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.package.manifest.id.as_str())
                .collect::<Vec<_>>(),
            batch
        );
        let loaded = f.store().load();
        assert!(loaded.report.errors.is_empty(), "{:?}", loaded.report);
        assert!(loaded.report.disabled.is_empty());
        assert_eq!(loaded.report.loaded, batch);
        for language in ["en", "zh-Hans", "ja"] {
            let base = lexicon::builtin(language).unwrap();
            let active = loaded.get(language).unwrap();
            assert_eq!(
                serde_json::to_value(base.word_layers()).unwrap(),
                serde_json::to_value(&active.word_layers()[..base.word_layers().len()]).unwrap()
            );
            assert_eq!(base.readings(), &active.readings()[..base.readings().len()]);
            assert_eq!(
                base.continuations().collect::<Vec<_>>(),
                active
                    .continuations()
                    .take(base.continuations().count())
                    .collect::<Vec<_>>()
            );
        }
        if batch_index == 0 {
            let before = fs::read(f.store().path()).unwrap();
            for id in INTERNET {
                let error = f
                    .store()
                    .install(find(&catalog, id).clone(), false)
                    .unwrap_err();
                assert_eq!(error, "at most 16 packs may be installed");
                assert_eq!(fs::read(f.store().path()).unwrap(), before);
                assert_eq!(f.store().list().unwrap().len(), 16);
                assert_eq!(f.store().load().report.loaded, batch);
            }
        }
    }
    assert_eq!(covered.len(), 18);
    assert_eq!(
        covered,
        catalog
            .iter()
            .map(|pack| pack.manifest.id.as_str())
            .collect::<BTreeSet<_>>()
    );
    for language in ["en", "zh-Hans", "ja"] {
        assert!(std::ptr::eq(
            lexicon::active(language).unwrap(),
            lexicon::builtin(language).unwrap()
        ));
    }
}
