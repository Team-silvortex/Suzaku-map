//! Fixed journey updates and meeting-position clarification, not live routing.
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
    ("wo'gang'shang'che", "我刚上车"),
    ("wo'hai'you'yi'zhan", "我还有一站"),
    ("wo'hai'you'ji'fen'zhong'jiu'dao", "我还有几分钟就到"),
    ("ni'xian'zai'yuan'di'deng'wo", "你先在原地等我"),
    ("wo'zai'ling'yi'ge'chu'kou", "我在另一个出口"),
    ("wo'zai'ma'lu'dui'mian", "我在马路对面"),
    ("qian'mian'feng'lu'le", "前面封路了"),
    ("wo'huan'tiao'lu'guo'qu", "我换条路过去"),
    ("hui'he", "会合"),
    ("peng'tou", "碰头"),
    ("yuan'di", "原地"),
    ("ling'yi'ce", "另一侧"),
    ("jiao'cha'kou", "交叉口"),
    ("feng'lu", "封路"),
    ("gai'dao", "改道"),
    ("xing'ren'tong'dao", "行人通道"),
    ("zhan'pai", "站牌"),
    ("chu'zhan'kou", "出站口"),
    ("ji'he'dian", "集合点"),
    ("mu'di'di", "目的地"),
    ("lu'biao", "路标"),
    ("lu'kuang", "路况"),
    ("lin'shi'ru'kou", "临时入口"),
    ("lin'shi'chu'kou", "临时出口"),
];
// Only these final-letter omissions, not arbitrary shorter prefixes, are fixed.
const PARTIALS: &[(&str, &str)] = &[
    ("wo'gang'shang'ch", "我刚上车"),
    ("wo'hai'you'yi'zha", "我还有一站"),
    ("wo'hai'you'ji'fen'zhong'jiu'da", "我还有几分钟就到"),
    ("ni'xian'zai'yuan'di'deng'w", "你先在原地等我"),
    ("wo'zai'ling'yi'ge'chu'ko", "我在另一个出口"),
    ("wo'zai'ma'lu'dui'mia", "我在马路对面"),
    ("qian'mian'feng'lu'l", "前面封路了"),
    ("wo'huan'tiao'lu'guo'q", "我换条路过去"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "我刚上车",
        [
            "我刚上车，等坐稳了再回你消息。",
            "我刚上车，下车前再跟你联系。",
        ],
    ),
    (
        "我还有一站",
        [
            "我还有一站，下车后就往你那边走。",
            "我还有一站，你先不用出来等。",
        ],
    ),
    (
        "我还有几分钟就到",
        [
            "我还有几分钟就到，你先找个方便等的位置。",
            "我还有几分钟就到，到了我会再发消息。",
        ],
    ),
    (
        "你先在原地等我",
        [
            "你先在原地等我，我过去找你。",
            "你先在原地等我，别来回找了。",
        ],
    ),
    (
        "我在另一个出口",
        [
            "我在另一个出口，先把旁边的标志发给你。",
            "我在另一个出口，我们确认一下再碰头。",
        ],
    ),
    (
        "我在马路对面",
        [
            "我在马路对面，你能看到那块招牌吗？",
            "我在马路对面，等我们确认好位置再走。",
        ],
    ),
    (
        "前面封路了",
        [
            "前面封路了，我先看看有没有别的通道。",
            "前面封路了，我可能要稍微晚一点。",
        ],
    ),
    (
        "我换条路过去",
        [
            "我换条路过去，到了再跟你说。",
            "我换条路过去，可能会比刚才估计的慢一点。",
        ],
    ),
];

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.into(),
        reading.replace('\'', " "),
        reading.replace('\'', ""),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let at = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} {text:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    if kind == CandidateKind::Word {
        assert!(at < PAGE_SIZE, "Word left page one: {text:?}");
    }
    at
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
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
    index(ime, seed, CandidateKind::Literal);
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    if !seed.is_empty() {
        draft(&ime, seed);
    }
    ime
}
fn commit(seed: &str, text: &str, kind: CandidateKind) {
    commit_from(&mut engine(seed), text, kind);
}
fn commit_from(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let seed = ime.snapshot().seed_text;
    draft(ime, &seed);
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let repeated = ime.commit(CommitOptions { force: true });
    assert!(!repeated.ok && repeated.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn route_uses_fixed_3244_reading_and_789_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (24, 8, 8));
    assert!(vocabulary.readings().len() >= 3268);
    for (entry, &(reading, text)) in vocabulary.readings()[3244..3268].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts: Vec<_> = vocabulary.continuations().skip(789).take(8).collect();
    assert_eq!(contexts.len(), 8);
    assert_eq!(
        contexts
            .iter()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        16
    );
    for ((context, values), &(word, sentences)) in contexts.into_iter().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            sentences
        );
    }
}

