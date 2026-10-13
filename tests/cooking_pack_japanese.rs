//! Authored optional kitchen vocabulary in private snapshots. These examples
//! describe ordinary household conversation, not food-safety instructions.
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

const PACK_ID: &str = "org.suzaku.ja.cooking";
const CHILD_CASE: &str = "SUZAKU_COOKING_JAPANESE_CHILD_CASE";
const JAPANESE_IDS: [&str; 9] = [
    "org.suzaku.ja.rail",
    "org.suzaku.ja.family",
    "org.suzaku.ja.internet",
    "org.suzaku.ja.shopping",
    "org.suzaku.ja.travel",
    "org.suzaku.ja.work",
    "org.suzaku.ja.study",
    PACK_ID,
    "org.suzaku.ja.outdoors",
];

// Fixed independent expectations authored before the optional JSON resource.
const WORDS: &[(&str, &str, &str)] = &[
    ("manaita", "まないた", "まな板"),
    ("houchou", "ほうちょう", "包丁"),
    ("keiryoukappu", "けいりょうかっぷ", "計量カップ"),
    ("keiryousupu-n", "けいりょうすぷーん", "計量スプーン"),
    ("saibashi", "さいばし", "菜箸"),
    ("otama", "おたま", "お玉"),
    ("furaipan", "ふらいぱん", "フライパン"),
    ("awadateki", "あわだてき", "泡立て器"),
    ("zaru", "ざる", "ザル"),
    ("bouru", "ぼうる", "ボウル"),
    ("fukin", "ふきん", "布巾"),
    ("rappu", "らっぷ", "ラップ"),
    ("arumihoiru", "あるみほいる", "アルミホイル"),
    ("kondate", "こんだて", "献立"),
    ("reshipi", "れしぴ", "レシピ"),
    ("shokuzai", "しょくざい", "食材"),
    ("choumiryou", "ちょうみりょう", "調味料"),
    ("komugiko", "こむぎこ", "小麦粉"),
    ("katakuriko", "かたくりこ", "片栗粉"),
    ("panko", "ぱんこ", "パン粉"),
    ("tamanegi", "たまねぎ", "玉ねぎ"),
    ("jagaimo", "じゃがいも", "ジャガイモ"),
    ("ninjin", "にんじん", "ニンジン"),
    ("shiitake", "しいたけ", "シイタケ"),
    ("mijingiri", "みじんぎり", "みじん切り"),
    ("sengiri", "せんぎり", "千切り"),
    ("usugiri", "うすぎり", "薄切り"),
    ("pan'yakiki", "ぱんやきき", "パン焼き器"),
    ("shitagoshirae", "したごしらえ", "下ごしらえ"),
    ("ajitsuke", "あじつけ", "味付け"),
    ("itamemono", "いためもの", "炒め物"),
    ("nimono", "にもの", "煮物"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "まな板",
        ["まな板を出しておきます。", "まな板はどこにしまいましたか。"],
    ),
    (
        "包丁",
        [
            "包丁を使いたいので貸してください。",
            "包丁は引き出しに入っています。",
        ],
    ),
    (
        "計量カップ",
        [
            "計量カップで分量を量ります。",
            "計量カップを取ってもらえますか。",
        ],
    ),
    (
        "菜箸",
        ["菜箸を取ってもらえますか。", "菜箸は引き出しにあります。"],
    ),
    (
        "フライパン",
        [
            "フライパンはどこにありますか。",
            "フライパンで野菜を炒めます。",
        ],
    ),
    (
        "献立",
        ["献立を一緒に考えませんか。", "献立はまだ決めていません。"],
    ),
    (
        "レシピ",
        [
            "レシピを見ながら作っています。",
            "レシピを後で教えてください。",
        ],
    ),
    (
        "食材",
        [
            "食材を買い物リストに書きます。",
            "食材は何が残っていますか。",
        ],
    ),
    (
        "調味料",
        ["調味料を棚に戻しておきます。", "調味料は小さじで量ります。"],
    ),
    (
        "玉ねぎ",
        ["玉ねぎを薄切りにします。", "玉ねぎを一つ使う予定です。"],
    ),
    (
        "下ごしらえ",
        [
            "下ごしらえを手伝ってもらえますか。",
            "下ごしらえが終わったら声をかけます。",
        ],
    ),
    (
        "味付け",
        [
            "味付けは好みに合わせて変えます。",
            "味付けをどうするか相談しましょう。",
        ],
    ),
];

