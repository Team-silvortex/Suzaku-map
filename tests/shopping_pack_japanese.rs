//! Authored optional shop/fitting vocabulary, not a promise of general Japanese
//! morphology or physical IBus key coverage. Each runtime uses a private store.
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

const PACK_ID: &str = "org.suzaku.ja.shopping";
const CHILD_CASE: &str = "SUZAKU_SHOPPING_JAPANESE_CHILD_CASE";
const COHORT: [&str; 6] = [
    "org.suzaku.ja.rail",
    "org.suzaku.ja.family",
    "org.suzaku.ja.internet",
    PACK_ID,
    "org.suzaku.ja.travel",
    "org.suzaku.ja.work",
];

// Fixed expectations written before the optional resource; neither list is
// inferred from the catalog or generated from the data under test.
const WORDS: &[(&str, &str, &str)] = &[
    ("shichakushitsu", "しちゃくしつ", "試着室"),
    ("reji", "れじ", "レジ"),
    ("reshi-to", "れしーと", "レシート"),
    ("saizu", "さいず", "サイズ"),
    ("irochigai", "いろちがい", "色違い"),
    ("nefuda", "ねふだ", "値札"),
    ("shouhin", "しょうひん", "商品"),
    ("shouhindana", "しょうひんだな", "商品棚"),
    ("ba-ko-do", "ばーこーど", "バーコード"),
    ("se-ru", "せーる", "セール"),
    ("ku-pon", "くーぽん", "クーポン"),
    ("pointoka-do", "ぽいんとかーど", "ポイントカード"),
    ("ekobaggu", "えこばっぐ", "エコバッグ"),
    ("rejibukuro", "れじぶくろ", "レジ袋"),
    ("rappingu", "らっぴんぐ", "ラッピング"),
    ("housou", "ほうそう", "包装"),
    ("torioki", "とりおき", "取り置き"),
    ("toriyose", "とりよせ", "取り寄せ"),
    ("nyuuka", "にゅうか", "入荷"),
    ("sainyuuka", "さいにゅうか", "再入荷"),
    ("uriba", "うりば", "売り場"),
    ("ten'in", "てんいん", "店員"),
    ("kaimonokago", "かいものかご", "買い物かご"),
    ("kaimonorisuto", "かいものりすと", "買い物リスト"),
    ("susoage", "すそあげ", "裾上げ"),
    ("sentakuhyouji", "せんたくひょうじ", "洗濯表示"),
    ("sozai", "そざい", "素材"),
    ("tezawari", "てざわり", "手触り"),
    ("iroai", "いろあい", "色合い"),
    ("kigokochi", "きごこち", "着心地"),
    ("shouhinbangou", "しょうひんばんごう", "商品番号"),
    ("henpinkigen", "へんぴんきげん", "返品期限"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "試着室",
        ["試着室はどこですか。", "試着室を使ってもいいですか。"],
    ),
    (
        "レシート",
        [
            "レシートを袋に入れてください。",
            "レシートは交換するときに必要ですか。",
        ],
    ),
    (
        "サイズ",
        [
            "サイズを一つ大きくできますか。",
            "サイズが合うか確かめたいです。",
        ],
    ),
    (
        "色違い",
        ["色違いも見せてもらえますか。", "色違いの在庫はありますか。"],
    ),
    (
        "値札",
        [
            "値札の金額を確認したいです。",
            "値札が見当たらないのですが。",
        ],
    ),
    (
        "レジ袋",
        ["レジ袋は要りません。", "レジ袋を一枚お願いします。"],
    ),
    (
        "ラッピング",
        [
            "ラッピングをお願いできますか。",
            "ラッピングに時間はかかりますか。",
        ],
    ),
    (
        "取り置き",
        [
            "取り置きはお願いできますか。",
            "取り置きできる期間を教えてください。",
        ],
    ),
    (
        "取り寄せ",
        [
            "取り寄せはできますか。",
            "取り寄せには何日くらいかかりますか。",
        ],
    ),
    (
        "入荷",
        [
            "入荷の予定を教えてください。",
            "入荷したら知らせてもらえますか。",
        ],
    ),
    (
        "裾上げ",
        [
            "裾上げをお願いできますか。",
            "裾上げにかかる時間を教えてください。",
        ],
    ),
    (
        "着心地",
        [
            "着心地を確かめてから決めたいです。",
            "着心地がよくて動きやすいです。",
        ],
    ),
];

