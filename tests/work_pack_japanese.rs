//! Authored optional workplace vocabulary in private snapshots. No workplace,
//! account, network, native keyboard or user preference state is accessed.
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

const PACK_ID: &str = "org.suzaku.ja.work";
const CHILD_CASE: &str = "SUZAKU_WORK_JAPANESE_CHILD_CASE";
const JAPANESE_IDS: [&str; 6] = [
    "org.suzaku.ja.rail",
    "org.suzaku.ja.family",
    "org.suzaku.ja.internet",
    "org.suzaku.ja.shopping",
    "org.suzaku.ja.travel",
    PACK_ID,
];

// Independent fixed expectations, written before the JSON resource/catalog entry.
const WORDS: &[(&str, &str, &str)] = &[
    ("uchiawase", "うちあわせ", "打ち合わせ"),
    ("gijiroku", "ぎじろく", "議事録"),
    ("gidai", "ぎだい", "議題"),
    ("shiryou", "しりょう", "資料"),
    ("kikakusho", "きかくしょ", "企画書"),
    ("teiansho", "ていあんしょ", "提案書"),
    ("sagyoutejun", "さぎょうてじゅん", "作業手順"),
    ("sagyoujikan", "さぎょうじかん", "作業時間"),
    ("shinchokuhoukoku", "しんちょくほうこく", "進捗報告"),
    ("hinshitsu", "ひんしつ", "品質"),
    ("kentou", "けんとう", "検討"),
    ("shuusei", "しゅうせい", "修正"),
    ("sashimodoshi", "さしもどし", "差し戻し"),
    ("shounin", "しょうにん", "承認"),
    ("teishutsu", "ていしゅつ", "提出"),
    ("saishuuban", "さいしゅうばん", "最終版"),
    ("shitagaki", "したがき", "下書き"),
    ("daiichian", "だいいちあん", "第一案"),
    ("nichiji", "にちじ", "日時"),
    ("kijitsu", "きじつ", "期日"),
    ("yuusenjun'i", "ゆうせんじゅんい", "優先順位"),
    ("tantousha", "たんとうしゃ", "担当者"),
    ("sekininsha", "せきにんしゃ", "責任者"),
    ("hikitsugisho", "ひきつぎしょ", "引き継ぎ書"),
    ("renrakujikou", "れんらくじこう", "連絡事項"),
    ("moushiokuri", "もうしおくり", "申し送り"),
    ("kyouyuujikou", "きょうゆうじこう", "共有事項"),
    ("goui", "ごうい", "合意"),
    ("gyoumu", "ぎょうむ", "業務"),
    ("zaitakukinmu", "ざいたくきんむ", "在宅勤務"),
    ("shukkin", "しゅっきん", "出勤"),
    ("taikin", "たいきん", "退勤"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "打ち合わせ",
        [
            "打ち合わせの時間を少し変更できますか。",
            "打ち合わせで確認したいことがあります。",
        ],
    ),
    (
        "議事録",
        [
            "議事録を後で共有します。",
            "議事録に決まったことをまとめました。",
        ],
    ),
    (
        "議題",
        [
            "議題を先に送ってもらえますか。",
            "議題を一つ追加してもよろしいですか。",
        ],
    ),
    (
        "資料",
        [
            "資料を確認してから返事をします。",
            "資料の内容について質問があります。",
        ],
    ),
    (
        "企画書",
        [
            "企画書の下書きができました。",
            "企画書を一緒に見直してもらえますか。",
        ],
    ),
    (
        "作業手順",
        [
            "作業手順をもう一度確認します。",
            "作業手順に変更があれば教えてください。",
        ],
    ),
    (
        "進捗報告",
        [
            "進捗報告は明日の午前中に送ります。",
            "進捗報告に今後の予定も書いておきます。",
        ],
    ),
    (
        "修正",
        [
            "修正が終わったら連絡します。",
            "修正する箇所を教えてもらえますか。",
        ],
    ),
    (
        "承認",
        [
            "承認をもらってから進めます。",
            "承認が必要な項目を確認しています。",
        ],
    ),
    (
        "提出",
        [
            "提出する前に内容を見直します。",
            "提出は明日の午後でも大丈夫ですか。",
        ],
    ),
    (
        "引き継ぎ書",
        [
            "引き継ぎ書を分かりやすくまとめます。",
            "引き継ぎ書に連絡先も載せておきます。",
        ],
    ),
    (
        "在宅勤務",
        [
            "在宅勤務の日はチャットで連絡してください。",
            "在宅勤務でも会議には参加できます。",
        ],
    ),
];

