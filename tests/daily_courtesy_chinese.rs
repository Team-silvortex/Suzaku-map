//! Independent authored fallback examples for thanks, apologies and courtesy.
//! Fixed expectations were written before production data was appended. The
//! full historical hashes protect every earlier reading and continuation.
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
    ("xie'xie'ni'bang'mang", "谢谢你帮忙"),
    ("xie'xie'ni'ti'xing'wo", "谢谢你提醒我"),
    ("xie'xie'ni'deng'wo", "谢谢你等我"),
    ("xie'xie'ni'pei'wo", "谢谢你陪我"),
    ("xie'xie'ni'li'jie", "谢谢你理解"),
    ("gan'xie'ni'de'nai'xin", "感谢你的耐心"),
    ("gan'xie'ni'de'zhi'chi", "感谢你的支持"),
    ("duo'kui'ni'bang'mang", "多亏你帮忙"),
    ("zhen'shi'bang'le'da'mang", "真是帮了大忙"),
    ("rang'ni'fei'xin'le", "让你费心了"),
    ("you'ni'bang'mang'zhen'hao", "有你帮忙真好"),
    ("zhe'ci'duo'xie'ni'le", "这次多谢你了"),
    ("bao'qian'lai'wan'le", "抱歉来晚了"),
    ("bao'qian'rang'ni'deng'le", "抱歉让你等了"),
    ("bao'qian'mei'shuo'qing'chu", "抱歉没说清楚"),
    ("bao'qian'gang'cai'da'duan'ni", "抱歉刚才打断你"),
    ("bu'hao'yi'si'da'rao'ni", "不好意思打扰你"),
    ("bu'hao'yi'si'ma'fan'ni", "不好意思麻烦你"),
    ("gang'cai'shi'wo'shu'hu'le", "刚才是我疏忽了"),
    ("shi'wo'kao'lv'bu'zhou", "是我考虑不周"),
    ("zhe'ci'shi'wo'de'wen'ti", "这次是我的问题"),
    ("wo'hui'zhu'yi'de", "我会注意的"),
    ("qing'ni'duo'bao'han", "请你多包涵"),
    ("rang'ni'dan'xin'le", "让你担心了"),
    ("ma'fan'bang'wo'kan'yi'xia", "麻烦帮我看一下"),
    ("ma'fan'bang'wo'di'yi'xia", "麻烦帮我递一下"),
    ("ma'fan'zai'shuo'man'yi'dian", "麻烦再说慢一点"),
    ("ma'fan'shao'deng'pian'ke", "麻烦稍等片刻"),
    ("neng'qing'ni'bang'ge'mang'ma", "能请你帮个忙吗"),
    ("fang'bian'de'hua'bang'wo'yi'xia", "方便的话帮我一下"),
    ("qing'bang'wo'liu'yi'yi'xia", "请帮我留意一下"),
    ("qing'jie'wo'yong'yi'xia", "请借我用一下"),
    ("qing'wen'ke'yi'zuo'zhe'li'ma", "请问可以坐这里吗"),
    ("qing'wen'xian'zai'fang'bian'ma", "请问现在方便吗"),
    ("lao'fan'ni'que'ren'yi'xia", "劳烦你确认一下"),
    ("ke'yi'rang'wo'xian'guo'ma", "可以让我先过吗"),
    ("bu'yong'xie'wo", "不用谢我"),
    ("bu'yong'fang'zai'xin'shang", "不用放在心上"),
    ("yi'dian'xiao'shi'er'yi", "一点小事而已"),
    ("neng'bang'dao'ni'jiu'hao", "能帮到你就好"),
    ("da'jia'hu'xiang'bang'mang", "大家互相帮忙"),
    ("ni'tai'ke'qi'le", "你太客气了"),
    ("mei'shi'bie'dan'xin", "没事别担心"),
    ("wo'mei'you'jie'yi", "我没有介意"),
    ("bu'bi'zhe'me'jian'wai", "不必这么见外"),
    ("xia'ci'zhu'yi'jiu'hao", "下次注意就好"),
    ("wo'li'jie'ni'de'nan'chu", "我理解你的难处"),
    ("xie'xie'ni'yuan'yi'bang'mang", "谢谢你愿意帮忙"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "谢谢你帮忙",
        ["谢谢你帮忙确认时间。", "谢谢你帮忙，剩下的我来处理。"],
    ),
    (
        "谢谢你等我",
        ["谢谢你等我，我们现在就出发吧。", "谢谢你等我，让你久等了。"],
    ),
    (
        "感谢你的耐心",
        [
            "感谢你的耐心，我再整理一下思路。",
            "感谢你的耐心，我们慢慢把话说清楚。",
        ],
    ),
    (
        "多亏你帮忙",
        [
            "多亏你帮忙，这件事顺利多了。",
            "多亏你帮忙提醒，我才及时想起来。",
        ],
    ),
    (
        "抱歉来晚了",
        [
            "抱歉来晚了，路上耽搁了一会儿。",
            "抱歉来晚了，我们现在开始吧。",
        ],
    ),
    (
        "抱歉没说清楚",
        [
            "抱歉没说清楚，我重新说明一下。",
            "抱歉没说清楚，让你理解错了。",
        ],
    ),
    (
        "刚才是我疏忽了",
        [
            "刚才是我疏忽了，我现在补上。",
            "刚才是我疏忽了，谢谢你及时提醒。",
        ],
    ),
    (
        "让你担心了",
        [
            "让你担心了，我这边已经没事了。",
            "让你担心了，我应该早点告诉你。",
        ],
    ),
    (
        "能请你帮个忙吗",
        [
            "能请你帮个忙吗，我想再确认一下。",
            "能请你帮个忙吗，帮我拿一下这个袋子。",
        ],
    ),
    (
        "请帮我留意一下",
        [
            "请帮我留意一下时间，快到了就提醒我。",
            "请帮我留意一下，有消息再告诉我。",
        ],
    ),
    (
        "请借我用一下",
        [
            "请借我用一下，我马上就还给你。",
            "请借我用一下，我的暂时找不到了。",
        ],
    ),
    (
        "请问现在方便吗",
        [
            "请问现在方便吗，我有件事想请教你。",
            "请问现在方便吗，不方便的话我晚点再来。",
        ],
    ),
    (
        "不用放在心上",
        [
            "不用放在心上，事情已经说清楚了。",
            "不用放在心上，下次记得就好。",
        ],
    ),
    (
        "能帮到你就好",
        [
            "能帮到你就好，下次有需要再说。",
            "能帮到你就好，这点事不用客气。",
        ],
    ),
    (
        "没事别担心",
        ["没事别担心，我们慢慢处理。", "没事别担心，我没有放在心上。"],
    ),
    (
        "我理解你的难处",
        [
            "我理解你的难处，我们再想个办法。",
            "我理解你的难处，你不用急着回答。",
        ],
    ),
];

