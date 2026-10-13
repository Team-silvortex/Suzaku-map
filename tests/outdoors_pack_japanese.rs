//! Authored optional night-sky vocabulary in private snapshots. These examples
//! make no observing forecasts and access no equipment, location or native IME.
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

const PACK_ID: &str = "org.suzaku.ja.outdoors";
const CHILD_CASE: &str = "SUZAKU_OUTDOORS_JAPANESE_CHILD_CASE";
const JAPANESE_IDS: [&str; 9] = [
    "org.suzaku.ja.rail",
    "org.suzaku.ja.family",
    "org.suzaku.ja.internet",
    "org.suzaku.ja.shopping",
    "org.suzaku.ja.travel",
    "org.suzaku.ja.work",
    "org.suzaku.ja.study",
    "org.suzaku.ja.cooking",
    PACK_ID,
];

// Independent fixed expectations, written before the JSON resource/catalog entry.
const WORDS: &[(&str, &str, &str)] = &[
    ("hoshizora", "ほしぞら", "星空"),
    ("yozora", "よぞら", "夜空"),
    ("seiza", "せいざ", "星座"),
    ("tentaikansoku", "てんたいかんそく", "天体観測"),
    ("bouenkyou", "ぼうえんきょう", "望遠鏡"),
    ("sougankyou", "そうがんきょう", "双眼鏡"),
    ("sankyaku", "さんきゃく", "三脚"),
    ("seizahayamiban", "せいざはやみばん", "星座早見盤"),
    ("seizu", "せいず", "星図"),
    ("hokkyokusei", "ほっきょくせい", "北極星"),
    ("hokutoshichisei", "ほくとしちせい", "北斗七星"),
    ("amanogawa", "あまのがわ", "天の川"),
    ("nagareboshi", "ながれぼし", "流れ星"),
    ("ryuuseigun", "りゅうせいぐん", "流星群"),
    ("mangetsu", "まんげつ", "満月"),
    ("mikaduki", "みかづき", "三日月"),
    ("tsukiakari", "つきあかり", "月明かり"),
    ("shingetsu", "しんげつ", "新月"),
    ("wakusei", "わくせい", "惑星"),
    ("getsumen", "げつめん", "月面"),
    ("seiun", "せいうん", "星雲"),
    ("seidan", "せいだん", "星団"),
    ("ginga", "ぎんが", "銀河"),
    ("uchuu", "うちゅう", "宇宙"),
    ("tentaishashin", "てんたいしゃしん", "天体写真"),
    ("pinto", "ぴんと", "ピント"),
    ("bairitsu", "ばいりつ", "倍率"),
    ("setsuganrenzu", "せつがんれんず", "接眼レンズ"),
    ("kansokukiroku", "かんそくきろく", "観測記録"),
    ("kansokuchi", "かんそくち", "観測地"),
    ("houi", "ほうい", "方位"),
    ("tenmondai", "てんもんだい", "天文台"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "星空",
        [
            "星空をゆっくり眺めたいです。",
            "星空の写真を見せてもらえますか。",
        ],
    ),
    (
        "夜空",
        [
            "夜空を見上げる時間が好きです。",
            "夜空に詳しい人と話してみたいです。",
        ],
    ),
    (
        "星座",
        [
            "星座の名前を一緒に調べましょう。",
            "星座を探す練習をしています。",
        ],
    ),
    (
        "天体観測",
        [
            "天体観測に使う道具をまとめています。",
            "天体観測の記録を残しておきます。",
        ],
    ),
    (
        "望遠鏡",
        [
            "望遠鏡の使い方を教えてください。",
            "望遠鏡を片付ける前に付属品を確認します。",
        ],
    ),
    (
        "双眼鏡",
        [
            "双眼鏡をケースから出します。",
            "双眼鏡のピントを合わせてみます。",
        ],
    ),
    (
        "三脚",
        [
            "三脚を置く場所を決めましょう。",
            "三脚をたたんで持ち帰ります。",
        ],
    ),
    (
        "星座早見盤",
        [
            "星座早見盤の読み方を覚えたいです。",
            "星座早見盤をかばんに入れました。",
        ],
    ),
    (
        "北極星",
        [
            "北極星の探し方を本で調べています。",
            "北極星についてもう少し知りたいです。",
        ],
    ),
    (
        "天の川",
        [
            "天の川の写真を見るのが好きです。",
            "天の川について図鑑で調べてみます。",
        ],
    ),
    (
        "流星群",
        [
            "流星群の名前をノートに書きました。",
            "流星群について友達と話しました。",
        ],
    ),
    (
        "観測記録",
        [
            "観測記録に場所と時刻を書きます。",
            "観測記録を後で読み返してみます。",
        ],
    ),
];