fn shopping() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|p| p.manifest.id == PACK_ID)
        .expect("Japanese shopping has its stable optional catalog ID")
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("suzaku-ja-shopping-{}", std::process::id()));
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
            COHORT
                .iter()
                .map(|id| {
                    catalog
                        .iter()
                        .find(|p| p.manifest.id == *id)
                        .unwrap()
                        .clone()
                })
                .collect::<Vec<_>>()
        } else {
            vec![shopping()]
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
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind, first_page: bool) -> usize {
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
    if first_page && kind != CandidateKind::Literal {
        assert!(
            position < PAGE_SIZE,
            "{text:?} not on first page: {:?}",
            ime.candidates()
        );
    }
    position
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind, first_page: bool) {
    ime.select_candidate(index(ime, text, kind, first_page));
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
fn choices(seed: &str, word: &str, first_page: bool) {
    let ime = engine(seed);
    draft(&ime, seed);
    let kind = if seed == word {
        CandidateKind::Literal
    } else {
        CandidateKind::Word
    };
    index(&ime, word, kind, first_page);
    commit_once(&mut engine(seed), word, kind, first_page);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal, false);
    if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
        for sentence in sentences {
            commit_once(
                &mut engine(seed),
                sentence,
                CandidateKind::Sentence,
                first_page,
            );
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
            choices(&seed, word, true);
        }
    }
}

