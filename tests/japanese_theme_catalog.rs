//! Independent compatibility contract for the three new Japanese themes.
use ring::digest::{SHA256, digest};
use serde_json::Value;
use std::{collections::BTreeSet, fmt::Write, fs, path::PathBuf, process::Command};
use suzaku_map::lexicon::{
    self,
    packs::{self, OfflinePack, PackStore},
};

#[path = "support/offline_pack_sets.rs"]
mod offline_pack_sets;

// Fixed before adding any new Japanese theme. Never regenerate from the catalog.
const OLD: [(&str, &str, &str); 19] = [
    (
        "en-outdoors",
        "org.suzaku.en.outdoors",
        "e1f798a4237fc24b4bf5a06b34eecfc4d5b089a87a77e3a0a72004616c170276",
    ),
    (
        "zh-Hans-outdoors",
        "org.suzaku.zh-hans.outdoors",
        "415cfd4a92d019d25451599ddbbef84c785a3b73436acadc694196d0fea72713",
    ),
    (
        "ja-rail",
        "org.suzaku.ja.rail",
        "842108fbb507076bc00b2ca1a32a8d2e6cbc611225ab4dd6dd1fe409eb350495",
    ),
    (
        "en-study",
        "org.suzaku.en.study",
        "331f80f23e96953410c328381023bd48033ca130645e3e17db4439989a20605f",
    ),
    (
        "zh-Hans-study",
        "org.suzaku.zh-hans.study",
        "665573da6842fe6ead5aa9277d1d510748a66db06e604fe592c0ff4c51a8cf52",
    ),
    (
        "en-cooking",
        "org.suzaku.en.cooking",
        "88dc5ada597e864aec7748a11f7632780fb862f771484dc3dceb07625f3e9490",
    ),
    (
        "zh-Hans-cooking",
        "org.suzaku.zh-hans.cooking",
        "b9e3f729aba0dcc494b1510d13a4454fafd2e5e88a3648b248f5c01f61935366",
    ),
    (
        "en-travel",
        "org.suzaku.en.travel",
        "5adb60d402715b9c91a927a2e5c37d2d7515969a59737a1067023d7e5951df49",
    ),
    (
        "zh-Hans-travel",
        "org.suzaku.zh-hans.travel",
        "e4d4ebaa57e208781edbb482bbbda7e5e5e7a9f95c0c99e21d302f81496aba51",
    ),
    (
        "en-work",
        "org.suzaku.en.work",
        "ab6479965912877ba78d403679910a0048008bc3ae57db819a44454e9e756a3d",
    ),
    (
        "zh-Hans-work",
        "org.suzaku.zh-hans.work",
        "d99bcb137ca55852bcf74f4268a99ee86b5fb293bf9df6697fcb14f5185722da",
    ),
    (
        "en-family",
        "org.suzaku.en.family",
        "219205e3f45c1b82d408a60df5c4fd6e2c16916aa0b99c9fafd1082270942df8",
    ),
    (
        "zh-Hans-family",
        "org.suzaku.zh-hans.family",
        "766ecb5c438e27ac8d4f9d94caf8306c07b65f3236a70535c0f6cafda817da15",
    ),
    (
        "ja-family",
        "org.suzaku.ja.family",
        "92c581ace3cba97f202016c61b29cda7ee7173ffa8fefcb93d479097c82382bc",
    ),
    (
        "en-shopping",
        "org.suzaku.en.shopping",
        "d13716a0ab1b3e7b7cecd70562282790b20562845a6aebfb930bf91077ce156b",
    ),
    (
        "zh-Hans-shopping",
        "org.suzaku.zh-hans.shopping",
        "74ecabdb7ff9101805c73221bc4167ae5cf934940617a7c4f3927d7e2be30499",
    ),
    (
        "en-internet",
        "org.suzaku.en.internet",
        "b433b0409ddf1831655119bf89f51cdcf66df94cb8f694dbcdc7ab24671b6241",
    ),
    (
        "zh-Hans-internet",
        "org.suzaku.zh-hans.internet",
        "72779e22518b8d9ea116a692094c35c4a1da6198396a1d03e03ebc9e6cfbd25d",
    ),
    (
        "ja-internet",
        "org.suzaku.ja.internet",
        "aa6e1f41aacfa55ec6431c1b228f1b6522269cd332fe28958fdf9939027480ec",
    ),
];
const NEW: [(&str, [&str; 2]); 3] = [
    ("org.suzaku.ja.shopping", ["shopping", "errands"]),
    ("org.suzaku.ja.travel", ["travel", "lodging"]),
    ("org.suzaku.ja.work", ["work", "collaboration"]),
];

