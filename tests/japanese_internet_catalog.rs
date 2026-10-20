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

fn remove_unique_append(source: &mut String, first: &str, after: &str) {
    assert_eq!(source.matches(first).count(), 1, "ambiguous append start");
    assert_eq!(source.matches(after).count(), 1, "ambiguous append end");
    let start = source.find(first).unwrap();
    let end = source.find(after).unwrap();
    assert!(start < end, "append range is empty or reversed");
    source.replace_range(start..end, "");
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
        let before_hygiene = builtin_before_daily_hygiene(language, source);
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
