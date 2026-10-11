//! Independent authored fallback examples for location, directions and meeting.
//! Fixed expectations predate production data. Historical hashes include every
//! prior reading and continuation, including the uncommitted courtesy batch.
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
    ("men'kou", "门口"),
    ("dui'mian", "对面"),
    ("zuo'bian", "左边"),
    ("you'bian", "右边"),
    ("qian'mian", "前面"),
    ("hou'mian", "后面"),
    ("li'mian", "里面"),
    ("wai'mian", "外面"),
    ("zhe'bian", "这边"),
    ("na'bian", "那边"),
    ("xie'dui'mian", "斜对面"),
    ("guai'jiao", "拐角"),
    ("zhuan'jiao", "转角"),
    ("wang'zuo", "往左"),
    ("wang'you", "往右"),
    ("zou'guo'tou'le", "走过头了"),
    ("zou'cuo'lu'le", "走错路了"),
    ("zai'na'li", "在哪里"),
    ("ni'zai'na'li", "你在哪里"),
    ("wo'zai'zhe'li", "我在这里"),
    ("wo'zai'lou'xia", "我在楼下"),
    ("wo'zai'men'kou", "我在门口"),
    ("ni'zai'ji'lou", "你在几楼"),
    ("wo'kan'jian'ni'le", "我看见你了"),
    ("zai'ru'kou'deng'ni", "在入口等你"),
    ("zai'chu'kou'deng'ni", "在出口等你"),
    ("zan'men'zai'na'er'jian", "咱们在哪儿见"),
    ("huan'ge'di'fang'jian", "换个地方见"),
    ("zai'lu'kou'peng'mian", "在路口碰面"),
    ("cong'na'bian'jin'qu", "从哪边进去"),
    ("cong'zhe'bian'zou", "从这边走"),
    ("rao'dao'hou'mian", "绕到后面"),
    ("wang'qian'zou'yi'dian", "往前走一点"),
    ("li'zhe'li'yuan'ma", "离这里远吗"),
    ("li'zhe'li'bu'yuan", "离这里不远"),
    ("jiu'zai'qian'mian", "就在前面"),
    ("xian'bie'zou'kai", "先别走开"),
    ("deng'wo'guo'qu", "等我过去"),
    ("wo'lai'zhao'ni", "我来找你"),
    ("ni'guo'lai'zhao'wo", "你过来找我"),
    ("wo'guo'qu'jie'ni", "我过去接你"),
    ("wo'hai'mei'zhao'dao", "我还没找到"),
    ("bei'bian", "北边"),
    ("nan'bian", "南边"),
    ("dong'bian", "东边"),
    ("xi'bian", "西边"),
    ("lu'bian", "路边"),
    ("tong'yi'ce", "同一侧"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "门口",
        ["门口有张长椅，可以在那里等。", "门口见吧，到了说一声。"],
    ),
    (
        "对面",
        ["对面就是我们要找的地方。", "对面有个路牌，我们在那里碰面。"],
    ),
    (
        "左边",
        ["左边有个入口，先看看标牌。", "左边那条路我还没走过。"],
    ),
    (
        "右边",
        ["右边是楼梯，我们从那里上去。", "右边有个空位，你坐那里吧。"],
    ),
    (
        "走过头了",
        [
            "走过头了，我们回刚才的路口看看。",
            "走过头了，难怪一直没看见入口。",
        ],
    ),
    (
        "走错路了",
        ["走错路了，我们先停下来问问。", "走错路了，换条路再试试。"],
    ),
    (
        "从哪边进去",
        ["从哪边进去比较方便？", "从哪边进去，我们先看一下指示牌。"],
    ),
    (
        "往前走一点",
        [
            "往前走一点，就能看到那个路口。",
            "往前走一点，我们在路牌旁边等。",
        ],
    ),
    (
        "我在楼下",
        ["我在楼下入口等你。", "我在楼下，你慢慢下来就好。"],
    ),
    (
        "我在门口",
        ["我在门口等你，你慢慢来。", "我在门口，到了叫我一声。"],
    ),
    (
        "你在几楼",
        ["你在几楼，我上去找你。", "你在几楼，我们先确认一下。"],
    ),
    (
        "在路口碰面",
        [
            "在路口碰面吧，那里比较好找。",
            "在路口碰面，到了互相说一声。",
        ],
    ),
    (
        "离这里远吗",
        ["离这里远吗，走路能到吗？", "离这里远吗，我们先问清楚再走。"],
    ),
    (
        "等我过去",
        ["等我过去，我们再一起进去。", "等我过去，你先在原处等一下。"],
    ),
    (
        "我来找你",
        ["我来找你，你先别走开。", "我来找你之前，先跟你说一声。"],
    ),
    (
        "我还没找到",
        [
            "我还没找到出口，可能得再问一下。",
            "我还没找到，你能说得具体一点吗？",
        ],
    ),
];

