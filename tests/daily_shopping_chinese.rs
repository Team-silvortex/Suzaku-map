//! Independent authored fallback examples for shopping, prices and payments.
//! Fixed expectations precede the production data; historical hashes protect
//! all prior readings, flags, contexts and sentences in their original order.
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
    ("hai'neng'pian'yi'yi'dian'ma", "还能便宜一点吗"),
    ("neng'shao'yi'dian'ma", "能少一点吗"),
    ("you'you'hui'ma", "有优惠吗"),
    ("zhe'jian'da'zhe'ma", "这件打折吗"),
    ("ke'yi'shi'yi'xia'ma", "可以试一下吗"),
    ("you'da'yi'hao'de'ma", "有大一号的吗"),
    ("you'xiao'yi'hao'de'ma", "有小一号的吗"),
    ("hai'you'bie'de'yan'se'ma", "还有别的颜色吗"),
    ("zhe'jian'hai'you'huo'ma", "这件还有货吗"),
    ("shen'me'shi'hou'bu'huo", "什么时候补货"),
    ("wo'xiang'tui'yi'xia", "我想退一下"),
    ("ke'yi'huan'yi'xia'ma", "可以换一下吗"),
    ("neng'huan'ge'chi'ma'ma", "能换个尺码吗"),
    ("tui'huo'zen'me'chu'li", "退货怎么处理"),
    ("xiao'piao'hai'zai", "小票还在"),
    ("xu'yao'liu'xiao'piao'ma", "需要留小票吗"),
    ("ke'yi'sao'ma'ma", "可以扫码吗"),
    ("neng'shua'ka'ma", "能刷卡吗"),
    ("zhi'chi'xian'jin'ma", "支持现金吗"),
    ("fu'kuan'cheng'gong'le'ma", "付款成功了吗"),
    ("wo'yi'jing'fu'le", "我已经付了"),
    ("bang'wo'kan'yi'xia'jia'ge", "帮我看一下价格"),
    ("zhe'liang'ge'you'shen'me'qu'bie", "这两个有什么区别"),
    ("wo'yao'zhe'yi'jian", "我要这一件"),
    ("xian'bu'mai'le", "先不买了"),
    ("wo'zai'bi'jiao'yi'xia", "我再比较一下"),
    ("xian'bang'wo'liu'zhe", "先帮我留着"),
    ("neng'song'dao'jia'ma", "能送到家吗"),
    ("yun'fei'zen'me'suan", "运费怎么算"),
    ("yi'gong'yao'duo'shao'qian", "一共要多少钱"),
    ("bao'zhuang'neng'jian'dan'yi'dian'ma", "包装能简单一点吗"),
    ("bang'wo'zhuang'dai", "帮我装袋"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "还能便宜一点吗",
        [
            "还能便宜一点吗？可以的话我就买这一件。",
            "还能便宜一点吗？不方便的话我再看看。",
        ],
    ),
    (
        "可以试一下吗",
        [
            "可以试一下吗？我想确认尺码。",
            "可以试一下吗？合适的话我就买。",
        ],
    ),
    (
        "有大一号的吗",
        [
            "有大一号的吗？这一件有点紧。",
            "有大一号的吗？我想穿得宽松一点。",
        ],
    ),
    (
        "还有别的颜色吗",
        [
            "还有别的颜色吗？我想再比较一下。",
            "还有别的颜色吗？没有的话就要这一件。",
        ],
    ),
    (
        "这件还有货吗",
        [
            "这件还有货吗？我想今天带走。",
            "这件还有货吗？没有的话我晚点再来。",
        ],
    ),
    (
        "可以换一下吗",
        [
            "可以换一下吗？这件尺码不合适。",
            "可以换一下吗？我把小票一起带来了。",
        ],
    ),
    (
        "能换个尺码吗",
        [
            "能换个尺码吗？这件有点小。",
            "能换个尺码吗？不方便的话我就退掉。",
        ],
    ),
    (
        "可以扫码吗",
        ["可以扫码吗？我现在付款。", "可以扫码吗？我没有带现金。"],
    ),
    (
        "付款成功了吗",
        [
            "付款成功了吗？麻烦帮我确认一下。",
            "付款成功了吗？我这边已经显示扣款了。",
        ],
    ),
    (
        "我再比较一下",
        [
            "我再比较一下价格，晚点决定。",
            "我再比较一下颜色，再选一件合适的。",
        ],
    ),
    (
        "运费怎么算",
        [
            "运费怎么算？我先确认总价。",
            "运费怎么算？合适的话就送到家。",
        ],
    ),
    (
        "一共要多少钱",
        [
            "一共要多少钱？我现在付。",
            "一共要多少钱？请把优惠也算进去。",
        ],
    ),
];

