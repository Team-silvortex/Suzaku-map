//! Independent authored daily routine and everyday status fallback expectations.
//! Fixed examples precede data; historical hashes protect every prior reading,
//! flag, context, sentence and original position.
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
    ("gang'qi'chuang", "刚起床"),
    ("hai'mei'shui", "还没睡"),
    ("mang'wan'le", "忙完了"),
    ("xia'ban'le", "下班了"),
    ("chi'bao'le", "吃饱了"),
    ("you'dian'leng", "有点冷"),
    ("you'dian're", "有点热"),
    ("xian'he'dian'shui", "先喝点水"),
    ("zhun'bei'chu'men'le", "准备出门了"),
    ("wo'qu'xi'shu", "我去洗漱"),
    ("jin'tian'qi'wan'le", "今天起晚了"),
    ("xie'yi'hui'er", "歇一会儿"),
    ("xi'shu", "洗漱"),
    ("shua'ya", "刷牙"),
    ("xi'lian", "洗脸"),
    ("leng'shui", "冷水"),
    ("hai'zai'mang", "还在忙"),
    ("gang'xia'ban", "刚下班"),
    ("jin'tian'bu'jia'ban", "今天不加班"),
    ("zhun'bei'shui'le", "准备睡了"),
    ("shui'qian'guan'deng", "睡前关灯"),
    ("zao'dian'xiu'xi", "早点休息"),
    ("xian'chi'dian'dong'xi", "先吃点东西"),
    ("fan'zuo'hao'le", "饭做好了"),
    ("gang'chi'wan'fan", "刚吃完饭"),
    ("hai'mei'chi'fan", "还没吃饭"),
    ("xian'huan'yi'fu", "先换衣服"),
    ("ba'wai'tao'chuan'shang", "把外套穿上"),
    ("wu'xiu'yi'hui'er", "午休一会儿"),
    ("shui'ge'hao'jiao", "睡个好觉"),
    ("zai'chu'men", "再出门"),
    ("zai'jie'zhe'mang", "再接着忙"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    ("刚起床", ["刚起床，我先洗漱一下。", "刚起床，等会儿再聊。"]),
    (
        "还没睡",
        ["还没睡，我把手头的事收个尾。", "还没睡，不过准备休息了。"],
    ),
    (
        "忙完了",
        ["忙完了，我先歇一会儿。", "忙完了，现在可以聊两句。"],
    ),
    (
        "下班了",
        ["下班了，我准备回家。", "下班了，今天的事明天再说。"],
    ),
    (
        "吃饱了",
        ["吃饱了，我们收拾一下桌子吧。", "吃饱了，我先坐一会儿。"],
    ),
    (
        "有点冷",
        ["有点冷，我去拿件外套。", "有点冷，先把窗户关上吧。"],
    ),
    (
        "有点热",
        ["有点热，我想开一会儿窗。", "有点热，等凉快一点再出门。"],
    ),
    (
        "先喝点水",
        ["先喝点水再出门，路上慢点。", "先喝点水再接着忙，不用着急。"],
    ),
    (
        "准备出门了",
        ["准备出门了，东西都带齐了。", "准备出门了，到了再跟你说。"],
    ),
    (
        "我去洗漱",
        ["我去洗漱，等会儿回来。", "我去洗漱，你先坐一会儿。"],
    ),
    (
        "今天起晚了",
        [
            "今天起晚了，我收拾好就出门。",
            "今天起晚了，早餐带着路上吃。",
        ],
    ),
    (
        "歇一会儿",
        ["歇一会儿，等下再继续。", "歇一会儿，我们不赶时间。"],
    ),
];

#[test]
fn routine_preserves_every_preexisting_reading_flag_context_and_sentence() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2816);
    assert!(vocabulary.continuations().count() >= 651);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2816) {
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
    assert_eq!(reading_hash, 0x69a0f5b42deb009f);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(651) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1300);
    assert_eq!(context_hash, 0xcd05f00d20dad7f4);
}

#[test]
fn routine_adds_the_fixed_thirty_two_readings_twelve_contexts_and_twenty_four_sentences() {
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2848);
    let readings = &vocabulary.readings()[2816..2848];
    let contexts = vocabulary
        .continuations()
        .skip(651)
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
fn routine_words_are_selectable_on_page_one_in_five_full_and_specific_partial_spellings() {
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
fn routine_full_and_specific_partial_contexts_offer_words_and_both_authored_sentences() {
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
fn adopted_routine_words_keep_six_horizontal_paddings_as_editable_uncommitted_text() {
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
fn routine_continues_after_more_pinyin_and_long_unicode_without_rewriting_the_frozen_prefix() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (tail_reading, tail_word, sentence) in [
                ("zai'chu'men", "再出门", "先喝点水再出门，路上慢点。"),
                (
                    "zai'jie'zhe'mang",
                    "再接着忙",
                    "先喝点水再接着忙，不用着急。",
                ),
            ] {
                let word = "先喝点水";
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

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "routine-chinese-synthetic-error"
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
fn routine_local_choices_survive_typed_missing_failed_timeout_and_empty_model_paths() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("gang qi chuang", "刚起床", "刚起床，我先洗漱一下。"),
            ("you dian len", "有点冷", "有点冷，我去拿件外套。"),
            (
                "xian he dian shui",
                "先喝点水",
                "先喝点水再出门，路上慢点。",
            ),
            (
                "zhun bei chu men le",
                "准备出门了",
                "准备出门了，东西都带齐了。",
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
fn routine_preserves_seven_control_boundaries_unknown_literals_and_valid_syllable_rules() {
    for boundary in [
        "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
    ] {
        for (tail, sentence) in [
            ("zai chu men", "先喝点水再出门，路上慢点。"),
            ("zai jie zhe mang", "先喝点水再接着忙，不用着急。"),
        ] {
            let word = "先喝点水";
            let seed = format!("{word}{boundary}{tail}");
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
        "routine@example.invalid",
        "https://example.invalid/routine?q=raw",
        "🙂\u{3000}zzqvjy",
        "先喝点水?? zzqvjy",
        "gang qi chu ang",
        "xian he dian sh ui",
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
    for seed in spellings("gang'qi'chuang") {
        commit_once_and_undo(
            &mut engine(&seed),
            "刚起床，我先洗漱一下。",
            CandidateKind::Sentence,
        );
    }
}

#[test]
fn routine_does_not_evict_short_preexisting_location_and_demonstrative_words() {
    for (reading, word, prior) in [
        ("zhe'g", "这个", &["这个词", "这个意思", "这个更合适"][..]),
        ("zhe'bia", "这边", &["这边弄好了"][..]),
        ("zai'na'l", "在哪里", &[][..]),
        (
            "neng",
            "能说具体点吗",
            &["能再解释一下吗", "能举个例子吗"][..],
        ),
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
