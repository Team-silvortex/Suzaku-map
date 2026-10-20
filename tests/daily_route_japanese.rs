//! Offline en-route messages, not GPS, navigation, location sharing or message delivery.
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

// Fixed before the route append: 556 readings / 279 contexts / 557 sentences.
// Arrival, directions, meeting times and the optional rail vocabulary already exist.
const WORDS: &[(&str, &str, &str, &str)] = &[
    (
        "machiawasebasho",
        "まちあわせばしょ",
        "マチアワセバショ",
        "待ち合わせ場所",
    ),
    (
        "imamukatteimasu",
        "いまむかっています",
        "イマムカッテイマス",
        "今向かっています",
    ),
    (
        "mousugutsukimasu",
        "もうすぐつきます",
        "モウスグツキマス",
        "もうすぐ着きます",
    ),
    ("atogofun", "あとごふん", "アトゴフン", "あと五分"),
    ("betsuru-to", "べつるーと", "ベツルート", "別ルート"),
    ("genzaichi", "げんざいち", "ゲンザイチ", "現在地"),
    ("ekimae", "えきまえ", "エキマエ", "駅前"),
    (
        "shoumengenkan",
        "しょうめんげんかん",
        "ショウメンゲンカン",
        "正面玄関",
    ),
    ("kitaguchi", "きたぐち", "キタグチ", "北口"),
    ("minamiguchi", "みなみぐち", "ミナミグチ", "南口"),
    ("hodoukyou", "ほどうきょう", "ホドウキョウ", "歩道橋"),
    ("toomawari", "とおまわり", "トオマワリ", "遠回り"),
];
const ALIASES: &[(&str, &str, &str)] = &[
    ("atogofunn", "あとごふん", "あと五分"),
    ("shoumengenkann", "しょうめんげんかん", "正面玄関"),
    ("shoumenngenkann", "しょうめんげんかん", "正面玄関"),
    ("gennzaichi", "げんざいち", "現在地"),
    (
        "imamukaltuteimasu",
        "いまむかっています",
        "今向かっています",
    ),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "待ち合わせ場所",
        [
            "待ち合わせ場所を駅前に変えてもいいですか。",
            "待ち合わせ場所は、この建物の入口です。",
        ],
    ),
    (
        "今向かっています",
        [
            "今向かっています。少し遅れそうです。",
            "今向かっていますので、着いたら連絡します。",
        ],
    ),
    (
        "もうすぐ着きます",
        [
            "もうすぐ着きます。どの入口にいますか。",
            "もうすぐ着きますので、そのまま待っていてください。",
        ],
    ),
    (
        "あと五分",
        [
            "あと五分くらいで着きます。",
            "あと五分だけ待ってもらえますか。",
        ],
    ),
    (
        "別ルート",
        [
            "別ルートで向かっています。",
            "別ルートに変えたので、少し遅れそうです。",
        ],
    ),
    (
        "現在地",
        [
            "現在地を送ってもらえますか。",
            "現在地を確認してから向かいます。",
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
fn route_fixed_spellings_existing_alias_rules_and_append_windows() {
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    for &(romaji, kana, katakana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
        assert_eq!(romaji_to_hiragana(katakana), katakana);
    }
    for &(seed, kana, _) in ALIASES {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    // No new phonetic aliases for missing long marks, a different long vowel or digits.
    assert_ne!(romaji_to_hiragana("betsuruto"), WORDS[4].1);
    assert_ne!(romaji_to_hiragana("toumawari"), WORDS[11].1);
    assert_ne!(romaji_to_hiragana("ato5fun"), WORDS[3].1);
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 568 && ja.continuations().count() >= 285);
    for (entry, &(_, kana, _, word)) in ja.readings()[556..568].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(279).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(279)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        557
    );
    assert_eq!(
        ja.continuations()
            .take(285)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        569
    );
}

#[test]
fn route_full_written_katakana_and_specific_partials_keep_deliberate_commit_choices() {
    for &(romaji, kana, katakana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
            word.to_owned(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            let kind = if seed == word {
                CandidateKind::Literal
            } else {
                CandidateKind::Word
            };
            first_page(&ime, word, kind);
            commit_once(&mut engine(&seed), word, kind);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            // Only these six contexts promise authored sentences, not the six supplements.
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
        // Preserve the user's Katakana spelling. Existing behavior does not
        // promise Katakana-to-Kanji replacement or a borrowed canonical sentence.
        let ime = engine(katakana);
        draft(&ime, katakana);
        first_page(&ime, katakana, CandidateKind::Literal);
        assert!(ime.candidates().iter().all(|candidate| {
            !SENTENCES
                .iter()
                .flat_map(|(_, ss)| ss)
                .any(|sentence| candidate.text.contains(*sentence))
        }));
        commit_once(&mut engine(katakana), katakana, CandidateKind::Literal);
    }
    for (seed, word) in [
        ("machiawasebash", "待ち合わせ場所"),
        ("imamukatteima", "今向かっています"),
        ("mousugutsukimas", "もうすぐ着きます"),
        ("atogofu", "あと五分"),
        ("betsuru-t", "別ルート"),
        ("genzaich", "現在地"),
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
    for &(seed, _, word) in ALIASES {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn route_horizontal_gaps_adopted_progress_and_reseed_preserve_editable_drafts() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("machiawase", "BASHO", "待ち合わせ場所"),
            ("ima", "MUKATTEIMASU", "今向かっています"),
            ("mousugu", "TSUKIMASU", "もうすぐ着きます"),
            ("ato", "GOFUN", "あと五分"),
            ("betsu", "RU-TO", "別ルート"),
            ("genzai", "CHI", "現在地"),
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
        ("待ち合わせ場所", "wo", SENTENCES[0].1[0]),
        ("今向かっています", "ので", SENTENCES[1].1[1]),
        ("もうすぐ着きます", "ので", SENTENCES[2].1[1]),
        ("あと五分", "だけ", SENTENCES[3].1[1]),
        ("別ルート", "de", SENTENCES[4].1[0]),
        ("現在地", "WO", SENTENCES[5].1[0]),
    ] {
        let reading = WORDS
            .iter()
            .find(|(_, _, _, text)| *text == word)
            .unwrap()
            .0;
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
fn route_keeps_existing_meeting_queue_contact_and_arrival_authors_on_page_one() {
    for (reading, word, sentences) in [
        (
            "shuugoujikan",
            "集合時間",
            [
                "集合時間をもう一度教えてください。",
                "集合時間に間に合いそうです。",
            ],
        ),
        (
            "seiriken",
            "整理券",
            [
                "整理券はどこでもらえますか。",
                "整理券を受け取ってから順番を待ちます。",
            ],
        ),
        (
            "renraku",
            "連絡",
            ["連絡ありがとうございます。", "連絡が遅くなってすみません。"],
        ),
        (
            "tsukimashita",
            "着きました",
            [
                "着きました、入り口で待っています。",
                "着きました、今からそちらに向かいます。",
            ],
        ),
        (
            "okuremasu",
            "遅れます",
            [
                "遅れます、先に始めていてください。",
                "遅れます、着いたら連絡します。",
            ],
        ),
    ] {
        let ime = engine(reading);
        draft(&ime, reading);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(reading), word, CandidateKind::Word);
        for sentence in sentences {
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(reading), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(reading), reading, CandidateKind::Literal);
    }
}

#[test]
fn route_protected_unknown_seven_line_and_long_drafts_never_borrow_authored_locations() {
    let mut seeds = [
        "https://genzaichi.example.invalid",
        "machiawasebasho@example.invalid",
        "xyzbetsuru-to",
        "現在地dexyz",
        "🙂待ち合わせ場所wo",
        "あと5分",
        "あと五分xyzn i",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("machiawase{boundary}basho"));
        seeds.push(format!("別ルート{boundary}de"));
    }
    for tail in ["genzaichi", "今向かっていますので", "あと五分だけ"] {
        let long = format!("{}{tail}", "前文。".repeat(90));
        assert!(long.chars().count() > 256);
        seeds.push(long);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            ime.candidates().iter().all(|candidate| {
                !SENTENCES
                    .iter()
                    .flat_map(|(_, ss)| ss)
                    .any(|sentence| candidate.text.contains(*sentence))
            }),
            "unexpected route author for {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-route-synthetic-model-error"
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
fn route_model_unavailable_timeout_and_empty_keep_each_complete_local_pool() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for &(word, sentences) in SENTENCES {
            let reading = WORDS
                .iter()
                .find(|(_, _, _, text)| *text == word)
                .unwrap()
                .0;
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