#[test]
fn shopping_preserves_every_preexisting_chinese_reading_flag_and_context() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2720);
    assert!(vocabulary.continuations().count() >= 615);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2720) {
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
    assert_eq!(reading_hash, 0xff6b58b517e008f5);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(615) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1228);
    assert_eq!(context_hash, 0x980e6c8e9f7db40e);
}

#[test]
fn shopping_adds_thirty_two_fixed_readings_twelve_contexts_and_twenty_four_sentences() {
    assert_eq!(WORDS.len(), 32);
    assert_eq!(SENTENCES.len(), 12);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    let readings = &vocabulary.readings()[2720..2752];
    let contexts = vocabulary
        .continuations()
        .skip(615)
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
fn shopping_words_offer_four_full_and_specific_partial_spellings_on_page_one() {
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
fn shopping_full_and_specific_partial_contexts_offer_both_sentences_and_words() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for fragment in [reading, &reading[..reading.len() - 1]] {
            if word == "我再比较一下" && fragment != reading {
                // The preexisting 我在 / 以西 split is deliberately preserved.
                // Its first-page price and second-page color choices are
                // independently checked below rather than displacing old words.
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
fn ambiguous_shopping_partial_preserves_old_split_and_exact_page_two_sentence_selection() {
    for seed in spellings("wo'zai'bi'jiao'yi'xi") {
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        assert_eq!(ime.candidates()[0].text, "我在比较以西");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
        for old_word in ["我在比较一下", "我在比较以谢谢", "我在比较以现在"] {
            first_page_index(&ime, old_word, CandidateKind::Word);
        }
        commit_once_and_undo(&mut engine(&seed), "我再比较一下", CandidateKind::Word);
        commit_once_and_undo(
            &mut engine(&seed),
            "我再比较一下价格，晚点决定。",
            CandidateKind::Sentence,
        );
        let second = "我再比较一下颜色，再选一件合适的。";
        let selected = ime
            .candidates()
            .iter()
            .position(|candidate| {
                candidate.text == second && candidate.kind == CandidateKind::Sentence
            })
            .unwrap();
        assert!((PAGE_SIZE..12).contains(&selected));
        commit_index_and_undo(&mut ime, selected, second, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

#[test]
fn adopted_shopping_words_preserve_horizontal_padding_and_exact_editable_draft() {
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
fn shopping_continues_after_adoption_more_pinyin_and_long_unicode_drafts() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for (tail_reading, tail_word, sentence) in [
                ("jia'ge", "价格", "我再比较一下价格，晚点决定。"),
                ("yan'se", "颜色", "我再比较一下颜色，再选一件合适的。"),
            ] {
                let word = "我再比较一下";
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
        "shopping-chinese-synthetic-error"
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
fn shopping_local_candidates_survive_typed_missing_failed_timed_out_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            (
                "hai neng pian yi yi dian ma",
                "还能便宜一点吗",
                "还能便宜一点吗？可以的话我就买这一件。",
            ),
            ("ke yi sao ma ma", "可以扫码吗", "可以扫码吗？我现在付款。"),
            (
                "yun fei zen me suan",
                "运费怎么算",
                "运费怎么算？我先确认总价。",
            ),
            (
                "fu kuan cheng gong le m",
                "付款成功了吗",
                "付款成功了吗？麻烦帮我确认一下。",
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
fn shopping_unknown_text_and_control_boundaries_preserve_literal_without_inventing_sentences() {
    for seed in [
        "zzqvjy",
        "qqzzv  note",
        "“raw”  123\t???",
        "shopping@example.invalid",
        "https://example.invalid/shopping?q=raw",
        "🙂\u{3000}zzqvjy",
        "我再比较一下??  zzqvjy",
        "我再比较一下\njia ge",
        "我再比较一下\rjia ge",
        "我再比较一下\u{2028}jia ge",
        "我再比较一下\u{2029}jia ge",
        "可以扫码吗。\tqqzzv",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for boundary in ["\n", "\r", "\u{2028}", "\u{2029}"] {
            let crossed = format!("我再比较一下{boundary}价格，晚点决定。");
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == crossed)
            );
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}
