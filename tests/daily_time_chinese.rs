//! Independent authored fallback examples for time, waiting and rescheduling.
//! Fixed expectations predate production data. Full historical hashes preserve
//! every prior reading and continuation in its existing order.
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
    ("lai'de'ji'ma", "来得及吗"),
    ("hai'lai'de'ji", "还来得及"),
    ("lai'de'ji'gan'guo'qu'ma", "来得及赶过去吗"),
    ("hai'you'duo'jiu", "还有多久"),
    ("hai'yao'duo'jiu", "还要多久"),
    ("da'gai'ji'dian'dao", "大概几点到"),
    ("wan'ji'fen'zhong", "晚几分钟"),
    ("zao'ji'fen'zhong", "早几分钟"),
    ("zai'deng'yi'hui", "再等一会"),
    ("neng'zai'deng'yi'hui'er'ma", "能再等一会儿吗"),
    ("wo'zai'deng'yi'hui'er", "我再等一会儿"),
    ("bie'deng'tai'jiu", "别等太久"),
    ("hai'yao'deng'yi'hui'er", "还要等一会儿"),
    ("kuai'lun'dao'wo'le", "快轮到我了"),
    ("shen'me'shi'hou'kai'shi", "什么时候开始"),
    ("shen'me'shi'hou'jie'shu", "什么时候结束"),
    ("ji'dian'neng'dao", "几点能到"),
    ("xian'zai'chu'fa'ma", "现在出发吗"),
    ("wo'men'wan'dian'chu'fa", "我们晚点出发"),
    ("neng'ti'qian'yi'dian'ma", "能提前一点吗"),
    ("neng'tui'chi'yi'dian'ma", "能推迟一点吗"),
    ("neng'gai'dao'ming'tian'ma", "能改到明天吗"),
    ("huan'ge'shi'jian'jian", "换个时间见"),
    ("huan'dao'xia'wu'ba", "换到下午吧"),
    ("gai'dao'zhou'mo'ba", "改到周末吧"),
    ("wang'hou'nuo'yi'dian", "往后挪一点"),
    ("xian'ba'shi'jian'ding'xia'lai", "先把时间定下来"),
    ("ding'hao'shi'jian'gao'su'wo", "定好时间告诉我"),
    ("dao'shi'hou'ti'xing'wo", "到时候提醒我"),
    ("ti'xing'wo'yi'xia", "提醒我一下"),
    ("bie'wang'le'shi'jian", "别忘了时间"),
    ("shi'jian'hai'mei'ding", "时间还没定"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "来得及吗",
        [
            "来得及吗？我们现在就准备出门。",
            "来得及吗？不行就换个时间。",
        ],
    ),
    (
        "还有多久",
        [
            "还有多久开始？我先去倒杯水。",
            "还有多久，我们先问清楚再走。",
        ],
    ),
    (
        "晚几分钟",
        [
            "晚几分钟也没关系，路上别着急。",
            "晚几分钟到的话，先告诉我一声。",
        ],
    ),
    (
        "再等一会",
        ["再等一会，我快收拾好了。", "再等一会，我们一起出发。"],
    ),
    (
        "能再等一会儿吗",
        [
            "能再等一会儿吗？我马上回来。",
            "能再等一会儿吗？还有一点没弄好。",
        ],
    ),
    (
        "大概几点到",
        ["大概几点到？我好安排时间。", "大概几点到，你方便时告诉我。"],
    ),
    (
        "能提前一点吗",
        [
            "能提前一点吗？我下午有别的安排。",
            "能提前一点吗？不方便的话就按原来的时间。",
        ],
    ),
    (
        "能改到明天吗",
        [
            "能改到明天吗？今天有点赶。",
            "能改到明天吗？我确认好时间再回复你。",
        ],
    ),
    (
        "换个时间见",
        [
            "换个时间见吧，选个大家都方便的。",
            "换个时间见也可以，我们再商量一下。",
        ],
    ),
    (
        "先把时间定下来",
        [
            "先把时间定下来，再安排其他事情。",
            "先把时间定下来，地点可以晚点确认。",
        ],
    ),
    (
        "到时候提醒我",
        [
            "到时候提醒我带伞，免得又忘了。",
            "到时候提醒我，我怕忙起来忘了。",
        ],
    ),
    (
        "提醒我一下",
        [
            "提醒我一下出发时间，我怕记错。",
            "提醒我一下，我就不会忘了。",
        ],
    ),
];

