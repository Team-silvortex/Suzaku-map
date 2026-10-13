//! Authored optional study and writing vocabulary in private snapshots. No
//! coursework, documents, network, native keyboard or user preferences are used.
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

const PACK_ID: &str = "org.suzaku.ja.study";
const CHILD_CASE: &str = "SUZAKU_STUDY_JAPANESE_CHILD_CASE";
const JAPANESE_IDS: [&str; 9] = [
    "org.suzaku.ja.rail",
    "org.suzaku.ja.family",
    "org.suzaku.ja.internet",
    "org.suzaku.ja.shopping",
    "org.suzaku.ja.travel",
    "org.suzaku.ja.work",
    PACK_ID,
    "org.suzaku.ja.cooking",
    "org.suzaku.ja.outdoors",
];

// Independent fixed expectations, written before the JSON resource/catalog entry.
const WORDS: &[(&str, &str, &str)] = &[
    ("yoshuu", "よしゅう", "予習"),
    ("fukushuu", "ふくしゅう", "復習"),
    ("kyoukasho", "きょうかしょ", "教科書"),
    ("mondaishuu", "もんだいしゅう", "問題集"),
    ("sankousho", "さんこうしょ", "参考書"),
    ("tangochou", "たんごちょう", "単語帳"),
    ("ondoku", "おんどく", "音読"),
    ("dokkai", "どっかい", "読解"),
    ("choukai", "ちょうかい", "聴解"),
    ("anki", "あんき", "暗記"),
    ("saiten", "さいてん", "採点"),
    ("gotou", "ごとう", "誤答"),
    ("seitou", "せいとう", "正答"),
    ("kaisetsu", "かいせつ", "解説"),
    ("enshuu", "えんしゅう", "演習"),
    ("jishuu", "じしゅう", "自習"),
    ("gakushuukeikaku", "がくしゅうけいかく", "学習計画"),
    ("gakushuukiroku", "がくしゅうきろく", "学習記録"),
    ("fukushuumondai", "ふくしゅうもんだい", "復習問題"),
    ("ronbun", "ろんぶん", "論文"),
    ("sakubun", "さくぶん", "作文"),
    ("kousei", "こうせい", "校正"),
    ("suikou", "すいこう", "推敲"),
    ("danraku", "だんらく", "段落"),
    ("kutouten", "くとうてん", "句読点"),
    ("in'you", "いんよう", "引用"),
    ("shutten", "しゅってん", "出典"),
    ("kyakuchuu", "きゃくちゅう", "脚注"),
    ("sankoubunken", "さんこうぶんけん", "参考文献"),
    ("mokuji", "もくじ", "目次"),
    ("hikki", "ひっき", "筆記"),
    ("genkouyoushi", "げんこうようし", "原稿用紙"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "予習",
        [
            "予習は次の章まで進めておきます。",
            "予習で分からなかったところを質問します。",
        ],
    ),
    (
        "復習",
        [
            "復習をしてから新しい問題に取り組みます。",
            "復習の時間を毎日少しずつ取っています。",
        ],
    ),
    (
        "教科書",
        [
            "教科書の例をもう一度読んでみます。",
            "教科書を明日の授業に持っていきます。",
        ],
    ),
    (
        "問題集",
        [
            "問題集を一日一ページずつ進めています。",
            "問題集の間違えたところに印を付けました。",
        ],
    ),
    (
        "単語帳",
        [
            "単語帳に新しく覚えた言葉を書きます。",
            "単語帳を使って意味を確かめます。",
        ],
    ),
    (
        "音読",
        [
            "音読をすると文章の流れがつかみやすいです。",
            "音読の練習を一緒にしませんか。",
        ],
    ),
    (
        "読解",
        [
            "読解の問題は短い文章から始めます。",
            "読解では段落ごとの要点をまとめます。",
        ],
    ),
    (
        "学習計画",
        [
            "学習計画を週末に見直します。",
            "学習計画に休憩の時間も入れました。",
        ],
    ),
    (
        "作文",
        [
            "作文を読んで感想を聞かせてもらえますか。",
            "作文には身近な出来事を書こうと思います。",
        ],
    ),
    (
        "推敲",
        [
            "推敲をして表現の重なりを減らしました。",
            "推敲の前に少し時間を置いてみます。",
        ],
    ),
    (
        "引用",
        [
            "引用した部分が分かるように印を付けます。",
            "引用の出典をもう一度確かめます。",
        ],
    ),
    (
        "参考文献",
        [
            "参考文献を最後にまとめて載せます。",
            "参考文献の書き方を教えてもらえますか。",
        ],
    ),
];

