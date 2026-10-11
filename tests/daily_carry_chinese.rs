//! Authored clothing and carry-before-leaving fallback expectations.
//! Fixed scenarios precede data, with all current historical content/order frozen.
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
    ("chu'men'dai'qi'dong'xi", "出门带齐东西"),
    ("wo'xian'huan'shuang'xie", "我先换双鞋"),
    ("ba'la'lian'la'shang", "把拉链拉上"),
    ("kou'dai'jian'cha'yi'xia", "口袋检查一下"),
    ("shui'bei'dai'shang'le", "水杯带上了"),
    ("mao'zi'fang'bao'li", "帽子放包里"),
    ("xie'dai'xi'hao'le", "鞋带系好了"),
    ("wei'jin'bie'wang'le", "围巾别忘了"),
    ("wa'zi", "袜子"),
    ("chen'shan", "衬衫"),
    ("duan'xiu", "短袖"),
    ("chang'xiu", "长袖"),
    ("bao'wai'tao", "薄外套"),
    ("hou'wai'tao", "厚外套"),
    ("yun'dong'xie", "运动鞋"),
    ("tuo'xie", "拖鞋"),
    ("xie'dai", "鞋带"),
    ("yi'ling", "衣领"),
    ("niu'kou", "纽扣"),
    ("la'lian", "拉链"),
    ("kou'dai", "口袋"),
    ("wei'jin", "围巾"),
    ("mao'zi", "帽子"),
    ("sui'shen'wu'pin", "随身物品"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "出门带齐东西",
        [
            "出门带齐东西，钥匙和手机再看一眼。",
            "出门带齐东西，免得走到一半又回来。",
        ],
    ),
    (
        "我先换双鞋",
        [
            "我先换双鞋再出门，你在门口等我一下。",
            "我先换双鞋再出门，走路会舒服一点。",
        ],
    ),
    (
        "把拉链拉上",
        [
            "把拉链拉上再出门，外面风有点大。",
            "把拉链拉上，别把衣角夹住了。",
        ],
    ),
    (
        "口袋检查一下",
        [
            "口袋检查一下，看看钥匙在不在。",
            "口袋检查一下，别把东西落家里。",
        ],
    ),
    (
        "水杯带上了",
        [
            "水杯带上了，路上就不用再买水。",
            "水杯带上了，放在包侧面就好。",
        ],
    ),
    (
        "帽子放包里",
        [
            "帽子放包里，到了外面再戴。",
            "帽子放包里，免得走的时候忘拿。",
        ],
    ),
    (
        "鞋带系好了",
        [
            "鞋带系好了，现在可以出门了。",
            "鞋带系好了，我们慢慢走过去。",
        ],
    ),
    (
        "围巾别忘了",
        [
            "围巾别忘了，晚上回来可能有点冷。",
            "围巾别忘了，我帮你放在门口。",
        ],
    ),
];

