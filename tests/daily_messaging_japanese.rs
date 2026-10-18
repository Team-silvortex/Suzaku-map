//! Offline daily messaging dialogue, not a network or message-delivery service.
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

// Frozen before adding messaging data to the 522-reading / 262-context baseline.
// Existing 返信/予定/連絡/通知 keep their original rows and authored sentences.
const WORDS: &[(&str, &str, &str)] = &[
    ("messe-ji", "めっせーじ", "メッセージ"),
    ("miotoshi", "みおとし", "見落とし"),
    (
        "atodehenshinshimasu",
        "あとでへんしんします",
        "後で返信します",
    ),
    ("yoteinokakunin", "よていのかくにん", "予定の確認"),
    ("shuugoujikan", "しゅうごうじかん", "集合時間"),
    ("henshinfuyou", "へんしんふよう", "返信不要"),
    ("tsuuchisettei", "つうちせってい", "通知設定"),
    ("renrakumore", "れんらくもれ", "連絡漏れ"),
    ("henshinmachi", "へんしんまち", "返信待ち"),
    ("yoteihenkou", "よていへんこう", "予定変更"),
    ("touchakuyotei", "とうちゃくよてい", "到着予定"),
    ("youhenshin", "ようへんしん", "要返信"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "メッセージ",
        [
            "メッセージを見落としていました。",
            "メッセージを確認してから返信します。",
        ],
    ),
    (
        "見落とし",
        [
            "見落としがあったので、もう一度確認します。",
            "見落としに気づきました。教えてくれてありがとうございます。",
        ],
    ),
    (
        "後で返信します",
        [
            "後で返信しますので、少しお待ちください。",
            "後で返信します。今は手が離せません。",
        ],
    ),
    (
        "予定の確認",
        [
            "予定の確認をお願いします。",
            "予定の確認ができたら連絡します。",
        ],
    ),
    (
        "集合時間",
        [
            "集合時間をもう一度教えてください。",
            "集合時間に間に合いそうです。",
        ],
    ),
    (
        "返信不要",
        [
            "返信不要です。お知らせだけ送っておきます。",
            "返信不要ですので、時間のあるときに確認してください。",
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
fn messaging_fixed_romaji_nasal_rules_and_append_windows() {
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    for &(romaji, kana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
    }
    for (seed, kana) in [
        ("yoteinokakuninn", "よていのかくにん"),
        ("shuugoujikann", "しゅうごうじかん"),
        ("youhenshinn", "ようへんしん"),
        ("henshinnmachi", "へんしんまち"),
        ("henshin", "へんしん"),
        ("henshinn", "へんしん"),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    assert_ne!(romaji_to_hiragana("messeji"), WORDS[0].1);
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 534 && ja.continuations().count() >= 268);
    for (entry, &(_, kana, word)) in ja.readings()[522..534].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(262).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(262)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        523
    );
    assert_eq!(
        ja.continuations()
            .take(268)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        535
    );
}

#[test]
fn messaging_complete_written_and_specific_partial_inputs_keep_explicit_commit_choices() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
            word.to_owned(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            // The exact written spelling is the single deduplicated raw choice.
            let kind = if seed == word {
                CandidateKind::Literal
            } else {
                CandidateKind::Word
            };
            first_page(&ime, word, kind);
            commit_once(&mut engine(&seed), word, kind);
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
        ("messe-j", "メッセージ"),
        ("miotosh", "見落とし"),
        ("atodehenshinshimas", "後で返信します"),
        ("yoteinokakuni", "予定の確認"),
        ("shuugoujika", "集合時間"),
        ("henshinfuyo", "返信不要"),
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
        ("yoteinokakuninn", "予定の確認"),
        ("shuugoujikann", "集合時間"),
        ("youhenshinn", "要返信"),
        ("henshinnmachi", "返信待ち"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn messaging_horizontal_spacing_adopted_particles_and_reseed_keep_drafts_editable() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("messe-", "JI", "メッセージ"),
            ("mio", "TOSHI", "見落とし"),
            ("atode", "HENSHINSHIMASU", "後で返信します"),
            ("yoteino", "KAKUNIN", "予定の確認"),
            ("shuugou", "JIKAN", "集合時間"),
            ("henshin", "FUYOU", "返信不要"),
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
        ("メッセージ", "wo", SENTENCES[0].1[0]),
        ("見落とし", "ga", SENTENCES[1].1[0]),
        ("後で返信します", "ので", SENTENCES[2].1[0]),
        ("予定の確認", "WO", SENTENCES[3].1[0]),
        ("集合時間", "ni", SENTENCES[4].1[1]),
        ("返信不要", "です", SENTENCES[5].1[0]),
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
fn messaging_keeps_old_reply_schedule_contact_and_notification_authors_on_page_one() {
    for (reading, kana, word, sentences) in [
        (
            "henshin",
            "へんしん",
            "返信",
            ["返信ありがとうございます。", "返信は後で大丈夫です。"],
        ),
        (
            "yotei",
            "よてい",
            "予定",
            [
                "予定が決まったら連絡します。",
                "予定を確認してから返事します。",
            ],
        ),
        (
            "renraku",
            "れんらく",
            "連絡",
            ["連絡ありがとうございます。", "連絡が遅くなってすみません。"],
        ),
        (
            "tsuuchi",
            "つうち",
            "通知",
            [
                "通知が来たら教えてください。",
                "通知を確認してから返信します。",
            ],
        ),
        (
            "atode",
            "あとで",
            "後で",
            ["後で連絡します。", "後で一緒に確認しましょう。"],
        ),
    ] {
        for seed in [reading, kana] {
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
    let ime = engine("kanji");
    draft(&ime, "kanji");
    assert!(index(&ime, "漢字", CandidateKind::Word) < index(&ime, "感じ", CandidateKind::Word));
}

#[test]
fn messaging_unknown_urls_seven_line_boundaries_and_long_prefixes_do_not_leak_authors() {
    let mut seeds = [
        "https://messe-ji.example.invalid",
        "miotoshi@example.invalid",
        "xyzyoteinokakunin",
        "予定の確認dexyz",
        "🙂返信不要desu",
        "集合時間xyzn i",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("mio{boundary}toshi"));
        seeds.push(format!("集合時間{boundary}ni"));
    }
    for tail in ["messe-ji", "予定の確認wo", "返信不要desu"] {
        let long = format!("{}{tail}", "前文。".repeat(90));
        assert!(long.chars().count() > 256);
        seeds.push(long);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            ime.candidates().iter().all(|c| {
                !SENTENCES
                    .iter()
                    .flat_map(|(_, ss)| ss)
                    .any(|sentence| c.text.ends_with(sentence))
            }),
            "unexpected messaging author for {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-messaging-synthetic-model-error"
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
fn messaging_model_unavailable_timeout_and_empty_preserve_all_six_local_contexts() {
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
