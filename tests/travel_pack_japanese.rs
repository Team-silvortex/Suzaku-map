//! Authored optional lodging vocabulary, exercised in private engine snapshots.
//! No booking, hotel policy, physical keys, native service or model is involved.
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::{
        japanese::romaji_to_hiragana,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{
        self, EntryKind,
        packs::{self, OfflinePack, PackStore},
    },
};

const PACK_ID: &str = "org.suzaku.ja.travel";
const CHILD_CASE: &str = "SUZAKU_TRAVEL_JAPANESE_CHILD_CASE";
const COEXIST_IDS: [&str; 6] = [
    "org.suzaku.ja.rail",
    "org.suzaku.ja.family",
    "org.suzaku.ja.internet",
    "org.suzaku.ja.shopping",
    PACK_ID,
    "org.suzaku.ja.work",
];

// Fixed independent expectations written before the optional JSON resource.
const WORDS: &[(&str, &str, &str)] = &[
    ("hoteru", "ほてる", "ホテル"),
    ("ryokan", "りょかん", "旅館"),
    ("shukuhaku", "しゅくはく", "宿泊"),
    ("furonto", "ふろんと", "フロント"),
    ("chekkuin", "ちぇっくいん", "チェックイン"),
    ("chekkuauto", "ちぇっくあうと", "チェックアウト"),
    ("yoyakubangou", "よやくばんごう", "予約番号"),
    ("nimotsu", "にもつ", "荷物"),
    ("su-tsuke-su", "すーつけーす", "スーツケース"),
    ("ru-muki-", "るーむきー", "ルームキー"),
    ("kyakushitsu", "きゃくしつ", "客室"),
    ("kuushitsu", "くうしつ", "空室"),
    ("manshitsu", "まんしつ", "満室"),
    ("renpaku", "れんぱく", "連泊"),
    ("ippaku", "いっぱく", "一泊"),
    ("sudomari", "すどまり", "素泊まり"),
    ("kin'enshitsu", "きんえんしつ", "禁煙室"),
    ("kitsuenshitsu", "きつえんしつ", "喫煙室"),
    ("shinguru", "しんぐる", "シングル"),
    ("tsuin", "ついん", "ツイン"),
    ("daburu", "だぶる", "ダブル"),
    ("robi-", "ろびー", "ロビー"),
    ("erebe-ta-", "えれべーたー", "エレベーター"),
    ("amenitexi", "あめにてぃ", "アメニティ"),
    ("basutaoru", "ばすたおる", "バスタオル"),
    ("surippa", "すりっぱ", "スリッパ"),
    ("kan'nai", "かんない", "館内"),
    ("daiyokujou", "だいよくじょう", "大浴場"),
    ("takuhaibin", "たくはいびん", "宅配便"),
    ("tenimotsu", "てにもつ", "手荷物"),
    ("azukari", "あずかり", "預かり"),
    ("shukuhakuryoukin", "しゅくはくりょうきん", "宿泊料金"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "ホテル",
        [
            "ホテルまでの行き方を教えてください。",
            "ホテルに着いたら連絡します。",
        ],
    ),
    (
        "旅館",
        [
            "旅館でゆっくり過ごしたいです。",
            "旅館の名前をもう一度教えてください。",
        ],
    ),
    (
        "宿泊",
        [
            "宿泊する日を確認したいです。",
            "宿泊について問い合わせたいです。",
        ],
    ),
    (
        "フロント",
        ["フロントはどこですか。", "フロントに荷物を預けられますか。"],
    ),
    (
        "チェックイン",
        [
            "チェックインをお願いします。",
            "チェックインは何時からですか。",
        ],
    ),
    (
        "チェックアウト",
        [
            "チェックアウトをお願いします。",
            "チェックアウトの時間を教えてください。",
        ],
    ),
    (
        "予約番号",
        [
            "予約番号を確認します。",
            "予約番号はどこに書いてありますか。",
        ],
    ),
    (
        "荷物",
        ["荷物を預かってもらえますか。", "荷物を受け取りに来ました。"],
    ),
    (
        "スーツケース",
        [
            "スーツケースを部屋に置いてきます。",
            "スーツケースを預けたいです。",
        ],
    ),
    (
        "ルームキー",
        [
            "ルームキーの使い方を教えてください。",
            "ルームキーを部屋に忘れました。",
        ],
    ),
    (
        "空室",
        [
            "空室があるか確認してもらえますか。",
            "空室の種類を教えてください。",
        ],
    ),
    (
        "連泊",
        [
            "連泊を希望しています。",
            "連泊中の清掃について教えてください。",
        ],
    ),
];

