//! Complete actual A/B vectors, not refreshed historical optimization goldens.
#[path = "support/work_communication_replay.rs"]
mod work_communication_replay;
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        Candidate, CommitOptions, EngineConfig, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource},
    },
    lexicon::packs::{self, PackStore},
};

fn corpus() -> Vec<String> {
    // Exact 91 entries (89 distinct seeds) of both existing Japanese replays.
    let phrases = [
        "nihongo wo benkyou shite",
        "hoshousho",
        "henkinn",
        "seirikenn",
        "kyouyu",
        "jyunbi gade ki",
        "konnpouzai",
    ];
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

fn fixture(rows: &[(&str, &str, u32)]) -> Vec<Candidate> {
    rows.iter()
        .map(|&(text, kind, bits)| Candidate {
            text: text.to_owned(),
            label: text.to_owned(),
            score: f32::from_bits(bits),
            kind: CandidateKind::parse(kind).unwrap(),
            source: CandidateSource::Local,
        })
        .collect()
}

// Independent literals transcribed from the two actual raw executions; these
// are not read from production JSON or from the helper's expected arrays.
fn after() -> Vec<Candidate> {
    fixture(&[
        ("へんk", "unknown", 1120403456),
        ("ヘンk", "unknown", 1119354880),
        ("変換", "word", 1119223808),
        ("変更", "word", 1119158272),
        (
            "変更履歴を見ながら、直した箇所を一緒に確認しましょう。",
            "sentence",
            1117388800,
        ),
        (
            "変更履歴に理由も残しておくので、引き継ぎのときに確認してください。",
            "sentence",
            1117323264,
        ),
        ("変更履歴", "word", 1119092736),
        ("返却期限", "word", 1119027200),
        ("返却期限を教えてもらえますか。", "sentence", 1117257728),
        ("返却期限までに持っていきます。", "sentence", 1117192192),
        ("henk", "literal", 1113325568),
        ("返却場所", "word", 1113194496),
    ])
}

fn before() -> Vec<Candidate> {
    fixture(&[
        ("へんk", "unknown", 1120403456),
        ("ヘンk", "unknown", 1119354880),
        ("変換", "word", 1119223808),
        ("変更", "word", 1119158272),
        ("返却期限を教えてもらえますか。", "sentence", 1117388800),
        ("返却期限までに持っていきます。", "sentence", 1117323264),
        ("変更履歴", "word", 1119092736),
        ("返却期限", "word", 1119027200),
        ("henk", "literal", 1113325568),
        ("返却場所", "word", 1113194496),
        ("返却", "word", 1113063424),
        ("返金", "word", 1112932352),
    ])
}

fn engine() -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime
}

