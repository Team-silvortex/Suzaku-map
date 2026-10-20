//! Fixed household storage and moving-item wording, not physical controls.
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
    ("xian'teng'dian'di'fang'chu'lai", "先腾点地方出来"),
    ("zhe'ge'xiang'zi'zhuang'bu'xia'le", "这个箱子装不下了"),
    ("chang'yong'de'fang'shou'bian", "常用的放手边"),
    ("zhe'xie'xian'fen'kai'fang", "这些先分开放"),
    ("bang'wo'nuo'dao'pang'bian", "帮我挪到旁边"),
    ("shou'hao'yi'hou'tie'ge'biao'qian", "收好以后贴个标签"),
    ("shou'na'dai", "收纳袋"),
    ("shou'na'xiang", "收纳箱"),
    ("kong'xiang'zi", "空箱子"),
    ("zhi'wu'jia", "置物架"),
    ("ge'ban", "隔板"),
    ("fang'chen'dai", "防尘袋"),
    ("fen'zhuang", "分装"),
    ("die'fang", "叠放"),
    ("yuan'wei", "原位"),
    ("gui'wei", "归位"),
    ("biao'qian'zhi", "标签纸"),
    ("gua'gou", "挂钩"),
];
// Only these final-letter omissions, not arbitrary shorter prefixes, are fixed.
const PARTIALS: &[(&str, &str)] = &[
    ("xian'teng'dian'di'fang'chu'la", "先腾点地方出来"),
    ("zhe'ge'xiang'zi'zhuang'bu'xia'l", "这个箱子装不下了"),
    ("chang'yong'de'fang'shou'bia", "常用的放手边"),
    ("zhe'xie'xian'fen'kai'fan", "这些先分开放"),
    ("bang'wo'nuo'dao'pang'bia", "帮我挪到旁边"),
    ("shou'hao'yi'hou'tie'ge'biao'qia", "收好以后贴个标签"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "先腾点地方出来",
        [
            "先腾点地方出来，等会儿这个箱子也要放进来。",
            "先腾点地方出来，我把桌上的东西挪开。",
        ],
    ),
    (
        "这个箱子装不下了",
        [
            "这个箱子装不下了，剩下的再找个袋子装。",
            "这个箱子装不下了，先别急着盖上。",
        ],
    ),
    (
        "常用的放手边",
        [
            "常用的放手边，拿起来会方便一些。",
            "常用的放手边，暂时不用的再收起来。",
        ],
    ),
    (
        "这些先分开放",
        [
            "这些先分开放，等整理好了再放进柜子。",
            "这些先分开放，免得下次又找不到。",
        ],
    ),
    (
        "帮我挪到旁边",
        [
            "帮我挪到旁边，我想先清理一下桌面。",
            "帮我挪到旁边，别挡住抽屉。",
        ],
    ),
    (
        "收好以后贴个标签",
        [
            "收好以后贴个标签，写清楚里面装的是什么。",
            "收好以后贴个标签，下次就不用每个盒子都打开了。",
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
fn storage_uses_fixed_3286_reading_and_803_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (18, 6, 6));
    assert!(vocabulary.readings().len() >= 3304);
    for (entry, &(reading, text)) in vocabulary.readings()[3286..3304].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts: Vec<_> = vocabulary.continuations().skip(803).take(6).collect();
    assert_eq!(contexts.len(), 6);
    assert_eq!(
        contexts
            .iter()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        12
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
fn storage_full_and_specific_partial_spellings_keep_words_authors_and_raw_selectable() {
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
fn storage_horizontal_spacing_and_unicode_prefixes_preserve_exact_text() {
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
fn storage_adoption_and_known_tail_progress_remain_editable_until_explicit_commit() {
    // 等 has no independent mapping: test written Han progress, not an invented
    // promise that deng converts to it. Other tails use existing mappings.
    for (context, han, reading, sentence) in [
        (0, "我", Some("wo"), SENTENCES[0].1[1]),
        (1, "先", Some("xian"), SENTENCES[1].1[1]),
        (2, "拿", Some("na"), SENTENCES[2].1[0]),
        (3, "等", None, SENTENCES[3].1[0]),
        (4, "我", Some("wo"), SENTENCES[4].1[0]),
        (5, "下次", Some("xia'ci"), SENTENCES[5].1[1]),
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
fn storage_unknown_protected_tokens_wrong_syllables_and_lines_do_not_borrow_authors() {
    for raw in [
        "zzqvjy",
        "storage@example.invalid",
        "https://example.invalid/xiantengdiandifangchulai",
        "file_xiantengdiandifangchulai",
        "“raw” 123\t???",
        "xian teng dian di fa ng chu lai",
    ] {
        let ime = engine(raw);
        assert!(!ime.candidates().iter().any(|c| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.iter().any(|s| c.text.contains(s)))
        }));
        commit(raw, raw, CandidateKind::Literal);
    }
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("xian teng dian di fang{line}chu lai");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()) })
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("先腾点地方出来{line}，我");
        let crossed = format!("先腾点地方出来{line}，我把桌上的东西挪开。");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == crossed || c.text == SENTENCES[0].1[1] })
        );
        commit(&raw, &raw, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "storage-chinese-synthetic-fallback"
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
fn storage_unavailable_timeout_and_empty_models_preserve_the_entire_local_pool() {
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
