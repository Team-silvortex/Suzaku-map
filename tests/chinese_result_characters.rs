//! Finite result/return/find readings, not semantic or tone disambiguation.
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

const RESULTS: &[(&str, &str, &str)] = &[
    ("guo", "过", "guo4"),
    ("wan", "完", "wan2"),
    ("hui", "回", "hui2"),
    ("zhao", "找", "zhao3"),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
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

fn primary(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert_eq!(index(ime, text, kind), 0);
    assert_eq!(ime.candidates()[0].score, 100.0);
}

fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn signature(ime: &XRTabletImeEngine) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for c in ime.candidates() {
        for byte in c
            .text
            .bytes()
            .chain([0])
            .chain(c.label.bytes())
            .chain([0])
            .chain(c.kind.id().bytes())
            .chain([0])
            .chain(c.source.id().bytes())
            .chain([0])
            .chain(c.score.to_bits().to_le_bytes())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[test]
fn three_missing_readings_and_one_homophone_append_without_authored_contexts() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(zh.readings().len(), 3442);
    assert_eq!(zh.continuations().count(), 959);
    assert_eq!(
        zh.continuations().map(|(_, v)| v.len()).sum::<usize>(),
        1916
    );
    for (entry, &(reading, text, _)) in zh.readings()[3170..3174].iter().zip(RESULTS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(!zh.continuations().any(|(context, _)| context == text));
    }
    let guo: Vec<_> = zh
        .readings()
        .iter()
        .filter(|r| r.reading == "guo")
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(guo, ["国", "过"]);
}

#[test]
fn shorter_prefixes_and_prior_ambiguities_keep_twelve_real_before_data_vectors() {
    // Samples include every text, label, kind, source, score bit and position.
    for (seed, expected) in [
        ("g", 0x24c152833426a4ff),
        ("gu", 0x46cdef80d081ff1e),
        ("w", 0x61eb3db9bdf754df),
        ("wa", 0x4f76cc21b89b9db9),
        ("h", 0x1dabe4aae9aae9f7),
        ("hu", 0x81d739caaaa52cf4),
        ("zh", 0x94cebaffe247bdf7),
        ("zha", 0xc78385901da61517),
        ("wo jie ni", 0x101197e5eddca6ac),
        ("shi yong", 0xeb03afc31a106dd9),
        ("haimeishu", 0x08b8a281d7806afd),
        ("huan ge", 0x999081dd912ed081),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(signature(&ime), expected, "{seed}: {:?}", ime.candidates());
    }
}

#[test]
fn bare_case_tone_and_padding_keep_guo_primary_raw_and_once_only_commit() {
    for &(reading, word, toned) in RESULTS {
        for seed in [
            reading.to_owned(),
            reading.to_ascii_uppercase(),
            toned.to_owned(),
            toned.to_ascii_uppercase(),
        ] {
            for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
                let raw = format!("{seed}{gap}");
                let ime = engine(&raw);
                draft(&ime, &raw);
                primary(
                    &ime,
                    if reading == "guo" { "国" } else { word },
                    CandidateKind::Word,
                );
                assert!(index(&ime, word, CandidateKind::Word) < PAGE_SIZE);
                commit_once(&mut engine(&raw), word, CandidateKind::Word);
                commit_once(&mut engine(&raw), &raw, CandidateKind::Literal);
            }
        }
    }
    // Tone digits normalize spelling, not the meaning or homophone priority.
    for seed in ["guo2", "GUO2", "guo4", "GUO4"] {
        primary(&engine(seed), "国", CandidateKind::Word);
        commit_once(&mut engine(seed), "过", CandidateKind::Word);
    }
}

