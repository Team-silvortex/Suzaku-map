//! Authored washing, tidying and before-leaving daily fallback expectations.
//! Fixed inputs precede data; no hygiene/medical advice or personal data lookup.
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
    ("wo'xian'xi'ba'lian", "我先洗把脸"),
    ("xian'shua'ge'ya", "先刷个牙"),
    ("mao'jin'fang'zhe'li", "毛巾放这里"),
    ("tou'fa'chui'gan'le", "头发吹干了"),
    ("chu'men'qian'shou'shi'yi'xia", "出门前收拾一下"),
    ("yong'wan'fang'hui'yuan'chu", "用完放回原处"),
    ("wo'qu'xi'ge'zao", "我去洗个澡"),
    ("xi'shu'yong'pin'fang'bao'li", "洗漱用品放包里"),
    ("ya'shua", "牙刷"),
    ("ya'gao", "牙膏"),
    ("shu'zi", "梳子"),
    ("mao'jin", "毛巾"),
    ("fei'zao", "肥皂"),
    ("xiang'zao", "香皂"),
    ("chui'feng'ji", "吹风机"),
    ("jing'zi", "镜子"),
    ("mian'jin", "面巾"),
    ("ya'bei", "牙杯"),
    ("xi'fa'shui", "洗发水"),
    ("mu'yu'lu", "沐浴露"),
    ("xi'shu'bao", "洗漱包"),
    ("xi'shu'yong'pin", "洗漱用品"),
    ("guan'hao'shui'long'tou", "关好水龙头"),
    ("chui'gan'tou'fa", "吹干头发"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "我先洗把脸",
        [
            "我先洗把脸再出门，你先坐一会儿。",
            "我先洗把脸再出门，等我两分钟。",
        ],
    ),
    (
        "先刷个牙",
        ["先刷个牙，等会儿再聊。", "先刷个牙，我马上就好。"],
    ),
    (
        "毛巾放这里",
        [
            "毛巾放这里，洗完手就能拿到。",
            "毛巾放这里，我等会儿挂起来。",
        ],
    ),
    (
        "头发吹干了",
        [
            "头发吹干了，现在可以出门了。",
            "头发吹干了，我把吹风机收起来。",
        ],
    ),
    (
        "出门前收拾一下",
        [
            "出门前收拾一下，别把东西落在桌上。",
            "出门前收拾一下，回来就不用再找了。",
        ],
    ),
    (
        "用完放回原处",
        [
            "用完放回原处，下次拿起来方便。",
            "用完放回原处，我等会儿也要用。",
        ],
    ),
    (
        "我去洗个澡",
        ["我去洗个澡，出来再回复你。", "我去洗个澡，你先看会儿电视。"],
    ),
    (
        "洗漱用品放包里",
        [
            "洗漱用品放包里，住一晚也够用。",
            "洗漱用品放包里，出门时一起带上。",
        ],
    ),
];

#[test]
fn care_preserves_every_current_reading_flag_context_sentence_and_position() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2896);
    assert!(vocabulary.continuations().count() >= 679);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2896) {
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
    assert_eq!(reading_hash, 0x5e805a7dd5856923);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(679) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1356);
    assert_eq!(context_hash, 0xeda1ef0fdf03b501);
}

#[test]
fn care_adds_fixed_twenty_four_readings_eight_contexts_and_sixteen_sentences() {
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 8);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2920);
    let readings = &vocabulary.readings()[2896..2920];
    let contexts = vocabulary
        .continuations()
        .skip(679)
        .take(8)
        .collect::<Vec<_>>();
    assert_eq!(readings.len(), 24);
    assert_eq!(contexts.len(), 8);
    assert_eq!(
        contexts
            .iter()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        16
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

fn spellings(reading: &str) -> [String; 5] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.to_owned(),
        reading.replace('\'', "\t"),
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
    commit_index_once_and_undo(ime, index, text);
}