fn work() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|p| p.manifest.id == PACK_ID)
        .expect("the optional Japanese work pack has its stable catalog ID")
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("suzaku-ja-work-{}", std::process::id()));
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
            vec![work()]
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
fn commit_second_page_once(ime: &mut XRTabletImeEngine, text: &str) {
    let position = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == CandidateKind::Sentence)
        .unwrap();
    assert_eq!(position, 7);
    assert_eq!(position / PAGE_SIZE, 1);
    ime.select_candidate(position);
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
fn work_japanese_has_fixed_novel_readings_and_authored_sentences() {
    let pack = work();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "ja");
    assert_eq!(pack.manifest.topics, ["work", "collaboration"]);
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
fn work_japanese_absent_catalog_keeps_builtin_and_creates_no_store() {
    fresh(
        "work_japanese_absent_catalog_keeps_builtin_and_creates_no_store",
        |fixture| {
            let _pack = work();
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
fn work_japanese_disabled_pack_preserves_builtin_choices() {
    fresh(
        "work_japanese_disabled_pack_preserves_builtin_choices",
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
fn work_japanese_complete_romaji_case_kana_and_words_offer_both_sentences() {
    fresh(
        "work_japanese_complete_romaji_case_kana_and_words_offer_both_sentences",
        |fixture| {
            fixture.initialize(true, false);
            complete_workflows();
        },
    );
}
#[test]
fn work_japanese_selected_partials_and_reading_separators_preserve_choices() {
    fresh(
        "work_japanese_selected_partials_and_reading_separators_preserve_choices",
        |fixture| {
            fixture.initialize(true, false);
            for (seed, word) in [
                ("uchiawas", "打ち合わせ"),
                ("うちあわ", "打ち合わせ"),
                ("gijirok", "議事録"),
                ("ぎじろ", "議事録"),
                ("kikakush", "企画書"),
                ("きかくし", "企画書"),
                ("sagyouteju", "作業手順"),
                ("さぎょうてじゅ", "作業手順"),
                ("shinchokuhoukok", "進捗報告"),
                ("しんちょくほうこ", "進捗報告"),
                ("hikitsugish", "引き継ぎ書"),
                ("zaitakukinm", "在宅勤務"),
                ("ざいたくきん", "在宅勤務"),
            ] {
                choices(seed, word);
            }
            // This Kana prefix shares the first-page sentence quota with the
            // existing segmented 引き継ぎ + し conversion. Freeze its actual order
            // and page-two choice instead of changing the shared first-page checks.
            let seed = "ひきつぎし";
            let ime = engine(seed);
            draft(&ime, seed);
            assert_eq!(
                ime.candidates()
                    .iter()
                    .map(|c| (c.text.as_str(), c.kind))
                    .collect::<Vec<_>>(),
                [
                    ("引き継ぎし", CandidateKind::Sentence),
                    ("引き継ぎ書", CandidateKind::Word),
                    ("引き継ぎ仕事", CandidateKind::Word),
                    ("ヒキツギシ", CandidateKind::Unspecified),
                    ("引き継ぎ失礼します", CandidateKind::Word),
                    (
                        "引き継ぎ書を分かりやすくまとめます。",
                        CandidateKind::Sentence
                    ),
                    ("引き継ぎ週末", CandidateKind::Word),
                    (
                        "引き継ぎ書に連絡先も載せておきます。",
                        CandidateKind::Sentence
                    ),
                    ("ひきつぎし", CandidateKind::Literal),
                    ("引き継ぎ締め切り", CandidateKind::Word),
                    ("引き継ぎ進捗", CandidateKind::Word),
                    ("引き継ぎ写真", CandidateKind::Word),
                ]
            );
            commit_once(&mut engine(seed), "引き継ぎ書", CandidateKind::Word);
            commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            commit_once(
                &mut engine(seed),
                "引き継ぎ書を分かりやすくまとめます。",
                CandidateKind::Sentence,
            );
            commit_second_page_once(&mut engine(seed), "引き継ぎ書に連絡先も載せておきます。");
            for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
                for (left, right, word) in [
                    ("UCHI", "AWASE", "打ち合わせ"),
                    ("giji", "roku", "議事録"),
                    ("きかく", "しょ", "企画書"),
                    ("zaitaku", "kinmu", "在宅勤務"),
                ] {
                    choices(&format!("{space}{left}{space}{right}{space}"), word);
                }
            }
            for seed in ["yuusenjun’i", "ゆうせん じゅんい"] {
                choices(seed, "優先順位");
            }
        },
    );
}
#[test]
fn work_japanese_adopted_words_continue_without_committing_draft() {
    fresh(
        "work_japanese_adopted_words_continue_without_committing_draft",
        |fixture| {
            fixture.initialize(true, false);
            for (reading, word, tail, sentence) in [
                (
                    "uchiawase",
                    "打ち合わせ",
                    "no",
                    "打ち合わせの時間を少し変更できますか。",
                ),
                ("gijiroku", "議事録", "wo", "議事録を後で共有します。"),
                ("gidai", "議題", "wo", "議題を先に送ってもらえますか。"),
                (
                    "shiryou",
                    "資料",
                    "の",
                    "資料の内容について質問があります。",
                ),
                (
                    "kikakusho",
                    "企画書",
                    "WO",
                    "企画書を一緒に見直してもらえますか。",
                ),
                (
                    "sagyoutejun",
                    "作業手順",
                    "wo",
                    "作業手順をもう一度確認します。",
                ),
                (
                    "shinchokuhoukoku",
                    "進捗報告",
                    "ha",
                    "進捗報告は明日の午前中に送ります。",
                ),
                ("shuusei", "修正", "ga", "修正が終わったら連絡します。"),
                ("shounin", "承認", "wo", "承認をもらってから進めます。"),
                (
                    "teishutsu",
                    "提出",
                    "suru",
                    "提出する前に内容を見直します。",
                ),
                (
                    "hikitsugisho",
                    "引き継ぎ書",
                    "ni",
                    "引き継ぎ書に連絡先も載せておきます。",
                ),
                (
                    "zaitakukinmu",
                    "在宅勤務",
                    "no",
                    "在宅勤務の日はチャットで連絡してください。",
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
fn work_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact() {
    fresh(
        "work_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact",
        |fixture| {
            fixture.initialize(true, false);
            let mut seeds: Vec<String> = [
                "https://example.invalid/gijiroku?q=shiryou",
                "teishutsu@example.invalid",
                "let shiryou = \"gijiroku\";",
                "C:/work/kikakusho.txt",
                "xyzuchiawase",
                "🙂資料wo",
                "修正gaxyz",
                "提出をが",
                "議事録{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("giji{boundary}roku"));
                seeds.push(format!("資料{boundary}wo"));
            }
            for tail in ["gijiroku", "提出suru", "修正ga"] {
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
        "ja-work-synthetic-failure"
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
fn work_japanese_fallback_survives_typed_and_empty_model_failures() {
    fresh(
        "work_japanese_fallback_survives_typed_and_empty_model_failures",
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
                    ("gijiroku", "議事録", "議事録を後で共有します。"),
                    (
                        "shinchokuhoukoku",
                        "進捗報告",
                        "進捗報告は明日の午前中に送ります。",
                    ),
                    (
                        "zaitakukinmu",
                        "在宅勤務",
                        "在宅勤務の日はチャットで連絡してください。",
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
fn work_japanese_coexists_with_fixed_six_packs_and_preserves_prior_japanese_order() {
    fresh(
        "work_japanese_coexists_with_fixed_six_packs_and_preserves_prior_japanese_order",
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