struct Fixture(PathBuf);
impl Fixture {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("suzaku-ja-themes-{label}-{}", std::process::id()));
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
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn find<'a>(catalog: &'a [OfflinePack], id: &str) -> &'a OfflinePack {
    let matches: Vec<_> = catalog.iter().filter(|p| p.manifest.id == id).collect();
    assert_eq!(matches.len(), 1, "required unique collection {id}");
    matches[0]
}

#[test]
fn all_previous_nineteen_sources_and_catalog_positions_are_unchanged() {
    let catalog = packs::recommended();
    assert!(catalog.len() >= OLD.len());
    for (index, (file, id, expected)) in OLD.iter().enumerate() {
        let source = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("data/offline-packs/{file}.json")),
        )
        .unwrap();
        let mut actual = String::new();
        for byte in digest(&SHA256, source.as_bytes()).as_ref() {
            write!(&mut actual, "{byte:02x}").unwrap();
        }
        assert_eq!(actual, *expected, "changed old bytes: {id}");
        assert_eq!(catalog[index].manifest.id, *id, "changed priority");
        assert_eq!(
            serde_json::to_value(&catalog[index]).unwrap(),
            serde_json::to_value(OfflinePack::from_json(&source).unwrap()).unwrap()
        );
    }
}

#[test]
fn three_appended_themes_keep_the_same_capacity_limits() {
    let catalog = offline_pack_sets::historical_twenty_two();
    assert_eq!(catalog.len(), 22);
    assert_eq!(
        catalog
            .iter()
            .map(|p| &p.manifest.id)
            .collect::<BTreeSet<_>>()
            .len(),
        22
    );
    for (index, (id, topics)) in NEW.iter().enumerate() {
        let pack = &catalog[19 + index];
        assert_eq!(pack.manifest.id, *id);
        assert_eq!(pack.manifest.language, "ja");
        assert_eq!(pack.manifest.version, "1.0.0");
        assert_eq!(pack.manifest.topics, *topics);
        assert_eq!(pack.units(), 68);
    }
    for (lang, count, units) in [("en", 8, 2976), ("zh-Hans", 8, 622), ("ja", 6, 388)] {
        let selected: Vec<_> = catalog
            .iter()
            .filter(|p| p.manifest.language == lang)
            .collect();
        assert_eq!(selected.len(), count, "{lang}");
        assert_eq!(
            selected.iter().map(|p| p.units()).sum::<usize>(),
            units,
            "{lang}"
        );
        assert!(units < 4096);
    }
    assert_eq!(packs::MAX_PACKS, 16);
    assert_eq!(packs::MAX_PACK_BYTES, 256 * 1024);
    assert_eq!(packs::MAX_PACK_UNITS, 2048);
    assert_eq!(packs::MAX_LANGUAGE_UNITS, 4096);
}

#[test]
fn read_only_catalog_filters_keep_rail_separate_from_lodging() {
    let f = Fixture::new("catalog");
    for args in [vec![], vec!["--details"]] {
        assert_eq!(f.catalog(&args).as_array().unwrap().len(), 25);
    }
    for (id, topics) in NEW {
        for topic in topics {
            let all = f.catalog(&["--topic", topic]);
            let expected = if topic == "travel" { 4 } else { 3 };
            assert_eq!(all.as_array().unwrap().len(), expected, "{topic}");
            let filtered = f.catalog(&["--language", "ja", "--topic", topic]);
            let detailed = f.catalog(&["--topic", topic, "--details", "--language", "ja"]);
            let ids: Vec<_> = filtered
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["id"].as_str().unwrap())
                .collect();
            assert_eq!(
                ids,
                if topic == "travel" {
                    vec!["org.suzaku.ja.rail", id]
                } else {
                    vec![id]
                }
            );
            assert_eq!(detailed.as_array().unwrap().len(), ids.len());
            for (manifest, row) in filtered
                .as_array()
                .unwrap()
                .iter()
                .zip(detailed.as_array().unwrap())
            {
                assert_eq!(*manifest, row["manifest"]);
                let catalog = packs::recommended();
                assert_eq!(
                    row["stats"],
                    serde_json::to_value(find(&catalog, manifest["id"].as_str().unwrap()).stats())
                        .unwrap()
                );
            }
        }
    }
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
    for lang in ["en", "zh-Hans", "ja"] {
        assert!(std::ptr::eq(
            lexicon::active(lang).unwrap(),
            lexicon::builtin(lang).unwrap()
        ));
    }
}

