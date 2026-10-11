//! Independent authored phone, network and everyday contact fallback expectations.
//! Fixed examples precede data; historical hashes protect all prior readings,
//! flags, contexts, sentences and their original order.
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
    ("ni'neng'ting'qing'ma", "你能听清吗"),
    ("wo'ting'de'qing", "我听得清"),
    ("ting'qi'lai'duan'duan'xu'xu", "听起来断断续续"),
    ("sheng'yin'hu'da'hu'xiao", "声音忽大忽小"),
    ("gang'cai'mei'ting'qing", "刚才没听清"),
    ("zai'shuo'man'yi'dian", "再说慢一点"),
    ("li'mai'ke'feng'jin'yi'dian", "离麦克风近一点"),
    ("ni'na'bian'you'hui'sheng", "你那边有回声"),
    ("xian'guan'yi'xia'mai'ke'feng", "先关一下麦克风"),
    ("neng'kai'yi'xia'sheng'yin'ma", "能开一下声音吗"),
    ("wo'ba'sheng'yin'tiao'da'yi'dian", "我把声音调大一点"),
    ("wo'men'chong'xin'bo'yi'xia", "我们重新拨一下"),
    ("dian'hua'tu'ran'duan'le", "电话突然断了"),
    ("wo'zai'da'guo'qu", "我再打过去"),
    ("fang'bian'jie'dian'hua'ma", "方便接电话吗"),
    ("xian'zai'bu'fang'bian'jie", "现在不方便接"),
    ("deng'xia'gei'ni'hui'dian'hua", "等下给你回电话"),
    ("ni'xian'bie'gua", "你先别挂"),
    ("xin'hao'bu'tai'wen'ding", "信号不太稳定"),
    ("wang'luo'you'dian'man", "网络有点慢"),
    ("ke'neng'shi'wang'luo'wen'ti", "可能是网络问题"),
    ("xian'jian'cha'yi'xia'wang'luo", "先检查一下网络"),
    ("lian'shang'wang'luo'le'ma", "连上网络了吗"),
    ("shi'zhe'chong'xin'lian'jie", "试着重新连接"),
    ("huan'ge'wang'luo'shi'shi", "换个网络试试"),
    ("wo'fa'xiao'xi'gei'ni", "我发消息给你"),
    ("xiao'xi'fa'chu'qu'le'ma", "消息发出去了吗"),
    ("xiao'xi'hai'mei'fa'chu'qu", "消息还没发出去"),
    ("shou'dao'le'jiu'hui'wo'yi'xia", "收到了就回我一下"),
    ("lian'xi'shang'le'ma", "联系上了吗"),
    ("wo'hai'mei'lian'xi'shang", "我还没联系上"),
    ("huan'ge'fang'shi'lian'xi", "换个方式联系"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "你能听清吗",
        [
            "你能听清吗？我把声音调大一点。",
            "你能听清吗？听不清的话我们重新拨。",
        ],
    ),
    (
        "刚才没听清",
        [
            "刚才没听清，麻烦再说一遍。",
            "刚才没听清，你可以说慢一点吗？",
        ],
    ),
    (
        "我们重新拨一下",
        [
            "我们重新拨一下，看看声音会不会好一点。",
            "我们重新拨一下，这边听着有点断。",
        ],
    ),
    (
        "方便接电话吗",
        [
            "方便接电话吗？我有点事想问你。",
            "方便接电话吗？不方便的话晚点再聊。",
        ],
    ),
    (
        "等下给你回电话",
        [
            "等下给你回电话，我现在不方便说话。",
            "等下给你回电话，等我找个安静的地方。",
        ],
    ),
    (
        "信号不太稳定",
        [
            "信号不太稳定，我们先用消息联系。",
            "信号不太稳定，我换个地方试试。",
        ],
    ),
    (
        "网络有点慢",
        ["网络有点慢，我等一下再发。", "网络有点慢，你先别急。"],
    ),
    (
        "连上网络了吗",
        [
            "连上网络了吗？我们再试一次。",
            "连上网络了吗？没有的话先检查设置。",
        ],
    ),
    (
        "换个网络试试",
        [
            "换个网络试试，看看能不能连上。",
            "换个网络试试，可能会好一点。",
        ],
    ),
    (
        "我发消息给你",
        ["我发消息给你确认一下时间。", "我发消息给你说明一下情况。"],
    ),
    (
        "收到了就回我一下",
        ["收到了就回我一下，我好放心。", "收到了就回我一下，不着急。"],
    ),
    (
        "换个方式联系",
        [
            "换个方式联系也可以，选个方便的。",
            "换个方式联系，免得一直等不到消息。",
        ],
    ),
];

#[test]
fn contact_preserves_every_preexisting_reading_flag_context_and_sentence() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2784);
    assert!(vocabulary.continuations().count() >= 639);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2784) {
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
    assert_eq!(reading_hash, 0xa2809d1ac7d8ab0f);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(639) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1276);
    assert_eq!(context_hash, 0x29d9ddacc78049b5);
}

#[test]
fn contact_adds_the_fixed_thirty_two_readings_twelve_contexts_and_twenty_four_sentences() {
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    let readings = &vocabulary.readings()[2784..2816];
    let contexts = vocabulary
        .continuations()
        .skip(639)
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
fn contact_words_are_selectable_on_page_one_in_four_full_and_specific_partial_spellings() {
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
fn contact_full_and_specific_partial_contexts_offer_words_and_both_authored_sentences() {
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
fn adopted_contact_words_keep_six_horizontal_paddings_as_editable_uncommitted_text() {
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
fn contact_continues_after_more_pinyin_and_long_unicode_without_rewriting_the_frozen_prefix() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (tail_reading, tail_word, sentence) in [
                ("que'ren", "确认", "我发消息给你确认一下时间。"),
                ("shuo'ming", "说明", "我发消息给你说明一下情况。"),
            ] {
                let word = "我发消息给你";
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
        "contact-chinese-synthetic-error"
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
fn contact_local_choices_survive_typed_missing_failed_timeout_and_empty_model_paths() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            (
                "ni neng ting qing ma",
                "你能听清吗",
                "你能听清吗？我把声音调大一点。",
            ),
            (
                "huan ge wang luo shi shi",
                "换个网络试试",
                "换个网络试试，看看能不能连上。",
            ),
            (
                "fang bian jie dian hua ma",
                "方便接电话吗",
                "方便接电话吗？我有点事想问你。",
            ),
            (
                "xin hao bu tai wen din",
                "信号不太稳定",
                "信号不太稳定，我们先用消息联系。",
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
fn contact_preserves_seven_control_boundaries_unknown_literals_and_valid_syllable_rules() {
    for boundary in [
        "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
    ] {
        for (tail, sentence) in [
            ("que ren", "我发消息给你确认一下时间。"),
            ("shuo ming", "我发消息给你说明一下情况。"),
        ] {
            let word = "我发消息给你";
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
        "contact@example.invalid",
        "https://example.invalid/contact?q=raw",
        "🙂\u{3000}zzqvjy",
        "我发消息给你?? zzqvjy",
        "ni neng ti ng qing ma",
        "huan ge wang lu o shi shi",
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
    for seed in spellings("ni'neng'ting'qing'ma") {
        commit_once_and_undo(
            &mut engine(&seed),
            "你能听清吗？我把声音调大一点。",
            CandidateKind::Sentence,
        );
    }
}

#[test]
fn contact_does_not_evict_short_preexisting_location_and_demonstrative_words() {
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
