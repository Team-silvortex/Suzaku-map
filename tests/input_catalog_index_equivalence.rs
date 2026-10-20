//! Actual dirty-restock before-index replay: all metadata, score bits and order.
//! Public synthetic drafts only; no native keys, user store or model requests.
fn corpus(language: &str) -> Vec<String> {
    let phrases: &[&str] = match language {
        "en" => &[
            "i still have the receipt",
            "please let me know the address",
            "i've taken photos of the damage",
            "could you check the opening hours",
        ],
        "zh-Hans" => &[
            "gou mai ping zheng zen me ti gong",
            "shou hou jin du bang wo cha yi xia",
            "na guo lai",
            "zhe ben shu gei ni",
        ],
        "ja" => &[
            "nihongo wo benkyou shite",
            "hoshousho",
            "henkinn",
            "seirikenn",
            "kyouyu",
            "jyunbi gade ki",
            "konnpouzai",
        ],
        _ => unreachable!(),
    };
    let mut seeds = Vec::new();
    for phrase in phrases {
        for end in phrase.char_indices().map(|(i, ch)| i + ch.len_utf8()) {
            seeds.push(phrase[..end].to_owned());
        }
    }
    for seed in [
        "",
        "https://example.invalid/hoshousho",
        "abc_xyz",
        "xyzunknown",
        "one\nline",
        "わからない",
        "包装",
        "保证書",
        "i",
        "p",
        "c",
    ] {
        seeds.push(seed.to_owned());
    }
    seeds.push(format!("{} {}", "既存の文章🙂 ".repeat(40), phrases[0]));
    seeds
}

fn fingerprint(ime: &mut XRTabletImeEngine, seeds: &[String]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for seed in seeds {
        ime.seed(seed);
        for byte in seed.bytes().chain([0]) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        for candidate in ime.candidates() {
            for byte in candidate
                .text
                .bytes()
                .chain([0])
                .chain(candidate.label.bytes())
                .chain([0])
                .chain(candidate.kind.id().bytes())
                .chain([0])
                .chain(candidate.source.id().bytes())
                .chain([0])
                .chain(candidate.score.to_bits().to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
    }
    hash
}
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{EngineConfig, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

fn engine(language: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime
}

fn replay(language: &str, count: usize, expected: u64) {
    let seeds = corpus(language);
    assert_eq!(seeds.len(), count);
    let mut ime = engine(language);
    for seed in &seeds {
        ime.seed(seed);
        assert_eq!(ime.snapshot().seed_text, *seed);
        assert!(ime.snapshot().committed_text.is_empty());
        assert!(ime.candidates().len() <= 12);
        assert_eq!(
            ime.candidates().len(),
            ime.candidates()
                .iter()
                .map(|c| &c.text)
                .collect::<HashSet<_>>()
                .len()
        );
    }
    assert_eq!(
        fingerprint(&mut ime, &seeds),
        expected,
        "{language} complete replay changed"
    );
}

#[test]
fn catalog_index_english_preserves_130_actual_before_vectors() {
    replay("en", 130, 0xc9b9b260b8fc9d84);
}
#[test]
fn catalog_index_chinese_preserves_107_actual_before_vectors() {
    replay("zh-Hans", 107, 0xd58c7555c30542b0);
}
#[test]
fn catalog_index_japanese_preserves_91_actual_before_vectors() {
    replay("ja", 91, 0x2a50212ca577d9e9);
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn catalog_index_japanese_with_nine_packs_preserves_91_actual_before_vectors() {
    const NAME: &str = "catalog_index_japanese_with_nine_packs_preserves_91_actual_before_vectors";
    const CHILD: &str = "SUZAKU_CATALOG_INDEX_CHILD";
    if std::env::var(CHILD).as_deref() != Ok(NAME) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
            .env(CHILD, NAME)
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("running 1 test"), "{stdout}");
        assert!(stdout.contains(&format!("test {NAME} ... ok")), "{stdout}");
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{stdout}"
        );
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "suzaku-catalog-index-equivalence-{}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    let fixture = Fixture(path);
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    let catalog = packs::recommended();
    let ids = [
        "org.suzaku.ja.rail",
        "org.suzaku.ja.family",
        "org.suzaku.ja.internet",
        "org.suzaku.ja.shopping",
        "org.suzaku.ja.travel",
        "org.suzaku.ja.work",
        "org.suzaku.ja.study",
        "org.suzaku.ja.cooking",
        "org.suzaku.ja.outdoors",
    ];
    for id in ids {
        let pack = catalog.iter().find(|p| p.manifest.id == id).unwrap();
        store.install(pack.clone(), false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), ids.len());
    for id in ids {
        assert!(report.loaded.iter().any(|loaded| loaded == id));
    }
    replay("ja", 91, 0x737eeb6ed7b4da16);
}