fn study() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|p| p.manifest.id == PACK_ID)
        .expect("the optional Japanese study pack has its stable catalog ID")
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("suzaku-ja-study-{}", std::process::id()));
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
            JAPANESE_IDS
                .into_iter()
                .map(|id| {
                    catalog
                        .iter()
                        .find(|p| p.manifest.id == id)
                        .unwrap()
                        .clone()
                })
                .collect::<Vec<_>>()
        } else {
            vec![study()]
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
            "{text:?} is not on page one: {:?}",
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
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
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
    if let Some((_, ss)) = SENTENCES.iter().find(|(context, _)| *context == word) {
        for sentence in ss {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn study_japanese_has_fixed_novel_readings_and_authored_sentences() {
    let pack = study();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "ja");
    assert_eq!(pack.manifest.topics, ["study", "writing"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    assert_eq!(pack.lexicon.readings().len(), WORDS.len());
    assert_eq!(pack.lexicon.continuations().count(), SENTENCES.len());
    for (entry, &(romaji, kana, word)) in pack.lexicon.readings().iter().zip(WORDS) {
        assert_eq!(romaji_to_hiragana(romaji), kana);
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, values), &(word, ss)) in pack.lexicon.continuations().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(values.iter().map(String::as_str).collect::<Vec<_>>(), ss);
        assert!(ss.iter().all(|s| s.starts_with(word)));
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
            .filter(|p| p.manifest.language == "ja" && p.manifest.id != PACK_ID)
            .map(|p| &p.lexicon),
    ) {
        for &(_, kana, word) in WORDS {
            assert!(
                lexicon
                    .readings()
                    .iter()
                    .all(|e| e.reading != kana && e.text != word),
                "old collision: {kana}/{word}"
            );
        }
        for &(context, ss) in SENTENCES {
            for (old_context, old_ss) in lexicon.continuations() {
                assert_ne!(context, old_context);
                assert!(ss.iter().all(|s| old_ss.iter().all(|old| old != s)));
            }
        }
    }
}

fn assert_unavailable() {
    for &(romaji, _, word) in WORDS {
        let mut ime = engine(romaji);
        draft(&ime, romaji);
        // An existing segmented conversion is not an installed Word entry.
        assert!(
            ime.candidates()
                .iter()
                .all(|c| c.text != word || c.kind != CandidateKind::Word)
        );
        commit_once(&mut ime, romaji, CandidateKind::Literal);
    }
    for &(word, ss) in SENTENCES {
        let ime = engine(word);
        assert!(
            ss.iter()
                .all(|s| ime.candidates().iter().all(|c| c.text != *s))
        );
    }
}
#[test]
fn study_japanese_absent_catalog_keeps_builtin_and_creates_no_store() {
    fresh(
        "study_japanese_absent_catalog_keeps_builtin_and_creates_no_store",
        |fixture| {
            let _pack = study();
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
fn study_japanese_disabled_pack_preserves_builtin_choices() {
    fresh(
        "study_japanese_disabled_pack_preserves_builtin_choices",
        |fixture| {
            fixture.initialize(false, false);
            assert_unavailable();
        },
    );
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
fn study_japanese_complete_romaji_case_kana_and_words_offer_both_sentences() {
    fresh(
        "study_japanese_complete_romaji_case_kana_and_words_offer_both_sentences",
        |fixture| {
            fixture.initialize(true, false);
            complete_workflows();
        },
    );
}
#[test]
fn study_japanese_selected_partials_and_reading_separators_preserve_choices() {
    fresh(
        "study_japanese_selected_partials_and_reading_separators_preserve_choices",
        |fixture| {
            fixture.initialize(true, false);
            for (seed, word) in [
                ("yoshu", "予習"),
                ("よしゅ", "予習"),
                ("kyoukash", "教科書"),
                ("きょうかし", "教科書"),
                ("mondaishu", "問題集"),
                ("もんだいしゅ", "問題集"),
                ("tangoch", "単語帳"),
                ("たんごちょ", "単語帳"),
                ("ondok", "音読"),
                ("おんど", "音読"),
                ("gakushuukeikak", "学習計画"),
                ("がくしゅうけいか", "学習計画"),
                ("sankoubunke", "参考文献"),
                ("さんこうぶんけ", "参考文献"),
            ] {
                choices(seed, word);
            }
            for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
                for (left, right, word) in [
                    ("KYOU", "KASHO", "教科書"),
                    ("tango", "chou", "単語帳"),
                    ("がくしゅう", "けいかく", "学習計画"),
                    ("sankou", "bunken", "参考文献"),
                ] {
                    choices(&format!("{space}{left}{space}{right}{space}"), word);
                }
            }
            for seed in ["in’you", "いん よう"] {
                choices(seed, "引用");
            }
        },
    );
}
#[test]
fn study_japanese_adopted_words_continue_without_committing_draft() {
    fresh(
        "study_japanese_adopted_words_continue_without_committing_draft",
        |fixture| {
            fixture.initialize(true, false);
            for (reading, word, tail, sentence) in [
                ("yoshuu", "予習", "ha", "予習は次の章まで進めておきます。"),
                (
                    "fukushuu",
                    "復習",
                    "wo",
                    "復習をしてから新しい問題に取り組みます。",
                ),
                (
                    "kyoukasho",
                    "教科書",
                    "の",
                    "教科書の例をもう一度読んでみます。",
                ),
                (
                    "mondaishuu",
                    "問題集",
                    "WO",
                    "問題集を一日一ページずつ進めています。",
                ),
                (
                    "tangochou",
                    "単語帳",
                    "ni",
                    "単語帳に新しく覚えた言葉を書きます。",
                ),
                (
                    "ondoku",
                    "音読",
                    "wo",
                    "音読をすると文章の流れがつかみやすいです。",
                ),
                ("dokkai", "読解", "no", "読解の問題は短い文章から始めます。"),
                (
                    "gakushuukeikaku",
                    "学習計画",
                    "wo",
                    "学習計画を週末に見直します。",
                ),
                (
                    "sakubun",
                    "作文",
                    "wo",
                    "作文を読んで感想を聞かせてもらえますか。",
                ),
                ("suikou", "推敲", "no", "推敲の前に少し時間を置いてみます。"),
                (
                    "in'you",
                    "引用",
                    "shita",
                    "引用した部分が分かるように印を付けます。",
                ),
                (
                    "sankoubunken",
                    "参考文献",
                    "wo",
                    "参考文献を最後にまとめて載せます。",
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
fn study_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact() {
    fresh(
        "study_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact",
        |fixture| {
            fixture.initialize(true, false);
            let mut seeds: Vec<String> = [
                "https://example.invalid/kyoukasho?q=yoshuu",
                "sakubun@example.invalid",
                "let yoshuu = \"kyoukasho\";",
                "C:/study/sakubun.txt",
                "xyzsankoubunken",
                "🙂単語帳wo",
                "推敲noxyz",
                "復習をが",
                "教科書{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("kyou{boundary}kasho"));
                seeds.push(format!("問題集{boundary}wo"));
            }
            for tail in ["kyoukasho", "引用shita", "復習wo"] {
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
        "ja-study-synthetic-failure"
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
fn study_japanese_fallback_survives_typed_and_empty_model_failures() {
    fresh(
        "study_japanese_fallback_survives_typed_and_empty_model_failures",
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
                    ("kyoukasho", "教科書", "教科書の例をもう一度読んでみます。"),
                    (
                        "gakushuukeikaku",
                        "学習計画",
                        "学習計画を週末に見直します。",
                    ),
                    ("in'you", "引用", "引用した部分が分かるように印を付けます。"),
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
fn study_japanese_coexists_with_fixed_nine_packs_and_preserves_prior_japanese_order() {
    fresh(
        "study_japanese_coexists_with_fixed_nine_packs_and_preserves_prior_japanese_order",
        |fixture| {
            fixture.initialize(true, true);
            complete_workflows();
            let builtin = lexicon::builtin("ja").unwrap();
            let active = lexicon::active("ja").unwrap();
            let mut offset = builtin.readings().len();
            for id in JAPANESE_IDS {
                let pack = packs::recommended()
                    .into_iter()
                    .find(|p| p.manifest.id == id)
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
                assert!(
                    ime.candidates()
                        .iter()
                        .position(|c| c.text == first)
                        .unwrap()
                        < ime
                            .candidates()
                            .iter()
                            .position(|c| c.text == other)
                            .unwrap()
                );
            }
            for (seed, word, sentence) in [
                ("kaisatsu", "改札", "改札はどこですか。"),
                ("kazoku", "家族", "家族で一緒に出かけませんか。"),
                ("kensaku", "検索", "検索しても見つかりません。"),
                ("madowaku", "窓枠", ""),
                ("sentakumono", "洗濯物", "洗濯物を取り込んでおきます。"),
                ("gijiroku", "議事録", "議事録を後で共有します。"),
            ] {
                let mut ime = engine(seed);
                draft(&ime, seed);
                index(&ime, word, CandidateKind::Word);
                if !sentence.is_empty() {
                    commit_once(&mut ime, sentence, CandidateKind::Sentence);
                }
            }
        },
    );
}
