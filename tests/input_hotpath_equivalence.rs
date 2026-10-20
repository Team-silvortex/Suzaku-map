//! Actual pre-optimization release replay: all metadata, score bits and order.
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

// Keep the optimization goldens independent of this reviewed vocabulary append.
// Check the complete new vector first; only then invert its one new Word and
// the two existing position-based sentence score changes. No other seed or
// metadata is projected, and the original corpus and four goldens stay fixed.
fn before_daily_route(language: &str, seed: &str, current: &[Candidate]) -> Vec<Candidate> {
    if language != "ja" || seed != "nihongo wo be" {
        return current.to_vec();
    }
    let expected = [
        ("日本語をべ", "sentence", 1120403456),
        ("にほんごをべ", "unknown", 1119354880),
        ("日本語を勉強", "word", 1119158272),
        ("日本語を別の", "word", 1119092736),
        ("ニホンゴヲベ", "unknown", 1119092736),
        ("日本語を勉強しています。", "sentence", 1117323264),
        ("日本語を便利", "word", 1119027200),
        ("日本語を別ルート", "word", 1118961664),
        ("日本語を勉強したいです。", "sentence", 1117257728),
        ("nihongo wo be", "literal", 1113325568),
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
        "the only accepted daily_route vector delta must match in full"
    );
    let mut before = current.to_vec();
    before.remove(7);
    before[5].score = f32::from_bits(1117388800);
    before[7].score = f32::from_bits(1117323264);
    before
}

// This later append enriches two bounded partial pools, not the primary or
// first-page choices. Record the complete accepted vectors before inverting:
// jyunbi g evicts one last backfill Word; jyunbi ga also moves one pack Word
// into backfill and evicts its final Word. Restoring historical metadata here
// does not claim those evicted tails are still in the current twelve slots.
fn before_daily_call(
    language: &str,
    seed: &str,
    current: &[Candidate],
    with_packs: bool,
) -> Vec<Candidate> {
    if language != "ja" || !matches!(seed, "jyunbi g" | "jyunbi ga") {
        return current.to_vec();
    }
    let mut expected = if seed == "jyunbi g" {
        vec![
            ("準備g", "unknown", 1120403456),
            ("じゅんびg", "unknown", 1119354880),
            ("準備言語", "word", 1119158272),
            ("準備学校", "word", 1119092736),
            ("ジュンビg", "unknown", 1119092736),
            ("準備ができたら連絡します。", "sentence", 1117323264),
            ("準備ごちそうさま", "word", 1119027200),
            ("準備ご案内", "word", 1118961664),
            ("jyunbi g", "literal", 1113325568),
            ("準備画面共有", "word", 1113194496),
            ("準備画像サイズ", "word", 1113063424),
            ("準備画面が止まっています", "word", 1112932352),
        ]
    } else {
        vec![
            ("準備が", "sentence", 1120403456),
            ("じゅんびが", "unknown", 1119354880),
            ("準備学校", "word", 1119158272),
            ("準備画面共有", "word", 1119092736),
            ("ジュンビガ", "unknown", 1119092736),
            ("準備ができたら連絡します。", "sentence", 1117323264),
            ("準備画像サイズ", "word", 1119027200),
            ("準備画面が止まっています", "word", 1118961664),
            ("jyunbi ga", "literal", 1113325568),
        ]
    };
    if seed == "jyunbi ga" && with_packs {
        expected.extend([
            ("準備画面", "word", 1113194496),
            ("準備画像", "word", 1113063424),
            ("準備画質", "word", 1112932352),
        ]);
    }
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
        "the accepted daily_call delta must match every field: {seed:?}, packs={with_packs}"
    );
    let mut before = current.to_vec();
    if seed == "jyunbi g" {
        let text = if with_packs {
            "準備画面"
        } else {
            "準備具体例"
        };
        before[11].text = text.to_owned();
        before[11].label = text.to_owned();
    } else if with_packs {
        for (index, text, bits) in [
            (7, "準備画面", 1118961664),
            (9, "準備画像", 1113194496),
            (10, "準備画質", 1113063424),
            (11, "準備学習計画", 1112932352),
        ] {
            before[index].text = text.to_owned();
            before[index].label = text.to_owned();
            before[index].score = f32::from_bits(bits);
        }
    } else {
        before.remove(7);
        before[5].score = f32::from_bits(1117388800);
    }
    before
}

// Storage adds one final backfill Word for this exact unfinished reading.
// The builtin pool grows from eleven to twelve; with all nine packs the old
// twelfth Word (日本語ウェブページ) is evicted. Do not claim it remains reachable
// in the current bounded pool, or project any other input or metadata.
fn before_daily_storage(
    language: &str,
    seed: &str,
    current: &[Candidate],
    with_packs: bool,
) -> Vec<Candidate> {
    if language != "ja" || seed != "nihongo w" {
        return current.to_vec();
    }
    let expected = [
        ("日本語w", "unknown", 1120403456),
        ("にほんごw", "unknown", 1119354880),
        ("日本語私", "word", 1119158272),
        ("日本語分かりました", "word", 1119092736),
        ("ニホンゴw", "unknown", 1119092736),
        ("日本語を勉強しています。", "sentence", 1117323264),
        ("日本語分かりません", "word", 1119027200),
        ("日本語割引", "word", 1118961664),
        ("nihongo w", "literal", 1113325568),
        ("日本語忘れ物", "word", 1113194496),
        ("日本語忘れ物センター", "word", 1113063424),
        ("日本語分けて置きます", "word", 1112932352),
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
        "the accepted daily_storage delta must match every field: packs={with_packs}"
    );
    let mut before = current.to_vec();
    if with_packs {
        before[11].text = "日本語ウェブページ".to_owned();
        before[11].label = "日本語ウェブページ".to_owned();
    } else {
        before.remove(11);
    }
    before
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
        let before_storage = before_daily_storage(language, seed, ime.candidates(), with_packs);
        let before_call = before_daily_call(language, seed, &before_storage, with_packs);
        let before_route = before_daily_route(language, seed, &before_call);
        for candidate in &before_route {
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
fn optimized_english_preserves_130_actual_before_vectors() {
    replay("en", 130, 0xc9b9b260b8fc9d84, false);
}
#[test]
fn optimized_chinese_preserves_107_actual_before_vectors() {
    replay("zh-Hans", 107, 0xd58c7555c30542b0, false);
}
#[test]
fn optimized_japanese_preserves_91_actual_before_vectors() {
    replay("ja", 91, 0xc088d2051394f163, false);
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn optimized_japanese_with_nine_packs_preserves_91_actual_before_vectors() {
    const NAME: &str = "optimized_japanese_with_nine_packs_preserves_91_actual_before_vectors";
    const CHILD: &str = "SUZAKU_INPUT_HOTPATH_CHILD";
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
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "suzaku-input-perf-equivalence-{}",
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
    replay("ja", 91, 0xc856096caef0187f, true);
}