#[test]
fn shopping_japanese_has_fixed_novel_readings_and_authored_sentences() {
    let pack = shopping();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "ja");
    assert_eq!(pack.manifest.topics, ["shopping", "errands"]);
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
            .filter(|p| p.manifest.language == "ja" && p.manifest.id != PACK_ID)
            .map(|p| &p.lexicon),
    ) {
        for &(_, kana, word) in WORDS {
            assert!(
                lexicon
                    .readings()
                    .iter()
                    .all(|e| e.reading != kana && e.text != word),
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
        // A generic katakana transliteration is not the optional typed Word.
        assert!(
            ime.candidates()
                .iter()
                .all(|c| c.text != word || c.kind != CandidateKind::Word)
        );
        commit_once(&mut ime, romaji, CandidateKind::Literal, false);
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
fn shopping_japanese_absent_catalog_does_not_install_or_change_builtin() {
    fresh(
        "shopping_japanese_absent_catalog_does_not_install_or_change_builtin",
        |fixture| {
            let _pack = shopping();
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
fn shopping_japanese_disabled_pack_preserves_builtin_choices() {
    fresh(
        "shopping_japanese_disabled_pack_preserves_builtin_choices",
        |fixture| {
            fixture.initialize(false, false);
            assert_unavailable();
        },
    );
}
#[test]
fn shopping_japanese_complete_romaji_case_kana_and_words_offer_both_sentences() {
    fresh(
        "shopping_japanese_complete_romaji_case_kana_and_words_offer_both_sentences",
        |fixture| {
            fixture.initialize(true, false);
            complete_workflows();
        },
    );
}
#[test]
fn shopping_japanese_selected_partials_and_reading_separators_preserve_choices() {
    fresh(
        "shopping_japanese_selected_partials_and_reading_separators_preserve_choices",
        |fixture| {
            fixture.initialize(true, false);
            // Near-complete prefixes must retain the authored alternatives; unlike
            // exact readings they do not promise every sentence is on page one.
            for (seed, word) in [
                ("shichakushits", "試着室"),
                ("しちゃくし", "試着室"),
                ("reshi-t", "レシート"),
                ("れしー", "レシート"),
                ("irochiga", "色違い"),
                ("いろちが", "色違い"),
                ("rejibukur", "レジ袋"),
                ("れじぶく", "レジ袋"),
                ("rappin", "ラッピング"),
                ("らっぴん", "ラッピング"),
                ("kigokoch", "着心地"),
                ("きごこ", "着心地"),
            ] {
                choices(seed, word, false);
            }
            for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
                for (left, right, word) in [
                    ("SHICHAKU", "SHITSU", "試着室"),
                    ("reji", "bukuro", "レジ袋"),
                    ("いろ", "ちがい", "色違い"),
                    ("kigo", "kochi", "着心地"),
                ] {
                    choices(&format!("{space}{left}{space}{right}{space}"), word, true);
                }
            }
            choices("ten’in", "店員", true);
            // Broad prefixes retain the literal and the bounded candidate list;
            // no claim that every matching word or sentence fits twelve slots.
            for seed in ["shi", "し", "ka", "か"] {
                let mut ime = engine(seed);
                draft(&ime, seed);
                commit_once(&mut ime, seed, CandidateKind::Literal, false);
            }
        },
    );
}
#[test]
fn shopping_japanese_adopted_words_allow_editable_authored_sentence_progress() {
    fresh(
        "shopping_japanese_adopted_words_allow_editable_authored_sentence_progress",
        |fixture| {
            fixture.initialize(true, false);
            for (reading, word, tail, sentence) in [
                ("shichakushitsu", "試着室", "ha", "試着室はどこですか。"),
                (
                    "reshi-to",
                    "レシート",
                    "wo",
                    "レシートを袋に入れてください。",
                ),
                ("saizu", "サイズ", "ga", "サイズが合うか確かめたいです。"),
                ("irochigai", "色違い", "MO", "色違いも見せてもらえますか。"),
                ("nefuda", "値札", "の", "値札の金額を確認したいです。"),
                ("rejibukuro", "レジ袋", "ha", "レジ袋は要りません。"),
                (
                    "rappingu",
                    "ラッピング",
                    "ni",
                    "ラッピングに時間はかかりますか。",
                ),
                (
                    "torioki",
                    "取り置き",
                    "dekiru",
                    "取り置きできる期間を教えてください。",
                ),
                (
                    "toriyose",
                    "取り寄せ",
                    "niha",
                    "取り寄せには何日くらいかかりますか。",
                ),
                (
                    "nyuuka",
                    "入荷",
                    "shitara",
                    "入荷したら知らせてもらえますか。",
                ),
                ("susoage", "裾上げ", "wo", "裾上げをお願いできますか。"),
                (
                    "kigokochi",
                    "着心地",
                    "ga",
                    "着心地がよくて動きやすいです。",
                ),
            ] {
                let mut ime = engine(reading);
                ime.select_candidate(index(&ime, word, CandidateKind::Word, true));
                assert_eq!(ime.selected_completion_text(true), Some(word));
                assert!(ime.snapshot().committed_text.is_empty());
                let spaced = format!("{word} ");
                ime.seed(&spaced);
                draft(&ime, &spaced);
                index(&ime, sentence, CandidateKind::Sentence, true);
                let continued = format!("{word} {tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                commit_once(&mut ime, sentence, CandidateKind::Sentence, true);
                commit_once(
                    &mut engine(&continued),
                    &continued,
                    CandidateKind::Literal,
                    false,
                );
                ime.seed(reading);
                draft(&ime, reading);
                index(&ime, word, CandidateKind::Word, true);
            }
        },
    );
}
#[test]
fn shopping_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact() {
    fresh(
        "shopping_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact",
        |fixture| {
            fixture.initialize(true, false);
            let mut seeds: Vec<String> = [
                "https://example.invalid/saizu?q=nefuda",
                "reshi-to@example.invalid",
                "let saizu = \"nefuda\";",
                "C:/files/saizu.txt",
                "xyzshichakushitsu",
                "🙂試着室ha",
                "サイズgaxyz",
                "レシートをが",
                "値札{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("shi{boundary}chakushitsu"));
                seeds.push(format!("試着室{boundary}ha"));
            }
            for tail in ["shichakushitsu", "サイズga", "レシートwo"] {
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
                commit_once(&mut ime, &seed, CandidateKind::Literal, false);
            }
        },
    );
}
struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-shopping-synthetic-failure"
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
fn shopping_japanese_fallback_survives_typed_and_empty_model_failures() {
    fresh(
        "shopping_japanese_fallback_survives_typed_and_empty_model_failures",
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
                    ("shichakushitsu", "試着室", "試着室はどこですか。"),
                    ("reshi-to", "レシート", "レシートを袋に入れてください。"),
                    ("kigokochi", "着心地", "着心地を確かめてから決めたいです。"),
                ] {
                    let mut ime = engine("");
                    ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
                    ime.seed(seed);
                    draft(&ime, seed);
                    index(&ime, word, CandidateKind::Word, true);
                    index(&ime, sentence, CandidateKind::Sentence, true);
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
                    commit_once(&mut ime, sentence, CandidateKind::Sentence, true);
                }
            }
        },
    );
}
#[test]
fn shopping_japanese_coexists_with_fixed_six_packs_and_preserves_prior_order() {
    fresh(
        "shopping_japanese_coexists_with_fixed_six_packs_and_preserves_prior_order",
        |fixture| {
            fixture.initialize(true, true);
            complete_workflows();
            let active = lexicon::active("ja").unwrap();
            let mut offset = lexicon::builtin("ja").unwrap().readings().len();
            for id in COHORT {
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
                ("sentakumono", "洗濯物", "洗濯物を取り込んでおきます。"),
                ("shichaku", "試着", ""),
            ] {
                let mut ime = engine(seed);
                draft(&ime, seed);
                index(&ime, word, CandidateKind::Word, true);
                if !sentence.is_empty() {
                    commit_once(&mut ime, sentence, CandidateKind::Sentence, true);
                }
            }
        },
    );
}