#[test]
fn six_explicit_batches_cover_twenty_two_without_raising_sixteen_slots() {
    let catalog = offline_pack_sets::historical_twenty_two();
    // Independent fixed membership, not a truncation of a growing catalog.
    let original: Vec<_> = OLD[..16].iter().map(|(_, id, _)| *id).collect();
    let prefix: Vec<_> = OLD[..14].iter().map(|(_, id, _)| *id).collect();
    let mut batches = vec![original];
    for tail in [
        ["org.suzaku.en.internet", "org.suzaku.zh-hans.internet"],
        ["org.suzaku.en.internet", "org.suzaku.ja.internet"],
        ["org.suzaku.ja.internet", NEW[0].0],
        ["org.suzaku.ja.internet", NEW[1].0],
        ["org.suzaku.ja.internet", NEW[2].0],
    ] {
        let mut batch = prefix.clone();
        batch.extend(tail);
        batches.push(batch);
    }
    let mut helpers = vec![
        offline_pack_sets::historical_sixteen(),
        offline_pack_sets::with_internet(),
        offline_pack_sets::with_japanese_internet(),
    ];
    for (id, _) in NEW {
        helpers.push(offline_pack_sets::with_japanese_theme(id));
    }
    let mut covered = BTreeSet::new();
    for (index, (batch, helper)) in batches.into_iter().zip(helpers).enumerate() {
        assert_eq!(
            helper
                .iter()
                .map(|p| p.manifest.id.as_str())
                .collect::<Vec<_>>(),
            batch
        );
        let f = Fixture::new(&format!("batch-{index}"));
        assert_eq!(batch.iter().collect::<BTreeSet<_>>().len(), 16);
        for id in &batch {
            f.store()
                .install(find(&catalog, id).clone(), false)
                .unwrap();
            covered.insert(*id);
        }
        let loaded = f.store().load();
        assert!(loaded.report.errors.is_empty(), "{:?}", loaded.report);
        assert!(loaded.report.disabled.is_empty());
        assert_eq!(loaded.report.loaded, batch);
        assert_eq!(f.store().list().unwrap().len(), 16);
        for lang in ["en", "zh-Hans", "ja"] {
            let base = lexicon::builtin(lang).unwrap();
            let active = loaded.get(lang).unwrap();
            assert_eq!(base.readings(), &active.readings()[..base.readings().len()]);
            assert_eq!(
                serde_json::to_value(base.word_layers()).unwrap(),
                serde_json::to_value(&active.word_layers()[..base.word_layers().len()]).unwrap()
            );
            assert_eq!(
                base.continuations().collect::<Vec<_>>(),
                active
                    .continuations()
                    .take(base.continuations().count())
                    .collect::<Vec<_>>()
            );
        }
        if index == 0 {
            let before = fs::read(f.store().path()).unwrap();
            for (id, _) in NEW {
                assert_eq!(
                    f.store()
                        .install(find(&catalog, id).clone(), false)
                        .unwrap_err(),
                    "at most 16 packs may be installed"
                );
                assert_eq!(fs::read(f.store().path()).unwrap(), before);
                assert_eq!(f.store().load().report.loaded, batch);
            }
        }
    }
    assert_eq!(covered.len(), 22);
    assert_eq!(
        covered,
        catalog.iter().map(|p| p.manifest.id.as_str()).collect()
    );
}