#[test]
fn route_full_and_specific_partial_spellings_keep_words_authors_and_raw_selectable() {
    for &(reading, word) in WORDS.iter().chain(PARTIALS) {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    commit(&seed, sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn route_horizontal_spacing_and_unicode_prefixes_preserve_exact_text() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
            let raw = format!("{}{gap}", reading.replace('\'', gap));
            commit(&raw, word, CandidateKind::Word);
            commit(&raw, &raw, CandidateKind::Literal);
            let adopted = format!("{word}{gap}");
            for sentence in sentences {
                commit(
                    &adopted,
                    &format!("{adopted}{}", sentence.strip_prefix(word).unwrap()),
                    CandidateKind::Sentence,
                );
            }
            commit(&adopted, &adopted, CandidateKind::Literal);
        }
    }
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(40)] {
        for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
            let raw = format!("{prefix}{}", reading.replace('\'', " "));
            assert!(
                engine(&raw)
                    .candidates()
                    .iter()
                    .all(|c| c.text.starts_with(&prefix))
            );
            commit(&raw, &format!("{prefix}{word}"), CandidateKind::Word);
            for sentence in sentences {
                commit(
                    &raw,
                    &format!("{prefix}{sentence}"),
                    CandidateKind::Sentence,
                );
            }
            commit(&raw, &raw, CandidateKind::Literal);
        }
    }
}

#[test]
fn route_adoption_and_known_tail_progress_remain_editable_until_explicit_commit() {
    // 下车 and 等 have no independent mappings here: their written Han progress
    // is tested, without inventing a promise that xia che / deng converts to it.
    for (context, han, reading, sentence) in [
        (0, "等", None, SENTENCES[0].1[0]),
        (1, "下车", None, SENTENCES[1].1[0]),
        (2, "你", Some("ni"), SENTENCES[2].1[0]),
        (3, "我", Some("wo"), SENTENCES[3].1[0]),
        (4, "先", Some("xian"), SENTENCES[4].1[0]),
        (5, "你", Some("ni"), SENTENCES[5].1[0]),
        (6, "我", Some("wo"), SENTENCES[6].1[0]),
        (7, "到了", Some("dao'le"), SENTENCES[7].1[0]),
    ] {
        let (full, word) = WORDS[context];
        assert!(sentence.starts_with(&format!("{word}，{han}")));
        let mut ime = engine(full);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Host-style selection/reseed only, not physical IBus key acceptance.
        for gap in ["", " ", "\t", "\u{3000}"] {
            for tail in
                std::iter::once(han.to_owned()).chain(reading.into_iter().flat_map(spellings))
            {
                let raw = format!("{word}{gap}，{tail}");
                let expected = format!("{word}{gap}{}", sentence.strip_prefix(word).unwrap());
                ime.seed(&raw);
                draft(&ime, &raw);
                index(&ime, &expected, CandidateKind::Sentence);
                ime.seed(full);
                draft(&ime, full);
                index(&ime, word, CandidateKind::Word);
                commit(&raw, &expected, CandidateKind::Sentence);
                commit(&raw, &raw, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn route_unknown_protected_tokens_wrong_syllables_and_lines_do_not_borrow_authors() {
    for raw in [
        "zzqvjy",
        "route@example.invalid",
        "https://example.invalid/wogangshangche",
        "file_wogangshangche",
        "“raw” 123\t???",
        "wo ga ng shang che",
        "yu an di",
    ] {
        let ime = engine(raw);
        assert!(!ime.candidates().iter().any(|c| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.iter().any(|s| c.text.contains(s)))
        }));
        if raw == "yu an di" {
            assert!(!ime.candidates().iter().any(|c| c.text == "原地"));
        }
        commit(raw, raw, CandidateKind::Literal);
    }
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("wo gang{line}shang che");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()))
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("我刚上车{line}，等");
        let crossed = format!("我刚上车{line}，等坐稳了再回你消息。");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == crossed || c.text == SENTENCES[0].1[0])
        );
        commit(&raw, &raw, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "route-chinese-synthetic-fallback"
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
fn route_unavailable_timeout_and_empty_models_preserve_the_entire_local_pool() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (&(seed, word), &(_, sentences)) in PARTIALS.iter().zip(SENTENCES) {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(seed);
            draft(&ime, seed);
            index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                index(&ime, sentence, CandidateKind::Sentence);
            }
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
            draft(&ime, seed);
            commit_from(&mut ime, sentences[0], CandidateKind::Sentence);
        }
    }
}
