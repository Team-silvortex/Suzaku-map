//! Authored borrowing/return/storage dialogue, not a lending or storage service.
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

// Fixed before extending the 510-reading baseline; existing short contexts stay authored.
const WORDS: &[(&str, &str, &str)] = &[
    ("henkyakukigen", "へんきゃくきげん", "返却期限"),
    ("kashidashikikan", "かしだしきかん", "貸出期間"),
    ("henkyakubasho", "へんきゃくばしょ", "返却場所"),
    ("ichijihokan", "いちじほかん", "一時保管"),
    ("azukesaki", "あずけさき", "預け先"),
    ("karimono", "かりもの", "借り物"),
    ("shakuyou", "しゃくよう", "借用"),
    ("kashikari", "かしかり", "貸し借り"),
    ("kashidashi", "かしだし", "貸し出し"),
    ("henkyaku", "へんきゃく", "返却"),
    ("enchou", "えんちょう", "延長"),
    ("kashidashika-do", "かしだしかーど", "貸出カード"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "返却期限",
        [
            "返却期限を教えてもらえますか。",
            "返却期限までに持っていきます。",
        ],
    ),
    (
        "貸出期間",
        [
            "貸出期間を確認してから借ります。",
            "貸出期間はどのくらいですか。",
        ],
    ),
    (
        "返却場所",
        ["返却場所はどこですか。", "返却場所まで一緒に行きましょう。"],
    ),
    (
        "一時保管",
        [
            "一時保管をお願いできますか。",
            "一時保管している荷物を取りに来ました。",
        ],
    ),
    (
        "預け先",
        [
            "預け先を決めてから出かけます。",
            "預け先は受付で確認しました。",
        ],
    ),
    (
        "借り物",
        [
            "借り物なので大切に使います。",
            "借り物は使い終わったら返します。",
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
fn borrowing_fixed_romaji_nasal_rules_and_append_windows() {
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    for &(romaji, kana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
    }
    for (seed, kana) in [
        ("henkyakukigenn", "へんきゃくきげん"),
        ("kashidashikikann", "かしだしきかん"),
        ("ichijihokann", "いちじほかん"),
        ("ennchou", "えんちょう"),
        ("hokan", "ほかん"),
        ("hokann", "ほかん"),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 522 && ja.continuations().count() >= 262);
    for (entry, &(_, kana, word)) in ja.readings()[510..522].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(256).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(256)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        511
    );
    assert_eq!(
        ja.continuations()
            .take(262)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        523
    );
}

#[test]
fn borrowing_full_specific_partial_and_nasal_inputs_keep_word_sentence_choices() {
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
        ("henkyakukige", "返却期限"),
        ("kashidashikika", "貸出期間"),
        ("henkyakubash", "返却場所"),
        ("ichijihoka", "一時保管"),
        ("azukesak", "預け先"),
        ("karimon", "借り物"),
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
        ("henkyakukigenn", "返却期限"),
        ("kashidashikikann", "貸出期間"),
        ("ichijihokann", "一時保管"),
        ("ennchou", "延長"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
    }
}

#[test]
fn borrowing_horizontal_spacing_adopted_particles_and_reseed_never_commit_early() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("henkyaku", "KIGEN", "返却期限"),
            ("kashidashi", "KIKAN", "貸出期間"),
            ("henkyaku", "BASHO", "返却場所"),
            ("ichiji", "HOKAN", "一時保管"),
            ("azuke", "SAKI", "預け先"),
            ("kari", "MONO", "借り物"),
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
        ("返却期限", "made", SENTENCES[0].1[1]),
        ("貸出期間", "wo", SENTENCES[1].1[0]),
        ("返却場所", "は", SENTENCES[2].1[0]),
        ("一時保管", "WO", SENTENCES[3].1[0]),
        ("預け先", "ha", SENTENCES[4].1[1]),
        ("借り物", "なので", SENTENCES[5].1[0]),
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
fn borrowing_keeps_old_lending_return_and_storage_neighbours_on_page_one() {
    for (reading, partial, word, sentences) in [
        (
            "kashite",
            "kashit",
            "貸して",
            [
                "貸してもらってもいいですか。",
                "貸してくれてありがとうございます。",
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
            "juudenki",
            "juudenk",
            "充電器",
            [
                "充電器を少し借りてもいいですか。",
                "充電器は鞄に入っています。",
            ],
        ),
        (
            "henpin",
            "henpi",
            "返品",
            ["返品できますか。", "返品の期限を教えてください。"],
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
        (
            "hokanbasho",
            "hokanbash",
            "保管場所",
            ["保管場所を確認してきます。", "保管場所は受付の奥です。"],
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
    for (seed, word) in [
        ("hokan", "保管"),
        ("azukarishou", "預かり証"),
        ("uketoribasho", "受け取り場所"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, word);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
    }
}

#[test]
fn borrowing_unknown_urls_and_line_boundaries_keep_raw_text_without_authored_leaks() {
    for seed in [
        "https://azukesaki.example.invalid",
        "karimono@example.invalid",
        "xyzhenkyakukigen",
        "貸出期間dexyz",
        "🙂預け先wo",
        "ichiji\nhokan",
        "返却期限\r\nwo",
        "返却場所\u{2028}ha",
        "借り物\u{2029}nanode",
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
        "ja-borrowing-synthetic-model-error"
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
fn borrowing_model_unavailable_timeout_and_empty_preserve_six_local_contexts() {
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
