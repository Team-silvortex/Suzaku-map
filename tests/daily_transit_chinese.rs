//! Independent authored public-transport fallback expectations, frozen before data.
//! Historical hashes include every earlier reading flag, context and sentence;
//! this batch cannot silently change old candidate order or replace old words.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{self, EntryKind},
};

const WORDS: &[(&str, &str)] = &[
    ("wang'na'bian'zuo'che", "往哪边坐车"),
    ("zuo'na'yi'lu", "坐哪一路"),
    ("zhe'tang'che'dao'na'li", "这趟车到哪里"),
    ("zhe'tang'che'qu'shi'zhong'xin'ma", "这趟车去市中心吗"),
    ("yao'huan'cheng'ma", "要换乘吗"),
    ("zai'na'li'huan'cheng", "在哪里换乘"),
    ("xia'yi'zhan'xia'che", "下一站下车"),
    ("dao'zhan'jiao'wo'yi'xia", "到站叫我一下"),
    ("bie'zuo'guo'zhan'le", "别坐过站了"),
    ("wo'zuo'guo'zhan'le", "我坐过站了"),
    ("zhe'zhan'xia'che'ma", "这站下车吗"),
    ("hai'you'ji'zhan'dao", "还有几站到"),
    ("zai'na'yi'zhan'xia", "在哪一站下"),
    ("mo'ban'che'ji'dian", "末班车几点"),
    ("shou'ban'che'ji'dian", "首班车几点"),
    ("che'piao'zen'me'mai", "车票怎么买"),
    ("neng'mai'dan'cheng'piao'ma", "能买单程票吗"),
    ("neng'mai'wang'fan'piao'ma", "能买往返票吗"),
    ("piao'mai'hao'le'ma", "票买好了吗"),
    ("wo'hai'mei'mai'piao", "我还没买票"),
    ("xu'yao'ti'qian'mai'piao'ma", "需要提前买票吗"),
    ("zai'na'ge'zhan'tai", "在哪个站台"),
    ("qu'na'ge'zhan'tai", "去哪个站台"),
    ("zhan'tai'zai'zhe'bian'ma", "站台在这边吗"),
    ("jian'piao'kou'zai'na'li", "检票口在哪里"),
    ("shen'me'shi'hou'jian'piao", "什么时候检票"),
    ("xian'zai'neng'shang'che'ma", "现在能上车吗"),
    ("zhe'bian'neng'shang'che'ma", "这边能上车吗"),
    ("xian'xia'hou'shang", "先下后上"),
    ("qing'wang'li'mian'zou", "请往里面走"),
    ("xing'li'fang'zai'na'li", "行李放在哪里"),
    ("che'lai'le'jiao'wo", "车来了叫我"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "坐哪一路",
        [
            "坐哪一路能到医院？先看一下线路。",
            "坐哪一路，我们先确认再上车。",
        ],
    ),
    (
        "这趟车到哪里",
        [
            "这趟车到哪里？会经过车站吗？",
            "这趟车到哪里，麻烦告诉我一下。",
        ],
    ),
    (
        "要换乘吗",
        ["要换乘吗？我想先确认路线。", "要换乘吗？不换乘的话更方便。"],
    ),
    (
        "在哪里换乘",
        [
            "在哪里换乘？能帮我指一下吗？",
            "在哪里换乘，我们看清楚再下车。",
        ],
    ),
    (
        "下一站下车",
        ["下一站下车，提前把东西拿好。", "下一站下车，我们一起走。"],
    ),
    (
        "到站叫我一下",
        [
            "到站叫我一下，我怕坐过站。",
            "到站叫我一下，我先闭眼休息一会儿。",
        ],
    ),
    (
        "还有几站到",
        [
            "还有几站到？我先看一下地图。",
            "还有几站到，麻烦提醒我一声。",
        ],
    ),
    (
        "车票怎么买",
        [
            "车票怎么买比较方便？能在手机上买吗？",
            "车票怎么买，我们先问清楚。",
        ],
    ),
    (
        "票买好了吗",
        ["票买好了吗？我们准备进站。", "票买好了吗？我来确认一下。"],
    ),
    (
        "在哪个站台",
        [
            "在哪个站台候车？我们先过去等。",
            "在哪个站台，我们再看一下车票。",
        ],
    ),
    (
        "现在能上车吗",
        [
            "现在能上车吗？我已经准备好了。",
            "现在能上车吗？不能的话就再等一会儿。",
        ],
    ),
    (
        "行李放在哪里",
        ["行李放在哪里比较稳妥？", "行李放在哪里？我怕挡住别人。"],
    ),
];

