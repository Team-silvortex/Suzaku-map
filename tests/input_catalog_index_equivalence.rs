//! Actual dirty-restock before-index replay: all metadata, score bits and order.
//! Public synthetic drafts only; no native keys, user store or model requests.
#[path = "support/daily_reading_replay.rs"]
mod daily_reading_replay;

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

// Room-comfort data fills one formerly unused twelfth backfill slot.
// Assert every field before removing that one new Word for the historical
// optimization replay. No existing row, score, corpus or golden is rewritten;
// the nine-pack pool was already full and remains byte-for-byte unchanged.
fn before_daily_room_comfort(
    language: &str,
    seed: &str,
    current: &[Candidate],
    with_packs: bool,
) -> Vec<Candidate> {
    if language != "ja" || seed != "nihongo wo benkyou shit" || with_packs {
        return current.to_vec();
    }
    let expected = [
        ("日本語を勉強しt", "unknown", 1120403456),
        ("にほんごをべんきょうしt", "unknown", 1119354880),
        ("日本語を勉強失礼します", "word", 1119158272),
        ("日本語を勉強質問", "word", 1119092736),
        ("日本語を勉強したいです。", "sentence", 1117323264),
        ("日本語を勉強しています。", "sentence", 1117257728),
        ("ニホンゴヲベンキョウシt", "unknown", 1119092736),
        ("日本語を勉強失礼しました", "word", 1119027200),
        ("日本語を勉強試着", "word", 1118961664),
        ("nihongo wo benkyou shit", "literal", 1113325568),
        ("日本語を勉強湿度", "word", 1113194496),
        ("日本語を勉強室温", "word", 1113063424),
    ];
    assert_eq!(
        current
            .iter()
            .map(|c| (
                c.text.as_str(),
                c.label.as_str(),
                c.kind.id(),
                c.source.id(),
                c.score.to_bits(),
            ))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|&(text, kind, bits)| (text, text, kind, "local", bits))
            .collect::<Vec<_>>(),
        "the only accepted daily_room_comfort delta must match in full"
    );
    current[..11].to_vec()
}

fn fingerprint(
    language: &str,
    ime: &mut XRTabletImeEngine,
    seeds: &[String],
    with_packs: bool,
) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for seed in seeds {
        ime.seed(seed);
        for byte in seed.bytes().chain([0]) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        let before_reading = daily_reading_replay::before_daily_reading(
            language,
            seed,
            ime.candidates(),
            with_packs,
        );
        let before_room = before_daily_room_comfort(language, seed, &before_reading, with_packs);
        for candidate in &before_room {
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
    ime::{Candidate, EngineConfig, XRTabletImeEngine},
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

fn replay(language: &str, count: usize, expected: u64, with_packs: bool) {
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
        fingerprint(language, &mut ime, &seeds, with_packs),
        expected,
        "{language} complete replay changed"
    );
}

#[test]
fn catalog_index_english_preserves_130_actual_before_vectors() {
    replay("en", 130, 0xc9b9b260b8fc9d84, false);
}
#[test]
fn catalog_index_chinese_preserves_107_actual_before_vectors() {
    replay("zh-Hans", 107, 0xd58c7555c30542b0, false);
}
#[test]
fn catalog_index_japanese_preserves_91_actual_before_vectors() {
    replay("ja", 91, 0x2a50212ca577d9e9, false);
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
    replay("ja", 91, 0x737eeb6ed7b4da16, true);
}
