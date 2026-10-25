//! Japanese catalog growth does not raise the sixteen-installed-pack limit.
#[path = "support/living_room_reading.rs"]
mod living_room_reading;
#[path = "support/offline_pack_sets.rs"]
mod offline_pack_sets;
#[path = "support/presence_resource.rs"]
mod presence_resource;
#[path = "support/release_storage_context.rs"]
mod release_storage_context;
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

fn remove_unique_append(source: &mut String, first: &str, after: &str) {
    assert_eq!(source.matches(first).count(), 1, "ambiguous append start");
    assert_eq!(source.matches(after).count(), 1, "ambiguous append end");
    let start = source.find(first).unwrap();
    let end = source.find(after).unwrap();
    assert!(start < end, "append range is empty or reversed");
    source.replace_range(start..end, "");
}

fn builtin_before_daily_progress(language: &str, source: &str) -> String {
    // Bind only this in-layer/tail append before the unchanged historical chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "899d6e92d29729e7ea31d5d3e4cda420f9c712381caf4dded030a9a99e4a7bd3"
            );
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 63);
            let layer = &layers[62];
            assert!(layer["words"].as_array().unwrap().is_empty());
            assert_eq!(layer["next_words"].as_array().unwrap().len(), 18);
            assert_eq!(layer["sentences"].as_array().unwrap().len(), 36);
            let layer_first = "\n    {\n      \"id\": \"daily_essentials_dialogue\",";
            assert_eq!(previous.matches(layer_first).count(), 1);
            let layer_start = previous.find(layer_first).unwrap();
            let layer_after = "\n    }\n  ],\n  \"phrase_endings\": [";
            let suffix = &previous[layer_start..];
            assert_eq!(suffix.matches(layer_after).count(), 1);
            let layer_end = layer_start + suffix.find(layer_after).unwrap();
            let mut layer = previous[layer_start..layer_end].to_owned();
            for (first, after) in [
                (
                    ",\n        [\n          \"the first part is\",",
                    "\n      ],\n      \"sentences\": [",
                ),
                (
                    ",\n        \"the first part is done, and i'm checking the rest.\"",
                    "\n      ]",
                ),
            ] {
                assert_eq!(layer.matches(first).count(), 1);
                let start = layer.find(first).unwrap();
                let suffix = &layer[start..];
                assert_eq!(suffix.matches(after).count(), 1);
                let end = start + suffix.find(after).unwrap();
                assert!(start < end);
                layer.replace_range(start..end, "");
            }
            previous.replace_range(layer_start..layer_end, &layer);
            "caba7d31753c4b472e404c40a3e0b82232f157c845bcdef69bc64f54982681c5"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "757614e5835d74559c81ea75feb2ab7841052bfb4761b83a6dd6b6ab27b09f15"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 906);
            remove_unique_append(&mut previous, ",\n    [\n      \"卡住\",", "\n  ]\n}");
            "2f2953249aba6680b4bb4b47da6a512a05eda177aa37d77234f541e052a27f22"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "68019e118f4422174fe6f6ba063129dba1ba70f87dd9e00a9c16e0f25b7d5269"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 394);
            remove_unique_append(
                &mut previous,
                ",\n    [\n      \"終わりました\",",
                "\n  ]\n}",
            );
            "df0e779bc24260a6bef3fcc39280b1d547ba1091cae93043918360a4fe0f1abe"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_clarity(language: &str, source: &str) -> String {
    // Bind only this in-layer/tail append before the unchanged historical chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "caba7d31753c4b472e404c40a3e0b82232f157c845bcdef69bc64f54982681c5"
            );
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 63);
            let layer = &layers[62];
            assert!(layer["words"].as_array().unwrap().is_empty());
            assert_eq!(layer["next_words"].as_array().unwrap().len(), 15);
            assert_eq!(layer["sentences"].as_array().unwrap().len(), 30);
            let layer_first = "\n    {\n      \"id\": \"daily_essentials_dialogue\",";
            assert_eq!(previous.matches(layer_first).count(), 1);
            let layer_start = previous.find(layer_first).unwrap();
            let layer_after = "\n    }\n  ],\n  \"phrase_endings\": [";
            let suffix = &previous[layer_start..];
            assert_eq!(suffix.matches(layer_after).count(), 1);
            let layer_end = layer_start + suffix.find(layer_after).unwrap();
            let mut layer = previous[layer_start..layer_end].to_owned();
            for (first, after) in [
                (
                    ",\n        [\n          \"before we start, i want to confirm\",",
                    "\n      ],\n      \"sentences\": [",
                ),
                (
                    ",\n        \"before we start, i want to confirm what you need.\"",
                    "\n      ]",
                ),
            ] {
                assert_eq!(layer.matches(first).count(), 1);
                let start = layer.find(first).unwrap();
                let suffix = &layer[start..];
                assert_eq!(suffix.matches(after).count(), 1);
                let end = start + suffix.find(after).unwrap();
                assert!(start < end);
                layer.replace_range(start..end, "");
            }
            previous.replace_range(layer_start..layer_end, &layer);
            "ad3fbdcaaadcb69a6d6056d82bc9f57af4cc3f89f140620a43f241205e5d3667"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "2f2953249aba6680b4bb4b47da6a512a05eda177aa37d77234f541e052a27f22"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 903);
            remove_unique_append(&mut previous, ",\n    [\n      \"分歧\",", "\n  ]\n}");
            "119bddcbce9d69a55e01038f52cfc566dd418997dcda248170f50e87f3e24f34"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "df0e779bc24260a6bef3fcc39280b1d547ba1091cae93043918360a4fe0f1abe"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 391);
            remove_unique_append(&mut previous, ",\n    [\n      \"必要\",", "\n  ]\n}");
            "083a8152b3e71594e60ac005d4e590d79cce815807582b6fea89480bb74ac693"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_coordination(language: &str, source: &str) -> String {
    // Bind only this in-layer/tail append before the unchanged historical chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "ad3fbdcaaadcb69a6d6056d82bc9f57af4cc3f89f140620a43f241205e5d3667"
            );
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 63);
            let layer = &layers[62];
            assert!(layer["words"].as_array().unwrap().is_empty());
            assert_eq!(layer["next_words"].as_array().unwrap().len(), 12);
            assert_eq!(layer["sentences"].as_array().unwrap().len(), 24);
            let layer_first = "\n    {\n      \"id\": \"daily_essentials_dialogue\",";
            assert_eq!(previous.matches(layer_first).count(), 1);
            let layer_start = previous.find(layer_first).unwrap();
            let layer_after = "\n    }\n  ],\n  \"phrase_endings\": [";
            let suffix = &previous[layer_start..];
            assert_eq!(suffix.matches(layer_after).count(), 1);
            let layer_end = layer_start + suffix.find(layer_after).unwrap();
            let mut layer = previous[layer_start..layer_end].to_owned();
            for (first, after) in [
                (
                    ",\n        [\n          \"give me a moment to\",",
                    "\n      ],\n      \"sentences\": [",
                ),
                (
                    ",\n        \"give me a moment to finish this message before we leave.\"",
                    "\n      ]",
                ),
            ] {
                assert_eq!(layer.matches(first).count(), 1);
                let start = layer.find(first).unwrap();
                let suffix = &layer[start..];
                assert_eq!(suffix.matches(after).count(), 1);
                let end = start + suffix.find(after).unwrap();
                assert!(start < end);
                layer.replace_range(start..end, "");
            }
            previous.replace_range(layer_start..layer_end, &layer);
            "7b83d83199a921f28b25a227ad150e98ef6e3ee8f60c5a2d5a0f084e7a39b44b"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "119bddcbce9d69a55e01038f52cfc566dd418997dcda248170f50e87f3e24f34"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 900);
            remove_unique_append(&mut previous, ",\n    [\n      \"碰头\",", "\n  ]\n}");
            "ed2e998006487fccdf70bcefe75e8980f26d2c21a4260e6da6676b887b3fe733"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "083a8152b3e71594e60ac005d4e590d79cce815807582b6fea89480bb74ac693"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 388);
            remove_unique_append(&mut previous, ",\n    [\n      \"予定変更\",", "\n  ]\n}");
            "537be7069924d76339c1f4ed64b09450510a6d57f5249bea1077654b5394962d"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_small_essentials(language: &str, source: &str) -> String {
    // The new bounded in-layer append precedes the unchanged personal/history chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "7b83d83199a921f28b25a227ad150e98ef6e3ee8f60c5a2d5a0f084e7a39b44b"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 63);
            let layer_first = "\n    {\n      \"id\": \"daily_essentials_dialogue\",";
            assert_eq!(previous.matches(layer_first).count(), 1);
            let layer_start = previous.find(layer_first).unwrap();
            let layer_after = "\n    }\n  ],\n  \"phrase_endings\": [";
            let suffix = &previous[layer_start..];
            assert_eq!(suffix.matches(layer_after).count(), 1);
            let layer_end = layer_start + suffix.find(layer_after).unwrap();
            let mut layer = previous[layer_start..layer_end].to_owned();
            for (first, after) in [
                (
                    ",\n        [\n          \"the comb is still in the\",",
                    "\n      ],\n      \"sentences\": [",
                ),
                (
                    ",\n        \"the comb is still in the drawer with my hairbrush.\"",
                    "\n      ]",
                ),
            ] {
                assert_eq!(layer.matches(first).count(), 1);
                let start = layer.find(first).unwrap();
                let suffix = &layer[start..];
                assert_eq!(suffix.matches(after).count(), 1);
                let end = start + suffix.find(after).unwrap();
                assert!(start < end);
                layer.replace_range(start..end, "");
            }
            previous.replace_range(layer_start..layer_end, &layer);
            "9ef6f80aedd0e1ffea70bc183abdb0d0bb551fc6808a80c1f92681bd141c3cb4"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "ed2e998006487fccdf70bcefe75e8980f26d2c21a4260e6da6676b887b3fe733"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 897);
            remove_unique_append(&mut previous, ",\n    [\n      \"雨伞\",", "\n  ]\n}");
            "dcc3f0a1c68ecea2e6c4c2e300611eecc3d89bef7e86e97552ce11f0737ca53d"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "537be7069924d76339c1f4ed64b09450510a6d57f5249bea1077654b5394962d"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 385);
            remove_unique_append(&mut previous, ",\n    [\n      \"日傘\",", "\n  ]\n}");
            "6b0e58f650657267c1bd5670a852fb1e23f1a6b93347c979ab80dcf600ea0282"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_personal_items(language: &str, source: &str) -> String {
    // Bind this exact append before the unchanged living-room/history chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "9ef6f80aedd0e1ffea70bc183abdb0d0bb551fc6808a80c1f92681bd141c3cb4"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 63);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_essentials_dialogue\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "cdcd49c76d7f7c7575dbe5d831a26c90a00db39f054d5c96ad71b792f5b73e5d"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "dcc3f0a1c68ecea2e6c4c2e300611eecc3d89bef7e86e97552ce11f0737ca53d"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 894);
            remove_unique_append(&mut previous, ",\n    [\n      \"梳子\",", "\n  ]\n}");
            "17cf01cc061faaea9f247a4713deb3cfaf3cc2048e9d14a2f64e04407c8d5a70"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "6b0e58f650657267c1bd5670a852fb1e23f1a6b93347c979ab80dcf600ea0282"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 382);
            remove_unique_append(&mut previous, ",\n    [\n      \"手袋\",", "\n  ]\n}");
            "be1c068f5cf7fc624099707fb95d283727fe43500cd8ff3ab8f556998f4cdcc9"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_living_room(language: &str, source: &str) -> String {
    // Bind this exact append, then enter the unchanged historical inverse chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = living_room_reading::without_living_room_reading(source);
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "cdcd49c76d7f7c7575dbe5d831a26c90a00db39f054d5c96ad71b792f5b73e5d"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 62);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_living_room\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "0093f628ea334d37d6649ab16cafaab4bc6041b310c4ab2785bd0481ccd5d640"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "17cf01cc061faaea9f247a4713deb3cfaf3cc2048e9d14a2f64e04407c8d5a70"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 891);
            remove_unique_append(&mut previous, ",\n    [\n      \"遥控器\",", "\n  ]\n}");
            "7e5e0e8cd2f223cd4bd8308399485477ca960980a1272c6fe2b31cec0fcaa490"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "be1c068f5cf7fc624099707fb95d283727fe43500cd8ff3ab8f556998f4cdcc9"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 379);
            remove_unique_append(&mut previous, ",\n    [\n      \"リモコン\",", "\n  ]\n}");
            "5b46a3322bbb39aba7285793189c04d3945945e33ca0405dd22eb8f84ef2f875"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_table_items(language: &str, source: &str) -> String {
    // Validate this raw append before entering the unchanged historical chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "0093f628ea334d37d6649ab16cafaab4bc6041b310c4ab2785bd0481ccd5d640"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 61);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_table_items\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "9731910e88a5a4d25f0d8b76b4387ceac9a26732a9c7dc4264c6e1b9c1eba16f"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "7e5e0e8cd2f223cd4bd8308399485477ca960980a1272c6fe2b31cec0fcaa490"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3441);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 888);
            remove_unique_append(&mut previous, ",\n    [\n      \"保温杯\",", "\n  ]\n}");
            "49706aca073e82be1c619c911a0177f9317050ffda8a7651ef63125c24e09592"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "5b46a3322bbb39aba7285793189c04d3945945e33ca0405dd22eb8f84ef2f875"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 376);
            remove_unique_append(&mut previous, ",\n    [\n      \"水筒\",", "\n  ]\n}");
            "119355914f5df3f2c73d2423f7e78c4ed833f81547b7ca09fcfafe7212a107ba"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_pocket_items(language: &str, source: &str) -> String {
    // Validate the newest raw append before entering the unchanged historic chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "9731910e88a5a4d25f0d8b76b4387ceac9a26732a9c7dc4264c6e1b9c1eba16f"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 60);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_pocket_items\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "cecf7b659558b2c0034a9031f75433d9452d43659a8cb73b07542784ffacbd4f"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "49706aca073e82be1c619c911a0177f9317050ffda8a7651ef63125c24e09592"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3441);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 885);
            remove_unique_append(&mut previous, ",\n    [\n      \"钱包\",", "\n  ]\n}");
            "bff559dfac7fe2ab2d4f1a00f0b79ef43c1e69ae8f418c6c552001a8964846b0"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "119355914f5df3f2c73d2423f7e78c4ed833f81547b7ca09fcfafe7212a107ba"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 373);
            remove_unique_append(&mut previous, ",\n    [\n      \"財布\",", "\n  ]\n}");
            "3d913e6ce519dc66781e990906028c549c8d8fac40258d2f3b62079cd41ac4fd"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_washroom(language: &str, source: &str) -> String {
    // Exact newest raw append inverse; every historical hash/wrapper stays intact.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "cecf7b659558b2c0034a9031f75433d9452d43659a8cb73b07542784ffacbd4f"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 59);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_washroom\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "643a76036fa82142288b188459efb70fd068bcb1bc4d738be2dae54ca789584a"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "bff559dfac7fe2ab2d4f1a00f0b79ef43c1e69ae8f418c6c552001a8964846b0"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3441);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 882);
            remove_unique_append(&mut previous, ",\n    [\n      \"毛巾\",", "\n  ]\n}");
            "3c27c55ce11194fe8a24f1e54b9b0affd6e2eb2bb3089dc2a529609ee9db8db6"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "3d913e6ce519dc66781e990906028c549c8d8fac40258d2f3b62079cd41ac4fd"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 370);
            remove_unique_append(&mut previous, ",\n    [\n      \"歯ブラシ\",", "\n  ]\n}");
            "77f1dc39cec5d0b959062bd04afc5656ac0d15c0864301345d4e9bd78ce54486"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected);
    previous
}

