//! A whole-reading terminal-n completion must not override an unauthored stem's old pages.
//! Frozen from the data-only library, independently of the rejected broader slot proof.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, offline},
    ime::{Candidate, CommitOptions, EngineConfig, LanguagePlugin, XRTabletImeEngine},
    languages::japanese::JapaneseLanguagePlugin,
    lexicon::{
        self, EntryKind,
        packs::{self, PackStore},
    },
};

type FrozenRow = (&'static str, CandidateKind, u32);
const CHILD: &str = "SUZAKU_JAPANESE_NASAL_STEM_BOUNDARIES_CHILD";
const BUILTIN: [FrozenRow; 12] = [
    ("乗車け", CandidateKind::Sentence, 1120403456),
    ("じょうしゃけ", CandidateKind::Unspecified, 1119354880),
    ("乗車券", CandidateKind::Word, 1119158272),
    ("乗車景色", CandidateKind::Word, 1119092736),
    ("ジョウシャケ", CandidateKind::Unspecified, 1119092736),
    (
        "乗車券はどこで買えますか。",
        CandidateKind::Sentence,
        1117323264,
    ),
    ("乗車結論", CandidateKind::Word, 1119027200),
    ("乗車今朝", CandidateKind::Word, 1118961664),
    (
        "乗車券を確認してから乗りましょう。",
        CandidateKind::Sentence,
        1117257728,
    ),
    ("joushake", CandidateKind::Literal, 1113325568),
    ("乗車圏外", CandidateKind::Word, 1113194496),
    ("乗車携帯", CandidateKind::Word, 1113063424),
];
const NINE_PACK: [FrozenRow; 12] = [
    ("整理け", CandidateKind::Sentence, 1120403456),
    ("せいりけ", CandidateKind::Unspecified, 1119354880),
    ("整理券", CandidateKind::Word, 1119158272),
    ("整理景色", CandidateKind::Word, 1119092736),
    ("セイリケ", CandidateKind::Unspecified, 1119092736),
    (
        "整理券はどこでもらえますか。",
        CandidateKind::Sentence,
        1117323264,
    ),
    ("整理結論", CandidateKind::Word, 1119027200),
    ("整理今朝", CandidateKind::Word, 1118961664),
    (
        "整理券を受け取ってから順番を待ちます。",
        CandidateKind::Sentence,
        1117257728,
    ),
    ("seirike", CandidateKind::Literal, 1113325568),
    ("整理圏外", CandidateKind::Word, 1113194496),
    ("整理携帯", CandidateKind::Word, 1113063424),
];
// Actual data-only 0..12 pools in both independently initialized catalogs.
// This freezes small-capacity literal rescue as well as the complete old pages.
const ORDERS: [&[usize]; 13] = [
    &[0],
    &[0],
    &[0, 9],
    &[0, 1, 9],
    &[0, 1, 2, 9],
    &[0, 1, 2, 3, 9],
    &[0, 1, 2, 3, 4, 9],
    &[0, 1, 2, 3, 4, 5, 9],
    &[0, 1, 2, 3, 4, 5, 6, 9],
    &[0, 1, 2, 3, 4, 5, 6, 7, 9],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
];

fn exact_pool(pool: &[Candidate], rows: &[FrozenRow; 12], order: &[usize]) {
    assert_eq!(pool.len(), order.len());
    assert_eq!(
        pool.len(),
        pool.iter().map(|c| &c.text).collect::<HashSet<_>>().len()
    );
    for (candidate, &old_index) in pool.iter().zip(order) {
        let (text, kind, bits) = rows[old_index];
        assert_eq!(candidate.text, text, "frozen pool order changed: {pool:?}");
        assert_eq!(candidate.label, text);
        assert_eq!(candidate.kind, kind);
        assert_eq!(candidate.source, CandidateSource::Local);
        assert_eq!(candidate.score.to_bits(), bits);
    }
}
fn position(pool: &[Candidate], text: &str, kind: CandidateKind) -> usize {
    pool.iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {pool:?}"))
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
fn commit_once(seed: &str, rows: &[FrozenRow; 12], row: usize) {
    let (text, kind, _) = rows[row];
    let mut ime = engine(seed);
    exact_pool(ime.candidates(), rows, ORDERS[12]);
    let at = position(ime.candidates(), text, kind);
    assert_eq!(at, row);
    if row == 8 {
        assert!(
            (PAGE_SIZE..2 * PAGE_SIZE).contains(&at),
            "old second ticket author left page two"
        );
    }
    ime.select_candidate(at);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    let repeated = ime.commit(CommitOptions { force: true });
    assert!(!repeated.ok && repeated.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn boundary(seed: &str, stem: &str, rows: &[FrozenRow; 12]) {
    let vocabulary = lexicon::active("ja").unwrap();
    assert!(
        vocabulary
            .readings()
            .iter()
            .any(|r| r.text == stem && r.kind == EntryKind::Word)
    );
    assert!(
        vocabulary
            .continuations()
            .all(|(context, _)| context != stem),
        "this negative control must not grow an authored stem family"
    );
    let compound = rows[2].0;
    let (_, authors) = vocabulary
        .continuations()
        .find(|(context, _)| *context == compound)
        .unwrap();
    assert_eq!(
        authors
            .iter()
            .take(2)
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [rows[5].0, rows[8].0]
    );
    assert_ne!(rows[5].0, rows[8].0);
    assert!(
        authors
            .iter()
            .take(2)
            .all(|s| s.starts_with(compound) && s.len() > compound.len())
    );
    assert_eq!(rows[0].0, format!("{stem}け"));
    assert_eq!(rows[0].1, CandidateKind::Sentence);
    assert_eq!(rows[0].2, 100.0_f32.to_bits());
    for (limit, order) in ORDERS.iter().enumerate() {
        let actual = offline(
            "ja",
            seed,
            "",
            JapaneseLanguagePlugin.direct_candidates(seed, 1.0).unwrap(),
            limit,
        );
        assert!(actual.len() <= limit.clamp(1, 12));
        exact_pool(&actual, rows, order);
        if limit >= 2 {
            position(&actual, seed, CandidateKind::Literal);
        }
    }
    let ime = engine(seed);
    exact_pool(ime.candidates(), rows, ORDERS[12]);
    assert_eq!(
        position(ime.candidates(), rows[5].0, CandidateKind::Sentence),
        5
    );
    assert_eq!(
        position(ime.candidates(), rows[8].0, CandidateKind::Sentence),
        8
    );
    assert_eq!(
        ime.candidates()
            .iter()
            .filter(|c| c.kind == CandidateKind::Word)
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>(),
        rows.iter()
            .filter(|row| row.1 == CandidateKind::Word)
            .map(|row| row.0)
            .collect::<Vec<_>>(),
    );
    for row in [2, 5, 8, 9] {
        commit_once(seed, rows, row);
    }
}

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn isolated(name: &str, pack_count: usize, body: impl FnOnce()) {
    assert!(matches!(pack_count, 0 | 9));
    let done = format!("completed nasal stem boundary {name} with {pack_count} private packs");
    if std::env::var(CHILD).as_deref() != Ok(name) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(CHILD, name)
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{out}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(out.contains("running 1 test"), "{out}");
        assert!(out.contains(&format!("test {name} ...")), "{out}");
        assert!(out.contains(&done), "{out}");
        assert!(
            out.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{out}"
        );
        return;
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "suzaku-nasal-stem-boundary-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    let owned = Owned(path);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    let selected = packs::recommended()
        .into_iter()
        .filter(|p| pack_count == 9 && p.manifest.language == "ja")
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), pack_count);
    let ids = selected
        .iter()
        .map(|p| p.manifest.id.clone())
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), pack_count);
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), pack_count);
    assert_eq!(report.loaded.iter().cloned().collect::<HashSet<_>>(), ids);
    body();
    println!("{done}");
}

