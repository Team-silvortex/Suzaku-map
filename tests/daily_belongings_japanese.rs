//! Authored belongings/retrieval dialogue, not a lost-property reporting service.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::{
        japanese::romaji_to_hiragana,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{EntryKind, builtin},
};

// Fixed against the 498-reading baseline before adding any belongings entries.
// Existing 忘れ物/落とし物 keep their own authored contexts and dictionary order.
const WORDS: &[(&str, &str, &str)] = &[
    ("mochinushi", "もちぬし", "持ち主"),
    ("nafuda", "なふだ", "名札"),
    ("tokuchou", "とくちょう", "特徴"),
    ("hokanbasho", "ほかんばしょ", "保管場所"),
    ("azukarishou", "あずかりしょう", "預かり証"),
    ("uketoribasho", "うけとりばしょ", "受け取り場所"),
    ("wasuremonosenta-", "わすれものせんたー", "忘れ物センター"),
    ("otoshimonomadoguchi", "おとしものまどぐち", "落とし物窓口"),
    ("funshitsutodoke", "ふんしつとどけ", "紛失届"),
    ("namaeiri", "なまえいり", "名前入り"),
    ("ki-ke-su", "きーけーす", "キーケース"),
    ("pasuke-su", "ぱすけーす", "パスケース"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "持ち主",
        ["持ち主を探しています。", "持ち主が見つかってよかったです。"],
    ),
    (
        "名札",
        [
            "名札に名前が書いてあります。",
            "名札は鞄の内側に付いています。",
        ],
    ),
    (
        "特徴",
        [
            "特徴をもう少し教えてください。",
            "特徴が分かる写真を送ります。",
        ],
    ),
    (
        "保管場所",
        ["保管場所を確認してきます。", "保管場所は受付の奥です。"],
    ),
    (
        "預かり証",
        [
            "預かり証を持ってきました。",
            "預かり証を見せればいいですか。",
        ],
    ),
    (
        "受け取り場所",
        [
            "受け取り場所を教えてください。",
            "受け取り場所は入口のそばです。",
        ],
    ),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
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
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    index(ime, seed, CandidateKind::Literal);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local && (0.0..=100.0).contains(&c.score))
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn first_page(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert!(
        index(ime, text, kind) < PAGE_SIZE,
        "{text:?}: {:?}",
        ime.candidates()
    );
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
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn belongings_fixed_romaji_nasal_rules_and_append_windows() {
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    for &(romaji, kana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
    }
    for (seed, kana) in [
        ("funshitsutodoke", "ふんしつとどけ"),
        ("funnshitsutodoke", "ふんしつとどけ"),
        ("hokannbasho", "ほかんばしょ"),
        ("wasuremonosennta-", "わすれものせんたー"),
        ("hokan", "ほかん"),
        ("hokann", "ほかん"),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 510 && ja.continuations().count() >= 256);
    for (entry, &(_, kana, word)) in ja.readings()[498..510].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(250).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(250)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        499
    );
    assert_eq!(
        ja.continuations()
            .take(256)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        511
    );
}