fn validate(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|row| &row.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn hash_vector(hash: &mut u64, seed: &str, candidates: &[Candidate]) {
    for byte in seed.bytes().chain([0]) {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    for candidate in candidates {
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
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
}

fn replay(with_packs: bool) {
    let seeds = corpus();
    assert_eq!(seeds.len(), 91);
    assert_eq!(seeds.iter().collect::<HashSet<_>>().len(), 89);
    let mut ime = engine();
    // Same warm-up validation pass as the measured dump and old replays.
    for seed in &seeds {
        ime.seed(seed);
        validate(&ime, seed);
    }
    let (mut current_hash, mut previous_hash) = (0xcbf29ce484222325_u64, 0xcbf29ce484222325_u64);
    for (index, seed) in seeds.iter().enumerate() {
        ime.seed(seed);
        validate(&ime, seed);
        let current = ime.candidates();
        let previous =
            work_communication_replay::before_work_communication("ja", seed, current, with_packs);
        if index == 36 {
            assert_eq!(seed, "henk");
            assert_eq!(current, after());
            assert_eq!(previous, before());
        } else {
            assert_eq!(
                previous, current,
                "another vector changed: {index} {seed:?}"
            );
        }
        hash_vector(&mut current_hash, seed, current);
        hash_vector(&mut previous_hash, seed, &previous);
    }
    // Actual raw batch A/B hashes, before any older historical projections.
    let expected = if with_packs {
        (0x8a9b67f126d0fb7f, 0x737eeb6ed7b4da16)
    } else {
        (0x165529d0ac986711, 0xb82d190fc598cfb4)
    };
    assert_eq!((current_hash, previous_hash), expected);
}

fn guard_mutations() {
    let current = after();
    for with_packs in [false, true] {
        assert_eq!(
            work_communication_replay::before_work_communication(
                "ja", "henk", &current, with_packs
            ),
            before()
        );
        let rejects = |rows: &[Candidate]| {
            assert!(
                std::panic::catch_unwind(|| {
                    work_communication_replay::before_work_communication(
                        "ja", "henk", rows, with_packs,
                    )
                })
                .is_err(),
                "exact-scope helper accepted a changed complete vector"
            );
        };
        for index in 0..current.len() {
            for field in 0..5 {
                let mut changed = current.clone();
                let row = &mut changed[index];
                match field {
                    0 => row.text.push_str("<changed>"),
                    1 => row.label.push_str("<changed>"),
                    2 => {
                        row.kind = if row.kind == CandidateKind::Word {
                            CandidateKind::Literal
                        } else {
                            CandidateKind::Word
                        };
                    }
                    3 => row.source = CandidateSource::Model,
                    _ => row.score = f32::from_bits(row.score.to_bits() + 1),
                }
                rejects(&changed);
            }
            let mut changed = current.clone();
            changed.remove(index);
            rejects(&changed);
        }
        for index in 0..current.len() - 1 {
            let mut changed = current.clone();
            changed.swap(index, index + 1);
            rejects(&changed);
        }
        for index in 0..=current.len() {
            let mut changed = current.clone();
            changed.insert(index, current[4].clone());
            rejects(&changed);
        }
        let mut changed = current.clone();
        changed[4].label.push_str("<changed>");
        for rows in [&current, &changed, &Vec::new()] {
            for language in ["", "en", "JA", "ja-JP", "zh-Hans"] {
                assert_eq!(
                    work_communication_replay::before_work_communication(
                        language, "henk", rows, with_packs
                    ),
                    *rows
                );
            }
            for seed in [
                "", "h", "hen", "HENK", "henk ", " henk", "henki", "henk\n", "へんk",
            ] {
                assert_eq!(
                    work_communication_replay::before_work_communication(
                        "ja", seed, rows, with_packs
                    ),
                    *rows
                );
            }
        }
    }
}

fn commit(seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine();
    ime.seed(seed);
    validate(&ime, seed);
    let pos = ime
        .candidates()
        .iter()
        .position(|row| row.text == text && row.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} {text:?} from {seed:?}: {:?}",
                ime.candidates()
            )
        });
    ime.select_candidate(pos);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn old_choices_remain_selectable() {
    let mut ime = engine();
    ime.seed("henk");
    assert_eq!(ime.candidates(), after());
    // Honest bounded-pool assertion: these two Words were actually evicted.
    for word in ["返却", "返金"] {
        assert!(!ime.candidates().iter().any(|row| row.text == word));
    }
    for author in [
        "返却期限を教えてもらえますか。",
        "返却期限までに持っていきます。",
    ] {
        commit("henk", author, CandidateKind::Sentence);
    }
    for seed in ["henkyaku", "へんきゃく"] {
        commit(seed, "返却", CandidateKind::Word);
    }
    for seed in ["henkin", "henkinn", "henkin'", "へんきん"] {
        commit(seed, "返金", CandidateKind::Word);
    }
}

#[test]
fn work_communication_builtin_replay_restores_only_the_verified_henk_vector() {
    replay(false);
    guard_mutations();
    old_choices_remain_selectable();
}

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn work_communication_fresh_nine_pack_replay_restores_only_the_verified_henk_vector() {
    const NAME: &str =
        "work_communication_fresh_nine_pack_replay_restores_only_the_verified_henk_vector";
    const CHILD: &str = "SUZAKU_WORK_COMMUNICATION_REPLAY_CHILD";
    const SUCCESS: &str = "WORK_COMMUNICATION_NINE_PACK_REPLAY_OK";
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
        assert!(stdout.contains(&format!("test {NAME} ...")), "{stdout}");
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{stdout}"
        );
        assert_eq!(stdout.matches(SUCCESS).count(), 1, "{stdout}");
        // Do not echo a successful private child's test summary as outer tests.
        return;
    }
    let directory = std::env::temp_dir().join(format!(
        "suzaku-work-communication-replay-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let owned = Owned(directory);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    assert!(store.list().unwrap().is_empty());
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
        let pack = catalog.iter().find(|pack| pack.manifest.id == id).unwrap();
        store.install(pack.clone(), false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(
        report.errors.is_empty() && report.disabled.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.loaded.len(), ids.len());
    assert_eq!(
        report
            .loaded
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>(),
        ids.into_iter().collect::<HashSet<_>>()
    );
    replay(true);
    old_choices_remain_selectable();
    println!("{SUCCESS}");
}