#[test]
fn builtin_ticket_stem_without_authors_keeps_every_original_zero_to_twelve_pool() {
    const NAME: &str =
        "builtin_ticket_stem_without_authors_keeps_every_original_zero_to_twelve_pool";
    isolated(NAME, 0, || boundary("joushake", "乗車", &BUILTIN));
}

#[test]
fn private_nine_pack_ticket_stem_without_authors_keeps_original_second_page_and_undo() {
    const NAME: &str =
        "private_nine_pack_ticket_stem_without_authors_keeps_original_second_page_and_undo";
    isolated(NAME, 9, || boundary("seirike", "整理", &NINE_PACK));
}

// The existing parent contexts below really have their own authored pair. The
// only reviewed delta is capacity six: keep the compound's second author in
// place of row 3's Katakana Unknown. Every tuple and every other capacity stay
// equal to the saved data-only 0..12 probes; no score or Word order is changed.
const PLANNING: [FrozenRow; 7] = [
    ("予定のかくに", CandidateKind::Sentence, 1120403456),
    ("よていのかくに", CandidateKind::Unspecified, 1119354880),
    ("予定の確認", CandidateKind::Word, 1119158272),
    ("ヨテイノカクニ", CandidateKind::Unspecified, 1119092736),
    (
        "予定の確認をお願いします。",
        CandidateKind::Sentence,
        1117519872,
    ),
    (
        "予定の確認ができたら連絡します。",
        CandidateKind::Sentence,
        1117454336,
    ),
    ("yoteinokakuni", CandidateKind::Literal, 1113325568),
];
const STAR_CHART: [FrozenRow; 7] = [
    ("星座はやみば", CandidateKind::Sentence, 1120403456),
    ("せいざはやみば", CandidateKind::Unspecified, 1119354880),
    ("星座早見盤", CandidateKind::Word, 1119158272),
    ("セイザハヤミバ", CandidateKind::Unspecified, 1119092736),
    (
        "星座早見盤の読み方を覚えたいです。",
        CandidateKind::Sentence,
        1117519872,
    ),
    (
        "星座早見盤をかばんに入れました。",
        CandidateKind::Sentence,
        1117454336,
    ),
    ("seizahayamiba", CandidateKind::Literal, 1113325568),
];
const WORK_STEPS: [FrozenRow; 7] = [
    ("作業てじゅ", CandidateKind::Sentence, 1120403456),
    ("さぎょうてじゅ", CandidateKind::Unspecified, 1119354880),
    ("作業手順", CandidateKind::Word, 1119158272),
    ("サギョウテジュ", CandidateKind::Unspecified, 1119092736),
    (
        "作業手順をもう一度確認します。",
        CandidateKind::Sentence,
        1117519872,
    ),
    (
        "作業手順に変更があれば教えてください。",
        CandidateKind::Sentence,
        1117454336,
    ),
    ("sagyouteju", CandidateKind::Literal, 1113325568),
];
const FAMILY_ORDERS: [&[usize]; 13] = [
    &[0],
    &[0],
    &[0, 6],
    &[0, 1, 6],
    &[0, 1, 2, 6],
    &[0, 1, 2, 3, 6],
    &[0, 1, 2, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6],
];
fn exact_family_pool(pool: &[Candidate], rows: &[FrozenRow; 7], order: &[usize]) {
    assert_eq!(pool.len(), order.len());
    assert_eq!(
        pool.len(),
        pool.iter().map(|c| &c.text).collect::<HashSet<_>>().len()
    );
    for (candidate, &row) in pool.iter().zip(order) {
        assert_eq!(candidate.text, rows[row].0, "{pool:?}");
        assert_eq!(candidate.label, rows[row].0);
        assert_eq!(candidate.kind, rows[row].1);
        assert_eq!(candidate.source, CandidateSource::Local);
        assert_eq!(candidate.score.to_bits(), rows[row].2);
    }
}
fn existing_family(seed: &str, stem: &str, stem_authors: [&str; 2], rows: &[FrozenRow; 7]) {
    let vocabulary = lexicon::active("ja").unwrap();
    for word in [stem, rows[2].0] {
        assert!(
            vocabulary
                .readings()
                .iter()
                .any(|r| r.text == word && r.kind == EntryKind::Word)
        );
    }
    assert!(rows[2].0.starts_with(stem) && rows[2].0.len() > stem.len());
    for (context, expected) in [(stem, stem_authors), (rows[2].0, [rows[4].0, rows[5].0])] {
        let (_, authors) = vocabulary
            .continuations()
            .find(|(key, _)| *key == context)
            .unwrap();
        assert_eq!(
            authors
                .iter()
                .take(2)
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );
        assert_ne!(expected[0], expected[1]);
        assert!(
            expected
                .iter()
                .all(|s| s.starts_with(context) && s.len() > context.len())
        );
    }
    for (limit, order) in FAMILY_ORDERS.iter().enumerate() {
        let actual = offline(
            "ja",
            seed,
            "",
            JapaneseLanguagePlugin.direct_candidates(seed, 1.0).unwrap(),
            limit,
        );
        assert!(actual.len() <= limit.clamp(1, 12));
        exact_family_pool(&actual, rows, order);
        if limit == PAGE_SIZE {
            assert_eq!(position(&actual, rows[4].0, CandidateKind::Sentence), 3);
            assert_eq!(position(&actual, rows[5].0, CandidateKind::Sentence), 4);
            assert_eq!(position(&actual, rows[2].0, CandidateKind::Word), 2);
            assert_eq!(position(&actual, seed, CandidateKind::Literal), 5);
        }
    }
    // The normal twelve-slot public engine remains byte-for-byte the old pool.
    // Select the real Word, either author, and raw text without double commit.
    for row in [2, 4, 5, 6] {
        let (text, kind, _) = rows[row];
        let mut ime = engine(seed);
        exact_family_pool(ime.candidates(), rows, FAMILY_ORDERS[12]);
        assert_eq!(position(ime.candidates(), text, kind), row);
        ime.select_candidate(row);
        assert_eq!(ime.selected_completion_text(true), Some(text));
        assert_eq!(ime.snapshot().seed_text, seed);
        assert!(ime.snapshot().committed_text.is_empty());
        let committed = ime.commit(CommitOptions { force: true });
        assert!(committed.ok);
        assert_eq!(committed.text.as_deref(), Some(text));
        assert_eq!(ime.snapshot().committed_text, text);
        assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
        let repeated = ime.commit(CommitOptions { force: true });
        assert!(!repeated.ok && repeated.text.is_none());
        assert_eq!(ime.snapshot().committed_text, text);
        assert!(ime.undo().unwrap().committed_text.is_empty());
        assert!(ime.undo().is_none());
    }
}