fn cooking() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|p| p.manifest.id == PACK_ID)
        .expect("the optional Japanese cooking pack has its stable catalog ID")
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("suzaku-ja-cooking-{}", std::process::id()));
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
            vec![cooking()]
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
fn cooking_japanese_has_fixed_novel_readings_and_authored_sentences() {
    let pack = cooking();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "ja");
    assert_eq!(pack.manifest.topics, ["cooking", "home"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    assert_eq!(pack.lexicon.readings().len(), WORDS.len());
    assert_eq!(pack.lexicon.continuations().count(), SENTENCES.len());
    assert_eq!(WORDS.iter().map(|w| w.1).collect::<HashSet<_>>().len(), 32);
    assert_eq!(WORDS.iter().map(|w| w.2).collect::<HashSet<_>>().len(), 32);
    for (entry, &(romaji, kana, word)) in pack.lexicon.readings().iter().zip(WORDS) {
        assert_eq!(romaji_to_hiragana(romaji), kana);
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, values), &(word, ss)) in pack.lexicon.continuations().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(values.iter().map(String::as_str).collect::<Vec<_>>(), ss);
        assert!(ss.iter().all(|s| s.starts_with(word) && s.ends_with('。')));
    }
    assert_eq!(
        pack.lexicon
            .continuations()
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        24
    );
    let catalog = packs::recommended();
    for old in std::iter::once(lexicon::builtin("ja").unwrap()).chain(
        catalog
            .iter()
            .filter(|p| p.manifest.language == "ja" && p.manifest.id != PACK_ID)
            .map(|p| &p.lexicon),
    ) {
        for &(_, kana, word) in WORDS {
            assert!(
                old.readings()
                    .iter()
                    .all(|e| e.reading != kana && e.text != word),
                "old collision: {kana}/{word}"
            );
        }
        for &(context, ss) in SENTENCES {
            for (old_context, old_ss) in old.continuations() {
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
fn cooking_japanese_absent_catalog_keeps_builtin_and_creates_no_store() {
    fresh(
        "cooking_japanese_absent_catalog_keeps_builtin_and_creates_no_store",
        |fixture| {
            let _pack = cooking();
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
fn cooking_japanese_disabled_pack_preserves_builtin_choices() {
    fresh(
        "cooking_japanese_disabled_pack_preserves_builtin_choices",
        |fixture| {
            fixture.initialize(false, false);
            assert_unavailable();
        },
    );
}
#[test]
fn cooking_japanese_complete_romaji_case_kana_and_words_offer_both_sentences() {
    fresh(
        "cooking_japanese_complete_romaji_case_kana_and_words_offer_both_sentences",
        |fixture| {
            fixture.initialize(true, false);
            complete_workflows();
        },
    );
}
#[test]
fn cooking_japanese_selected_partials_and_reading_separators_preserve_choices() {
    fresh(
        "cooking_japanese_selected_partials_and_reading_separators_preserve_choices",
        |fixture| {
            fixture.initialize(true, false);
            for (seed, word) in [
                ("manait", "まな板"),
                ("まない", "まな板"),
                ("houcho", "包丁"),
                ("ほうちょ", "包丁"),
                ("keiryoukapp", "計量カップ"),
                ("けいりょうかっ", "計量カップ"),
                ("saibash", "菜箸"),
                ("さいば", "菜箸"),
                ("furaipa", "フライパン"),
                ("ふらいぱ", "フライパン"),
                ("kondat", "献立"),
                ("こんだ", "献立"),
                ("reship", "レシピ"),
                ("れし", "レシピ"),
                ("choumiryo", "調味料"),
                ("ちょうみりょ", "調味料"),
                ("tamaneg", "玉ねぎ"),
                ("たまね", "玉ねぎ"),
                ("shitagoshira", "下ごしらえ"),
                ("したごしら", "下ごしらえ"),
                ("ajitsuk", "味付け"),
                ("あじつ", "味付け"),
            ] {
                choices(seed, word);
            }
            for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
                for (left, right, word) in [
                    ("MANA", "ITA", "まな板"),
                    ("keiryou", "kappu", "計量カップ"),
                    ("ちょうみ", "りょう", "調味料"),
                    ("shita", "goshirae", "下ごしらえ"),
                ] {
                    choices(&format!("{space}{left}{space}{right}{space}"), word);
                }
            }
            for seed in ["pan’yakiki", "ぱん やきき"] {
                choices(seed, "パン焼き器");
            }
            choices("jagaimo", "ジャガイモ");
            choices("keiryoukaxtsupu", "計量カップ");
        },
    );
}
#[test]
fn cooking_japanese_adopted_words_continue_without_committing_draft() {
    fresh(
        "cooking_japanese_adopted_words_continue_without_committing_draft",
        |fixture| {
            fixture.initialize(true, false);
            for (reading, word, tail, sentence) in [
                ("manaita", "まな板", "wo", "まな板を出しておきます。"),
                ("houchou", "包丁", "ha", "包丁は引き出しに入っています。"),
                (
                    "keiryoukappu",
                    "計量カップ",
                    "de",
                    "計量カップで分量を量ります。",
                ),
                ("saibashi", "菜箸", "WO", "菜箸を取ってもらえますか。"),
                (
                    "furaipan",
                    "フライパン",
                    "で",
                    "フライパンで野菜を炒めます。",
                ),
                ("kondate", "献立", "wo", "献立を一緒に考えませんか。"),
                ("reshipi", "レシピ", "wo", "レシピを見ながら作っています。"),
                ("shokuzai", "食材", "ha", "食材は何が残っていますか。"),
                ("choumiryou", "調味料", "wo", "調味料を棚に戻しておきます。"),
                ("tamanegi", "玉ねぎ", "wo", "玉ねぎを薄切りにします。"),
                (
                    "shitagoshirae",
                    "下ごしらえ",
                    "ga",
                    "下ごしらえが終わったら声をかけます。",
                ),
                (
                    "ajitsuke",
                    "味付け",
                    "ha",
                    "味付けは好みに合わせて変えます。",
                ),
            ] {
                let mut ime = engine(reading);
                ime.select_candidate(index(&ime, word, CandidateKind::Word));
                assert_eq!(ime.selected_completion_text(true), Some(word));
                assert!(ime.snapshot().committed_text.is_empty());
                let spaced = format!("{word} ");
                ime.seed(&spaced);
                draft(&ime, &spaced);
                for s in SENTENCES
                    .iter()
                    .find(|(context, _)| *context == word)
                    .unwrap()
                    .1
                {
                    index(&ime, s, CandidateKind::Sentence);
                }
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
fn cooking_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact() {
    fresh(
        "cooking_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact",
        |fixture| {
            fixture.initialize(true, false);
            let mut seeds: Vec<String> = [
                "https://example.invalid/reshipi?q=kondate",
                "shokuzai@example.invalid",
                "let reshipi = \"manaita\";",
                "C:/cooking/kondate.txt",
                "xyzmanaita",
                "🙂献立wo",
                "味付けhaxyz",
                "玉ねぎをが",
                "レシピ{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("mana{boundary}ita"));
                seeds.push(format!("レシピ{boundary}wo"));
            }
            for tail in ["manaita", "下ごしらえga", "味付けha"] {
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
        "ja-cooking-synthetic-failure"
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
fn cooking_japanese_fallback_survives_typed_and_empty_model_failures() {
    fresh(
        "cooking_japanese_fallback_survives_typed_and_empty_model_failures",
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
                    ("manaita", "まな板", "まな板を出しておきます。"),
                    ("choumiryou", "調味料", "調味料を棚に戻しておきます。"),
                    (
                        "shitagoshirae",
                        "下ごしらえ",
                        "下ごしらえが終わったら声をかけます。",
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
fn cooking_japanese_coexists_with_fixed_nine_packs_and_preserves_prior_japanese_order() {
    fresh(
        "cooking_japanese_coexists_with_fixed_nine_packs_and_preserves_prior_japanese_order",
        |fixture| {
            fixture.initialize(true, true);
            complete_workflows();
            let builtin = lexicon::builtin("ja").unwrap();
            let active = lexicon::active("ja").unwrap();
            let mut offset = builtin.readings().len();
            let mut contexts = builtin.continuations().collect::<Vec<_>>();
            let catalog = packs::recommended();
            for id in JAPANESE_IDS {
                let pack = catalog.iter().find(|p| p.manifest.id == id).unwrap();
                let end = offset + pack.lexicon.readings().len();
                assert_eq!(pack.lexicon.readings(), &active.readings()[offset..end]);
                offset = end;
                contexts.extend(pack.lexicon.continuations());
            }
            assert_eq!(offset, active.readings().len());
            assert_eq!(contexts, active.continuations().collect::<Vec<_>>());
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
                ("shichakushitsu", "試着室", "試着室はどこですか。"),
                ("gijiroku", "議事録", "議事録を後で共有します。"),
                ("madowaku", "窓枠", ""),
                ("sentakumono", "洗濯物", "洗濯物を取り込んでおきます。"),
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