#[test]
fn belongings_complete_and_specific_partial_inputs_offer_first_page_words_and_sentences() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, word, CandidateKind::Word);
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
    for (seed, word) in [
        ("mochinush", "持ち主"),
        ("nafud", "名札"),
        ("tokucho", "特徴"),
        ("hokanbash", "保管場所"),
        ("azukarisho", "預かり証"),
        ("uketoribash", "受け取り場所"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        for sentence in SENTENCES
            .iter()
            .find(|(context, _)| *context == word)
            .unwrap()
            .1
        {
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for (seed, word) in [
        ("funnshitsutodoke", "紛失届"),
        ("hokannbasho", "保管場所"),
        ("wasuremonosennta-", "忘れ物センター"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn belongings_horizontal_spacing_adopted_particles_and_reseed_preserve_editable_drafts() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("mochi", "NUSHI", "持ち主"),
            ("na", "FUDA", "名札"),
            ("toku", "CHOU", "特徴"),
            ("hokan", "BASHO", "保管場所"),
            ("azukari", "SHOU", "預かり証"),
            ("uketori", "BASHO", "受け取り場所"),
        ] {
            let seed = format!("{separator}{left}{separator}{right}{separator}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, word, CandidateKind::Word);
            for sentence in SENTENCES
                .iter()
                .find(|(context, _)| *context == word)
                .unwrap()
                .1
            {
                first_page(&ime, sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
    for (word, particle, sentence) in [
        ("持ち主", "ga", SENTENCES[0].1[1]),
        ("名札", "ni", SENTENCES[1].1[0]),
        ("特徴", "WO", SENTENCES[2].1[0]),
        ("保管場所", "は", SENTENCES[3].1[1]),
        ("預かり証", "wo", SENTENCES[4].1[0]),
        ("受け取り場所", "を", SENTENCES[5].1[0]),
    ] {
        let reading = WORDS.iter().find(|(_, _, text)| *text == word).unwrap().0;
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        for seed in [
            word.to_owned(),
            format!("{word}{particle}"),
            format!("{word} {particle}"),
            format!("{word}\u{3000}{particle}"),
        ] {
            ime.seed(&seed);
            draft(&ime, &seed);
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
        }
    }
}

#[test]
fn belongings_keep_adjacent_old_lost_property_keys_hat_and_receipt_choices_on_page_one() {
    for (reading, partial, word, sentences) in [
        (
            "wasuremono",
            "wasuremon",
            "忘れ物",
            ["忘れ物がないか確認しましょう。", "忘れ物を取りに戻ります。"],
        ),
        (
            "otoshimono",
            "otoshimon",
            "落とし物",
            [
                "落とし物を見つけたので、受付に届けます。",
                "落とし物の問い合わせはどこですか。",
            ],
        ),
        (
            "kagi",
            "kag",
            "鍵",
            [
                "鍵を少し貸してもらえますか。",
                "鍵が見つからないので、一緒に探してください。",
            ],
        ),
        (
            "boushi",
            "boush",
            "帽子",
            [
                "帽子を忘れずに持っていきます。",
                "帽子が見つからないので、少し待ってください。",
            ],
        ),
        (
            "uketori",
            "uketor",
            "受け取り",
            [
                "受け取りました、ありがとうございます。",
                "受け取りの確認をお願いします。",
            ],
        ),
    ] {
        for seed in [reading, partial] {
            let ime = engine(seed);
            draft(&ime, seed);
            first_page(&ime, word, CandidateKind::Word);
            commit_once(&mut engine(seed), word, CandidateKind::Word);
            for sentence in sentences {
                first_page(&ime, sentence, CandidateKind::Sentence);
                commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(seed), seed, CandidateKind::Literal);
        }
    }
    for (seed, word) in [("hokan", "保管"), ("namae", "名前"), ("kagitaba", "鍵束")] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, word);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
    }
}

#[test]
fn belongings_unknown_urls_and_line_boundaries_preserve_raw_text_without_authored_leaks() {
    for seed in [
        "https://nafuda.example.invalid",
        "mochinushi@example.invalid",
        "xyztokuchou",
        "保管場所dexyz",
        "🙂預かり証wo",
        "mochi\nnushi",
        "名札\r\nni",
        "特徴\u{2028}ga",
        "受け取り場所\u{2029}wo",
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert!(ime.candidates().iter().all(|c| {
            !SENTENCES
                .iter()
                .flat_map(|(_, ss)| ss)
                .any(|sentence| c.text.ends_with(sentence))
        }));
        commit_once(&mut ime, seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-belongings-synthetic-model-error"
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
fn belongings_model_unavailable_timeout_and_empty_keep_all_six_local_contexts() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for &(word, sentences) in SENTENCES {
            let reading = WORDS.iter().find(|(_, _, text)| *text == word).unwrap().0;
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page(&ime, sentence, CandidateKind::Sentence);
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
            commit_once(&mut ime, sentences[0], CandidateKind::Sentence);
        }
    }
}
