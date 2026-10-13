//! Japanese catalog growth does not raise the sixteen-installed-pack limit.
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

// Frozen before the Japanese internet source or recommendation existed. Do not
// derive these bytes or the compatibility cohorts from the growing catalog.
const PREVIOUS: [(&str, &str, &str); 18] = [
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
    (
        "org.suzaku.en.internet",
        include_str!("../data/offline-packs/en-internet.json"),
        "b433b0409ddf1831655119bf89f51cdcf66df94cb8f694dbcdc7ab24671b6241",
    ),
    (
        "org.suzaku.zh-hans.internet",
        include_str!("../data/offline-packs/zh-Hans-internet.json"),
        "72779e22518b8d9ea116a692094c35c4a1da6198396a1d03e03ebc9e6cfbd25d",
    ),
];
const BUILTIN: [(&str, &str, &str); 3] = [
    (
        "en",
        include_str!("../data/lexicons/en.json"),
        "e3b4f10507073bd284b9b7529cd118ab98a9a41e244897e2028fb458d28c22a4",
    ),
    (
        "zh-Hans",
        include_str!("../data/lexicons/zh-Hans.json"),
        "457725d02b7f62250c8b734cf54d9dca84de9ad1d1ed888518bc8f523d2914fd",
    ),
    (
        "ja",
        include_str!("../data/lexicons/ja.json"),
        "54cb830c6360eb94a95bf1f0142a29970ce48b169f1ade91f9bf9780948b3ae9",
    ),
];
const INTERNET: [&str; 3] = [
    "org.suzaku.en.internet",
    "org.suzaku.zh-hans.internet",
    "org.suzaku.ja.internet",
];
// These are exact, independent regression cohorts. No dynamic truncation or
// replacement is allowed to silently drop an old assertion's data source.
const HISTORICAL: [&str; 16] = [
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
    "org.suzaku.en.shopping",
    "org.suzaku.zh-hans.shopping",
];
const WITH_EN_ZH_INTERNET: [&str; 16] = [
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
const WITH_EN_JA_INTERNET: [&str; 16] = [
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
    "org.suzaku.ja.internet",
];

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-ja-internet-catalog-{}-{}",
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

fn source_sha256(source: &str) -> String {
    let mut actual = String::new();
    for byte in digest(&SHA256, source.as_bytes()).as_ref() {
        write!(&mut actual, "{byte:02x}").unwrap();
    }
    actual
}

fn find<'a>(catalog: &'a [OfflinePack], id: &str) -> &'a OfflinePack {
    let mut matching = catalog.iter().filter(|pack| pack.manifest.id == id);
    let pack = matching.next().unwrap_or_else(|| panic!("missing {id}"));
    assert!(matching.next().is_none(), "duplicate catalog ID {id}");
    pack
}

#[test]
fn japanese_internet_preserves_all_eighteen_source_bytes_and_catalog_positions() {
    let catalog = packs::recommended();
    assert!(
        catalog.len() >= PREVIOUS.len(),
        "lost an existing collection"
    );
    for (index, (id, source, expected)) in PREVIOUS.iter().enumerate() {
        assert_eq!(
            source_sha256(source),
            *expected,
            "changed source bytes for {id}"
        );
        assert_eq!(
            catalog[index].manifest.id, *id,
            "changed old catalog priority"
        );
        assert_eq!(
            serde_json::to_value(&catalog[index]).unwrap(),
            serde_json::to_value(OfflinePack::from_json(source).unwrap()).unwrap(),
            "catalog content differs from the frozen source of {id}"
        );
    }
    for (language, source, expected) in BUILTIN {
        assert_eq!(
            source_sha256(source),
            expected,
            "changed builtin {language}"
        );
    }
}