fn travel() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == PACK_ID)
        .expect("the Japanese lodging pack has its stable catalog ID")
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("suzaku-ja-travel-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
    fn initialize(&self, enabled: bool, coexist: bool) {
        let store = self.store();
        assert!(store.list().unwrap().is_empty());
        let selected = if coexist {
            let catalog = packs::recommended();
            COEXIST_IDS
                .into_iter()
                .map(|id| {
                    catalog
                        .iter()
                        .find(|pack| pack.manifest.id == id)
                        .unwrap()
                        .clone()
                })
                .collect::<Vec<_>>()
        } else {
            vec![travel()]
        };
        assert!(selected.len() <= packs::MAX_PACKS);
        for pack in &selected {
            store.install(pack.clone(), false).unwrap();
        }
        if !enabled {
            store.set_enabled(PACK_ID, false).unwrap();
        }
        let report = packs::initialize(&store).unwrap();
        assert!(report.errors.is_empty(), "{report:?}");
        if enabled {
            assert_eq!(report.loaded.len(), selected.len());
            assert!(report.loaded.iter().any(|id| id == PACK_ID));
        } else {
            assert!(report.loaded.is_empty());
            assert_eq!(report.disabled, [PACK_ID]);
            assert!(std::ptr::eq(
                lexicon::active("ja").unwrap(),
                lexicon::builtin("ja").unwrap()
            ));
        }
        let builtin = lexicon::builtin("ja").unwrap();
        let active = lexicon::active("ja").unwrap();
        assert_eq!(
            builtin.readings(),
            &active.readings()[..builtin.readings().len()]
        );
        assert_eq!(
            builtin.continuations().collect::<Vec<_>>(),
            active
                .continuations()
                .take(builtin.continuations().count())
                .collect::<Vec<_>>()
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fresh(name: &str, body: impl FnOnce(&Fixture)) {
    if std::env::var(CHILD_CASE).as_deref() == Ok(name) {
        body(&Fixture::new());
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_CASE, name)
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
        "fresh {name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == seed && c.kind == CandidateKind::Literal)
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
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
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let position = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    if kind != CandidateKind::Literal {
        assert!(
            position < PAGE_SIZE,
            "{text:?} is not on the first page: {:?}",
            ime.candidates()
        );
    }
    position
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn choices(seed: &str, word: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    let kind = if seed == word {
        CandidateKind::Literal
    } else {
        CandidateKind::Word
    };
    index(&ime, word, kind);
    commit_once(&mut engine(seed), word, kind);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
        for sentence in sentences {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
}
fn complete_workflows() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
            word.to_owned(),
        ] {
            choices(&seed, word);
        }
    }
    for &(word, _) in SENTENCES {
        choices(word, word);
    }
}

#[test]
fn travel_japanese_has_fixed_novel_readings_and_authored_sentences() {
    let pack = travel();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "ja");
    assert_eq!(pack.manifest.topics, ["travel", "lodging"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    assert_eq!(pack.lexicon.readings().len(), WORDS.len());
    assert_eq!(pack.lexicon.continuations().count(), SENTENCES.len());
    for (entry, &(romaji, kana, word)) in pack.lexicon.readings().iter().zip(WORDS) {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, values), &(word, sentences)) in pack.lexicon.continuations().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            sentences
        );
        assert!(sentences.iter().all(|text| text.starts_with(word)));
    }
    assert_eq!(
        pack.lexicon
            .continuations()
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        24
    );
    let catalog = packs::recommended();
    for lexicon in std::iter::once(lexicon::builtin("ja").unwrap()).chain(
        catalog
            .iter()
            .filter(|other| other.manifest.language == "ja" && other.manifest.id != PACK_ID)
            .map(|other| &other.lexicon),
    ) {
        for &(_, kana, word) in WORDS {
            assert!(
                lexicon
                    .readings()
                    .iter()
                    .all(|entry| entry.reading != kana && entry.text != word),
                "collision: {kana}/{word}"
            );
        }
        for &(context, sentences) in SENTENCES {
            for (old_context, old_sentences) in lexicon.continuations() {
                assert_ne!(context, old_context);
                assert!(
                    sentences
                        .iter()
                        .all(|s| old_sentences.iter().all(|old| old != s))
                );
            }
        }
    }
}