#[test]
fn location_preserves_all_preexisting_chinese_readings_and_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2640);
    assert!(vocabulary.continuations().count() >= 587);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2640) {
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
    assert_eq!(reading_hash, 0x7ffa9fdb218d09a4);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(587) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1172);
    assert_eq!(context_hash, 0x6eb706d5d22d008d);
}

#[test]
fn location_adds_the_fixed_authored_readings_and_independent_contexts() {
    assert_eq!(WORDS.len(), 48);
    assert_eq!(SENTENCES.len(), 16);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    // Bound this authored batch so later append-only expansions retain it.
    let readings = &vocabulary.readings()[2640..2688];
    let contexts = vocabulary
        .continuations()
        .skip(587)
        .take(16)
        .collect::<Vec<_>>();
    assert_eq!(readings.len(), 48);
    assert_eq!(contexts.len(), 16);
    assert_eq!(
        contexts
            .iter()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        32
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
        ime.candidates().iter().any(|candidate| {
            candidate.text == seed && candidate.kind == CandidateKind::Literal
        })
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
    assert_eq!(ime.candidates()[index].text, text);
    assert_eq!(ime.candidates()[index].kind, kind);
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
fn location_words_are_on_page_one_in_all_four_spellings_without_a_model() {
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn explicit_location_readings_offer_words_and_both_sentences_on_page_one() {
    for &(word, sentences) in SENTENCES {
        for seed in spellings(reading_for(word)) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn unfinished_location_readings_keep_words_and_authored_sentence_choices() {
    for &(reading, word) in WORDS {
        for seed in spellings(&reading[..reading.len() - 1]) {
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
        }
    }
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for seed in spellings(&reading[..reading.len() - 1]) {
            let ime = engine(&seed);
            first_page_index(&ime, sentences[0], CandidateKind::Sentence);
            for sentence in sentences {
                assert!(
                    ime.candidates().iter().any(|candidate| {
                        candidate.text == sentence && candidate.kind == CandidateKind::Sentence
                    }),
                    "missing {sentence:?} for {seed:?}: {:?}",
                    ime.candidates()
                );
            }
        }
    }
}

#[test]
fn adopted_location_words_preserve_space_padding_without_committing() {
    for &(word, sentences) in SENTENCES {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for sentence in sentences {
                let mut ime = engine(reading_for(word));
                ime.select_candidate(first_page_index(&ime, word, CandidateKind::Word));
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                // Host-style editable adoption/Space, not physical-key simulation.
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
fn location_sentences_continue_after_adoption_and_more_pinyin_with_long_drafts() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (word, tail_reading, tail_word, sentence) in [
                ("我在楼下", "ru'kou", "入口", "我在楼下入口等你。"),
                (
                    "我还没找到",
                    "chu'kou",
                    "出口",
                    "我还没找到出口，可能得再问一下。",
                ),
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
                let partial = &tail_reading[..tail_reading.len() - 1];
                for tail in std::iter::once(tail_word.to_owned())
                    .chain(spellings(tail_reading))
                    .chain(spellings(partial))
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
                    let seed = format!("{prefix}{adopted}{padding}{tail_word}{trailing}");
                    let suffix = sentence
                        .strip_prefix(word)
                        .unwrap()
                        .strip_prefix(tail_word)
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

struct UnavailableProvider(LlmProviderError);

impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "location-chinese-synthetic-error"
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

#[test]
fn location_candidates_survive_synthetic_provider_failures() {
    // Typed failures do not send network requests or evaluate any model.
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            ("men kou", "门口", "门口有张长椅，可以在那里等。"),
            ("zou cuo lu le", "走错路了", "走错路了，我们先停下来问问。"),
            ("ni zai ji lo", "你在几楼", "你在几楼，我上去找你。"),
            ("deng wo guo qu", "等我过去", "等我过去，我们再一起进去。"),
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
            assert_eq!(ime.prediction_error(), Some(&error));
            assert_eq!(ime.candidates(), before);
            ime.configure_prediction(None);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            assert_local_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn location_unknown_text_and_control_boundaries_keep_the_exact_literal_choice() {
    for seed in [
        "zzqvjy",
        "qqzzv  note",
        "“raw”  123\t???",
        "location@example.invalid",
        "https://example.invalid/location?q=raw",
        "🙂\u{3000}zzqvjy",
        "我在楼下??  zzqvjy",
        "我在楼下\nru kou",
        "我在楼下\rru kou",
        "我在楼下\u{2028}ru kou",
        "我在楼下\u{2029}ru kou",
        "我还没找到。\tqqzzv",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for boundary in ["\n", "\r", "\u{2028}", "\u{2029}"] {
            let crossed = format!("我在楼下{boundary}入口等你。");
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == crossed)
            );
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}
