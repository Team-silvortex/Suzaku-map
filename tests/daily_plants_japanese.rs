//! Finite plant dialogue, frozen against the actual 609/333/665 vocabulary.
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, LanguagePlugin, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, offline},
    },
    languages::{
        japanese::{JapaneseLanguagePlugin, romaji_to_hiragana},
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{
        EntryKind, builtin,
        packs::{self, PackStore},
    },
};

// 植物 is already the Word at index 176. Add only its missing context;
// the other five Words and all six authored contexts are this finite append.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("shokubutsu", "しょくぶつ", "ショクブツ", "植物"),
    ("uekibachi", "うえきばち", "ウエキバチ", "植木鉢"),
    ("happa", "はっぱ", "ハッパ", "葉っぱ"),
    ("mizuyari", "みずやり", "ミズヤリ", "水やり"),
    ("jouro", "じょうろ", "ジョウロ", "じょうろ"),
    ("madobe", "まどべ", "マドベ", "窓辺"),
];
const NEW_READINGS: &[(&str, &str)] = &[
    ("うえきばち", "植木鉢"),
    ("はっぱ", "葉っぱ"),
    ("みずやり", "水やり"),
    ("じょうろ", "じょうろ"),
    ("まどべ", "窓辺"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "植物",
        [
            "植物を窓辺に置いています。",
            "植物の名前を小さな札に書きます。",
        ],
    ),
    (
        "植木鉢",
        [
            "植木鉢をベランダの隅に並べます。",
            "植木鉢の下に受け皿を置きます。",
        ],
    ),
    (
        "葉っぱ",
        [
            "葉っぱに付いた土をそっと払います。",
            "葉っぱの写真を撮っておきます。",
        ],
    ),
    (
        "水やり",
        [
            "水やりをしたら、じょうろを片付けます。",
            "水やりの順番を家族と決めます。",
        ],
    ),
    (
        "じょうろ",
        [
            "じょうろをベランダの隅に置きます。",
            "じょうろを使ったあとで棚に戻します。",
        ],
    ),
    (
        "窓辺",
        [
            "窓辺に小さな鉢を並べます。",
            "窓辺の植物を眺めながら休みます。",
        ],
    ),
];
// These six near-complete branches promise both authors on page one in the
// default twelve-row pool. An explicit six-row pool retains older pagination
// when the old 上/窓 conversion and another Word compete with the raw row.
const PARTIALS: &[&str] = &["shokubuts", "uekibach", "happ", "mizuyar", "jour", "madob"];
const ALIASES: &[(&str, usize)] = &[
    ("syokubutu", 0),
    ("uekibati", 1),
    ("haxtsupa", 2),
    ("jyouro", 4),
];
const CHILD: &str = "SUZAKU_PLANTS_JAPANESE_CATALOG";

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn catalogs(name: &str, body: impl FnOnce()) {
    let mode = std::env::var(CHILD);
    if mode.is_err() {
        // Capture both catalogs even in the real before run: a builtin failure
        // must not prevent the independent nine-pack missing-data observation.
        let results = ["builtin", "packs"].map(|mode| {
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
        let details = results
            .iter()
            .map(|(mode, output)| {
                format!(
                    "{mode}: {}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            results.iter().all(|(_, output)| output.status.success()),
            "{details}"
        );
        for (mode, output) in &results {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
                "{stdout}"
            );
            assert!(
                stdout.contains(&format!("completed plants {name} with {mode}")),
                "{stdout}"
            );
        }
        return;
    }
    let mode = mode.unwrap();
    assert!(matches!(mode.as_str(), "builtin" | "packs"));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("suzaku-plants-ja-{}-{nonce}", std::process::id()));
    fs::create_dir(&path).unwrap();
    let owned = Owned(path);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    let selected = packs::recommended()
        .into_iter()
        .filter(|p| mode == "packs" && p.manifest.language == "ja")
        .collect::<Vec<_>>();
    let expected = if mode == "packs" { 9 } else { 0 };
    assert_eq!(selected.len(), expected);
    let ids = selected
        .iter()
        .map(|p| p.manifest.id.clone())
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), expected);
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(
        report.errors.is_empty() && report.disabled.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.loaded.len(), expected);
    assert_eq!(report.loaded.iter().cloned().collect::<HashSet<_>>(), ids);
    body();
    println!("completed plants {name} with {mode}");
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
}
fn raw_kind(ime: &XRTabletImeEngine, seed: &str) -> CandidateKind {
    let row = ime
        .candidates()
        .iter()
        .find(|c| c.text == seed)
        .unwrap_or_else(|| panic!("missing raw {seed:?}: {:?}", ime.candidates()));
    assert!(matches!(
        row.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    row.kind
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 2 * PAGE_SIZE);
    raw_kind(ime, seed);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local
                && c.label == c.text
                && c.score.is_finite()
                && (0.0..=100.0).contains(&c.score))
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn first_page(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert!(
        index(ime, text, kind) < PAGE_SIZE,
        "{text:?}: {:?}",
        ime.candidates()
    );
}
fn authors(ime: &XRTabletImeEngine, which: usize) {
    for sentence in SENTENCES[which].1 {
        first_page(ime, sentence, CandidateKind::Sentence);
    }
}
fn no_plants_authors(ime: &XRTabletImeEngine, seed: &str) {
    assert!(
        ime.candidates().iter().all(|c| !SENTENCES
            .iter()
            .flat_map(|(_, ss)| ss)
            .any(|sentence| c.text.contains(*sentence))),
        "unexpected plant author for {seed:?}: {:?}",
        ime.candidates()
    );
}
fn commit_once(ime: &mut XRTabletImeEngine, seed: &str, text: &str, kind: CandidateKind) {
    assert_eq!(ime.snapshot().seed_text, seed);
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn commit_raw(ime: &mut XRTabletImeEngine, seed: &str) {
    let kind = raw_kind(ime, seed);
    commit_once(ime, seed, seed, kind);
}

#[test]
fn plants_exact_five_new_readings_six_contexts_twelve_authors_and_one_reused_word() {
    assert_eq!(
        (
            WORDS.len(),
            NEW_READINGS.len(),
            SENTENCES.len(),
            PARTIALS.len()
        ),
        (6, 5, 6, 6)
    );
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 614 && ja.continuations().count() >= 339);
    for (entry, &(kana, word)) in ja.readings()[609..614].iter().zip(NEW_READINGS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(ja.readings()[..609].iter().all(|old| old.text != word));
    }
    let reused = &ja.readings()[176];
    assert_eq!((&*reused.reading, &*reused.text), ("しょくぶつ", "植物"));
    assert_eq!(reused.kind, EntryKind::Word);
    assert!(!reused.require_separators);
    for &(_, kana, _, word) in WORDS {
        assert_eq!(
            ja.readings()
                .iter()
                .filter(|e| e.reading == kana && e.text == word)
                .count(),
            1
        );
        assert_eq!(ja.readings().iter().filter(|e| e.text == word).count(), 1);
        assert!(
            ja.continuations()
                .take(333)
                .all(|(context, _)| context != word)
        );
        assert_eq!(
            ja.continuations()
                .filter(|(context, _)| *context == word)
                .count(),
            1
        );
    }
    for ((context, values), &(word, expected)) in
        ja.continuations().skip(333).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
        assert_ne!(expected[0], expected[1]);
        assert!(
            expected
                .iter()
                .all(|s| s.starts_with(word) && s.len() > word.len())
        );
    }
    assert_eq!(
        ja.continuations()
            .take(339)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        677
    );
}

