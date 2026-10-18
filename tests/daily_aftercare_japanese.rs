//! Offline aftercare dialogue; no store policy, warranty entitlement or refund is determined.
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

// Frozen before this batch against 546 readings / 274 contexts / 547 sentences.
// Existing 領収書/返品/交換/故障/修理 are not new capabilities in this batch.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("hoshousho", "ほしょうしょ", "ホショウショ", "保証書"),
    ("kounyuubi", "こうにゅうび", "コウニュウビ", "購入日"),
    ("fuzokuhin", "ふぞくひん", "フゾクヒン", "付属品"),
    (
        "hensouhouhou",
        "へんそうほうほう",
        "ヘンソウホウホウ",
        "返送方法",
    ),
    ("konpouzai", "こんぽうざい", "コンポウザイ", "梱包材"),
    ("mikaifuu", "みかいふう", "ミカイフウ", "未開封"),
    ("sotobako", "そとばこ", "ソトバコ", "外箱"),
    ("hinban", "ひんばん", "ヒンバン", "品番"),
    ("henkin", "へんきん", "ヘンキン", "返金"),
    ("fuguai", "ふぐあい", "フグアイ", "不具合"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "保証書",
        [
            "保証書を持ってきました。",
            "保証書はこの封筒に入っています。",
        ],
    ),
    (
        "購入日",
        [
            "購入日を確認してから問い合わせます。",
            "購入日が分かるものを持っていけばいいですか。",
        ],
    ),
    (
        "付属品",
        [
            "付属品は全部そろっています。",
            "付属品も一緒に持っていけばいいですか。",
        ],
    ),
    (
        "返送方法",
        [
            "返送方法を教えてもらえますか。",
            "返送方法を確認してから荷物を送ります。",
        ],
    ),
    (
        "梱包材",
        [
            "梱包材はそのまま残してあります。",
            "梱包材を使って包み直してもいいですか。",
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
fn aftercare_fixed_romaji_existing_n_aliases_and_append_windows() {
    assert_eq!(WORDS.len(), 10);
    assert_eq!(SENTENCES.len(), 5);
    for &(romaji, kana, katakana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
        assert_eq!(romaji_to_hiragana(katakana), katakana);
    }
    for (seed, kana) in [
        ("fuzokuhinn", "ふぞくひん"),
        ("hinnban", "ひんばん"),
        ("hinbann", "ひんばん"),
        ("hennkin", "へんきん"),
        ("henkinn", "へんきん"),
        ("konnpouzai", "こんぽうざい"),
        ("hennsouhouhou", "へんそうほうほう"),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    // 購入 has にゅ, not a syllabic ん before にゅ. No new spelling alias is invented.
    assert_ne!(romaji_to_hiragana("kounnyuubi"), WORDS[1].1);
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 556 && ja.continuations().count() >= 279);
    for (entry, &(_, kana, _, word)) in ja.readings()[546..556].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(274).take(5).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(274)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        547
    );
    assert_eq!(
        ja.continuations()
            .take(279)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        557
    );
}

#[test]
fn aftercare_full_written_katakana_and_five_partials_keep_explicit_commit_choices() {
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
            // Only five contexts have authored sentence pairs. 返金 and the
            // other supplementary words do not imply store policy or new sentences.
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
        // Existing behavior preserves an adopted Katakana spelling. It does not
        // promise Katakana-to-Kanji conversion or borrow a canonical Kanji sentence.
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
        ("hoshoush", "保証書"),
        ("kounyuub", "購入日"),
        ("fuzokuhi", "付属品"),
        ("hensouhouho", "返送方法"),
        ("konpouza", "梱包材"),
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
        ("fuzokuhinn", "付属品"),
        ("hinnban", "品番"),
        ("hinbann", "品番"),
        ("hennkin", "返金"),
        ("henkinn", "返金"),
        ("konnpouzai", "梱包材"),
        ("hennsouhouhou", "返送方法"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn aftercare_horizontal_gaps_adopted_particles_and_reseed_keep_drafts_editable() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("hoshou", "SHO", "保証書"),
            ("kounyuu", "BI", "購入日"),
            ("fuzoku", "HIN", "付属品"),
            ("hensou", "HOUHOU", "返送方法"),
            ("konpou", "ZAI", "梱包材"),
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
        ("保証書", "wo", SENTENCES[0].1[0]),
        ("購入日", "を", SENTENCES[1].1[0]),
        ("付属品", "ha", SENTENCES[2].1[0]),
        ("返送方法", "WO", SENTENCES[3].1[0]),
        ("梱包材", "は", SENTENCES[4].1[0]),
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
fn aftercare_keeps_preexisting_receipts_returns_repairs_and_exchange_distinct() {
    for (reading, word, sentences) in [
        (
            "ryoushuusho",
            "領収書",
            ["領収書をお願いします。", "領収書に名前を入れてください。"],
        ),
        (
            "henpin",
            "返品",
            ["返品できますか。", "返品の期限を教えてください。"],
        ),
        (
            "henpinn",
            "返品",
            ["返品できますか。", "返品の期限を教えてください。"],
        ),
        (
            "koshou",
            "故障",
            [
                "故障したので、修理をお願いしたいです。",
                "故障の状況を説明します。",
            ],
        ),
        (
            "shuuri",
            "修理",
            [
                "修理に来てもらえる日時を教えてください。",
                "修理の予定を確認したいです。",
            ],
        ),
        (
            "azukarishou",
            "預かり証",
            [
                "預かり証を持ってきました。",
                "預かり証を見せればいいですか。",
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
    // 交換 was already a Word, but has no builtin authored context to promise.
    for seed in ["koukan", "koukann", "こうかん"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "交換");
        first_page(&ime, "交換", CandidateKind::Word);
        commit_once(&mut engine(seed), "交換", CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn aftercare_protected_unknown_seven_line_and_long_drafts_never_borrow_new_authors() {
    let mut seeds = [
        "https://hensouhouhou.example.invalid",
        "hoshousho@example.invalid",
        "xyzkonpouzai",
        "購入日dexyz",
        "🙂付属品ha",
        "梱包材xyzn i",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("hoshou{boundary}sho"));
        seeds.push(format!("付属品{boundary}ha"));
    }
    for tail in ["hoshousho", "返送方法wo", "梱包材ha"] {
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
            "unexpected aftercare author for {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-aftercare-synthetic-model-error"
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
fn aftercare_model_unavailable_timeout_and_empty_keep_all_five_local_pools() {
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