#[test]
fn time_preserves_all_preexisting_chinese_readings_and_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2688);
    assert!(vocabulary.continuations().count() >= 603);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2688) {
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
    assert_eq!(reading_hash, 0x184312ceac6f9a97);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(603) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1204);
    assert_eq!(context_hash, 0xcf028dabad3666b1);
}

#[test]
fn time_adds_the_fixed_authored_readings_and_independent_contexts() {
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    // Bound this fixed batch so future append-only expansions retain it.
    let readings = &vocabulary.readings()[2688..2720];
    let contexts = vocabulary
        .continuations()
        .skip(603)
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
    commit_index_and_undo(ime, index, text, kind);
}

fn commit_index_and_undo(
    ime: &mut XRTabletImeEngine,
    index: usize,
    text: &str,
    kind: CandidateKind,
) {
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
fn time_words_are_on_page_one_in_four_full_and_partial_spellings_without_a_model() {
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
fn full_and_specific_partial_time_readings_offer_words_and_both_sentences_on_page_one() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for fragment in [reading, &reading[..reading.len() - 1]] {
            if word == "提醒我一下" && fragment != reading {
                // The ambiguous 以西 path is covered below, including its
                // later-page second sentence and exact selection.
                continue;
            }
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
fn ambiguous_reminder_partial_keeps_old_split_and_second_page_sentence() {
    for seed in spellings("ti'xing'wo'yi'xi") {
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        assert_eq!(ime.candidates()[0].text, "提醒我以西");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
        first_page_index(&ime, "提醒我以谢谢", CandidateKind::Word);
        commit_once_and_undo(&mut engine(&seed), "提醒我一下", CandidateKind::Word);
        commit_once_and_undo(
            &mut engine(&seed),
            "提醒我一下出发时间，我怕记错。",
            CandidateKind::Sentence,
        );
        let second = "提醒我一下，我就不会忘了。";
        let selected = ime
            .candidates()
            .iter()
            .position(|c| c.text == second && c.kind == CandidateKind::Sentence)
            .unwrap();
        assert!((PAGE_SIZE..12).contains(&selected));
        commit_index_and_undo(&mut ime, selected, second, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

#[test]
fn adopted_time_words_preserve_space_padding_without_committing() {
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
fn time_sentences_continue_after_adoption_and_more_pinyin_with_long_drafts() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (word, tail_reading, tail_word, sentence) in [
                (
                    "到时候提醒我",
                    "dai'san",
                    "带伞",
                    "到时候提醒我带伞，免得又忘了。",
                ),
                (
                    "提醒我一下",
                    "chu'fa'shi'jian",
                    "出发时间",
                    "提醒我一下出发时间，我怕记错。",
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
        "time-chinese-synthetic-error"
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
fn time_candidates_survive_synthetic_provider_failures() {
    // Typed failures do not send network requests or evaluate any model.
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            ("lai de ji ma", "来得及吗", "来得及吗？我们现在就准备出门。"),
            (
                "huan ge shi jian jian",
                "换个时间见",
                "换个时间见吧，选个大家都方便的。",
            ),
            ("zai deng yi hui", "再等一会", "再等一会，我快收拾好了。"),
            (
                "dao shi hou ti xing w",
                "到时候提醒我",
                "到时候提醒我带伞，免得又忘了。",
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
fn time_unknown_text_and_control_boundaries_keep_the_exact_literal_choice() {
    for seed in [
        "zzqvjy",
        "qqzzv  note",
        "“raw”  123\t???",
        "time@example.invalid",
        "https://example.invalid/time?q=raw",
        "🙂\u{3000}zzqvjy",
        "到时候提醒我??  zzqvjy",
        "到时候提醒我\ndai san",
        "到时候提醒我\rdai san",
        "到时候提醒我\u{2028}dai san",
        "到时候提醒我\u{2029}dai san",
        "提醒我一下。\tqqzzv",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for boundary in ["\n", "\r", "\u{2028}", "\u{2029}"] {
            let crossed = format!("到时候提醒我{boundary}带伞，免得又忘了。");
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == crossed)
            );
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}