#[test]
fn courtesy_preserves_all_preexisting_chinese_readings_and_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2592);
    assert!(vocabulary.continuations().count() >= 571);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2592) {
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
    assert_eq!(reading_hash, 0x2d3774f9fc2e81e1);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(571) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1140);
    assert_eq!(context_hash, 0x1e16d3943c413dda);
}

#[test]
fn courtesy_adds_exactly_the_authored_readings_and_independent_contexts() {
    assert_eq!(WORDS.len(), 48);
    assert_eq!(SENTENCES.len(), 16);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    // Fix this authored window, not the size of a later append-only catalog.
    // Strict current totals remain covered by lexicon_resources.
    let readings = &vocabulary.readings()[2592..2640];
    let contexts = vocabulary
        .continuations()
        .skip(571)
        .take(16)
        .collect::<Vec<_>>();
    assert_eq!(readings.len(), 48);
    assert_eq!(contexts.len(), 16);
    assert_eq!(
        vocabulary
            .continuations()
            .take(587)
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1172
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
fn courtesy_words_are_on_page_one_in_all_four_spellings_without_a_model() {
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
fn explicit_courtesy_readings_offer_words_and_both_sentences_on_page_one() {
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
fn unfinished_courtesy_readings_keep_words_and_authored_sentence_choices() {
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
fn adopted_courtesy_words_preserve_space_padding_without_committing() {
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
fn courtesy_sentences_continue_after_adoption_and_more_pinyin_with_long_drafts() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (word, tail_reading, tail_word, sentence) in [
                ("谢谢你帮忙", "que'ren", "确认", "谢谢你帮忙确认时间。"),
                (
                    "请帮我留意一下",
                    "shi'jian",
                    "时间",
                    "请帮我留意一下时间，快到了就提醒我。",
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
        "courtesy-chinese-synthetic-error"
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
fn courtesy_candidates_survive_synthetic_provider_failures() {
    // Typed failures do not send network requests or evaluate any model.
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            ("xie xie ni bang mang", "谢谢你帮忙", "谢谢你帮忙确认时间。"),
            (
                "bao qian lai wan le",
                "抱歉来晚了",
                "抱歉来晚了，路上耽搁了一会儿。",
            ),
            (
                "qing bang wo liu yi yi xi",
                "请帮我留意一下",
                "请帮我留意一下时间，快到了就提醒我。",
            ),
            (
                "mei shi bie dan xin",
                "没事别担心",
                "没事别担心，我们慢慢处理。",
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
fn courtesy_unknown_text_and_control_boundaries_keep_the_exact_literal_choice() {
    for seed in [
        "zzqvjy",
        "qqzzv  note",
        "“raw”  123\t???",
        "courtesy@example.invalid",
        "https://example.invalid/courtesy?q=raw",
        "🙂\u{3000}zzqvjy",
        "谢谢你帮忙??  zzqvjy",
        "谢谢你帮忙\nque ren",
        "谢谢你帮忙\rque ren",
        "谢谢你帮忙\u{2028}que ren",
        "谢谢你帮忙\u{2029}que ren",
        "请帮我留意一下。\tqqzzv",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for boundary in ["\n", "\r", "\u{2028}", "\u{2029}"] {
            let crossed = format!("谢谢你帮忙{boundary}确认时间。");
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == crossed)
            );
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}
