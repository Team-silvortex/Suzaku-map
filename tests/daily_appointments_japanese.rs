//! Offline appointments and queue dialogue, not a booking or service-information system.
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

// Frozen against the 534-reading / 268-context baseline before any appointment additions.
// Existing reservation/reception words may gain relevant completions without losing their primary.
const WORDS: &[(&str, &str, &str)] = &[
    ("yoyakuhenkou", "よやくへんこう", "予約変更"),
    ("raitenjikan", "らいてんじかん", "来店時間"),
    ("machijikan", "まちじかん", "待ち時間"),
    ("eigyoujikan", "えいぎょうじかん", "営業時間"),
    ("seiriken", "せいりけん", "整理券"),
    ("teikyuubi", "ていきゅうび", "定休日"),
    ("yoyakukakunin", "よやくかくにん", "予約確認"),
    ("uketsukejikan", "うけつけじかん", "受付時間"),
    ("toujitsuuketsuke", "とうじつうけつけ", "当日受付"),
    ("saikoubi", "さいこうび", "最後尾"),
    ("yobidashi", "よびだし", "呼び出し"),
    ("rinjikyuugyou", "りんじきゅうぎょう", "臨時休業"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "予約変更",
        [
            "予約変更をお願いできますか。",
            "予約変更ができるか確認します。",
        ],
    ),
    (
        "来店時間",
        [
            "来店時間を少し遅らせてもいいですか。",
            "来店時間が決まったら連絡します。",
        ],
    ),
    (
        "待ち時間",
        [
            "待ち時間はどのくらいですか。",
            "待ち時間に少し外へ出てもいいですか。",
        ],
    ),
    (
        "営業時間",
        [
            "営業時間を教えてください。",
            "営業時間を確認してから行きます。",
        ],
    ),
    (
        "整理券",
        [
            "整理券はどこでもらえますか。",
            "整理券を受け取ってから順番を待ちます。",
        ],
    ),
    (
        "定休日",
        [
            "定休日は何曜日ですか。",
            "定休日だったので、別の日に行きます。",
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
fn appointments_fixed_romaji_terminal_n_and_append_windows() {
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    for &(romaji, kana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
    }
    for (seed, kana) in [
        ("raitenjikann", "らいてんじかん"),
        ("machijikann", "まちじかん"),
        ("eigyoujikann", "えいぎょうじかん"),
        ("seirikenn", "せいりけん"),
        ("yoyakukakuninn", "よやくかくにん"),
        ("uketsukejikann", "うけつけじかん"),
        ("rinnjikyuugyou", "りんじきゅうぎょう"),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 546 && ja.continuations().count() >= 274);
    for (entry, &(_, kana, word)) in ja.readings()[534..546].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(268).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(268)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        535
    );
    assert_eq!(
        ja.continuations()
            .take(274)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        547
    );
}

#[test]
fn appointments_complete_written_and_specific_partial_inputs_keep_words_sentences_and_raw() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
            word.to_owned(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            // Exact written spelling is the single deduplicated raw candidate.
            let kind = if seed == word {
                CandidateKind::Literal
            } else {
                CandidateKind::Word
            };
            first_page(&ime, word, kind);
            commit_once(&mut engine(&seed), word, kind);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            // Only the six authored contexts promise sentence choices.
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
    for (seed, word) in [
        ("yoyakuhenko", "予約変更"),
        ("raitenjika", "来店時間"),
        ("machijika", "待ち時間"),
        ("eigyoujika", "営業時間"),
        ("seirike", "整理券"),
        ("teikyuub", "定休日"),
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
        ("raitenjikann", "来店時間"),
        ("machijikann", "待ち時間"),
        ("eigyoujikann", "営業時間"),
        ("seirikenn", "整理券"),
        ("yoyakukakuninn", "予約確認"),
        ("uketsukejikann", "受付時間"),
        ("rinnjikyuugyou", "臨時休業"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn appointments_horizontal_spacing_adopted_particles_and_reseed_do_not_submit_drafts() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("yoyaku", "HENKOU", "予約変更"),
            ("raiten", "JIKAN", "来店時間"),
            ("machi", "JIKAN", "待ち時間"),
            ("eigyou", "JIKAN", "営業時間"),
            ("seiri", "KEN", "整理券"),
            ("teikyuu", "BI", "定休日"),
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
        ("予約変更", "wo", SENTENCES[0].1[0]),
        ("来店時間", "を", SENTENCES[1].1[0]),
        ("待ち時間", "ha", SENTENCES[2].1[0]),
        ("営業時間", "WO", SENTENCES[3].1[0]),
        ("整理券", "は", SENTENCES[4].1[0]),
        ("定休日", "ha", SENTENCES[5].1[0]),
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
fn appointments_preserve_existing_reservation_reception_and_neighboring_authored_choices() {
    // These old words had no authored context. New relevant completions are allowed.
    for (reading, kana, word) in [
        ("yoyaku", "よやく", "予約"),
        ("uketsuke", "うけつけ", "受付"),
        ("uketsukebangou", "うけつけばんごう", "受付番号"),
    ] {
        for seed in [reading, kana] {
            let ime = engine(seed);
            draft(&ime, seed);
            assert_eq!(ime.candidates()[0].text, word);
            first_page(&ime, word, CandidateKind::Word);
            commit_once(&mut engine(seed), word, CandidateKind::Word);
            commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            if word == "受付" {
                index(&ime, "受付番号", CandidateKind::Word);
            }
        }
    }
    for (reading, kana, word, sentences) in [
        (
            "junban",
            "じゅんばん",
            "順番",
            [
                "順番が来たら教えてください。",
                "順番を待っているので、少し遅れます。",
            ],
        ),
        (
            "madoguchi",
            "まどぐち",
            "窓口",
            ["窓口はどこにありますか。", "窓口で手続きをしたいです。"],
        ),
        (
            "jikantai",
            "じかんたい",
            "時間帯",
            [
                "時間帯を変えても大丈夫ですか。",
                "時間帯が決まったら教えてください。",
            ],
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
}

#[test]
fn appointments_protected_unknown_seven_line_and_long_drafts_never_borrow_new_authors() {
    let mut seeds = [
        "https://yoyakuhenkou.example.invalid",
        "eigyoujikan@example.invalid",
        "xyzseiriken",
        "来店時間dexyz",
        "🙂営業時間wo",
        "定休日xyzn i",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("yoyaku{boundary}henkou"));
        seeds.push(format!("整理券{boundary}ha"));
    }
    for tail in ["machijikan", "営業時間wo", "定休日ha"] {
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
                    .any(|sentence| c.text.contains(*sentence))
            }),
            "unexpected appointment author for {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-appointments-synthetic-model-error"
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
fn appointments_model_unavailable_timeout_and_empty_keep_each_complete_local_pool() {
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