fn commit_second_page_once_and_undo(ime: &mut XRTabletImeEngine, text: &str) {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == CandidateKind::Sentence)
        .unwrap_or_else(|| {
            panic!(
                "missing later-page sentence {text:?}: {:?}",
                ime.candidates()
            )
        });
    assert!((PAGE_SIZE..2 * PAGE_SIZE).contains(&index));
    commit_index_once_and_undo(ime, index, text);
}

fn commit_index_once_and_undo(ime: &mut XRTabletImeEngine, index: usize, text: &str) {
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
fn care_words_are_selectable_in_five_full_and_specific_partial_spellings() {
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
fn care_specific_context_spellings_offer_words_and_both_fixed_sentences() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for fragment in [reading, &reading[..reading.len() - 1]] {
            for seed in spellings(fragment) {
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                first_page_index(&ime, word, CandidateKind::Word);
                for (branch, sentence) in sentences.into_iter().enumerate() {
                    if word == "出门前收拾一下" && fragment != reading && branch == 1 {
                        // Old 以 + 谢谢/现在/喜欢 words retain the first page.
                        // The verified whole-reading continuation is on page two.
                        commit_second_page_once_and_undo(&mut engine(&seed), sentence);
                    } else {
                        commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                    }
                }
            }
        }
    }
}