#[test]
fn broad_decoder_four_keep_order_and_all_old_words_remain_selectable() {
    for (seed, word, core, extra) in [
        ("guo", "过", ["过滤", "过道", "过马路", "过期"], "过去式"),
        ("wan", "完", ["晚上好", "晚饭", "晚餐", "完成"], "晚上"),
        ("hui", "回", ["回复", "恢复", "会议", "回家"], "回退"),
        (
            "zhao",
            "找",
            ["照片", "照顾", "着急", "找回密码"],
            "照顾好自己",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        let positions = core.map(|old| index(&ime, old, CandidateKind::Word));
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
        for old in core.into_iter().chain([extra]) {
            commit_once(&mut engine(seed), old, CandidateKind::Word);
        }
    }
}

#[test]
fn full_words_keep_old_primaries_and_sensitive_partials_keep_existing_authored_choices() {
    for (reading, word) in [
        ("hui'fu", "回复"),
        ("hui'yi", "会议"),
        ("hui'jia", "回家"),
        ("yi'hui", "一会"),
        ("dai'hui'jian", "待会见"),
        ("wan'fan", "晚饭"),
        ("wan'kuai", "碗筷"),
        ("wan'cheng", "完成"),
        ("zhao'pian", "照片"),
        ("zhao'ji", "着急"),
        ("zhao'hui'mi'ma", "找回密码"),
        ("guo'lv", "过滤"),
        ("guo'qi", "过期"),
        ("jie'guo", "结果"),
        ("na'guo'lai", "拿过来"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            draft(&engine(&seed), &seed);
            primary(&engine(&seed), word, CandidateKind::Word);
        }
    }
    let ime = engine("hui fu");
    assert!(index(&ime, "回复", CandidateKind::Word) < index(&ime, "恢复", CandidateKind::Word));
    for (seed, word, sentences) in [
        (
            "yong wan huan gei w",
            "用完还给我",
            ["用完还给我就行，不用着急。", "用完还给我，我明天还要用。"],
        ),
        (
            "hai mei zhao da",
            "还没找到",
            [
                "还没找到，我再看看包里。",
                "还没找到，你记得最后放在哪了吗？",
            ],
        ),
    ] {
        assert!(index(&engine(seed), word, CandidateKind::Word) < PAGE_SIZE);
        for sentence in sentences {
            assert!(index(&engine(seed), sentence, CandidateKind::Sentence) < PAGE_SIZE);
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
    commit_once(
        &mut engine("wo yong wan"),
        "我用完还给你",
        CandidateKind::Word,
    );
}

#[test]
fn finite_combinations_are_sentence_conversions_not_new_fixed_phrase_words() {
    for (reading, sentence) in [
        ("yong'wan", "用完"),
        ("wo'yong'wan", "我用完"),
        ("wo'hui'qu", "我回去"),
        ("wo'zhao'shu", "我找书"),
        ("hui'lai", "回来"),
        ("hui'qu", "回去"),
        ("wo'yong'guo", "我用过"),
        ("fang'guo'qu", "放过去"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            draft(&engine(&seed), &seed);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }
    // Appending 过 does not silently disambiguate the pre-existing 国 reading.
    primary(&engine("wo yong guo"), "我用国", CandidateKind::Sentence);
    primary(&engine("fang guo qu"), "放国去", CandidateKind::Sentence);
}

#[test]
fn adopted_characters_mixed_existing_phrases_and_unicode_prefixes_remain_lossless() {
    for (reading, word, tail, converted) in [
        ("guo", "过", "lai", "来"),
        ("wan", "完", "le", "了"),
        ("hui", "回", "qu", "去"),
        ("zhao", "找", "shu", "书"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let seed = format!("{word}{gap}{tail}");
            let expected = format!("{word}{gap}{converted}");
            ime.seed(&seed);
            draft(&ime, &seed);
            commit_once(&mut engine(&seed), &expected, CandidateKind::Sentence);
        }
    }
    commit_once(&mut engine("用wan"), "用完", CandidateKind::Sentence);
    for gap in ["", " ", "\t", "\u{3000}"] {
        let seed = format!("用完{gap}huan gei ni");
        let word = format!("用完{gap}还给你");
        // Before-data sampling confirms that a literal internal gap is not a
        // whole dictionary Word. Keep its decoded Sentence and original tail
        // suggestions, not an invented spaced whole-phrase context.
        let kind = if gap.is_empty() {
            CandidateKind::Word
        } else {
            CandidateKind::Sentence
        };
        commit_once(&mut engine(&seed), &word, kind);
        let suffixes = if gap.is_empty() {
            ["，不会拿走的。", "，我就在这里用。"]
        } else {
            ["现在方便吗？", "有什么建议？"]
        };
        for suffix in suffixes {
            commit_once(
                &mut engine(&seed),
                &format!("{word}{suffix}"),
                CandidateKind::Sentence,
            );
        }
    }
    primary(&engine("拿guo lai"), "拿国来", CandidateKind::Sentence);
    commit_once(&mut engine("拿guo lai"), "拿过来", CandidateKind::Word);
    for text in [
        "我还没找到出口，可能得再问一下。",
        "我还没找到，你能说得具体一点吗？",
    ] {
        commit_once(&mut engine("我还没zhao到"), text, CandidateKind::Sentence);
    }
    commit_once(
        &mut engine("我还没zhao到"),
        "我还没找到",
        CandidateKind::Word,
    );
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(40)] {
        for &(reading, word, _) in RESULTS {
            let seed = format!("{prefix}{reading}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
            commit_once(
                &mut engine(&seed),
                &format!("{prefix}{word}"),
                CandidateKind::Word,
            );
        }
    }
}

#[test]
fn quotes_digits_wrong_splits_and_protected_or_line_boundaries_stay_literal_safe() {
    for &(reading, word, _) in RESULTS {
        for ending in ["'", "4.5", "2:30", "40"] {
            let seed = format!("{reading}{ending}");
            draft(&engine(&seed), &seed);
            commit_once(
                &mut engine(&seed),
                &format!("{word}{ending}"),
                CandidateKind::Sentence,
            );
            assert!(!engine(&seed).candidates().iter().any(|c| c.text == word));
            if reading == "guo" {
                primary(
                    &engine(&seed),
                    &format!("国{ending}"),
                    CandidateKind::Sentence,
                );
            }
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
        for boundary in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            let seed = format!("{reading}{boundary}shu");
            draft(&engine(&seed), &seed);
            assert!(
                !engine(&seed)
                    .candidates()
                    .iter()
                    .any(|c| c.text == format!("{word}书"))
            );
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
        let ime = engine(word);
        draft(&ime, word);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == word && c.kind == CandidateKind::Word)
        );
    }
    for seed in [
        "gu o",
        "wa n",
        "hu i",
        "zha o",
        "https://example.invalid/guo",
        "person.wan@example.invalid",
        "file_hui",
        "找zzqvjy",
    ] {
        draft(&engine(seed), seed);
        assert!(
            !engine(seed)
                .candidates()
                .iter()
                .any(|c| RESULTS.iter().any(|(_, w, _)| c.text == *w))
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

struct FailedProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedProvider {
    fn provider_id(&self) -> &str {
        "result-characters-synthetic-error"
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
fn unavailable_timeout_and_empty_model_responses_keep_entire_local_vectors() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for &(reading, word, _) in RESULTS {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedProvider(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            primary(
                &ime,
                if reading == "guo" { "国" } else { word },
                CandidateKind::Word,
            );
            index(&ime, word, CandidateKind::Word);
            let local = ime.candidates().to_vec();
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
            assert_eq!(ime.candidates(), local);
            commit_once(&mut ime, word, CandidateKind::Word);
        }
    }
}