#[test]
fn plants_old_609_readings_flags_333_contexts_665_authors_and_neighbors_stay() {
    const NAME: &str = "plants_old_609_readings_flags_333_contexts_665_authors_and_neighbors_stay";
    catalogs(NAME, || {
        fn feed(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
            for byte in bytes {
                *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        let ja = builtin("ja").unwrap();
        assert!(ja.readings().len() >= 609 && ja.continuations().count() >= 333);
        let mut h = 0xcbf29ce484222325;
        for entry in &ja.readings()[..609] {
            let kind = match entry.kind {
                EntryKind::Word => "word",
                EntryKind::Sentence => "sentence",
            };
            for value in [&*entry.reading, &*entry.text, kind] {
                feed(&mut h, value.bytes().chain([0]));
            }
            feed(&mut h, [u8::from(entry.require_separators)]);
        }
        assert_eq!(h, 0xf5c0c6947b96d0b2);
        let mut h = 0xcbf29ce484222325;
        let mut count = 0;
        for (context, sentences) in ja.continuations().take(333) {
            feed(&mut h, context.bytes().chain([0]));
            for sentence in sentences {
                feed(&mut h, sentence.bytes().chain([0]));
            }
            feed(&mut h, [0xff]);
            count += sentences.len();
        }
        assert_eq!((count, h), (665, 0x65e57220fcbaa923));
        for &(roman, kana, katakana, _) in WORDS {
            assert_eq!(romaji_to_hiragana(roman), kana);
            assert_eq!(romaji_to_hiragana(&roman.to_ascii_uppercase()), kana);
            assert_eq!(romaji_to_hiragana(katakana), katakana);
        }
        for &(alias, which) in ALIASES {
            assert_eq!(romaji_to_hiragana(alias), WORDS[which].1);
        }
        assert_eq!(romaji_to_hiragana("joro"), "じょろ");
        assert_eq!(romaji_to_hiragana("jo-ro"), "じょーろ");
        assert_eq!(romaji_to_hiragana("jyo-ro"), "じょーろ");
        assert_eq!(romaji_to_hiragana("di"), "ぢ");
        for (roman, word, sentences) in [
            (
                "hanami",
                "花見",
                ["花見に行きませんか。", "花見の予定を相談しましょう。"],
            ),
            (
                "hare",
                "晴れ",
                [
                    "晴れの日に洗濯物を干します。",
                    "晴れたら散歩に行きたいです。",
                ],
            ),
            (
                "madoguchi",
                "窓口",
                ["窓口はどこにありますか。", "窓口で手続きをしたいです。"],
            ),
            (
                "shiori",
                "栞",
                [
                    "栞を挟んでから本を閉じます。",
                    "栞が見つからないので、付箋を目印にします。",
                ],
            ),
        ] {
            let ime = engine(roman);
            draft(&ime, roman);
            first_page(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page(&ime, sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(roman), roman, word, CandidateKind::Word);
        }
        for (seed, expected) in [
            ("mado", &["窓枠", "窓口"][..]),
            ("jou", &["条件", "情報", "乗車", "乗車券"][..]),
            ("ha", &["はじめまして", "早い", "花見"][..]),
        ] {
            let ime = engine(seed);
            draft(&ime, seed);
            let positions = expected
                .iter()
                .map(|word| index(&ime, word, CandidateKind::Word))
                .collect::<Vec<_>>();
            assert!(positions.windows(2).all(|v| v[0] < v[1]));
            commit_raw(&mut engine(seed), seed);
        }
        first_page(&engine("ue"), "上", CandidateKind::Word);
        first_page(&engine("hizashi"), "日差し", CandidateKind::Word);
        if std::env::var(CHILD).as_deref() == Ok("packs") {
            first_page(&engine("mado"), "窓", CandidateKind::Word);
            for (roman, word) in [("haha", "母"), ("kouen", "公園")] {
                first_page(&engine(roman), word, CandidateKind::Word);
            }
        }
    });
}

#[test]
fn plants_whole_case_kana_canonical_aliases_and_six_bounded_partials_commit_once_in_both_catalogs()
{
    const NAME: &str = "plants_whole_case_kana_canonical_aliases_and_six_bounded_partials_commit_once_in_both_catalogs";
    catalogs(NAME, || {
        for (which, &(roman, kana, katakana, word)) in WORDS.iter().enumerate() {
            for seed in [
                roman.to_owned(),
                roman.to_ascii_uppercase(),
                kana.to_owned(),
                word.to_owned(),
            ] {
                let ime = engine(&seed);
                draft(&ime, &seed);
                let kind = if seed == word {
                    raw_kind(&ime, &seed)
                } else {
                    CandidateKind::Word
                };
                first_page(&ime, word, kind);
                authors(&ime, which);
                commit_once(&mut engine(&seed), &seed, word, kind);
                for sentence in SENTENCES[which].1 {
                    commit_once(&mut engine(&seed), &seed, sentence, CandidateKind::Sentence);
                }
                commit_raw(&mut engine(&seed), &seed);
            }
            let ime = engine(katakana);
            draft(&ime, katakana);
            if katakana == word {
                authors(&ime, which);
            } else {
                no_plants_authors(&ime, katakana);
            }
            commit_raw(&mut engine(katakana), katakana);
            let partial = PARTIALS[which];
            let ime = engine(partial);
            draft(&ime, partial);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
            let six = offline(
                "ja",
                partial,
                "",
                JapaneseLanguagePlugin
                    .direct_candidates(partial, 1.0)
                    .unwrap(),
                PAGE_SIZE,
            );
            assert!(six.len() <= PAGE_SIZE);
            let pack_mode = std::env::var(CHILD).as_deref() == Ok("packs");
            // The legacy six-row/raw budget only has one authored slot for
            // these exact old conversion paths. The default twelve-row pool
            // above still puts both real authors on its first page.
            let narrow = which == 1 || (which == 5 && pack_mode);
            let expected_authors = if narrow {
                &SENTENCES[which].1[..1]
            } else {
                &SENTENCES[which].1[..]
            };
            for &(text, kind) in &[
                (word, CandidateKind::Word),
                (partial, CandidateKind::Literal),
            ] {
                assert!(
                    six.iter().any(|c| c.text == text && c.kind == kind),
                    "{partial}: {six:?}"
                );
            }
            for sentence in expected_authors {
                assert!(
                    six.iter()
                        .any(|c| c.text == *sentence && c.kind == CandidateKind::Sentence),
                    "{partial}: {six:?}"
                );
            }
            if narrow {
                assert!(!six.iter().any(|c| c.text == SENTENCES[which].1[1]));
                let expected = if which == 1 {
                    vec![
                        ("上きばch", "unknown", 1120403456),
                        ("うえきばch", "unknown", 1119354880),
                        ("植木鉢", "word", 1119158272),
                        ("ウエキバch", "unknown", 1119092736),
                        (SENTENCES[1].1[0], "sentence", 1117519872),
                        ("uekibach", "literal", 1113325568),
                    ]
                } else {
                    vec![
                        ("窓b", "unknown", 1120403456),
                        ("まどb", "unknown", 1119354880),
                        ("窓辺", "word", 1119158272),
                        ("窓勉強", "word", 1119092736),
                        (SENTENCES[5].1[0], "sentence", 1117323264),
                        ("madob", "literal", 1113325568),
                    ]
                };
                assert_eq!(
                    six.iter()
                        .map(|c| (
                            c.text.as_str(),
                            c.label.as_str(),
                            c.kind.id(),
                            c.source.id(),
                            c.score.to_bits()
                        ))
                        .collect::<Vec<_>>(),
                    expected
                        .iter()
                        .map(|&(text, kind, bits)| (text, text, kind, "local", bits))
                        .collect::<Vec<_>>()
                );
            }
            commit_once(&mut engine(partial), partial, word, CandidateKind::Word);
            for sentence in SENTENCES[which].1 {
                commit_once(
                    &mut engine(partial),
                    partial,
                    sentence,
                    CandidateKind::Sentence,
                );
            }
            commit_raw(&mut engine(partial), partial);
        }
        for &(seed, which) in ALIASES {
            let ime = engine(seed);
            draft(&ime, seed);
            first_page(&ime, WORDS[which].3, CandidateKind::Word);
            authors(&ime, which);
            commit_once(&mut engine(seed), seed, WORDS[which].3, CandidateKind::Word);
            for sentence in SENTENCES[which].1 {
                commit_once(&mut engine(seed), seed, sentence, CandidateKind::Sentence);
            }
            commit_raw(&mut engine(seed), seed);
        }
    });
}

#[test]
fn plants_six_horizontal_gaps_and_known_word_adoption_reseed_particles_keep_raw() {
    const NAME: &str =
        "plants_six_horizontal_gaps_and_known_word_adoption_reseed_particles_keep_raw";
    catalogs(NAME, || {
        let splits = [
            ("shoku", "BUTSU"),
            ("ueki", "BACHI"),
            ("ha", "PPA"),
            ("mizu", "YARI"),
            ("jou", "RO"),
            ("mado", "BE"),
        ];
        for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
            for (which, (left, right)) in splits.iter().enumerate() {
                let seed = format!("{separator}{left}{separator}{right}{separator}");
                let ime = engine(&seed);
                draft(&ime, &seed);
                first_page(&ime, WORDS[which].3, CandidateKind::Word);
                authors(&ime, which);
                commit_once(
                    &mut engine(&seed),
                    &seed,
                    WORDS[which].3,
                    CandidateKind::Word,
                );
                commit_raw(&mut engine(&seed), &seed);
            }
        }
        for (which, particle) in ["wo", "wo", "ni", "wo", "wo", "ni"].iter().enumerate() {
            let (reading, _, _, word) = WORDS[which];
            let mut ime = engine(reading);
            ime.select_candidate(index(&ime, word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word));
            assert_eq!(ime.snapshot().seed_text, reading);
            assert!(ime.snapshot().committed_text.is_empty());
            for separator in ["", " ", "\u{3000}"] {
                let seed = format!("{word}{separator}{particle}");
                ime.seed(&seed);
                draft(&ime, &seed);
                first_page(&ime, SENTENCES[which].1[0], CandidateKind::Sentence);
                commit_once(
                    &mut engine(&seed),
                    &seed,
                    SENTENCES[which].1[0],
                    CandidateKind::Sentence,
                );
                commit_raw(&mut engine(&seed), &seed);
                ime.seed(reading);
                draft(&ime, reading);
                first_page(&ime, word, CandidateKind::Word);
                authors(&ime, which);
            }
        }
    });
}

#[test]
fn plants_protected_unknown_unicode_long_drafts_and_seven_lines_never_recover_authors() {
    const NAME: &str =
        "plants_protected_unknown_unicode_long_drafts_and_seven_lines_never_recover_authors";
    catalogs(NAME, || {
        let mut seeds = [
            "https://example.invalid/mizuyari",
            "user+jouro@example.invalid",
            "xyzmizuyari",
            "🙂葉っぱni",
            "窓辺\u{200b}ni",
            "ｊｏｕｒｏ",
            "joro",
            "jo-ro",
            "jyo-ro",
            "植木鉢xyz",
            "葉っぱ\u{fe0f}",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let splits = [
            ("shoku", "butsu"),
            ("ueki", "bachi"),
            ("ha", "ppa"),
            ("mizu", "yari"),
            ("jou", "ro"),
            ("mado", "be"),
        ];
        for boundary in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            for (which, (left, right)) in splits.iter().enumerate() {
                seeds.push(format!("{left}{boundary}{right}"));
                seeds.push(format!("{}{boundary}wo", WORDS[which].3));
            }
        }
        for &(reading, _, _, word) in WORDS {
            for tail in [reading.to_owned(), format!("{word}wo")] {
                let seed = format!("{}{tail}", "前文。".repeat(90));
                assert!(seed.chars().count() > 256);
                seeds.push(seed);
            }
        }
        for seed in seeds {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            no_plants_authors(&ime, &seed);
            commit_raw(&mut ime, &seed);
        }
    });
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-plants-synthetic-model-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.0.clone().map_or_else(|| Ok(Vec::new()), Err)
    }
}

#[test]
fn plants_pending_absent_typed_failures_timeouts_and_empty_models_preserve_full_local_pools() {
    const NAME: &str =
        "plants_pending_absent_typed_failures_timeouts_and_empty_models_preserve_full_local_pools";
    catalogs(NAME, || {
        for (which, &(reading, _, _, word)) in WORDS.iter().enumerate() {
            let baseline = engine(reading).candidates().to_vec();
            let mut disabled = engine(reading);
            disabled.configure_prediction(None);
            assert_eq!(disabled.prediction_status(), PredictionStatus::Disabled);
            assert!(disabled.prediction_error().is_none());
            assert_eq!(disabled.candidates(), baseline);
            authors(&disabled, which);
            commit_once(
                &mut disabled,
                reading,
                SENTENCES[which].1[0],
                CandidateKind::Sentence,
            );
            for error in [
                Some(LlmProviderError::NoLocalModel),
                Some(LlmProviderError::Unavailable),
                Some(LlmProviderError::Timeout),
                Some(LlmProviderError::HttpStatus(503)),
                Some(LlmProviderError::InvalidResponse),
                Some(LlmProviderError::ResponseTooLarge),
                Some(LlmProviderError::MissingCredentials),
                None,
            ] {
                let mut ime = engine("");
                ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
                ime.seed(reading);
                assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
                assert!(ime.prediction_pending());
                assert!(ime.prediction_error().is_none());
                draft(&ime, reading);
                first_page(&ime, word, CandidateKind::Word);
                authors(&ime, which);
                assert_eq!(ime.candidates(), baseline);
                let deadline = Instant::now() + Duration::from_secs(2);
                while ime.prediction_pending() && Instant::now() < deadline {
                    ime.poll_prediction();
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert!(!ime.prediction_pending());
                assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                assert_eq!(
                    ime.prediction_error(),
                    Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
                );
                assert_eq!(ime.candidates(), baseline);
                commit_once(
                    &mut ime,
                    reading,
                    SENTENCES[which].1[0],
                    CandidateKind::Sentence,
                );
            }
        }
    });
}
