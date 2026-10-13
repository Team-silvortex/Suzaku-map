//! Fixed authored examples for the optional Chinese shopping pack, not a corpus
//! or real shop/model/physical-key benchmark. Every runtime snapshot is private.
#[path = "support/offline_pack_sets.rs"]
mod offline_pack_sets;

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
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{
        self, EntryKind,
        packs::{self, OfflinePack, PackStore},
    },
};

const PACK_ID: &str = "org.suzaku.zh-hans.shopping";
const CHILD_CASE: &str = "SUZAKU_SHOPPING_CHINESE_CHILD_CASE";

const WORDS: &[(&str, &str)] = &[
    ("fu'zhuang'qu", "服装区"),
    ("ri'yong'pin'qu", "日用品区"),
    ("shi'yi'jing", "试衣镜"),
    ("shi'chuan'yi'xia", "试穿一下"),
    ("shi'yi'jian'zai'na", "试衣间在哪"),
    ("zhe'jian'tai'jin'le", "这件太紧了"),
    ("zhe'jian'tai'song'le", "这件太松了"),
    ("xiu'kou'you'dian'jin", "袖口有点紧"),
    ("ku'jiao'you'dian'duan", "裤脚有点短"),
    ("jian'bang'you'dian'jin", "肩膀有点紧"),
    ("yao'wei'bu'he'shi", "腰围不合适"),
    ("chuan'zhe'ting'he'shen", "穿着挺合身"),
    ("huan'jian'tong'kuan'de", "换件同款的"),
    ("zhe'kuan'you'zhong'hao'ma", "这款有中号吗"),
    ("you'shen'yi'dian'de'yan'se'ma", "有深一点的颜色吗"),
    ("wo'xiang'kan'yi'xia'cai'zhi", "我想看一下材质"),
    ("zhe'jian'zen'me'xi", "这件怎么洗"),
    ("diao'pai'xian'bie'jian", "吊牌先别剪"),
    ("qing'gei'wo'yi'zhang'shou'ju", "请给我一张收据"),
    ("shou'ju'fang'zai'dai'zi'li", "收据放在袋子里"),
    ("xiao'piao'zhao'bu'dao'le", "小票找不到了"),
    ("huan'huo'xu'yao'dai'shen'me", "换货需要带什么"),
    ("ke'yi'xian'wen'yi'xia'dian'yuan", "可以先问一下店员"),
    ("wo'xiang'huan'tong'yang'de'kuan'shi", "我想换同样的款式"),
    ("zhe'ge'chi'ma'chuan'bu'xia", "这个尺码穿不下"),
    ("neng'bang'wo'liang'yi'xia'ma", "能帮我量一下吗"),
    ("qing'bang'wo'he'dui'yi'xia", "请帮我核对一下"),
    ("huo'jia'shang'mei'kan'jian", "货架上没看见"),
    ("gou'wu'lan'fang'na'li", "购物篮放哪里"),
    ("wo'zi'ji'dai'le'dai'zi", "我自己带了袋子"),
    ("zhe'ge'xian'fang'hui'qu", "这个先放回去"),
    ("xian'kan'kan'bie'de'kuan'shi", "先看看别的款式"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "试衣间在哪",
        [
            "试衣间在哪？我想试一下这件。",
            "试衣间在哪？麻烦指一下方向。",
        ],
    ),
    (
        "这件太紧了",
        [
            "这件太紧了，我想试试大一号的。",
            "这件太紧了，我再看看别的款式。",
        ],
    ),
    (
        "这件太松了",
        [
            "这件太松了，我想试试小一号的。",
            "这件太松了，有没有收腰一点的款式？",
        ],
    ),
    (
        "这款有中号吗",
        [
            "这款有中号吗？我想试穿比较一下。",
            "这款有中号吗？没有的话我再看看。",
        ],
    ),
    (
        "我想看一下材质",
        [
            "我想看一下材质，麻烦把标签给我看看。",
            "我想看一下材质，这两件摸起来不太一样。",
        ],
    ),
    (
        "这件怎么洗",
        [
            "这件怎么洗？我想先看看洗涤标签。",
            "这件怎么洗？可以帮我看一下说明吗？",
        ],
    ),
    (
        "请给我一张收据",
        [
            "请给我一张收据，我想留作记录。",
            "请给我一张收据，麻烦和衣服放在一起。",
        ],
    ),
    (
        "收据放在袋子里",
        [
            "收据放在袋子里就好，我回去再整理。",
            "收据放在袋子里了吗？我想确认一下。",
        ],
    ),
    (
        "小票找不到了",
        [
            "小票找不到了，还能帮我查询一下吗？",
            "小票找不到了，请问换货需要哪些凭证？",
        ],
    ),
    (
        "换货需要带什么",
        [
            "换货需要带什么？我先确认一下再过来。",
            "换货需要带什么？请问要把包装一起带来吗？",
        ],
    ),
    (
        "我自己带了袋子",
        [
            "我自己带了袋子，麻烦不用另外装袋。",
            "我自己带了袋子，这些东西我来装就好。",
        ],
    ),
    (
        "先看看别的款式",
        [
            "先看看别的款式再决定要哪件。",
            "先看看别的款式也好，我想找一件宽松的。",
        ],
    ),
];