fn builtin_before_daily_entryway(language: &str, source: &str) -> String {
    // Verify this append, then replay the exact dirty pre-entryway bytes.
    // Keep every older inverse and historical hash unchanged.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "643a76036fa82142288b188459efb70fd068bcb1bc4d738be2dae54ca789584a"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 58);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_entryway\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "40392789063fa5e4e01ebe36291a9706623d10f4fc036d624186679119516ed0"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "3c27c55ce11194fe8a24f1e54b9b0affd6e2eb2bb3089dc2a529609ee9db8db6"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3441);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 879);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"xie'gui'li'hai'you'yi'shuang\", \"text\": \"鞋柜里还有一双\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                ",\n    [\n      \"鞋柜里还有一双\",",
                "\n  ]\n}",
            );
            "23f481f15542d626f6dde4e1948f230e9f40c213200adc5a58c1e429932dd026"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "77f1dc39cec5d0b959062bd04afc5656ac0d15c0864301345d4e9bd78ce54486"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 638);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 367);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"くつばこ\", \"text\": \"靴箱\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"靴箱\",", "\n  ]\n}");
            "14ca791a3de16c9c404e380bdcf3769892ad08d51f29835747b06475f1f8791f"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_accessories(language: &str, source: &str) -> String {
    // Verify this append, then replay the exact dirty pre-accessories bytes.
    // Keep every older inverse and historical hash unchanged.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "40392789063fa5e4e01ebe36291a9706623d10f4fc036d624186679119516ed0"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 57);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_accessories\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "9f144f170a3036ea97b95e7c3db56d28fbb1c56ca99ace2ed13024f04ac4e118"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "23f481f15542d626f6dde4e1948f230e9f40c213200adc5a58c1e429932dd026"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3432);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 873);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"cha'xian'ban'xian'fang'zhe'bian\", \"text\": \"插线板先放这边\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                ",\n    [\n      \"插线板先放这边\",",
                "\n  ]\n}",
            );
            "95263b4549158cafedb2b625467195021bd6837692ec1267eb4e2a08a0f5fcf8"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "14ca791a3de16c9c404e380bdcf3769892ad08d51f29835747b06475f1f8791f"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 633);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 361);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"えんちょうこーど\", \"text\": \"延長コード\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"延長コード\",", "\n  ]\n}");
            "d18a5541b5a337a9d1eb685d07c9d1b073372233836ed16957d0806254544ab3"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_recycling(language: &str, source: &str) -> String {
    // Verify the new append independently, then pass exact old bytes through
    // every unchanged historical inverse. No historical hash is refreshed.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "9f144f170a3036ea97b95e7c3db56d28fbb1c56ca99ace2ed13024f04ac4e118"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 56);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_recycling\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "7189fce8257ba5ca685f916483fd97b1b4fd4c745d2a17170878c3da64667e49"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "95263b4549158cafedb2b625467195021bd6837692ec1267eb4e2a08a0f5fcf8"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3422);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 867);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"zhi'xiang'xian'ya'ping\", \"text\": \"纸箱先压平\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"纸箱先压平\",", "\n  ]\n}");
            "f8ea73423ac9040d14c946a7f09fd6e96e98c569737a266227a06f7572fad553"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "d18a5541b5a337a9d1eb685d07c9d1b073372233836ed16957d0806254544ab3"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 628);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 355);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"しゅうしゅうび\", \"text\": \"収集日\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"収集日\",", "\n  ]\n}");
            "8d4c2f1d8a48eddcf046ec271808bf12441744428c666a48b504b6fd02290dd5"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_bedding(language: &str, source: &str) -> String {
    // Restore the exact kitchen resource before every unchanged older inverse.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "7189fce8257ba5ca685f916483fd97b1b4fd4c745d2a17170878c3da64667e49"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 55);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_bedding\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "b1ef05fa98967f14b2ccfecae03a19cf3f0fe33743638e9b8868793d11148de4"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "f8ea73423ac9040d14c946a7f09fd6e96e98c569737a266227a06f7572fad553"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3410);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 861);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"zhen'tou'fang'zai'chuang'tou\", \"text\": \"枕头放在床头\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                ",\n    [\n      \"枕头放在床头\",",
                "\n  ]\n}",
            );
            "0fb23eb192fed8e96523ec1af10a71ff10d05a3b4a327ac191fadbc66a6231eb"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "8d4c2f1d8a48eddcf046ec271808bf12441744428c666a48b504b6fd02290dd5"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 622);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 349);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"しーつ\", \"text\": \"シーツ\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"シーツ\",", "\n  ]\n}");
            "d9775473653d87e7726b931b5bde0bb579c694e927f14f64074d5356741df87b"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_kitchen(language: &str, source: &str) -> String {
    // Remove only this append, then run every unchanged historical inverse.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "b1ef05fa98967f14b2ccfecae03a19cf3f0fe33743638e9b8868793d11148de4"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 54);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_kitchen\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "3102776d198a40a6ae64829666c8ba74e457ef2c4fd57e134c9ac7c3b76705cc"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "0fb23eb192fed8e96523ec1af10a71ff10d05a3b4a327ac191fadbc66a6231eb"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3402);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 857);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"wo'xian'ba'cai'xi'hao\", \"text\": \"我先把菜洗好\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                ",\n    [\n      \"我先把菜洗好\",",
                "\n  ]\n}",
            );
            "1023a1295cd0180296ee0ff281f113b7e11ff5bfa9c52370152802de46988044"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "d9775473653d87e7726b931b5bde0bb579c694e927f14f64074d5356741df87b"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 619);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 345);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"すぷーん\", \"text\": \"スプーン\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"箸\",", "\n  ]\n}");
            "6b41345d2866fc199b5430e9a6776c6dc3ca4b2633597bcd7c0114745e106419"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_plants(language: &str, source: &str) -> String {
    // Restore only this plant append before the immutable reading and older epochs.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "3102776d198a40a6ae64829666c8ba74e457ef2c4fd57e134c9ac7c3b76705cc"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 53);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_plants\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "898583b85564981168cd53f86e3ec67e7b9a5befc312948edb5ae69eb66d7601"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "1023a1295cd0180296ee0ff281f113b7e11ff5bfa9c52370152802de46988044"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3390);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 851);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"zhe'pen'hua'xian'fang'chuang'bian\", \"text\": \"这盆花先放窗边\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                ",\n    [\n      \"这盆花先放窗边\",",
                "\n  ]\n}",
            );
            "0a0cbbbb4054f0a87e225ecbfdd483c094931fbf9d25a69efd09fa7f6ad7f960"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "6b41345d2866fc199b5430e9a6776c6dc3ca4b2633597bcd7c0114745e106419"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 614);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 339);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"うえきばち\", \"text\": \"植木鉢\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"植物\",", "\n  ]\n}");
            "ca50999a51fd3565733856502501c8399e86a144f5e8abed0ee5e0a2a1603b72"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_reading(language: &str, source: &str) -> String {
    // Restore the exact stationery epoch before every unchanged older golden.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "898583b85564981168cd53f86e3ec67e7b9a5befc312948edb5ae69eb66d7601"
            );
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 52);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_reading\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "b17162a23be3e3ccb51aeddf35badf7f1935c24900c83b505e8373b547193f22"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "0a0cbbbb4054f0a87e225ecbfdd483c094931fbf9d25a69efd09fa7f6ad7f960"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3378);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 845);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"wo'du'dao'zhe'yi'ye'le\", \"text\": \"我读到这一页了\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                ",\n    [\n      \"我读到这一页了\",",
                "\n  ]\n}",
            );
            "86276e05eecb1c6c9c3966812ed9efd33c113e801c3d21fd8bc9678463c9f863"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "ca50999a51fd3565733856502501c8399e86a144f5e8abed0ee5e0a2a1603b72"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 609);
            assert_eq!(current["continuations"].as_array().unwrap().len(), 333);
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"しおり\", \"text\": \"栞\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"栞\",", "\n  ]\n}");
            "98aaa74789eac7f5af52ea3fda659a2c2d59489c61f39ce295572fd6bbc629b8"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_stationery(language: &str, source: &str) -> String {
    // Validate only the finite stationery append, then restore the exact
    // actual dirty room baseline for every older helper and historical golden.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "b17162a23be3e3ccb51aeddf35badf7f1935c24900c83b505e8373b547193f22"
            );
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 51);
            assert_eq!(
                layers
                    .iter()
                    .map(|layer| layer["next_words"].as_array().map_or(0, Vec::len))
                    .sum::<usize>(),
                845
            );
            assert_eq!(
                layers
                    .iter()
                    .map(|layer| layer["sentences"].as_array().map_or(0, Vec::len))
                    .sum::<usize>(),
                1654
            );
            // Count canonical index forms, including sentence tokens; the raw
            // words-array occurrence count is not the English index length.
            let mut forms = BTreeSet::new();
            let mut insert = |word: &str| {
                forms.insert(word.replace('’', "'").to_ascii_lowercase());
            };
            for layer in layers {
                for word in layer["words"].as_array().into_iter().flatten() {
                    insert(word.as_str().unwrap());
                }
                for row in layer["next_words"].as_array().into_iter().flatten() {
                    for word in row[1].as_array().unwrap() {
                        insert(word.as_str().unwrap());
                    }
                }
                for sentence in layer["sentences"].as_array().into_iter().flatten() {
                    for word in sentence
                        .as_str()
                        .unwrap()
                        .split(|ch: char| !(ch.is_ascii_alphabetic() || matches!(ch, '\'' | '’')))
                        .filter(|word| !word.is_empty())
                    {
                        insert(word);
                    }
                }
            }
            assert_eq!(forms.len(), 6167);
            let expected: Value = serde_json::from_str(r#"[{"id":"daily_stationery","words":["pen","pens","pencil","pencils","eraser","ruler","sticky","ink"],"next_words":[["inside my pencil case i keep",["pens","pencils"]],["drawing a straight line is easier with a",["ruler","pencil"]],["the ink in this pen has",["run","left"]],["the sticky note next to my keyboard is",["for","from"]],["the eraser next to my notebook is",["small","clean"]],["once i've finished writing i'll put the",["pen","ruler"]]],"sentences":["inside my pencil case i keep pens for taking notes.","inside my pencil case i keep pencils for drawing small maps.","drawing a straight line is easier with a ruler on the paper.","drawing a straight line is easier with a pencil than with this pen.","the ink in this pen has run out, so i'll use a pencil.","the ink in this pen has left a mark on my notebook.","the sticky note next to my keyboard is for the shopping list.","the sticky note next to my keyboard is from our meeting this morning.","the eraser next to my notebook is small enough to fit in my pencil case.","the eraser next to my notebook is clean, so it won't leave a mark on the paper.","once i've finished writing i'll put the pen back in the drawer.","once i've finished writing i'll put the ruler next to my notebook."]}]"#).unwrap();
            assert_eq!(&layers[50..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_stationery\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "aead36583e5654d2bae6690ed3568beb9321b947d5d38b2df6e195acf4e7360d"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "86276e05eecb1c6c9c3966812ed9efd33c113e801c3d21fd8bc9678463c9f863"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3366);
            let expected: Value = serde_json::from_str(r#"[{"reading":"qian'bi'you'dian'dun'le","text":"铅笔有点钝了","kind":"word"},{"reading":"zhe'ge'zi'xie'cuo'le","text":"这个字写错了","kind":"word"},{"reading":"bang'wo'hua'yi'tiao'xian","text":"帮我画一条线","kind":"word"},{"reading":"xian'na'zhang'cao'gao'zhi","text":"先拿张草稿纸","kind":"word"},{"reading":"bi'mao'ji'de'gai'hao","text":"笔帽记得盖好","kind":"word"},{"reading":"zhe'ji'zhang'zhi'ding'yi'xia","text":"这几张纸订一下","kind":"word"},{"reading":"qian'bi","text":"铅笔","kind":"word"},{"reading":"chi'zi","text":"尺子","kind":"word"},{"reading":"jiao'dai","text":"胶带","kind":"word"},{"reading":"bian'qian","text":"便签","kind":"word"},{"reading":"cao'gao'zhi","text":"草稿纸","kind":"word"},{"reading":"yuan'zhu'bi","text":"圆珠笔","kind":"word"}]"#).unwrap();
            assert_eq!(
                &current["readings"].as_array().unwrap()[3354..],
                expected.as_array().unwrap()
            );
            let contexts = current["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 839);
            assert_eq!(
                contexts
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                1676
            );
            let expected: Value = serde_json::from_str(r#"[["铅笔有点钝了",["铅笔有点钝了，我先削一下再写。","铅笔有点钝了，换一支尖一点的吧。"]],["这个字写错了",["这个字写错了，我先擦掉再重写。","这个字写错了，在旁边改一下就好。"]],["帮我画一条线",["帮我画一条线，我想把这两段分开。","帮我画一条线，用尺子画会整齐一点。"]],["先拿张草稿纸",["先拿张草稿纸，我想算一下这道题。","先拿张草稿纸，这张先别放进文件夹。"]],["笔帽记得盖好",["笔帽记得盖好，我先收一下桌面。","笔帽记得盖好，用完就收回笔袋吧。"]],["这几张纸订一下",["这几张纸订一下，我想把它们放在一起。","这几张纸订一下，订好后放进文件夹吧。"]]]"#).unwrap();
            assert_eq!(&contexts[833..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"qian'bi'you'dian'dun'le\", \"text\": \"铅笔有点钝了\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                ",\n    [\n      \"铅笔有点钝了\",",
                "\n  ]\n}",
            );
            "d326f03db824f8261be30db48613d2427157da39375ce968ea3e5e6559bb08f6"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "98aaa74789eac7f5af52ea3fda659a2c2d59489c61f39ce295572fd6bbc629b8"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 605);
            let expected: Value = serde_json::from_str(r#"[{"reading":"ふせん","text":"付箋","kind":"word"},{"reading":"じょうぎ","text":"定規","kind":"word"},{"reading":"ふでばこ","text":"筆箱","kind":"word"},{"reading":"ほちきす","text":"ホチキス","kind":"word"}]"#).unwrap();
            assert_eq!(
                &current["readings"].as_array().unwrap()[601..],
                expected.as_array().unwrap()
            );
            let contexts = current["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 327);
            assert_eq!(
                contexts
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                653
            );
            let expected: Value = serde_json::from_str(r#"[["鉛筆",["鉛筆を一本貸してもらえますか。","鉛筆で下書きをしてから、ペンで書きます。"]],["消しゴム",["消しゴムでこの線を消してもいいですか。","消しゴムのかすを集めて捨てます。"]],["付箋",["付箋に書いて、机の端に貼っておきます。","付箋の色を変えて、目印にします。"]],["定規",["定規でまっすぐな線を引きます。","定規を使ったら、筆箱に戻します。"]],["筆箱",["筆箱に鉛筆と消しゴムを入れます。","筆箱を開けて、必要なものを取り出します。"]],["ホチキス",["ホチキスでこの紙を留めてもいいですか。","ホチキスの針がなくなったので、補充します。"]]]"#).unwrap();
            assert_eq!(&contexts[321..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"ふせん\", \"text\": \"付箋\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"鉛筆\",", "\n  ]\n}");
            "b35c0c25d97566e64fc49de92f1fef77368409ce1714b686bc69386da9e81c54"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_room_comfort(language: &str, source: &str) -> String {
    // Undo only this finite append into the actual dirty dressing baseline;
    // every older projection and golden continues to receive its own epoch.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(
                source_sha256(source),
                "aead36583e5654d2bae6690ed3568beb9321b947d5d38b2df6e195acf4e7360d"
            );
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 50);
            assert_eq!(
                layers
                    .iter()
                    .map(|layer| layer["next_words"].as_array().map_or(0, Vec::len))
                    .sum::<usize>(),
                839
            );
            assert_eq!(
                layers
                    .iter()
                    .map(|layer| layer["sentences"].as_array().map_or(0, Vec::len))
                    .sum::<usize>(),
                1642
            );
            let expected: Value = serde_json::from_str(r#"[{"id":"daily_room_comfort","words":["curtains","lamp","lamps","fan","fans","sunlight","lighting","stuffy"],"next_words":[["the curtains by my desk are",["open","closed"]],["the glare on my screen makes",["reading","work"]],["the air in this room feels",["fresh","stuffy"]],["this part of the room feels",["warm","cool"]],["without more light i can't",["read","see"]],["the lighting near my chair is",["bright","dim"]]],"sentences":["the curtains by my desk are open to let the sunlight in.","the curtains by my desk are closed to keep the room cool.","the glare on my screen makes reading difficult at this desk.","the glare on my screen makes work difficult in the afternoon.","the air in this room feels fresh with the window open.","the air in this room feels stuffy, so i'll open the window for a while.","this part of the room feels warm even with the fan on.","this part of the room feels cool enough to sit here for a while.","without more light i can't read the small print under this lamp.","without more light i can't see where i put my keys.","the lighting near my chair is bright enough for reading.","the lighting near my chair is dim, so i'll sit next to the window."]}]"#).unwrap();
            assert_eq!(&layers[49..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_room_comfort\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "5aea3238d903d97f16a85d8faa8c4ec4d803dbf539e0cd660c1fd008527be900"
        }
        "zh-Hans" => {
            assert_eq!(
                source_sha256(source),
                "d326f03db824f8261be30db48613d2427157da39375ce968ea3e5e6559bb08f6"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 3354);
            let expected: Value = serde_json::from_str(r#"[{"reading":"fang'jian'you'dian'men","text":"房间有点闷","kind":"word"},{"reading":"kai'chuang'tong'tong'feng","text":"开窗通通风","kind":"word"},{"reading":"ba'chuang'lian'la'kai","text":"把窗帘拉开","kind":"word"},{"reading":"deng'guang'you'dian'ci'yan","text":"灯光有点刺眼","kind":"word"},{"reading":"feng'shan'tiao'xiao'yi'dian","text":"风扇调小一点","kind":"word"},{"reading":"kong'tiao'wen'du'tiao'gao'yi'dian","text":"空调温度调高一点","kind":"word"},{"reading":"guang'xian","text":"光线","kind":"word"},{"reading":"tong'feng","text":"通风","kind":"word"},{"reading":"tai'deng","text":"台灯","kind":"word"},{"reading":"feng'shan","text":"风扇","kind":"word"},{"reading":"kong'tiao","text":"空调","kind":"word"},{"reading":"wen'du","text":"温度","kind":"word"}]"#).unwrap();
            assert_eq!(
                &current["readings"].as_array().unwrap()[3342..],
                expected.as_array().unwrap()
            );
            let contexts = current["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 833);
            assert_eq!(
                contexts
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                1664
            );
            let expected: Value = serde_json::from_str(r#"[["房间有点闷",["房间有点闷，我想开一会儿窗。","房间有点闷，我们换个位置坐吧。"]],["开窗通通风",["开窗通通风，我先把桌上的纸压好。","开窗通通风，等会儿再关上。"]],["把窗帘拉开",["把窗帘拉开，我想让房间亮一点。","把窗帘拉开，看看外面的天气。"]],["灯光有点刺眼",["灯光有点刺眼，我想换个位置坐。","灯光有点刺眼，能不能调暗一点？"]],["风扇调小一点",["风扇调小一点，我这边风有点大。","风扇调小一点，别对着桌上的纸吹。"]],["空调温度调高一点",["空调温度调高一点，我觉得有点冷。","空调温度调高一点，先这样试一会儿吧。"]]]"#).unwrap();
            assert_eq!(&contexts[827..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"fang'jian'you'dian'men\", \"text\": \"房间有点闷\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"房间有点闷\",", "\n  ]\n}");
            "93c98ed1543cde1fe141c2c7c53e712ff464398672b892a77913a6beed74c369"
        }
        "ja" => {
            assert_eq!(
                source_sha256(source),
                "b35c0c25d97566e64fc49de92f1fef77368409ce1714b686bc69386da9e81c54"
            );
            assert_eq!(current["readings"].as_array().unwrap().len(), 601);
            let expected: Value = serde_json::from_str(r#"[{"reading":"しょうめい","text":"照明","kind":"word"},{"reading":"かーてん","text":"カーテン","kind":"word"},{"reading":"かぜとおし","text":"風通し","kind":"word"},{"reading":"しつおん","text":"室温","kind":"word"},{"reading":"えあこん","text":"エアコン","kind":"word"}]"#).unwrap();
            assert_eq!(
                &current["readings"].as_array().unwrap()[596..],
                expected.as_array().unwrap()
            );
            let contexts = current["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 321);
            assert_eq!(
                contexts
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                641
            );
            let expected: Value = serde_json::from_str(r#"[["照明",["照明を少し暗くしてもいいですか。","照明をつけると、手元がよく見えます。"]],["カーテン",["カーテンを開けると、部屋が明るくなります。","カーテンを閉めてから着替えます。"]],["風通し",["風通しをよくするために、窓を少し開けます。","風通しがよくて、気持ちがいいですね。"]],["室温",["室温を確かめてから、エアコンを調節します。","室温がちょうどよくて、過ごしやすいです。"]],["エアコン",["エアコンの風が直接当たらない席に移ります。","エアコンを少し弱くしてもいいですか。"]],["換気扇",["換気扇を回して、こもった空気を外に出します。","換気扇を止めてから部屋を出ます。"]]]"#).unwrap();
            assert_eq!(&contexts[315..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"しょうめい\", \"text\": \"照明\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"照明\",", "\n  ]\n}");
            "5152ebc14bec074c52a9428617cca4b2b29966a0c7943b05f4f49a8d1cd1f4bf"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_dressing(language: &str, source: &str) -> String {
    // Validate this finite append before returning the exact 6edfa3e resource
    // bytes to the unchanged hygiene and older raw-byte projection chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 49);
            assert_eq!(
                layers
                    .iter()
                    .map(|layer| layer["next_words"].as_array().map_or(0, Vec::len))
                    .sum::<usize>(),
                833
            );
            assert_eq!(
                layers
                    .iter()
                    .map(|layer| layer["sentences"].as_array().map_or(0, Vec::len))
                    .sum::<usize>(),
                1630
            );
            let expected: Value = serde_json::from_str(r#"[{"id":"daily_dressing","words":[],"next_words":[["my shirt feels",["tight","loose"]],["the sleeves on this shirt are",["long","short"]],["the zipper on my jacket is",["stuck","open"]],["before leaving i'll adjust my",["belt","scarf"]],["my glasses are inside the",["case","bag"]],["i'll change into a",["shirt","sweater"]]],"sentences":["my shirt feels tight when i sit down.","my shirt feels loose, so i'll try a smaller one.","the sleeves on this shirt are long, so i'll fold them back.","the sleeves on this shirt are short, so i'll wear a jacket over it.","the zipper on my jacket is stuck near the bottom.","the zipper on my jacket is open, so i'll close it before we leave.","before leaving i'll adjust my belt so it fits better.","before leaving i'll adjust my scarf to keep it in place.","my glasses are inside the case on the shelf.","my glasses are inside the bag next to my notebook.","i'll change into a shirt before we go out.","i'll change into a sweater because it's cold outside."]}]"#).unwrap();
            assert_eq!(&layers[48..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_dressing\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "a9d60a7fa10d505c28485555e580c9d6996b6007ea8db3aa71d020d645265ed4"
        }
        "zh-Hans" => {
            assert_eq!(current["readings"].as_array().unwrap().len(), 3342);
            let expected: Value = serde_json::from_str(r#"[{"reading":"la'lian'ka'zhu'le","text":"拉链卡住了","kind":"word"},{"reading":"niu'kou'diao'le","text":"纽扣掉了","kind":"word"},{"reading":"xiu'zi'you'dian'duan","text":"袖子有点短","kind":"word"},{"reading":"xie'dai'you'song'le","text":"鞋带又松了","kind":"word"},{"reading":"ku'yao'you'dian'jin","text":"裤腰有点紧","kind":"word"},{"reading":"ling'kou'you'dian'da","text":"领口有点大","kind":"word"},{"reading":"ku'yao","text":"裤腰","kind":"word"},{"reading":"ling'kou","text":"领口","kind":"word"},{"reading":"yao'dai","text":"腰带","kind":"word"},{"reading":"song'jin'dai","text":"松紧带","kind":"word"},{"reading":"yi'jiao","text":"衣角","kind":"word"},{"reading":"xiu'chang","text":"袖长","kind":"word"}]"#).unwrap();
            assert_eq!(
                &current["readings"].as_array().unwrap()[3330..],
                expected.as_array().unwrap()
            );
            let contexts = current["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 827);
            assert_eq!(
                contexts
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                1652
            );
            let expected: Value = serde_json::from_str(r#"[["拉链卡住了",["拉链卡住了，我先看看有没有夹到衣角。","拉链卡住了，先别用力拉。"]],["纽扣掉了",["纽扣掉了，我先把它收好。","纽扣掉了，这件衣服先放一边。"]],["袖子有点短",["袖子有点短，我再换一件试试。","袖子有点短，抬手的时候不太方便。"]],["鞋带又松了",["鞋带又松了，我先重新系一下。","鞋带又松了，等我一下再走。"]],["裤腰有点紧",["裤腰有点紧，我想换一条试试。","裤腰有点紧，坐下来不太舒服。"]],["领口有点大",["领口有点大，我换一件比较一下。","领口有点大，这件先放回去吧。"]]]"#).unwrap();
            assert_eq!(&contexts[821..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"la'lian'ka'zhu'le\", \"text\": \"拉链卡住了\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"拉链卡住了\",", "\n  ]\n}");
            "bf58670c119b6fac15095a2fc07623d87069e67d6f26e7bb3a84ef3cb0c06bab"
        }
        "ja" => {
            assert_eq!(current["readings"].as_array().unwrap().len(), 596);
            let expected: Value = serde_json::from_str(r#"[{"reading":"ふぁすなー","text":"ファスナー","kind":"word"},{"reading":"ぼたん","text":"ボタン","kind":"word"},{"reading":"くつひも","text":"靴ひも","kind":"word"},{"reading":"きつい","text":"きつい","kind":"word"}]"#).unwrap();
            assert_eq!(
                &current["readings"].as_array().unwrap()[592..],
                expected.as_array().unwrap()
            );
            let contexts = current["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 315);
            assert_eq!(
                contexts
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                629
            );
            let expected: Value = serde_json::from_str(r#"[["ファスナー",["ファスナーを最後まで閉めます。","ファスナーが引っかかったので、ゆっくり動かします。"]],["ボタン",["ボタンを留めてから出かけます。","ボタンが一つ取れてしまいました。"]],["袖",["袖を少しまくってから洗い物をします。","袖が長いので、少し折り返します。"]],["靴ひも",["靴ひもを結び直してから歩きます。","靴ひもがほどけたので、少し待ってください。"]],["ポケット",["ポケットに鍵を入れておきます。","ポケットの中を確認してから洗濯します。"]],["きつい",["きつい服は動きにくいです。","きついので、一つ大きいサイズを試します。"]]]"#).unwrap();
            assert_eq!(&contexts[309..], expected.as_array().unwrap());
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"ふぁすなー\", \"text\": \"ファスナー\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"ファスナー\",", "\n  ]\n}");
            "6ce3c061e37c5f0d34fafd90c28639599cf8e2e474492eeb83107b16e6167f28"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_hygiene(language: &str, source: &str) -> String {
    // Invert only the exact frozen hygiene append. All earlier raw-byte
    // projections and independent hashes continue receiving their own epoch.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            assert_eq!(current["word_layers"].as_array().unwrap().len(), 48);
            let expected: Value = serde_json::from_str(r#"[{"id":"daily_hygiene","words":["paper","soap","shampoo","toothpaste","tissue","tissues","conditioner","mouthwash"],"next_words":[["could you bring me some",["paper","tissues"]],["we've run out of",["soap","toothpaste"]],["i've left the",["shampoo","conditioner"]],["could you hand me a",["tissue","towel"]],["there's another bottle of",["shampoo","mouthwash"]],["please check if we need",["toothpaste","conditioner"]]],"sentences":["could you bring me some paper from the desk?","could you bring me some tissues from the bathroom?","we've run out of soap by the sink.","we've run out of toothpaste so i'll add it to the list.","i've left the shampoo on the bathroom shelf.","i've left the conditioner next to the shampoo.","could you hand me a tissue from that box?","could you hand me a towel from the shelf?","there's another bottle of shampoo in the cupboard.","there's another bottle of mouthwash on the bathroom shelf.","please check if we need toothpaste before you go out.","please check if we need conditioner before buying more."]}]"#).unwrap();
            assert_eq!(
                &current["word_layers"].as_array().unwrap()[47..],
                expected.as_array().unwrap()
            );
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_hygiene\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "a35baeabf35f5eebd793e01db0b40824bf2027a837af6c2a3cb560e8b156a8ad"
        }
        "zh-Hans" => {
            assert_eq!(current["readings"].as_array().unwrap().len(), 3330);
            let expected: Value = serde_json::from_str(r#"[{"reading":"zhi'zhang","text":"纸张","kind":"word"},{"reading":"hu'fa'su","text":"护发素","kind":"word"},{"reading":"hu'shou'shuang","text":"护手霜","kind":"word"},{"reading":"run'chun'gao","text":"润唇膏","kind":"word"},{"reading":"mian'jin'zhi","text":"面巾纸","kind":"word"},{"reading":"ca'shou'zhi","text":"擦手纸","kind":"word"},{"reading":"fei'zao'he","text":"肥皂盒","kind":"word"},{"reading":"ti'huan'shua'tou","text":"替换刷头","kind":"word"}]"#).unwrap();
            assert_eq!(
                &current["readings"].as_array().unwrap()[3322..],
                expected.as_array().unwrap()
            );
            assert_eq!(current["continuations"].as_array().unwrap().len(), 821);
            let expected: Value = serde_json::from_str(r#"[["纸张",["纸张放在打印机旁边了。","纸张还有一些，我先拿几张过来。"]],["香皂",["香皂放在洗手台旁边。","香皂还有一块，先不用再找了。"]],["洗发水",["洗发水在浴室的架子上。","洗发水我已经放进洗漱包了。"]],["护发素",["护发素和洗发水分开放着。","护发素还有一瓶没开封。"]],["牙膏",["牙膏我放在牙刷旁边了。","牙膏快用完了，我先记在清单上。"]],["牙刷",["牙刷放在右边的杯子里。","牙刷我已经装进洗漱包了。"]]]"#).unwrap();
            assert_eq!(
                &current["continuations"].as_array().unwrap()[815..],
                expected.as_array().unwrap()
            );
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"zhi'zhang\", \"text\": \"纸张\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"纸张\",", "\n  ]\n}");
            "bf387e09a3da44fefd21748856441154dcfe55f57aa5652b50707cf47c3eeff3"
        }
        "ja" => {
            assert_eq!(current["readings"].as_array().unwrap().len(), 592);
            let expected: Value = serde_json::from_str(r#"[{"reading":"しゃんぷー","text":"シャンプー","kind":"word"},{"reading":"りんす","text":"リンス","kind":"word"},{"reading":"はんどそーぷ","text":"ハンドソープ","kind":"word"},{"reading":"へあぶらし","text":"ヘアブラシ","kind":"word"},{"reading":"そーぷけーす","text":"ソープケース","kind":"word"},{"reading":"せんがんりょう","text":"洗顔料","kind":"word"}]"#).unwrap();
            assert_eq!(
                &current["readings"].as_array().unwrap()[586..],
                expected.as_array().unwrap()
            );
            assert_eq!(current["continuations"].as_array().unwrap().len(), 309);
            let expected: Value = serde_json::from_str(r#"[["シャンプー",["シャンプーの詰め替えはありますか。","シャンプーが少なくなったので、買い物のメモに書いておきます。"]],["リンス",["リンスはどこに置きましたか。","リンスを買う前に、家にある分を確認します。"]],["ハンドソープ",["ハンドソープの残りを確認します。","ハンドソープは洗面台の横にあります。"]],["ヘアブラシ",["ヘアブラシはどこに置きましたか。","ヘアブラシを洗面所に戻しておきます。"]],["ソープケース",["ソープケースを洗っておきます。","ソープケースは棚の上にあります。"]],["洗顔料",["洗顔料を買い物のメモに追加します。","洗顔料の予備はまだあります。"]]]"#).unwrap();
            assert_eq!(
                &current["continuations"].as_array().unwrap()[303..],
                expected.as_array().unwrap()
            );
            remove_unique_append(
                &mut previous,
                ",\n    {\"reading\": \"しゃんぷー\", \"text\": \"シャンプー\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, ",\n    [\n      \"シャンプー\",", "\n  ]\n}");
            "45b217176c7c36f056c84e04a9f2419d2237c2c55ce5ef1deed6ab9b8f38f07b"
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_restock(language: &str, source: &str) -> String {
    // Project only this reviewed append back to the exact dirty-storage before-image. Keep every
    // older raw-byte projection and its independent golden below unchanged.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 47);
            assert_eq!(layers[46]["id"], "daily_restock");
            assert!(layers[46]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[46]["next_words"].as_array().unwrap().len(), 6);
            assert_eq!(layers[46]["sentences"].as_array().unwrap().len(), 12);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_restock\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "31e3d7105a593e2d4c4c2b7dfe894e02660101822f98c30151d46abe9760b2a9"
        }
        "zh-Hans" | "ja" => {
            let (readings, contexts, sentences, first_reading, first_context, sha) =
                if language == "zh-Hans" {
                    (
                        3322,
                        815,
                        1628,
                        "zhi'jin'zhi'sheng'yi'bao'le",
                        "纸巾只剩一包了",
                        "b589210dd52d35d8569aa6bc85461a1eb6919592665bab75c169b0366a4773c5",
                    )
                } else {
                    (
                        586,
                        303,
                        605,
                        "てぃっしゅ",
                        "ティッシュ",
                        "5df031dfd68c605a6912a81557aaf915eda30b8793b52c1fb1b39beaed3f0855",
                    )
                };
            assert_eq!(current["readings"].as_array().unwrap().len(), readings);
            let values = current["continuations"].as_array().unwrap();
            assert_eq!(values.len(), contexts);
            assert_eq!(
                values
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentences,
            );
            remove_unique_append(
                &mut previous,
                &format!(
                    ",\n    {{\"reading\": \"{first_reading}\", \"text\": \"{first_context}\", \"kind\": \"word\"}}"
                ),
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    [\n      \"{first_context}\","),
                "\n  ]\n}",
            );
            sha
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_storage(language: &str, source: &str) -> String {
    // Project only this reviewed append back to the exact dirty-call before-image. Keep every
    // older raw-byte projection and its independent golden below unchanged.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 46);
            assert_eq!(layers[45]["id"], "daily_storage");
            assert!(layers[45]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[45]["next_words"].as_array().unwrap().len(), 6);
            assert_eq!(layers[45]["sentences"].as_array().unwrap().len(), 12);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_storage\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "a6e83271b75ee86f79b022580bf7f0c627b8976a653ea4b6c278df4bd3eeaf77"
        }
        "zh-Hans" | "ja" => {
            let (readings, contexts, sentences, first_reading, first_context, sha) =
                if language == "zh-Hans" {
                    (
                        3304,
                        809,
                        1616,
                        "xian'teng'dian'di'fang'chu'lai",
                        "先腾点地方出来",
                        "1d149fe0400e324216b289a490bd55d6bcb08679d68a52b85acd17db93f65949",
                    )
                } else {
                    (
                        580,
                        297,
                        593,
                        "しゅうのう",
                        "収納",
                        "31b0c5646a7076124f73a411de5fc056b342d0177fea857d59ef2b3d9fd9f40d",
                    )
                };
            assert_eq!(current["readings"].as_array().unwrap().len(), readings);
            let values = current["continuations"].as_array().unwrap();
            assert_eq!(values.len(), contexts);
            assert_eq!(
                values
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentences,
            );
            remove_unique_append(
                &mut previous,
                &format!(
                    ",\n    {{\"reading\": \"{first_reading}\", \"text\": \"{first_context}\", \"kind\": \"word\"}}"
                ),
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    [\n      \"{first_context}\","),
                "\n  ]\n}",
            );
            sha
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_call(language: &str, source: &str) -> String {
    // Project only this reviewed append back to the exact dirty-route before-image. Keep every
    // older raw-byte projection and its independent golden below unchanged.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 45);
            assert_eq!(layers[44]["id"], "daily_call");
            assert!(layers[44]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[44]["next_words"].as_array().unwrap().len(), 6);
            assert_eq!(layers[44]["sentences"].as_array().unwrap().len(), 12);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_call\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "b176189ecf8ac299e8b6bb506493ef234b87e260e773bfebfa8bd00ee57e72e5"
        }
        "zh-Hans" | "ja" => {
            let (readings, contexts, sentences, first_reading, first_context, sha) =
                if language == "zh-Hans" {
                    (
                        3286,
                        803,
                        1604,
                        "ni'de'hua'mian'ka'zhu'le",
                        "你的画面卡住了",
                        "bde0584a65c2db839f96a5d29e0cca287eda256298d1a1eab5015de0811cc5d9",
                    )
                } else {
                    (
                        574,
                        291,
                        581,
                        "きこえにくい",
                        "聞こえにくい",
                        "ddb75d90d6b83309f54ee3ebdb8cc49496f91dcd89a8c27776efd7cacb19a431",
                    )
                };
            assert_eq!(current["readings"].as_array().unwrap().len(), readings);
            let values = current["continuations"].as_array().unwrap();
            assert_eq!(values.len(), contexts);
            assert_eq!(
                values
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentences,
            );
            remove_unique_append(
                &mut previous,
                &format!(
                    ",\n    {{\"reading\": \"{first_reading}\", \"text\": \"{first_context}\", \"kind\": \"word\"}}"
                ),
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    [\n      \"{first_context}\","),
                "\n  ]\n}",
            );
            sha
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_route(language: &str, source: &str) -> String {
    // Project only this reviewed append back to clean 4108bc0. Keep every
    // older raw-byte projection and its independent golden below unchanged.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 44);
            assert_eq!(layers[43]["id"], "daily_route");
            assert!(layers[43]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[43]["next_words"].as_array().unwrap().len(), 8);
            assert_eq!(layers[43]["sentences"].as_array().unwrap().len(), 16);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_route\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "32a45d063e3ca3e4cb9a5c27501d14771329a3a3d1ab338218c1a88dd9b02597"
        }
        "zh-Hans" | "ja" => {
            let (readings, contexts, sentences, first_reading, first_context, sha) =
                if language == "zh-Hans" {
                    (
                        3268,
                        797,
                        1592,
                        "wo'gang'shang'che",
                        "我刚上车",
                        "3f09409d89c99b773c5ff8fb746aa5efd7e0bc5942cb9e6e62db5c43923da988",
                    )
                } else {
                    (
                        568,
                        285,
                        569,
                        "まちあわせばしょ",
                        "待ち合わせ場所",
                        "f827bc88bccd03509696a728ff369c4530dd602f593091308276135a3e742b56",
                    )
                };
            assert_eq!(current["readings"].as_array().unwrap().len(), readings);
            let values = current["continuations"].as_array().unwrap();
            assert_eq!(values.len(), contexts);
            assert_eq!(
                values
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentences,
            );
            remove_unique_append(
                &mut previous,
                &format!(
                    ",\n    {{\"reading\": \"{first_reading}\", \"text\": \"{first_context}\", \"kind\": \"word\"}}"
                ),
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    [\n      \"{first_context}\","),
                "\n  ]\n}",
            );
            sha
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_aftercare(language: &str, source: &str) -> String {
    // Restore the exact prior working-tree resources, including messaging and appointments,
    // before entering the unchanged historical raw-byte projection chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 43);
            assert_eq!(layers[42]["id"], "daily_aftercare");
            assert!(layers[42]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[42]["next_words"].as_array().unwrap().len(), 6);
            assert_eq!(layers[42]["sentences"].as_array().unwrap().len(), 12);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_aftercare\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "2dea601ac30fe1f80072e6147f873b422e21372674da854904425934a14bfc03"
        }
        "zh-Hans" | "ja" => {
            let (readings, contexts, sentences, first_reading, first_context, sha) =
                if language == "zh-Hans" {
                    (
                        3244,
                        789,
                        1576,
                        "zhe'ge'shou'dao'shi'jiu'huai'le",
                        "这个收到时就坏了",
                        "83c673262924d5e13b67b1d7f2a4770e056b61bd3a7e63eb4cdc9beef80c7f00",
                    )
                } else {
                    (
                        556,
                        279,
                        557,
                        "ほしょうしょ",
                        "保証書",
                        "040a15ec2176bda35796e3d9849afb631644325b49511e0cfb137e26d3333570",
                    )
                };
            assert_eq!(current["readings"].as_array().unwrap().len(), readings);
            let values = current["continuations"].as_array().unwrap();
            assert_eq!(values.len(), contexts);
            assert_eq!(
                values
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentences
            );
            remove_unique_append(
                &mut previous,
                &format!(
                    ",\n    {{\"reading\": \"{first_reading}\", \"text\": \"{first_context}\", \"kind\": \"word\"}}"
                ),
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    [\n      \"{first_context}\","),
                "\n  ]\n}",
            );
            sha
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_appointments(language: &str, source: &str) -> String {
    // Restore the exact prior working-tree resources, including messaging,
    // before entering the unchanged historical raw-byte projection chain.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 42);
            assert_eq!(layers[41]["id"], "daily_appointments");
            assert!(layers[41]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[41]["next_words"].as_array().unwrap().len(), 8);
            assert_eq!(layers[41]["sentences"].as_array().unwrap().len(), 16);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_appointments\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "873ed5d19d8ea7074fc145850f1826ea3982d8b3bc1255c4d6fa6933bbd0a5fd"
        }
        "zh-Hans" | "ja" => {
            let (readings, contexts, sentences, first_reading, first_context, sha) =
                if language == "zh-Hans" {
                    (
                        3226,
                        783,
                        1564,
                        "xu'yao'ti'qian'yu'yue'ma",
                        "需要提前预约吗",
                        "46f2f6845b4a1f31c8d3fb0bbb4cbd8168b87776e9e41e258db55a56c4a78f86",
                    )
                } else {
                    (
                        546,
                        274,
                        547,
                        "よやくへんこう",
                        "予約変更",
                        "b03caa4651c054067eeca896b9f09746cd5d1c9d905a7d48a18c4b715ee25736",
                    )
                };
            assert_eq!(current["readings"].as_array().unwrap().len(), readings);
            let values = current["continuations"].as_array().unwrap();
            assert_eq!(values.len(), contexts);
            assert_eq!(
                values
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentences
            );
            remove_unique_append(
                &mut previous,
                &format!(
                    ",\n    {{\"reading\": \"{first_reading}\", \"text\": \"{first_context}\", \"kind\": \"word\"}}"
                ),
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    [\n      \"{first_context}\","),
                "\n  ]\n}",
            );
            sha
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_messaging(language: &str, source: &str) -> String {
    // Remove only this verified append. The entire older projection/hash chain
    // below still receives its exact released 0.9.1 bytes, not new goldens.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 41);
            assert_eq!(layers[40]["id"], "daily_messaging");
            assert!(layers[40]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[40]["next_words"].as_array().unwrap().len(), 8);
            assert_eq!(layers[40]["sentences"].as_array().unwrap().len(), 16);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_messaging\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "09515ef76dc1cd24cef8c8171f2b8058bb11501877fd1e9fc54bf32854fc2e14"
        }
        "zh-Hans" | "ja" => {
            let (readings, contexts, sentences, first_reading, first_context, sha) =
                if language == "zh-Hans" {
                    (
                        3202,
                        775,
                        1548,
                        "gang'kan'dao'ni'de'xiao'xi",
                        "刚看到你的消息",
                        "043376c6f148bdd4a4c5dee691c1337bc6819c182e76b9c1a5cd36e0bc96c23a",
                    )
                } else {
                    (
                        534,
                        268,
                        535,
                        "めっせーじ",
                        "メッセージ",
                        "740c452b36eac1c87cd6af083e0bff6aadaac02dd9a0abd9078168c43165d194",
                    )
                };
            assert_eq!(current["readings"].as_array().unwrap().len(), readings);
            let values = current["continuations"].as_array().unwrap();
            assert_eq!(values.len(), contexts);
            assert_eq!(
                values
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentences
            );
            remove_unique_append(
                &mut previous,
                &format!(
                    ",\n    {{\"reading\": \"{first_reading}\", \"text\": \"{first_context}\", \"kind\": \"word\"}}"
                ),
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    [\n      \"{first_context}\","),
                "\n  ]\n}",
            );
            sha
        }
        _ => unreachable!(),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_reference_characters(language: &str, source: &str) -> String {
    if language != "zh-Hans" {
        return source.to_owned();
    }
    let current: Value = serde_json::from_str(source).unwrap();
    let readings = current["readings"].as_array().unwrap();
    assert_eq!(readings.len(), 3178);
    for (entry, (reading, text)) in readings[3176..].iter().zip([("zhe", "这"), ("ben", "本")]) {
        assert_eq!(entry["reading"], reading);
        assert_eq!(entry["text"], text);
        assert_eq!(entry["kind"], "word");
    }
    assert_eq!(current["continuations"].as_array().unwrap().len(), 767);
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"zhe\", \"text\": \"这\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    assert_eq!(
        source_sha256(&previous),
        "a829f6ae589bc865225513e4dc5c5ede957bbb39307e7a9432973f13b254ead0"
    );
    previous
}

fn builtin_before_arrival_characters(language: &str, source: &str) -> String {
    if language != "zh-Hans" {
        return source.to_owned();
    }
    let current: Value = serde_json::from_str(source).unwrap();
    let readings = current["readings"].as_array().unwrap();
    assert_eq!(readings.len(), 3176);
    for (entry, (reading, text)) in readings[3174..].iter().zip([("dao", "到"), ("jia", "家")]) {
        assert_eq!(entry["reading"], reading);
        assert_eq!(entry["text"], text);
        assert_eq!(entry["kind"], "word");
    }
    assert_eq!(current["continuations"].as_array().unwrap().len(), 767);
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"dao\", \"text\": \"到\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    assert_eq!(
        source_sha256(&previous),
        "927b8940451a0e2d585ed35b8acd3634c8ea21d3e7162d0e06c79a4340c28ea4"
    );
    previous
}

fn builtin_before_result_characters(language: &str, source: &str) -> String {
    if language != "zh-Hans" {
        return source.to_owned();
    }
    let current: Value = serde_json::from_str(source).unwrap();
    let readings = current["readings"].as_array().unwrap();
    assert_eq!(readings.len(), 3174);
    for (entry, (reading, text)) in
        readings[3170..]
            .iter()
            .zip([("guo", "过"), ("wan", "完"), ("hui", "回"), ("zhao", "找")])
    {
        assert_eq!(entry["reading"], reading);
        assert_eq!(entry["text"], text);
        assert_eq!(entry["kind"], "word");
    }
    assert_eq!(current["continuations"].as_array().unwrap().len(), 767);
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"guo\", \"text\": \"过\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    assert_eq!(
        source_sha256(&previous),
        "5e5bf32538490c988b7116bf3dfafcf0c4e63228ea2a235e95c96d13db72949f"
    );
    previous
}

fn builtin_before_action_characters(language: &str, source: &str) -> String {
    if language != "zh-Hans" {
        return source.to_owned();
    }
    let current: Value = serde_json::from_str(source).unwrap();
    let readings = current["readings"].as_array().unwrap();
    assert_eq!(readings.len(), 3170);
    for (entry, (reading, text)) in
        readings[3166..]
            .iter()
            .zip([("na", "拿"), ("fang", "放"), ("dai", "带"), ("yong", "用")])
    {
        assert_eq!(entry["reading"], reading);
        assert_eq!(entry["text"], text);
        assert_eq!(entry["kind"], "word");
    }
    assert_eq!(current["continuations"].as_array().unwrap().len(), 767);
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"na\", \"text\": \"拿\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    assert_eq!(
        source_sha256(&previous),
        "96852c00402fc799dda6d273c2b1bb26443d75899ee7177edc55b661c6fd5836"
    );
    previous
}

fn builtin_before_giving_characters(language: &str, source: &str) -> String {
    if language != "zh-Hans" {
        return source.to_owned();
    }
    let current: Value = serde_json::from_str(source).unwrap();
    let readings = current["readings"].as_array().unwrap();
    assert_eq!(readings.len(), 3166);
    for (entry, (reading, text)) in readings[3164..].iter().zip([("gei", "给"), ("shu", "书")]) {
        assert_eq!(entry["reading"], reading);
        assert_eq!(entry["text"], text);
        assert_eq!(entry["kind"], "word");
    }
    assert_eq!(current["continuations"].as_array().unwrap().len(), 767);
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"gei\", \"text\": \"给\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    assert_eq!(
        source_sha256(&previous),
        "49fd40d87d1fff86290249d47b883106178da9a4a8b1630f76a174ab33c2dd14"
    );
    previous
}

fn builtin_before_lending_characters(language: &str, source: &str) -> String {
    if language != "zh-Hans" {
        return source.to_owned();
    }
    let current: Value = serde_json::from_str(source).unwrap();
    let readings = current["readings"].as_array().unwrap();
    assert_eq!(readings.len(), 3164);
    for (entry, (reading, text)) in readings[3162..].iter().zip([("jie", "借"), ("huan", "还")]) {
        assert_eq!(entry["reading"], reading);
        assert_eq!(entry["text"], text);
        assert_eq!(entry["kind"], "word");
    }
    assert_eq!(current["continuations"].as_array().unwrap().len(), 767);
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"jie\", \"text\": \"借\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    assert_eq!(
        source_sha256(&previous),
        "5856488e8461b851560e0c75cf815d536c5cbff3dd277dcd7bc2d0d00a25b1c9"
    );
    previous
}

fn builtin_before_daily_borrowing(language: &str, source: &str) -> String {
    // Restore the exact previously validated, uncommitted belongings snapshot.
    // Every older projection below continues receiving its original raw bytes.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 40);
            assert_eq!(layers[38]["id"], "daily_belongings");
            assert_eq!(layers[39]["id"], "daily_borrowing");
            assert!(layers[39]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[39]["next_words"].as_array().unwrap().len(), 8);
            assert_eq!(layers[39]["sentences"].as_array().unwrap().len(), 16);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_borrowing\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "a82974d60229dcee5751bf7400e7054349412fad9b119895e818690c1360cfee"
        }
        "zh-Hans" | "ja" => {
            let (readings, contexts, first, first_reading, last, sentences, sha) =
                if language == "zh-Hans" {
                    (
                        3162,
                        767,
                        "这个能借我两天吗",
                        "zhe'ge'neng'jie'wo'liang'tian'ma",
                        "借书卡",
                        1532,
                        "4bc27922b35d1df2aae11cc693e116588428e6e408e3669f2df78ba9fb3e14fc",
                    )
                } else {
                    (
                        522,
                        262,
                        "返却期限",
                        "へんきゃくきげん",
                        "貸出カード",
                        523,
                        "b177c38d3e7afc78b0059b9397436ece66bf8f22d2c9864fb087837819069ac5",
                    )
                };
            let rows = current["readings"].as_array().unwrap();
            let continuation = current["continuations"].as_array().unwrap();
            let added_contexts = if language == "ja" { 6 } else { 8 };
            let added_readings = if language == "ja" { 12 } else { 24 };
            assert_eq!(rows.len(), readings);
            assert_eq!(continuation.len(), contexts);
            assert_eq!(rows[readings - added_readings]["text"], first);
            assert_eq!(rows.last().unwrap()["text"], last);
            assert_eq!(continuation[contexts - added_contexts][0], first);
            assert!(
                continuation[contexts - added_contexts..]
                    .iter()
                    .all(|row| row[1].as_array().unwrap().len() == 2)
            );
            assert_eq!(
                continuation
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentences
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    {{\"reading\": \"{first_reading}\","),
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(
                &mut previous,
                &format!(",\n    [\n      \"{first}\","),
                "\n  ]\n}\n",
            );
            sha
        }
        _ => panic!("unexpected builtin language"),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_belongings(language: &str, source: &str) -> String {
    // Peel only this batch. The exact ef0e920 bytes then enter the unchanged
    // sharing and older projections; no historical fingerprint is refreshed.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 39);
            assert_eq!(layers[37]["id"], "daily_sharing");
            assert_eq!(layers[38]["id"], "daily_belongings");
            assert!(layers[38]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[38]["next_words"].as_array().unwrap().len(), 8);
            assert_eq!(layers[38]["sentences"].as_array().unwrap().len(), 16);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_belongings\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "0cfbbf740bf2bb2906e0ff469283492d5b9b8072a60182c61ed8260585999af3"
        }
        "zh-Hans" | "ja" => {
            let (
                old_readings,
                old_contexts,
                added_readings,
                added_contexts,
                first_word,
                last_word,
                first_append,
                first_context_append,
                sentence_count,
                sha,
            ) = if language == "zh-Hans" {
                (
                    3114,
                    751,
                    24,
                    8,
                    "钥匙忘在家里了",
                    "领取登记",
                    ",\n    {\"reading\": \"yao'shi'wang'zai'jia'li'le\",",
                    ",\n    [\n      \"钥匙忘在家里了\",",
                    1516,
                    "495b0bb85bd4da7459aee2e52f4af5bf55d9ba4776f97b31cdde946c59f0fb08",
                )
            } else {
                (
                    498,
                    250,
                    12,
                    6,
                    "持ち主",
                    "パスケース",
                    ",\n    {\"reading\": \"もちぬし\",",
                    ",\n    [\n      \"持ち主\",",
                    511,
                    "34dd6b8cb4422e06571828105e7d73307d1e6bc6352763df65a2c3d016ea7f91",
                )
            };
            let readings = current["readings"].as_array().unwrap();
            let contexts = current["continuations"].as_array().unwrap();
            assert_eq!(readings.len(), old_readings + added_readings);
            assert_eq!(contexts.len(), old_contexts + added_contexts);
            assert_eq!(readings[old_readings]["text"], first_word);
            assert_eq!(readings.last().unwrap()["text"], last_word);
            assert_eq!(contexts[old_contexts][0], first_word);
            assert!(
                contexts[old_contexts..]
                    .iter()
                    .all(|row| row[1].as_array().unwrap().len() == 2)
            );
            assert_eq!(
                contexts
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                sentence_count
            );
            remove_unique_append(
                &mut previous,
                first_append,
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, first_context_append, "\n  ]\n}\n");
            sha
        }
        _ => panic!("unexpected builtin language"),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_daily_sharing(language: &str, source: &str) -> String {
    // Peel only this exact append before passing untouched historical bytes to
    // the older projections. These fingerprints were fixed at the 0.9.0 source.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    let expected_sha = match language {
        "en" => {
            let layers = current["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 38);
            assert_eq!(layers[36]["id"], "daily_devices");
            assert_eq!(layers[37]["id"], "daily_sharing");
            assert!(layers[37]["words"].as_array().unwrap().is_empty());
            assert_eq!(layers[37]["next_words"].as_array().unwrap().len(), 8);
            assert_eq!(layers[37]["sentences"].as_array().unwrap().len(), 16);
            remove_unique_append(
                &mut previous,
                ",\n    {\n      \"id\": \"daily_sharing\",",
                "\n  ],\n  \"phrase_endings\": [",
            );
            "28cdfb440cb057e99cb077fd04ee25827bc31bd7b1e2834901b4ff0911c50d86"
        }
        "zh-Hans" | "ja" => {
            let (
                old_readings,
                old_contexts,
                added_readings,
                added_contexts,
                first_word,
                first_context,
                last_word,
                expected_sentences,
                first_append,
                first_context_append,
                sha,
            ) = if language == "zh-Hans" {
                (
                    3090,
                    743,
                    24,
                    8,
                    "照片发原图吧",
                    "照片发原图吧",
                    "修改记录",
                    1500,
                    ",\n    {\"reading\": \"zhao'pian'fa'yuan'tu'ba\",",
                    ",\n    [\n      \"照片发原图吧\",",
                    "e1983c1917ad5e1130697bc8a56b5c99c0306347a17dc024bfd11c2f5f499c07",
                )
            } else {
                (
                    486,
                    244,
                    12,
                    6,
                    "ファイル名",
                    "拡張子",
                    "解像度",
                    499,
                    ",\n    {\"reading\": \"ふぁいるめい\",",
                    ",\n    [\n      \"拡張子\",",
                    "08b39a5dceeafe14ee82f70b2d23b0cbae208dd52631fe242fbf5e2db2b87c38",
                )
            };
            let readings = current["readings"].as_array().unwrap();
            let contexts = current["continuations"].as_array().unwrap();
            assert_eq!(readings.len(), old_readings + added_readings);
            assert_eq!(contexts.len(), old_contexts + added_contexts);
            assert_eq!(readings[old_readings]["text"], first_word);
            assert_eq!(readings.last().unwrap()["text"], last_word);
            assert_eq!(contexts[old_contexts][0], first_context);
            assert!(
                contexts[old_contexts..]
                    .iter()
                    .all(|row| row[1].as_array().unwrap().len() == 2)
            );
            assert_eq!(
                contexts
                    .iter()
                    .map(|row| row[1].as_array().unwrap().len())
                    .sum::<usize>(),
                expected_sentences
            );
            remove_unique_append(
                &mut previous,
                first_append,
                "\n  ],\n  \"continuations\": [",
            );
            remove_unique_append(&mut previous, first_context_append, "\n  ]\n}\n");
            sha
        }
        _ => panic!("unexpected builtin language"),
    };
    assert_eq!(source_sha256(&previous), expected_sha);
    previous
}

fn builtin_before_japanese_cleaning(language: &str, source: &str) -> String {
    if language != "ja" {
        return source.to_owned();
    }
    // Remove only the fixed cleaning/waste-sorting append. Keep the exact prior
    // bytes for the independent weather, errands and devices projections below;
    // neither their historical windows nor their SHA fingerprints are refreshed.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], "ja");
    let readings = current["readings"].as_array().unwrap();
    let contexts = current["continuations"].as_array().unwrap();
    assert_eq!(readings.len(), 486);
    assert_eq!(contexts.len(), 244);
    assert_eq!(readings[461]["text"], "保管");
    assert_eq!(readings[462]["reading"], "そうじき");
    assert_eq!(readings[462]["text"], "掃除機");
    assert_eq!(readings[485]["reading"], "あきかん");
    assert_eq!(readings[485]["text"], "空き缶");
    assert_eq!(contexts[231][0], "置き場所");
    assert_eq!(contexts[232][0], "掃除機");
    assert_eq!(contexts[243][0], "床");
    for context in &contexts[232..] {
        assert_eq!(context[1].as_array().unwrap().len(), 2);
    }
    assert_eq!(
        contexts
            .iter()
            .map(|context| context[1].as_array().unwrap().len())
            .sum::<usize>(),
        487
    );
    for last in [
        "{\"reading\": \"あきかん\", \"text\": \"空き缶\", \"kind\": \"word\"}",
        "[\n      \"床\",",
    ] {
        assert_eq!(source.matches(last).count(), 1, "ambiguous append tail");
    }
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"そうじき\", \"text\": \"掃除機\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    remove_unique_append(&mut previous, ",\n    [\n      \"掃除機\",", "\n  ]\n}\n");
    let parsed: Value = serde_json::from_str(&previous).unwrap();
    assert_eq!(parsed["readings"].as_array().unwrap().len(), 462);
    let contexts = parsed["continuations"].as_array().unwrap();
    assert_eq!(contexts.len(), 232);
    assert_eq!(
        contexts
            .iter()
            .map(|context| context[1].as_array().unwrap().len())
            .sum::<usize>(),
        463
    );
    previous
}

fn builtin_before_japanese_weather(language: &str, source: &str) -> String {
    if language != "ja" {
        return source.to_owned();
    }
    // Remove only this fixed weather/storage append before applying the unchanged
    // errands and devices projections. The new suite freezes its exact contents;
    // all previous byte fingerprints and append-window checks remain independent.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], "ja");
    let readings = current["readings"].as_array().unwrap();
    let contexts = current["continuations"].as_array().unwrap();
    assert_eq!(readings.len(), 462);
    assert_eq!(contexts.len(), 232);
    assert_eq!(readings[437]["text"], "宅配ボックス");
    assert_eq!(readings[438]["reading"], "てんきよほう");
    assert_eq!(readings[438]["text"], "天気予報");
    assert_eq!(readings[461]["reading"], "ほかん");
    assert_eq!(readings[461]["text"], "保管");
    assert_eq!(contexts[219][0], "宅配ボックス");
    assert_eq!(contexts[220][0], "天気予報");
    assert_eq!(contexts[231][0], "置き場所");
    for context in &contexts[220..] {
        assert_eq!(context[1].as_array().unwrap().len(), 2);
    }
    assert_eq!(
        contexts
            .iter()
            .map(|context| context[1].as_array().unwrap().len())
            .sum::<usize>(),
        463
    );
    for last in [
        "{\"reading\": \"ほかん\", \"text\": \"保管\", \"kind\": \"word\"}",
        "[\n      \"置き場所\",",
    ] {
        assert_eq!(source.matches(last).count(), 1, "ambiguous append tail");
    }
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"てんきよほう\", \"text\": \"天気予報\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    remove_unique_append(&mut previous, ",\n    [\n      \"天気予報\",", "\n  ]\n}\n");
    let parsed: Value = serde_json::from_str(&previous).unwrap();
    assert_eq!(parsed["readings"].as_array().unwrap().len(), 438);
    let contexts = parsed["continuations"].as_array().unwrap();
    assert_eq!(contexts.len(), 220);
    assert_eq!(
        contexts
            .iter()
            .map(|context| context[1].as_array().unwrap().len())
            .sum::<usize>(),
        439
    );
    previous
}

fn builtin_before_japanese_errands(language: &str, source: &str) -> String {
    if language != "ja" {
        return source.to_owned();
    }
    // Strip only this fixed visits/deliveries append, whose content and window
    // are frozen independently. Feed the exact prior bytes into the unchanged
    // daily_devices projection, retaining all older SHA and order assertions.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], "ja");
    let readings = current["readings"].as_array().unwrap();
    let contexts = current["continuations"].as_array().unwrap();
    assert_eq!(readings.len(), 438);
    assert_eq!(contexts.len(), 220);
    assert_eq!(readings[413]["text"], "音声");
    assert_eq!(readings[414]["reading"], "らいきゃく");
    assert_eq!(readings[414]["text"], "来客");
    assert_eq!(readings[437]["reading"], "たくはいぼっくす");
    assert_eq!(readings[437]["text"], "宅配ボックス");
    assert_eq!(contexts[207][0], "明るさ");
    assert_eq!(contexts[208][0], "来客");
    assert_eq!(contexts[219][0], "宅配ボックス");
    for context in &contexts[208..] {
        assert_eq!(context[1].as_array().unwrap().len(), 2);
    }
    assert_eq!(
        contexts
            .iter()
            .map(|context| context[1].as_array().unwrap().len())
            .sum::<usize>(),
        439
    );
    for last in [
        "{\"reading\": \"たくはいぼっくす\", \"text\": \"宅配ボックス\", \"kind\": \"word\"}",
        "[\n      \"宅配ボックス\",",
    ] {
        assert_eq!(source.matches(last).count(), 1, "ambiguous append tail");
    }
    let mut previous = source.to_owned();
    remove_unique_append(
        &mut previous,
        ",\n    {\"reading\": \"らいきゃく\", \"text\": \"来客\", \"kind\": \"word\"}",
        "\n  ],\n  \"continuations\": [",
    );
    remove_unique_append(&mut previous, ",\n    [\n      \"来客\",", "\n  ]\n}\n");
    let parsed: Value = serde_json::from_str(&previous).unwrap();
    assert_eq!(parsed["readings"].as_array().unwrap().len(), 414);
    assert_eq!(parsed["continuations"].as_array().unwrap().len(), 208);
    previous
}

fn builtin_before_daily_devices(language: &str, source: &str) -> String {
    // The current builtins legitimately append the daily_devices batch. Its exact
    // contents/windows are independently frozen in tests/daily_devices_{english,
    // chinese,japanese}.rs. Project out ONLY those explicit append ranges here:
    // the historical SHA below still checks every original byte and its order,
    // including formatting, rather than silently refreshing an old fingerprint.
    let current: Value = serde_json::from_str(source).unwrap();
    assert_eq!(current["language"], language);
    let mut previous = source.to_owned();
    if language == "en" {
        let layers = current["word_layers"].as_array().unwrap();
        assert_eq!(layers.len(), 37);
        assert_eq!(layers[35]["id"], "daily_laundry");
        assert_eq!(layers[36]["id"], "daily_devices");
        assert!(layers[36]["words"].as_array().unwrap().is_empty());
        assert_eq!(layers[36]["next_words"].as_array().unwrap().len(), 8);
        assert_eq!(layers[36]["sentences"].as_array().unwrap().len(), 16);
        remove_unique_append(
            &mut previous,
            ",\n    {\n      \"id\": \"daily_devices\",",
            "\n  ],\n  \"phrase_endings\": [",
        );
        let parsed: Value = serde_json::from_str(&previous).unwrap();
        assert_eq!(parsed["word_layers"].as_array().unwrap().len(), 36);
    } else {
        let (
            readings_before,
            contexts_before,
            added_readings,
            added_contexts,
            first_reading,
            first_context,
            last_word,
            last_context,
        ) = match language {
            "zh-Hans" => (
                3066,
                735,
                24,
                8,
                ",\n    {\"reading\": \"shou'ji'kuai'mei'dian'le\", \"text\": \"手机快没电了\", \"kind\": \"word\"}",
                ",\n    [\n      \"手机快没电了\",",
                "铃声音量",
                "耳机放盒里了",
            ),
            "ja" => (
                402,
                202,
                12,
                6,
                ",\n    {\"reading\": \"じゅうでんき\", \"text\": \"充電器\", \"kind\": \"word\"}",
                ",\n    [\n      \"充電器\",",
                "音声",
                "明るさ",
            ),
            _ => panic!("no historical projection for {language}"),
        };
        let readings = current["readings"].as_array().unwrap();
        let contexts = current["continuations"].as_array().unwrap();
        assert_eq!(readings.len(), readings_before + added_readings);
        assert_eq!(contexts.len(), contexts_before + added_contexts);
        assert_eq!(readings.last().unwrap()["text"], last_word);
        assert_eq!(contexts.last().unwrap()[0], last_context);
        for context in &contexts[contexts_before..] {
            assert_eq!(context[1].as_array().unwrap().len(), 2);
        }
        remove_unique_append(
            &mut previous,
            first_reading,
            "\n  ],\n  \"continuations\": [",
        );
        remove_unique_append(&mut previous, first_context, "\n  ]\n}\n");
        let parsed: Value = serde_json::from_str(&previous).unwrap();
        assert_eq!(
            parsed["readings"].as_array().unwrap().len(),
            readings_before
        );
        assert_eq!(
            parsed["continuations"].as_array().unwrap().len(),
            contexts_before
        );
    }
    previous
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
        let before_presence = presence_resource::without_presence(source);
        let before_release_contexts =
            release_storage_context::restore_pre_release_contexts(&before_presence);
        let before_progress = builtin_before_daily_progress(language, &before_release_contexts);
        let before_clarity = builtin_before_daily_clarity(language, &before_progress);
        let before_coordination = builtin_before_daily_coordination(language, &before_clarity);
        let before_small_essentials =
            builtin_before_daily_small_essentials(language, &before_coordination);
        let before_personal_items =
            builtin_before_daily_personal_items(language, &before_small_essentials);
        let before_living_room = builtin_before_daily_living_room(language, &before_personal_items);
        let before_table_items = builtin_before_daily_table_items(language, &before_living_room);
        let before_pocket_items = builtin_before_daily_pocket_items(language, &before_table_items);
        let before_washroom = builtin_before_daily_washroom(language, &before_pocket_items);
        let before_entryway = builtin_before_daily_entryway(language, &before_washroom);
        let before_accessories = builtin_before_daily_accessories(language, &before_entryway);
        let before_recycling = builtin_before_daily_recycling(language, &before_accessories);
        let before_bedding = builtin_before_daily_bedding(language, &before_recycling);
        let before_kitchen = builtin_before_daily_kitchen(language, &before_bedding);
        let before_plants = builtin_before_daily_plants(language, &before_kitchen);
        let before_reading = builtin_before_daily_reading(language, &before_plants);
        let before_stationery = builtin_before_daily_stationery(language, &before_reading);
        let before_room = builtin_before_daily_room_comfort(language, &before_stationery);
        let before_dressing = builtin_before_daily_dressing(language, &before_room);
        let before_hygiene = builtin_before_daily_hygiene(language, &before_dressing);
        let before_restock = builtin_before_daily_restock(language, &before_hygiene);
        let before_storage = builtin_before_daily_storage(language, &before_restock);
        let before_call = builtin_before_daily_call(language, &before_storage);
        let before_route = builtin_before_daily_route(language, &before_call);
        let before_aftercare = builtin_before_daily_aftercare(language, &before_route);
        let before_appointments = builtin_before_daily_appointments(language, &before_aftercare);
        let before_messaging = builtin_before_daily_messaging(language, &before_appointments);
        let before_references = builtin_before_reference_characters(language, &before_messaging);
        let before_arrivals = builtin_before_arrival_characters(language, &before_references);
        let before_results = builtin_before_result_characters(language, &before_arrivals);
        let before_actions = builtin_before_action_characters(language, &before_results);
        let before_giving = builtin_before_giving_characters(language, &before_actions);
        let before_characters = builtin_before_lending_characters(language, &before_giving);
        let before_borrowing = builtin_before_daily_borrowing(language, &before_characters);
        let before_belongings = builtin_before_daily_belongings(language, &before_borrowing);
        let before_sharing = builtin_before_daily_sharing(language, &before_belongings);
        let before_cleaning = builtin_before_japanese_cleaning(language, &before_sharing);
        let before_weather = builtin_before_japanese_weather(language, &before_cleaning);
        let before_errands = builtin_before_japanese_errands(language, &before_weather);
        assert_eq!(
            source_sha256(&builtin_before_daily_devices(language, &before_errands)),
            expected,
            "changed historical builtin bytes before daily_devices: {language}"
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