#[test]
fn adopted_care_words_keep_six_horizontal_paddings_editable_and_uncommitted() {
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
fn care_pinyin_and_long_unicode_progress_never_rewrite_adopted_frozen_prefixes() {
    let word = "我先洗把脸";
    let tail_reading = "zai'chu'men";
    let tail_word = "再出门";
    let partial = &tail_reading[..tail_reading.len() - 1];
    let sentences = [
        "我先洗把脸再出门，你先坐一会儿。",
        "我先洗把脸再出门，等我两分钟。",
    ];
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (branch, sentence) in sentences.into_iter().enumerate() {
                for tail in std::iter::once(tail_word.to_owned())
                    .chain(spellings(tail_reading))
                    .chain(spellings(partial))
                {
                    let seed = format!("{prefix}{word}{padding}{tail}");
                    let expected = format!(
                        "{prefix}{word}{padding}{}",
                        sentence.strip_prefix(word).unwrap()
                    );
                    let mut ime = engine(&seed);
                    assert_local_draft(&ime, &seed);
                    if branch == 1 && spellings(tail_reading).contains(&tail) {
                        // Complete zai retains the old 在 homophone and 出门 words.
                        // Partial readings and adopted Han progress stay first-page.
                        commit_second_page_once_and_undo(&mut ime, &expected);
                    } else {
                        commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                    }
                    commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
                for trailing in ["", " ", "  ", "\u{3000}"] {
                    let seed = format!("{prefix}{word}{padding}{tail_word}{trailing}");
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

#[test]
fn care_ambiguous_complete_tail_preserves_old_words_and_exact_page_two_submission() {
    for tail in spellings("zai'chu'men") {
        let seed = format!("我先洗把脸{tail}");
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        first_page_index(&ime, "我先洗把脸在出门之前", CandidateKind::Word);
        first_page_index(
            &ime,
            "我先洗把脸再出门，你先坐一会儿。",
            CandidateKind::Sentence,
        );
        commit_second_page_once_and_undo(&mut ime, "我先洗把脸再出门，等我两分钟。");
        commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

#[test]
fn care_ambiguous_whole_reading_completion_generates_its_own_sentences_without_reordering_words() {
    for seed in spellings("chu'men'qian'shou'shi'yi'xi") {
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        assert_eq!(ime.candidates()[0].text, "出门签收释义西");
        assert_eq!(ime.candidates()[0].score, 100.0);
        for (word, score) in [
            ("出门前收拾一下", 84.0),
            ("出门签收释义谢谢", 82.0),
            ("出门签收释义现在", 80.0),
            ("出门签收释义喜欢", 78.0),
        ] {
            let index = first_page_index(&ime, word, CandidateKind::Word);
            assert_eq!(ime.candidates()[index].score, score);
        }
        let first = "出门前收拾一下，别把东西落在桌上。";
        first_page_index(&ime, first, CandidateKind::Sentence);
        commit_second_page_once_and_undo(&mut ime, "出门前收拾一下，回来就不用再找了。");
        commit_once_and_undo(&mut engine(&seed), first, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

#[test]
fn whole_reading_recovery_works_for_old_words_but_keeps_exact_words_and_invalid_boundaries() {
    // This context/reading predates daily_care: the fix must not be a new-word
    // special case. Its correct Word is beyond the five finished conversions.
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(
        vocabulary
            .readings()
            .iter()
            .take(2896)
            .any(|entry| { entry.reading == "zai'shi'yi'ci" && entry.text == "再试一次" })
    );
    let sentences = ["再试一次看看能否连接。", "再试一次，不行的话稍后再处理。"];
    let (_, old_sentences) = vocabulary
        .continuations()
        .take(679)
        .find(|(context, _)| *context == "再试一次")
        .unwrap();
    assert_eq!(old_sentences, sentences);
    for seed in spellings("zai'shi'yi") {
        let ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        assert_eq!(ime.candidates()[0].text, "在释义");
        first_page_index(&ime, "再试一次", CandidateKind::Word);
        for sentence in sentences {
            let mut ime = engine(&seed);
            let index = ime
                .candidates()
                .iter()
                .position(|candidate| {
                    candidate.text == sentence && candidate.kind == CandidateKind::Sentence
                })
                .unwrap_or_else(|| {
                    panic!(
                        "missing old continuation {sentence:?}: {:?}",
                        ime.candidates()
                    )
                });
            assert!(index < 2 * PAGE_SIZE);
            commit_index_once_and_undo(&mut ime, index, sentence);
        }
        commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
    for seed in spellings("shi'jian") {
        let ime = engine(&seed);
        assert_eq!(ime.candidates()[0].text, "时间");
        first_page_index(&ime, "世间", CandidateKind::Word);
        assert!(!ime.candidates().iter().any(|candidate| {
            candidate.kind == CandidateKind::Sentence && candidate.text.starts_with("时间安排")
        }));
    }
    for seed in [
        "zai\nshi yi",
        "zai sh i yi",
        "https://zaishiyi",
        "id_zaishiyi",
        "unknownprefixzaishiyi",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|candidate| sentences.contains(&candidate.text.as_str()))
        );
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "care-chinese-synthetic-error"
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
fn care_local_choices_survive_typed_missing_failure_timeout_and_empty_model_paths() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            (
                "wo xian xi ba lian",
                "我先洗把脸",
                "我先洗把脸再出门，你先坐一会儿。",
            ),
            ("xian shua ge y", "先刷个牙", "先刷个牙，我马上就好。"),
            (
                "yong wan fang hui yuan chu",
                "用完放回原处",
                "用完放回原处，下次拿起来方便。",
            ),
            (
                "tou fa chui gan le",
                "头发吹干了",
                "头发吹干了，现在可以出门了。",
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
fn care_keeps_control_boundaries_unknown_literals_and_valid_syllable_rules() {
    for boundary in [
        "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
    ] {
        let word = "我先洗把脸";
        let seed = format!("{word}{boundary}zai chu men");
        for suffix in ["再出门，你先坐一会儿。", "再出门，等我两分钟。"] {
            let crossed = format!("{word}{boundary}{suffix}");
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
        "care@example.invalid",
        "https://example.invalid/care?q=raw",
        "🙂\u{3000}zzqvjy",
        "我先洗把脸?? zzqvjy",
        "wo xian xi ba li an",
        "chu men qi an shou shi yi xia",
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
}

#[test]
fn care_does_not_evict_preexisting_daily_and_demonstrative_words() {
    for (reading, word, prior) in [
        ("zhe'g", "这个", &["这个词", "这个意思", "这个更合适"][..]),
        ("zhe'bia", "这边", &["这边弄好了"][..]),
        ("zai'na'l", "在哪里", &[][..]),
        (
            "neng",
            "能说具体点吗",
            &["能再解释一下吗", "能举个例子吗"][..],
        ),
        ("shua'ya", "刷牙", &[][..]),
        ("xi'lian", "洗脸", &[][..]),
        ("wo'xian'huan'shuang'xie", "我先换双鞋", &[][..]),
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