fn outdoors() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|p| p.manifest.id == PACK_ID)
        .expect("the optional Japanese night-sky pack has its stable catalog ID")
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("suzaku-ja-outdoors-{}", std::process::id()));
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
            vec![outdoors()]
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
fn outdoors_japanese_has_fixed_novel_readings_and_authored_sentences() {
    let pack = outdoors();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.name, "星空 · 星座と天体観測");
    assert_eq!(pack.manifest.language, "ja");
    assert_eq!(pack.manifest.topics, ["outdoors", "astronomy"]);
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
fn outdoors_japanese_absent_catalog_keeps_builtin_and_creates_no_store() {
    fresh(
        "outdoors_japanese_absent_catalog_keeps_builtin_and_creates_no_store",
        |fixture| {
            let _pack = outdoors();
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
fn outdoors_japanese_disabled_pack_preserves_builtin_choices() {
    fresh(
        "outdoors_japanese_disabled_pack_preserves_builtin_choices",
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
}
#[test]
fn outdoors_japanese_complete_romaji_case_kana_and_words_offer_both_sentences() {
    fresh(
        "outdoors_japanese_complete_romaji_case_kana_and_words_offer_both_sentences",
        |fixture| {
            fixture.initialize(true, false);
            complete_workflows();
        },
    );
}
#[test]
fn outdoors_japanese_selected_partials_and_reading_separators_preserve_choices() {
    fresh(
        "outdoors_japanese_selected_partials_and_reading_separators_preserve_choices",
        |fixture| {
            fixture.initialize(true, false);
            for (seed, word) in [
                ("hoshizor", "星空"),
                ("ほしぞ", "星空"),
                ("yozor", "夜空"),
                ("よぞ", "夜空"),
                ("tentaikansok", "天体観測"),
                ("てんたいかんそ", "天体観測"),
                ("bouenky", "望遠鏡"),
                ("ぼうえんきょ", "望遠鏡"),
                ("souganky", "双眼鏡"),
                ("そうがんきょ", "双眼鏡"),
                ("seizahayamib", "星座早見盤"),
                ("せいざはやみば", "星座早見盤"),
                ("hokkyokuse", "北極星"),
                ("ほっきょくせ", "北極星"),
                ("ryuuseigu", "流星群"),
                ("りゅうせいぐ", "流星群"),
                ("kansokukirok", "観測記録"),
                ("かんそくきろ", "観測記録"),
            ] {
                choices(seed, word);
            }
            for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
                for (left, right, word) in [
                    ("HOSHI", "ZORA", "星空"),
                    ("bouen", "kyou", "望遠鏡"),
                    ("てんたい", "かんそく", "天体観測"),
                    ("kansoku", "kiroku", "観測記録"),
                ] {
                    choices(&format!("{space}{left}{space}{right}{space}"), word);
                }
            }
        },
    );
}
#[test]
fn outdoors_japanese_adopted_words_continue_without_committing_draft() {
    fresh(
        "outdoors_japanese_adopted_words_continue_without_committing_draft",
        |fixture| {
            fixture.initialize(true, false);
            for (reading, word, tail, sentence) in [
                ("hoshizora", "星空", "wo", "星空をゆっくり眺めたいです。"),
                ("yozora", "夜空", "に", "夜空に詳しい人と話してみたいです。"),
                ("seiza", "星座", "no", "星座の名前を一緒に調べましょう。"),
                (
                    "tentaikansoku",
                    "天体観測",
                    "ni",
                    "天体観測に使う道具をまとめています。",
                ),
                (
                    "bouenkyou",
                    "望遠鏡",
                    "の",
                    "望遠鏡の使い方を教えてください。",
                ),
                ("sougankyou", "双眼鏡", "WO", "双眼鏡をケースから出します。"),
                ("sankyaku", "三脚", "wo", "三脚を置く場所を決めましょう。"),
                (
                    "seizahayamiban",
                    "星座早見盤",
                    "no",
                    "星座早見盤の読み方を覚えたいです。",
                ),
                (
                    "hokkyokusei",
                    "北極星",
                    "no",
                    "北極星の探し方を本で調べています。",
                ),
                (
                    "amanogawa",
                    "天の川",
                    "ni",
                    "天の川について図鑑で調べてみます。",
                ),
                (
                    "ryuuseigun",
                    "流星群",
                    "no",
                    "流星群の名前をノートに書きました。",
                ),
                (
                    "kansokukiroku",
                    "観測記録",
                    "ni",
                    "観測記録に場所と時刻を書きます。",
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
fn outdoors_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact() {
    fresh(
        "outdoors_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact",
        |fixture| {
            fixture.initialize(true, false);
            let mut seeds: Vec<String> = [
                "https://example.invalid/hoshizora?q=bouenkyou",
                "seiza@example.invalid",
                "let bouenkyou = \"hoshizora\";",
                "C:/sky/kansokukiroku.txt",
                "xyzhoshizora",
                "🙂望遠鏡no",
                "星空woxyz",
                "天体観測をが",
                "流星群{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("hoshi{boundary}zora"));
                seeds.push(format!("望遠鏡{boundary}no"));
            }
            for tail in ["hoshizora", "観測記録ni", "星空wo"] {
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
        "ja-outdoors-synthetic-failure"
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
fn outdoors_japanese_fallback_survives_typed_and_empty_model_failures() {
    fresh(
        "outdoors_japanese_fallback_survives_typed_and_empty_model_failures",
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
                    ("hoshizora", "星空", "星空をゆっくり眺めたいです。"),
                    ("bouenkyou", "望遠鏡", "望遠鏡の使い方を教えてください。"),
                    (
                        "kansokukiroku",
                        "観測記録",
                        "観測記録に場所と時刻を書きます。",
                    ),
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
fn outdoors_japanese_coexists_with_fixed_nine_packs_and_preserves_prior_japanese_order() {
    fresh(
        "outdoors_japanese_coexists_with_fixed_nine_packs_and_preserves_prior_japanese_order",
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
                ("shichakushitsu", "試着室", "試着室はどこですか。"),
                ("hoteru", "ホテル", "ホテルまでの行き方を教えてください。"),
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