#[test]
fn builtin_authored_parent_keeps_existing_planning_family_at_six_slots() {
    const NAME: &str = "builtin_authored_parent_keeps_existing_planning_family_at_six_slots";
    isolated(NAME, 0, || {
        existing_family(
            "yoteinokakuni",
            "予定",
            [
                "予定が決まったら連絡します。",
                "予定を確認してから返事します。",
            ],
            &PLANNING,
        )
    });
}

#[test]
fn private_nine_pack_authored_parents_keep_planning_chart_and_steps_at_six_slots() {
    const NAME: &str =
        "private_nine_pack_authored_parents_keep_planning_chart_and_steps_at_six_slots";
    isolated(NAME, 9, || {
        existing_family(
            "yoteinokakuni",
            "予定",
            [
                "予定が決まったら連絡します。",
                "予定を確認してから返事します。",
            ],
            &PLANNING,
        );
        existing_family(
            "seizahayamiba",
            "星座",
            [
                "星座の名前を一緒に調べましょう。",
                "星座を探す練習をしています。",
            ],
            &STAR_CHART,
        );
        existing_family(
            "sagyouteju",
            "作業",
            [
                "作業が終わったら連絡します。",
                "作業を分担して進めましょう。",
            ],
            &WORK_STEPS,
        );
    });
}
