//! Frozen actual-library regression: short Japanese pack Words remain selectable.
//! Each assertion uses fresh private family/family+rail catalogs, never user data.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        Candidate, CommitOptions, EngineConfig, LanguagePlugin, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, offline},
    },
    languages::japanese::JapaneseLanguagePlugin,
    lexicon::packs::{self, PackStore},
};
type Row = (&'static str, &'static str, &'static str, &'static str, u32);
const CHILD: &str = "SUZAKU_FAMILY_SHORT_READING_CASE";
const KANA: &[Row] = &[
    ("ま", "ま", "literal", "local", 1120403456),
    ("マ", "マ", "unknown", "local", 1119354880),
    (
        "待ってください",
        "待ってください",
        "word",
        "local",
        1119223808,
    ),
    ("また明日", "また明日", "word", "local", 1119158272),
    (
        "待ってください、今確認しています。",
        "待ってください、今確認しています。",
        "sentence",
        "local",
        1117388800,
    ),
    (
        "待ってください、すぐに戻ります。",
        "待ってください、すぐに戻ります。",
        "sentence",
        "local",
        1117323264,
    ),
    ("迷う", "迷う", "word", "local", 1119092736),
    ("窓枠", "窓枠", "word", "local", 1119027200),
    ("窓口", "窓口", "word", "local", 1113194496),
    ("待ち時間", "待ち時間", "word", "local", 1113063424),
    (
        "待ち合わせ場所",
        "待ち合わせ場所",
        "word",
        "local",
        1112932352,
    ),
    ("窓", "窓", "word", "local", 1112801280),
];
const ROMAJI: &[Row] = &[
    ("ま", "ま", "unknown", "local", 1120403456),
    ("マ", "マ", "unknown", "local", 1119354880),
    (
        "待ってください",
        "待ってください",
        "word",
        "local",
        1119223808,
    ),
    ("また明日", "また明日", "word", "local", 1119158272),
    (
        "待ってください、今確認しています。",
        "待ってください、今確認しています。",
        "sentence",
        "local",
        1117388800,
    ),
    (
        "待ってください、すぐに戻ります。",
        "待ってください、すぐに戻ります。",
        "sentence",
        "local",
        1117323264,
    ),
    ("迷う", "迷う", "word", "local", 1119092736),
    ("窓枠", "窓枠", "word", "local", 1119027200),
    ("ma", "ma", "literal", "local", 1113325568),
    ("窓口", "窓口", "word", "local", 1113194496),
    ("待ち時間", "待ち時間", "word", "local", 1113063424),
    (
        "待ち合わせ場所",
        "待ち合わせ場所",
        "word",
        "local",
        1112932352,
    ),
];
const KATAKANA: &[Row] = &[("マ", "マ", "literal", "local", 1120403456)];
fn expected(seed: &str, limit: usize) -> Vec<Row> {
    let (source, indices): (&[Row], &[usize]) = match (seed, limit) {
        ("ま", 0) => (KANA, &[0]),
        ("ま", 1) => (KANA, &[0]),
        ("ま", 2) => (KANA, &[0, 2]),
        ("ま", 3) => (KANA, &[0, 2, 4]),
        ("ま", 4) => (KANA, &[0, 1, 2, 4]),
        ("ま", 5) => (KANA, &[0, 2, 3, 4, 5]),
        ("ま", 6) => (KANA, &[0, 1, 2, 3, 4, 5]),
        ("ま", 7) => (KANA, &[0, 1, 2, 3, 4, 5, 6]),
        ("ま", 8) => (KANA, &[0, 1, 2, 3, 4, 5, 6, 7]),
        ("ま", 9) => (KANA, &[0, 1, 2, 3, 4, 5, 6, 7, 8]),
        ("ま", 10) => (KANA, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ("ま", 11) => (KANA, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
        ("ま", 12) => (KANA, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        ("ma", 0) => (ROMAJI, &[0]),
        ("ma", 1) => (ROMAJI, &[0]),
        ("ma", 2) => (ROMAJI, &[0, 8]),
        ("ma", 3) => (ROMAJI, &[0, 2, 8]),
        ("ma", 4) => (ROMAJI, &[0, 1, 2, 8]),
        ("ma", 5) => (ROMAJI, &[0, 2, 3, 4, 8]),
        ("ma", 6) => (ROMAJI, &[0, 1, 2, 3, 4, 8]),
        ("ma", 7) => (ROMAJI, &[0, 1, 2, 3, 4, 5, 8]),
        ("ma", 8) => (ROMAJI, &[0, 1, 2, 3, 4, 5, 6, 8]),
        ("ma", 9) => (ROMAJI, &[0, 1, 2, 3, 4, 5, 6, 7, 8]),
        ("ma", 10) => (ROMAJI, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ("ma", 11) => (ROMAJI, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
        ("ma", 12) => (ROMAJI, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        ("マ", 0) => (KATAKANA, &[0]),
        ("マ", 1) => (KATAKANA, &[0]),
        ("マ", 2) => (KATAKANA, &[0]),
        ("マ", 3) => (KATAKANA, &[0]),
        ("マ", 4) => (KATAKANA, &[0]),
        ("マ", 5) => (KATAKANA, &[0]),
        ("マ", 6) => (KATAKANA, &[0]),
        ("マ", 7) => (KATAKANA, &[0]),
        ("マ", 8) => (KATAKANA, &[0]),
        ("マ", 9) => (KATAKANA, &[0]),
        ("マ", 10) => (KATAKANA, &[0]),
        ("マ", 11) => (KATAKANA, &[0]),
        ("マ", 12) => (KATAKANA, &[0]),
        _ => unreachable!(),
    };
    indices.iter().map(|&index| source[index]).collect()
}
fn tuples(pool: &[Candidate]) -> Vec<(&str, &str, &str, &str, u32)> {
    pool.iter()
        .map(|c| {
            (
                c.text.as_str(),
                c.label.as_str(),
                c.kind.id(),
                c.source.id(),
                c.score.to_bits(),
            )
        })
        .collect()
}
struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn catalogs(name: &str, body: impl FnOnce()) {
    let mode = std::env::var(CHILD);
    if mode.is_err() {
        let outcomes = ["family", "family-rail"].map(|mode| {
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", name, "--nocapture", "--test-threads=1"])
                .env(CHILD, mode)
                .env_remove("SUZAKU_LEXICON_DIR")
                .env_remove("SUZAKU_IME_CONFIG")
                .env_remove("SUZAKU_LINUX_IME_SOCKET")
                .env_remove("DISPLAY")
                .env_remove("WAYLAND_DISPLAY")
                .env_remove("DBUS_SESSION_BUS_ADDRESS")
                .output()
                .unwrap();
            (mode, output)
        });
        let details = outcomes
            .iter()
            .map(|(mode, out)| {
                format!(
                    "{mode}:\n{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            outcomes.iter().all(|(_, out)| out.status.success()),
            "{details}"
        );
        for (mode, out) in outcomes {
            assert!(
                String::from_utf8_lossy(&out.stdout)
                    .contains(&format!("completed {name} with {mode}"))
            );
        }
        return;
    }
    let mode = mode.unwrap();
    assert!(matches!(mode.as_str(), "family" | "family-rail"));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "suzaku-family-short-tests-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    let owned = Owned(path);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    for id in ["org.suzaku.ja.family"]
        .into_iter()
        .chain((mode == "family-rail").then_some("org.suzaku.ja.rail"))
    {
        let pack = packs::recommended()
            .into_iter()
            .find(|p| p.manifest.id == id)
            .unwrap();
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.disabled.is_empty() && report.errors.is_empty());
    assert_eq!(report.loaded.len(), if mode == "family" { 1 } else { 2 });
    body();
    println!("completed {name} with {mode}");
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    ime
}
fn check(pool: &[Candidate], limit: usize) {
    assert!(pool.len() <= limit.max(1));
    assert_eq!(
        pool.len(),
        pool.iter().map(|c| &c.text).collect::<HashSet<_>>().len()
    );
    assert!(
        pool.iter().all(|c| c.source == CandidateSource::Local
            && c.label == c.text
            && c.score.is_finite())
    );
}
fn commit_once(seed: &str, word: &str, kind: CandidateKind) {
    let mut ime = engine(seed);
    let index = ime
        .candidates()
        .iter()
        .position(|c| c.text == word && c.kind == kind)
        .unwrap();
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(word));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(word));
    assert_eq!(ime.snapshot().committed_text, word);
    assert!(ime.snapshot().seed_text.is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
#[test]
fn single_kana_retains_the_older_pack_word_after_existing_words_and_authors() {
    const NAME: &str = "single_kana_retains_the_older_pack_word_after_existing_words_and_authors";
    catalogs(NAME, || {
        let direct = JapaneseLanguagePlugin.direct_candidates("ま", 1.0).unwrap();
        let pool = offline("ja", "ま", "", direct, 12);
        check(&pool, 12);
        assert_eq!(tuples(&pool), expected("ま", 12));
        let ime = engine("ま");
        assert_eq!(tuples(ime.candidates()), expected("ま", 12));
        let index = ime
            .candidates()
            .iter()
            .position(|c| c.text == "窓" && c.kind == CandidateKind::Word)
            .unwrap();
        assert_eq!(index, 11);
        assert!(index >= PAGE_SIZE);
        commit_once("ま", "窓", CandidateKind::Word);
        commit_once("ま", "ま", CandidateKind::Literal);
    });
}
#[test]
fn smaller_quotas_roman_short_input_and_katakana_keep_exact_old_pool_policy() {
    const NAME: &str = "smaller_quotas_roman_short_input_and_katakana_keep_exact_old_pool_policy";
    catalogs(NAME, || {
        for seed in ["ま", "ma", "マ"] {
            let direct = JapaneseLanguagePlugin.direct_candidates(seed, 1.0).unwrap();
            let max = if seed == "ま" { 11 } else { 12 };
            for limit in 0..=max {
                let pool = offline("ja", seed, "", direct.clone(), limit);
                check(&pool, limit);
                assert_eq!(
                    tuples(&pool),
                    expected(seed, limit),
                    "{seed:?} limit {limit}"
                );
            }
        }
        assert_eq!(tuples(engine("ma").candidates()), expected("ma", 12));
        assert_eq!(tuples(engine("マ").candidates()), expected("マ", 12));
        assert!(engine("ma").candidates().iter().all(|c| c.text != "窓"));
        assert!(engine("マ").candidates().iter().all(|c| c.text != "窓"));
        commit_once("ma", "ma", CandidateKind::Literal);
        commit_once("マ", "マ", CandidateKind::Literal);
    });
}
#[test]
fn complete_pack_word_stays_primary_with_older_author_order_and_raw() {
    const NAME: &str = "complete_pack_word_stays_primary_with_older_author_order_and_raw";
    catalogs(NAME, || {
        for seed in ["まど", "mado"] {
            let direct = JapaneseLanguagePlugin.direct_candidates(seed, 1.0).unwrap();
            assert_eq!(direct[0].text, "窓");
            for limit in [6, 12] {
                let pool = offline("ja", seed, "", direct.clone(), limit);
                check(&pool, limit);
                assert_eq!(pool[0].text, "窓");
                assert_eq!(pool[0].kind, CandidateKind::Word);
                assert_eq!(pool[0].score.to_bits(), 100.0_f32.to_bits());
                assert!(pool.iter().any(|c| c.text == seed
                    && c.kind == CandidateKind::Literal
                    && c.score.to_bits() == 55.0_f32.to_bits()));
            }
            let ime = engine(seed);
            let words = ["窓", "窓枠", "窓口"].map(|word| {
                ime.candidates()
                    .iter()
                    .position(|c| c.text == word && c.kind == CandidateKind::Word)
                    .unwrap()
            });
            assert!(words.windows(2).all(|pair| pair[0] < pair[1]));
            let authors =
                ["窓口はどこにありますか。", "窓口で手続きをしたいです。"].map(|sentence| {
                    ime.candidates()
                        .iter()
                        .position(|c| c.text == sentence && c.kind == CandidateKind::Sentence)
                        .unwrap()
                });
            assert_eq!(authors, [4, 5]);
            commit_once(seed, "窓", CandidateKind::Word);
            commit_once(seed, seed, CandidateKind::Literal);
        }
    });
}