#[test]
fn historical_nineteen_collections_keep_all_existing_capacity_limits() {
    let catalog = offline_pack_sets::historical_nineteen();
    assert_eq!(catalog.len(), 19);
    assert_eq!(catalog[18].manifest.id, INTERNET[2]);
    assert_eq!(
        catalog
            .iter()
            .map(|p| &p.manifest.id)
            .collect::<BTreeSet<_>>()
            .len(),
        19
    );
    for (language, expected) in [("en", 8), ("zh-Hans", 8), ("ja", 3)] {
        let selected: Vec<_> = catalog
            .iter()
            .filter(|p| p.manifest.language == language)
            .collect();
        assert_eq!(selected.len(), expected, "{language}");
        let units: usize = selected.iter().map(|p| p.units()).sum();
        assert!(units > 0 && units < 4096, "{language}: {units}");
    }
    let japanese = find(&catalog, INTERNET[2]);
    assert_eq!(japanese.manifest.language, "ja");
    assert_eq!(japanese.manifest.version, "1.0.0");
    assert_eq!(japanese.manifest.topics, ["internet", "digital"]);
    assert!((1..=2048).contains(&japanese.units()));
    assert_eq!(packs::MAX_PACKS, 16);
    assert_eq!(packs::MAX_PACK_BYTES, 256 * 1024);
    assert_eq!(packs::MAX_PACK_UNITS, 2048);
    assert_eq!(packs::MAX_LANGUAGE_UNITS, 4096);
}

#[test]
fn catalog_and_topic_details_include_japanese_without_reading_or_creating_a_registry() {
    let f = Fixture::new();
    let plain = f.catalog(&[]);
    let details = f.catalog(&["--details"]);
    assert_eq!(plain.as_array().unwrap().len(), 25);
    assert_eq!(details.as_array().unwrap().len(), 25);
    for (entry, detailed) in plain
        .as_array()
        .unwrap()
        .iter()
        .zip(details.as_array().unwrap())
    {
        assert_eq!(detailed["manifest"], *entry);
        assert!(detailed["stats"]["indexing_units"].as_u64().unwrap() > 0);
    }
    for topic in ["internet", "digital"] {
        let all = f.catalog(&["--topic", topic]);
        assert_eq!(all.as_array().unwrap().len(), 3);
        assert_eq!(
            all.as_array()
                .unwrap()
                .iter()
                .map(|p| p["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            INTERNET
        );
        for (language, id) in [
            ("en", INTERNET[0]),
            ("zh-Hans", INTERNET[1]),
            ("ja", INTERNET[2]),
        ] {
            let filtered = f.catalog(&["--language", language, "--topic", topic]);
            let detailed = f.catalog(&["--topic", topic, "--details", "--language", language]);
            assert_eq!(filtered.as_array().unwrap().len(), 1);
            assert_eq!(detailed.as_array().unwrap().len(), 1);
            assert_eq!(filtered[0]["id"], id);
            assert_eq!(filtered[0]["language"], language);
            assert_eq!(detailed[0]["manifest"], filtered[0]);
            let catalog = packs::recommended();
            assert_eq!(
                detailed[0]["stats"],
                serde_json::to_value(find(&catalog, id).stats()).unwrap()
            );
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
fn three_explicit_sixteen_pack_batches_cover_nineteen_and_refuse_a_seventeenth() {
    let catalog = offline_pack_sets::historical_nineteen();
    assert_eq!(catalog.len(), 19);
    assert_eq!(
        HISTORICAL,
        PREVIOUS[..16]
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>()
            .as_slice()
    );
    let mut covered = BTreeSet::new();
    for (batch_index, batch) in [HISTORICAL, WITH_EN_ZH_INTERNET, WITH_EN_JA_INTERNET]
        .into_iter()
        .enumerate()
    {
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
        assert!(entries.iter().all(|p| p.enabled));
        assert_eq!(
            entries
                .iter()
                .map(|p| p.package.manifest.id.as_str())
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
    assert_eq!(covered.len(), 19);
    assert_eq!(
        covered,
        catalog
            .iter()
            .map(|p| p.manifest.id.as_str())
            .collect::<BTreeSet<_>>()
    );
    for language in ["en", "zh-Hans", "ja"] {
        assert!(std::ptr::eq(
            lexicon::active(language).unwrap(),
            lexicon::builtin(language).unwrap()
        ));
    }
}