struct Fixture(PathBuf);

impl Fixture {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-shopping-chinese-{}-{case}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn in_fresh_process(name: &str, test: impl FnOnce(&Fixture)) {
    if std::env::var(CHILD_CASE).as_deref() == Ok(name) {
        test(&Fixture::new(name));
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_CASE, name)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Chinese shopping case {name:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn shopping_pack() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == PACK_ID)
        .expect("Chinese shopping pack must be available in the optional catalog")
}

fn initialize(fixture: &Fixture, enabled: bool, all_packs: bool) {
    let base = lexicon::builtin("zh-Hans").unwrap();
    let original = serde_json::to_value(base).unwrap();
    let store = fixture.store();
    assert!(store.list().unwrap().is_empty());
    assert!(!store.path().parent().unwrap().exists());
    let expected = if all_packs {
        offline_pack_sets::historical_sixteen()
    } else {
        vec![shopping_pack()]
    };
    for pack in &expected {
        store.install(pack.clone(), false).unwrap();
    }
    if !enabled {
        store.set_enabled(PACK_ID, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    if enabled {
        assert_eq!(report.loaded.len(), expected.len());
        assert!(report.loaded.iter().any(|id| id == PACK_ID));
        assert!(report.disabled.is_empty());
    } else {
        assert!(report.loaded.is_empty());
        assert_eq!(report.disabled, [PACK_ID]);
        assert!(std::ptr::eq(base, lexicon::active("zh-Hans").unwrap()));
    }
    let active = lexicon::active("zh-Hans").unwrap();
    assert_eq!(base.readings(), &active.readings()[..base.readings().len()]);
    assert_eq!(
        base.continuations().collect::<Vec<_>>(),
        active
            .continuations()
            .take(base.continuations().count())
            .collect::<Vec<_>>()
    );
    assert_eq!(serde_json::to_value(base).unwrap(), original);
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.to_owned(),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}

fn reading_for(word: &str) -> &'static str {
    WORDS.iter().find(|(_, text)| *text == word).unwrap().0
}

fn assert_draft(ime: &XRTabletImeEngine, seed: &str) {
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

fn first_page_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = if kind == CandidateKind::Literal {
        ime.candidates()
            .iter()
            .position(|c| c.text == text && c.kind == kind)
            .unwrap()
    } else {
        first_page_index(ime, text, kind)
    };
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert_eq!(second.text, None);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn shopping_catalog_has_fixed_novel_readings_and_contexts() {
    let pack = shopping_pack();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "zh-Hans");
    assert_eq!(pack.manifest.topics, ["shopping", "errands"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    assert_eq!(pack.lexicon.readings().len(), WORDS.len());
    assert_eq!(pack.lexicon.continuations().count(), SENTENCES.len());
    for (entry, &(reading, word)) in pack.lexicon.readings().iter().zip(WORDS) {
        assert_eq!(entry.reading, reading);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, values), &(word, sentences)) in pack.lexicon.continuations().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            sentences
        );
        assert!(sentences.iter().all(|sentence| sentence.starts_with(word)));
    }
    assert_eq!(
        pack.lexicon
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        24
    );
    let base = lexicon::builtin("zh-Hans").unwrap();
    let all = packs::recommended();
    let old = std::iter::once(base).chain(
        all.iter()
            .filter(|other| other.manifest.id != PACK_ID && other.manifest.language == "zh-Hans")
            .map(|other| &other.lexicon),
    );
    for lexicon in old {
        for &(reading, word) in WORDS {
            assert!(
                lexicon
                    .readings()
                    .iter()
                    .all(|entry| entry.reading != reading && entry.text != word)
            );
        }
        for &(context, sentences) in SENTENCES {
            for (old_context, old_sentences) in lexicon.continuations() {
                assert_ne!(context, old_context);
                assert!(
                    sentences
                        .iter()
                        .all(|sentence| old_sentences.iter().all(|old| old != sentence))
                );
            }
        }
    }
}

#[test]
fn disabled_shopping_pack_leaves_builtin_and_literal_choices_unchanged() {
    in_fresh_process(
        "disabled_shopping_pack_leaves_builtin_and_literal_choices_unchanged",
        |fixture| {
            initialize(fixture, false, false);
            for &(reading, word) in WORDS {
                let seed = reading.replace('\'', " ");
                let mut ime = engine(&seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                assert_draft(&ime, &seed);
                assert!(
                    ime.candidates()
                        .iter()
                        .all(|c| c.text != word || c.kind != CandidateKind::Word)
                );
                commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
            }
            for &(word, sentences) in SENTENCES {
                let ime = engine(word);
                assert!(
                    sentences
                        .iter()
                        .all(|sentence| ime.candidates().iter().all(|c| c.text != *sentence))
                );
            }
        },
    );
}

fn check_word_and_sentence_workflows() {
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
    for &(word, sentences) in SENTENCES {
        for seed in spellings(reading_for(word)) {
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page_index(&ime, sentence, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn shopping_full_spellings_offer_word_and_both_sentence_choices() {
    in_fresh_process(
        "shopping_full_spellings_offer_word_and_both_sentence_choices",
        |fixture| {
            initialize(fixture, true, false);
            check_word_and_sentence_workflows();
        },
    );
}

fn check_unfinished_readings() {
    // These 32 near-complete prefixes were checked against built-ins and every
    // older pack; this does not promise a new word for short ambiguous prefixes.
    for &(reading, word) in WORDS {
        for seed in spellings(&reading[..reading.len() - 1]) {
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
        }
    }
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for seed in spellings(&reading[..reading.len() - 1]) {
            let ime = engine(&seed);
            first_page_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page_index(&ime, sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn shopping_near_complete_readings_keep_word_and_sentence_choices() {
    in_fresh_process(
        "shopping_near_complete_readings_keep_word_and_sentence_choices",
        |fixture| {
            initialize(fixture, true, false);
            check_unfinished_readings();
        },
    );
}

#[test]
fn adopted_shopping_contexts_keep_spacing_and_long_drafts_without_committing() {
    in_fresh_process(
        "adopted_shopping_contexts_keep_spacing_and_long_drafts_without_committing",
        |fixture| {
            initialize(fixture, true, false);
            for &(word, sentences) in SENTENCES {
                let mut ime = engine(reading_for(word));
                ime.select_candidate(first_page_index(&ime, word, CandidateKind::Word));
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
                    for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
                        let seed = format!("{prefix}{adopted}{padding}");
                        for sentence in sentences {
                            ime.seed(&seed);
                            assert_draft(&ime, &seed);
                            let expected =
                                format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                            commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                        }
                    }
                }
            }
        },
    );
}

#[test]
fn adopted_shopping_sentences_continue_with_more_pinyin_or_han_text() {
    in_fresh_process(
        "adopted_shopping_sentences_continue_with_more_pinyin_or_han_text",
        |fixture| {
            initialize(fixture, true, false);
            for (word, reading, tail, sentence) in [
                (
                    "收据放在袋子里",
                    None,
                    "就",
                    "收据放在袋子里就好，我回去再整理。",
                ),
                (
                    "收据放在袋子里",
                    Some("le"),
                    "了",
                    "收据放在袋子里了吗？我想确认一下。",
                ),
                (
                    "先看看别的款式",
                    Some("zai'jue'ding"),
                    "再决定",
                    "先看看别的款式再决定要哪件。",
                ),
            ] {
                for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
                    for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
                        let mut adopted_ime = engine(reading_for(word));
                        adopted_ime.select_candidate(first_page_index(
                            &adopted_ime,
                            word,
                            CandidateKind::Word,
                        ));
                        let adopted = adopted_ime
                            .selected_completion_text(true)
                            .unwrap()
                            .to_owned();
                        assert_eq!(adopted, word);
                        assert!(adopted_ime.snapshot().committed_text.is_empty());
                        for suffix in std::iter::once(tail.to_owned())
                            .chain(reading.into_iter().flat_map(spellings))
                        {
                            let seed = format!("{prefix}{adopted}{padding}{suffix}");
                            let expected = format!(
                                "{prefix}{adopted}{padding}{}",
                                sentence.strip_prefix(word).unwrap()
                            );
                            let mut ime = engine(&seed);
                            assert_draft(&ime, &seed);
                            commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                        }
                    }
                }
            }
        },
    );
}

#[test]
fn unlisted_jiu_reading_preserves_existing_words_and_uncommitted_raw_input() {
    in_fresh_process(
        "unlisted_jiu_reading_preserves_existing_words_and_uncommitted_raw_input",
        |fixture| {
            initialize(fixture, true, false);
            // The current vocabulary has no jiu -> 就 (or jiu'hao -> 就好)
            // reading. Authored sentences do not invent phonetic conversions.
            // Adding that vocabulary later requires an explicit contract update;
            // until then this is a recorded limitation, not supported progress.
            let active = lexicon::active("zh-Hans").unwrap();
            assert!(active.readings().iter().all(|entry| {
                !matches!(
                    (entry.reading.as_str(), entry.text.as_str()),
                    ("jiu", "就") | ("jiu'hao", "就好")
                )
            }));
            let seed = "收据放在袋子里jiu";
            let ime = engine(seed);
            assert_draft(&ime, seed);
            assert_eq!(
                ime.candidates()
                    .iter()
                    .filter(|candidate| candidate.kind == CandidateKind::Word)
                    .map(|candidate| candidate.text.as_str())
                    .collect::<Vec<_>>(),
                [
                    "收据放在袋子里酒店",
                    "收据放在袋子里久等",
                    "收据放在袋子里纠错",
                    "收据放在袋子里就近",
                ]
            );
            assert!(ime.candidates().iter().all(|candidate| {
                candidate.text != "收据放在袋子里就好，我回去再整理。"
            }));
            // Merely requesting suggestions must not adopt or commit any of the
            // unrelated existing words, nor alter the literal draft.
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
        },
    );
}

struct UnavailableProvider(LlmProviderError);

impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "shopping-chinese-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Err(self.0.clone())
    }
}

fn check_provider_failures() {
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            (
                "shi yi jian zai na",
                "试衣间在哪",
                "试衣间在哪？我想试一下这件。",
            ),
            (
                "huan huo xu yao dai shen m",
                "换货需要带什么",
                "换货需要带什么？我先确认一下再过来。",
            ),
        ] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(seed);
            assert_draft(&ime, seed);
            first_page_index(&ime, word, CandidateKind::Word);
            first_page_index(&ime, sentence, CandidateKind::Sentence);
            let before = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(ime.prediction_error(), Some(&error));
            assert_eq!(ime.candidates(), before);
            assert_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn shopping_choices_survive_typed_provider_failures() {
    in_fresh_process(
        "shopping_choices_survive_typed_provider_failures",
        |fixture| {
            initialize(fixture, true, false);
            check_provider_failures();
        },
    );
}

#[test]
fn shopping_workflows_coexist_with_historical_sixteen_packs() {
    in_fresh_process(
        "shopping_workflows_coexist_with_historical_sixteen_packs",
        |fixture| {
            assert_eq!(offline_pack_sets::historical_sixteen().len(), 16);
            initialize(fixture, true, true);
            check_word_and_sentence_workflows();
            check_unfinished_readings();
            check_provider_failures();
        },
    );
}