fn assert_unavailable() {
    for &(romaji, _, word) in WORDS {
        let mut ime = engine(romaji);
        draft(&ime, romaji);
        // Generic Katakana transliteration is not an optional typed Word.
        assert!(
            ime.candidates()
                .iter()
                .all(|c| c.text != word || c.kind != CandidateKind::Word)
        );
        commit_once(&mut ime, romaji, CandidateKind::Literal);
    }
    for &(word, sentences) in SENTENCES {
        let ime = engine(word);
        assert!(
            sentences
                .iter()
                .all(|s| ime.candidates().iter().all(|c| c.text != *s))
        );
    }
}
#[test]
fn travel_japanese_absent_catalog_preserves_builtin_without_registry_writes() {
    fresh(
        "travel_japanese_absent_catalog_preserves_builtin_without_registry_writes",
        |fixture| {
            let _pack = travel();
            let store = fixture.store();
            let report = packs::initialize(&store).unwrap();
            assert!(report.loaded.is_empty() && report.errors.is_empty());
            assert!(std::ptr::eq(
                lexicon::active("ja").unwrap(),
                lexicon::builtin("ja").unwrap()
            ));
            assert_unavailable();
            assert!(!store.path().exists());
        },
    );
}
#[test]
fn travel_japanese_disabled_pack_preserves_builtin_choices() {
    fresh(
        "travel_japanese_disabled_pack_preserves_builtin_choices",
        |fixture| {
            fixture.initialize(false, false);
            assert_unavailable();
        },
    );
}
#[test]
fn travel_japanese_complete_romaji_case_kana_and_words_offer_both_sentences() {
    fresh(
        "travel_japanese_complete_romaji_case_kana_and_words_offer_both_sentences",
        |fixture| {
            fixture.initialize(true, false);
            complete_workflows();
        },
    );
}
#[test]
fn travel_japanese_selected_partials_and_reading_separators_preserve_choices() {
    fresh(
        "travel_japanese_selected_partials_and_reading_separators_preserve_choices",
        |fixture| {
            fixture.initialize(true, false);
            for (seed, word) in [
                ("hoter", "ホテル"),
                ("ほて", "ホテル"),
                ("ryoka", "旅館"),
                ("りょか", "旅館"),
                ("furont", "フロント"),
                ("ふろん", "フロント"),
                ("chekkuaut", "チェックアウト"),
                ("ちぇっくあう", "チェックアウト"),
                ("su-tsuke-s", "スーツケース"),
                ("すーつけー", "スーツケース"),
                ("ru-muk", "ルームキー"),
                ("るーむき", "ルームキー"),
                ("kuushits", "空室"),
                ("くうし", "空室"),
            ] {
                choices(seed, word);
            }
            for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
                for (left, right, word) in [
                    ("HO", "TERU", "ホテル"),
                    ("furon", "to", "フロント"),
                    ("すーつ", "けーす", "スーツケース"),
                ] {
                    choices(&format!("{space}{left}{space}{right}{space}"), word);
                }
            }
            for seed in ["kin’enshitsu", "きんえん しつ"] {
                choices(seed, "禁煙室");
            }
        },
    );
}
#[test]
fn travel_japanese_adopted_words_allow_editable_sentence_progress() {
    fresh(
        "travel_japanese_adopted_words_allow_editable_sentence_progress",
        |fixture| {
            fixture.initialize(true, false);
            for (reading, word, tail, sentence) in [
                ("hoteru", "ホテル", "ni", "ホテルに着いたら連絡します。"),
                ("ryokan", "旅館", "de", "旅館でゆっくり過ごしたいです。"),
                ("furonto", "フロント", "ha", "フロントはどこですか。"),
                (
                    "chekkuin",
                    "チェックイン",
                    "wo",
                    "チェックインをお願いします。",
                ),
                (
                    "chekkuauto",
                    "チェックアウト",
                    "の",
                    "チェックアウトの時間を教えてください。",
                ),
                ("yoyakubangou", "予約番号", "WO", "予約番号を確認します。"),
                ("nimotsu", "荷物", "を", "荷物を預かってもらえますか。"),
                (
                    "ru-muki-",
                    "ルームキー",
                    "no",
                    "ルームキーの使い方を教えてください。",
                ),
            ] {
                let mut ime = engine(reading);
                ime.select_candidate(index(&ime, word, CandidateKind::Word));
                assert_eq!(ime.selected_completion_text(true), Some(word));
                assert!(ime.snapshot().committed_text.is_empty());
                let spaced = format!("{word} ");
                ime.seed(&spaced);
                draft(&ime, &spaced);
                index(&ime, sentence, CandidateKind::Sentence);
                let continued = format!("{word} {tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                commit_once(&mut ime, sentence, CandidateKind::Sentence);
                commit_once(&mut engine(&continued), &continued, CandidateKind::Literal);
                ime.seed(reading);
                draft(&ime, reading);
                index(&ime, word, CandidateKind::Word);
            }
        },
    );
}
#[test]
fn travel_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact() {
    fresh(
        "travel_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact",
        |fixture| {
            fixture.initialize(true, false);
            let mut seeds: Vec<String> = [
                "https://example.invalid/hoteru?q=ryokan",
                "furonto@example.invalid",
                "let hoteru = \"ryokan\";",
                "C:/files/chekkuin.txt",
                "xyzhoteru",
                "🙂ホテルni",
                "荷物woxyz",
                "チェックインをが",
                "フロント{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("ho{boundary}teru"));
                seeds.push(format!("ホテル{boundary}ni"));
            }
            for tail in ["hoteru", "チェックインwo", "荷物wo"] {
                let seed = format!("{}{tail}", "前文。".repeat(90));
                assert!(seed.chars().count() > 256);
                seeds.push(seed);
            }
            for seed in seeds {
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                assert!(
                    ime.candidates().iter().all(|c| !SENTENCES
                        .iter()
                        .flat_map(|(_, ss)| ss)
                        .any(|s| c.text.ends_with(s))),
                    "manufactured sentence from {seed:?}: {:?}",
                    ime.candidates()
                );
                commit_once(&mut ime, &seed, CandidateKind::Literal);
            }
        },
    );
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-travel-synthetic-failure"
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
fn travel_japanese_fallback_survives_typed_and_empty_model_failures() {
    fresh(
        "travel_japanese_fallback_survives_typed_and_empty_model_failures",
        |fixture| {
            fixture.initialize(true, false);
            for error in [
                Some(LlmProviderError::NoLocalModel),
                Some(LlmProviderError::Unavailable),
                Some(LlmProviderError::Timeout),
                Some(LlmProviderError::HttpStatus(503)),
                None,
            ] {
                for (seed, word, sentence) in [
                    ("hoteru", "ホテル", "ホテルに着いたら連絡します。"),
                    ("chekkuin", "チェックイン", "チェックインをお願いします。"),
                    ("nimotsu", "荷物", "荷物を預かってもらえますか。"),
                ] {
                    let mut ime = engine("");
                    ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
                    ime.seed(seed);
                    draft(&ime, seed);
                    index(&ime, word, CandidateKind::Word);
                    index(&ime, sentence, CandidateKind::Sentence);
                    let before = ime.candidates().to_vec();
                    let deadline = Instant::now() + Duration::from_secs(2);
                    while ime.prediction_pending() && Instant::now() < deadline {
                        ime.poll_prediction();
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                    assert_eq!(
                        ime.prediction_error(),
                        Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
                    );
                    assert_eq!(ime.candidates(), before);
                    commit_once(&mut ime, sentence, CandidateKind::Sentence);
                }
            }
        },
    );
}
#[test]
fn travel_japanese_coexists_with_fixed_six_japanese_packs_preserving_prior_order() {
    fresh(
        "travel_japanese_coexists_with_fixed_six_japanese_packs_preserving_prior_order",
        |fixture| {
            fixture.initialize(true, true);
            complete_workflows();
            let builtin = lexicon::builtin("ja").unwrap();
            let active = lexicon::active("ja").unwrap();
            let mut offset = builtin.readings().len();
            for id in COEXIST_IDS {
                let pack = packs::recommended()
                    .into_iter()
                    .find(|pack| pack.manifest.id == id)
                    .unwrap();
                let end = offset + pack.lexicon.readings().len();
                assert_eq!(pack.lexicon.readings(), &active.readings()[offset..end]);
                offset = end;
            }
            assert_eq!(offset, active.readings().len());
            for (seed, first, other) in [("kanji", "漢字", "感じ"), ("sentaku", "選択", "洗濯")]
            {
                let ime = engine(seed);
                assert_eq!(ime.candidates()[0].text, first);
                let first_position = ime
                    .candidates()
                    .iter()
                    .position(|c| c.text == first)
                    .unwrap();
                let other_position = ime
                    .candidates()
                    .iter()
                    .position(|c| c.text == other)
                    .unwrap();
                assert!(first_position < other_position);
            }
            for (seed, word, sentence) in [
                ("kaisatsu", "改札", "改札はどこですか。"),
                ("kazoku", "家族", "家族で一緒に出かけませんか。"),
                ("kensaku", "検索", "検索しても見つかりません。"),
                ("sentakumono", "洗濯物", "洗濯物を取り込んでおきます。"),
            ] {
                let mut ime = engine(seed);
                draft(&ime, seed);
                index(&ime, word, CandidateKind::Word);
                commit_once(&mut ime, sentence, CandidateKind::Sentence);
            }
        },
    );
}