#[test]
fn transit_preserves_every_preexisting_reading_flag_context_and_sentence() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2752);
    assert!(vocabulary.continuations().count() >= 627);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2752) {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for byte in entry
            .reading
            .bytes()
            .chain([0])
            .chain(entry.text.bytes())
            .chain([0])
            .chain(kind.bytes())
            .chain([u8::from(entry.require_separators)])
        {
            reading_hash = (reading_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(reading_hash, 0xee53f3efe6741189);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(627) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1252);
    assert_eq!(context_hash, 0x6397321baa815303);
}

#[test]
fn transit_adds_the_fixed_thirty_two_readings_twelve_contexts_and_twenty_four_sentences() {
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    let readings = &vocabulary.readings()[2752..2784];
    let contexts = vocabulary
        .continuations()
        .skip(627)
        .take(12)
        .collect::<Vec<_>>();
    assert_eq!(readings.len(), 32);
    assert_eq!(contexts.len(), 12);
    assert_eq!(
        contexts
            .iter()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        24
    );
    for (entry, &(reading, text)) in readings.iter().zip(WORDS) {
        assert_eq!(entry.reading, reading);
        assert_eq!(entry.text, text);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, values), &(word, sentences)) in contexts.into_iter().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            sentences
        );
    }
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
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

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|candidate| candidate.text == seed && candidate.kind == CandidateKind::Literal)
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| candidate.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn first_page_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = if kind == CandidateKind::Literal {
        ime.candidates()
            .iter()
            .position(|candidate| candidate.text == text && candidate.kind == kind)
            .unwrap_or_else(|| panic!("missing literal {text:?}: {:?}", ime.candidates()))
    } else {
        first_page_index(ime, text, kind)
    };
    assert_eq!(ime.candidates()[index].source, CandidateSource::Local);
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
fn transit_words_are_selectable_on_page_one_in_four_full_and_specific_partial_spellings() {
    for &(reading, word) in WORDS {
        for fragment in [reading, &reading[..reading.len() - 1]] {
            for seed in spellings(fragment) {
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_once_and_undo(&mut ime, word, CandidateKind::Word);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn transit_full_and_specific_partial_contexts_offer_words_and_both_authored_sentences() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for fragment in [reading, &reading[..reading.len() - 1]] {
            for seed in spellings(fragment) {
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                first_page_index(&ime, word, CandidateKind::Word);
                for sentence in sentences {
                    commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn adopted_transit_words_keep_six_horizontal_paddings_as_editable_uncommitted_text() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                ime.select_candidate(first_page_index(&ime, word, CandidateKind::Word));
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                let seed = format!("{adopted}{padding}");
                ime.seed(&seed);
                assert_local_draft(&ime, &seed);
                assert_eq!(ime.candidates()[0].text, seed);
                let expected = format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn transit_continues_after_more_pinyin_and_long_unicode_without_rewriting_the_frozen_prefix() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (word, sentence) in [
                ("车票怎么买", "车票怎么买比较方便？能在手机上买吗？"),
                ("行李放在哪里", "行李放在哪里比较稳妥？"),
            ] {
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
                for tail in std::iter::once("比较".to_owned())
                    .chain(spellings("bi'jiao"))
                    .chain(spellings("bi'jia"))
                {
                    let seed = format!("{prefix}{adopted}{padding}{tail}");
                    let expected = format!(
                        "{prefix}{adopted}{padding}{}",
                        sentence.strip_prefix(word).unwrap()
                    );
                    let mut ime = engine(&seed);
                    assert_local_draft(&ime, &seed);
                    commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                    commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
                for trailing in ["", " ", "  ", "\u{3000}"] {
                    let seed = format!("{prefix}{adopted}{padding}比较{trailing}");
                    let suffix = sentence
                        .strip_prefix(word)
                        .unwrap()
                        .strip_prefix("比较")
                        .unwrap();
                    let expected = format!("{seed}{suffix}");
                    let mut ime = engine(&seed);
                    assert_local_draft(&ime, &seed);
                    commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                    commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
            }
        }
    }
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "transit-chinese-synthetic-error"
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
fn transit_local_choices_survive_typed_missing_failed_timeout_and_empty_model_paths() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            (
                "yao huan cheng ma",
                "要换乘吗",
                "要换乘吗？我想先确认路线。",
            ),
            (
                "che piao zen me mai",
                "车票怎么买",
                "车票怎么买比较方便？能在手机上买吗？",
            ),
            (
                "zai na li huan cheng",
                "在哪里换乘",
                "在哪里换乘？能帮我指一下吗？",
            ),
            (
                "xing li fang zai na l",
                "行李放在哪里",
                "行李放在哪里比较稳妥？",
            ),
        ] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(seed);
            assert_local_draft(&ime, seed);
            first_page_index(&ime, word, CandidateKind::Word);
            first_page_index(&ime, sentence, CandidateKind::Sentence);
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
            ime.configure_prediction(None);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            assert_local_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn transit_preserves_seven_control_boundaries_unknown_literals_and_valid_syllable_rules() {
    for boundary in [
        "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
    ] {
        for (word, sentence) in [
            ("车票怎么买", "车票怎么买比较方便？能在手机上买吗？"),
            ("行李放在哪里", "行李放在哪里比较稳妥？"),
        ] {
            let seed = format!("{word}{boundary}bi jiao");
            let crossed = format!("{word}{boundary}{}", sentence.strip_prefix(word).unwrap());
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == crossed)
            );
            commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
        }
    }
    for seed in [
        "zzqvjy",
        "qqzzv  note",
        "“raw”  123\t???",
        "transit@example.invalid",
        "https://example.invalid/transit?q=raw",
        "🙂\u{3000}zzqvjy",
        "行李放在哪里?? zzqvjy",
        "ya o huan cheng ma",
        "che piao ze n me mai",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        assert!(!ime.candidates().iter().any(|candidate| {
            SENTENCES
                .iter()
                .any(|(_, sentences)| sentences.contains(&candidate.text.as_str()))
        }));
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
    // Valid separated/joined readings are positive controls for malformed ones.
    for seed in spellings("yao'huan'cheng'ma") {
        commit_once_and_undo(
            &mut engine(&seed),
            "要换乘吗？我想先确认路线。",
            CandidateKind::Sentence,
        );
    }
}

#[test]
fn transit_does_not_evict_short_preexisting_location_and_demonstrative_words() {
    for (reading, word, prior) in [
        ("zhe'g", "这个", &["这个词", "这个意思", "这个更合适"][..]),
        ("zhe'bia", "这边", &["这边弄好了"][..]),
        ("zai'na'l", "在哪里", &[][..]),
    ] {
        for seed in spellings(reading) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            for previous in prior {
                first_page_index(&ime, previous, CandidateKind::Word);
            }
            commit_once_and_undo(&mut engine(&seed), word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}