#[test]
fn carry_preserves_every_current_reading_flag_context_sentence_and_position() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2872);
    assert!(vocabulary.continuations().count() >= 671);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2872) {
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
    assert_eq!(reading_hash, 0x8b14113ba2166398);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut historical_sentences = 0;
    for (context, values) in vocabulary.continuations().take(671) {
        historical_sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(historical_sentences, 1340);
    assert_eq!(context_hash, 0x1e3a5ee0ef76014b);
}

#[test]
fn carry_adds_fixed_twenty_four_readings_eight_contexts_and_sixteen_sentences() {
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 8);
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2896);
    let readings = &vocabulary.readings()[2872..2896];
    let contexts = vocabulary
        .continuations()
        .skip(671)
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
    assert!(
        (PAGE_SIZE..2 * PAGE_SIZE).contains(&index),
        "expected page two {text:?}: {:?}",
        ime.candidates()
    );
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
fn carry_words_are_selectable_in_five_full_and_specific_partial_spellings() {
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
fn carry_full_and_specific_partial_contexts_offer_words_and_both_fixed_sentences() {
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for fragment in [reading, &reading[..reading.len() - 1]] {
            for seed in spellings(fragment) {
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                first_page_index(&ime, word, CandidateKind::Word);
                for (branch, sentence) in sentences.into_iter().enumerate() {
                    if word == "口袋检查一下" && fragment != reading && branch == 1 {
                        // xi also expands the old 以 + 谢谢/现在/喜欢 branch.
                        // Keep that priority; this sentence remains on page two.
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
fn adopted_carry_words_keep_six_horizontal_paddings_editable_and_uncommitted() {
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
fn carry_pinyin_and_long_unicode_progress_never_rewrite_adopted_frozen_prefixes() {
    let word = "我先换双鞋";
    let tail_reading = "zai'chu'men";
    let tail_word = "再出门";
    let partial = &tail_reading[..tail_reading.len() - 1];
    let sentences = [
        "我先换双鞋再出门，你在门口等我一下。",
        "我先换双鞋再出门，走路会舒服一点。",
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
                    if spellings(tail_reading).contains(&tail) && branch == 1 {
                        // 在/再 and 出门-prefix words retain their earlier slots.
                        // Complete men is broader than the partial me spelling.
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
fn carry_ambiguous_tails_keep_prior_words_and_split_sentence_pages() {
    for seed in spellings("kou'dai'jian'cha'yi'xi") {
        let ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        for word in ["口袋检查一下", "口袋检查以谢谢", "口袋检查以现在"] {
            first_page_index(&ime, word, CandidateKind::Word);
        }
        commit_once_and_undo(
            &mut engine(&seed),
            "口袋检查一下，看看钥匙在不在。",
            CandidateKind::Sentence,
        );
        commit_second_page_once_and_undo(&mut engine(&seed), "口袋检查一下，别把东西落家里。");
    }
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for tail in spellings("zai'chu'men") {
            let seed = format!("{prefix}我先换双鞋{tail}");
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            first_page_index(
                &ime,
                &format!("{prefix}我先换双鞋在出门之前"),
                CandidateKind::Word,
            );
            commit_once_and_undo(
                &mut engine(&seed),
                &format!("{prefix}我先换双鞋再出门，你在门口等我一下。"),
                CandidateKind::Sentence,
            );
            commit_second_page_once_and_undo(
                &mut engine(&seed),
                &format!("{prefix}我先换双鞋再出门，走路会舒服一点。"),
            );
        }
    }
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "carry-chinese-synthetic-error"
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
fn carry_local_choices_survive_typed_missing_failure_timeout_and_empty_model_paths() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            (
                "wo xian huan shuang xie",
                "我先换双鞋",
                "我先换双鞋再出门，你在门口等我一下。",
            ),
            (
                "ba la lian la shan",
                "把拉链拉上",
                "把拉链拉上再出门，外面风有点大。",
            ),
            (
                "wei jin bie wang le",
                "围巾别忘了",
                "围巾别忘了，晚上回来可能有点冷。",
            ),
            (
                "kou dai jian cha yi xia",
                "口袋检查一下",
                "口袋检查一下，看看钥匙在不在。",
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
fn carry_keeps_control_boundaries_unknown_literals_and_valid_syllable_rules() {
    for boundary in [
        "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
    ] {
        let word = "我先换双鞋";
        let seed = format!("{word}{boundary}zai chu men");
        for suffix in ["再出门，你在门口等我一下。", "再出门，走路会舒服一点。"]
        {
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
        "carry@example.invalid",
        "https://example.invalid/carry?q=raw",
        "🙂\u{3000}zzqvjy",
        "我先换双鞋?? zzqvjy",
        "ba la li an la shang",
        "wo xian hu an shuang xie",
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
fn carry_does_not_evict_short_preexisting_location_and_demonstrative_words() {
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
