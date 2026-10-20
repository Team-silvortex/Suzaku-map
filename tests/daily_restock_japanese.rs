//! Finite household-supply dialogue, not stock measurement or automatic purchasing.
//! Frozen before this append against the dirty storage catalog: 580/297/593.
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

const WORDS: &[(&str, &str, &str, &str)] = &[
    ("thisshu", "てぃっしゅ", "ティッシュ", "ティッシュ"),
    (
        "toirettope-pa-",
        "といれっとぺーぱー",
        "トイレットペーパー",
        "トイレットペーパー",
    ),
    ("tsumekae", "つめかえ", "ツメカエ", "詰め替え"),
    ("kaitashi", "かいたし", "カイタシ", "買い足し"),
    ("yobi", "よび", "ヨビ", "予備"),
    (
        "nokorigasukunai",
        "のこりがすくない",
        "ノコリガスクナイ",
        "残りが少ない",
    ),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "ティッシュ",
        [
            "ティッシュが残り少なくなりました。",
            "ティッシュを一箱買い足しておきます。",
        ],
    ),
    (
        "トイレットペーパー",
        [
            "トイレットペーパーの予備はありますか。",
            "トイレットペーパーを買い物のメモに追加します。",
        ],
    ),
    (
        "詰め替え",
        [
            "詰め替えを買う前に、容器を確認します。",
            "詰め替えはいつもの棚にあります。",
        ],
    ),
    (
        "買い足し",
        [
            "買い足しが必要なものを教えてください。",
            "買い足しの前に、家にある数を確認します。",
        ],
    ),
    (
        "予備",
        ["予備は戸棚の中にあります。", "予備を一つ買っておきます。"],
    ),
    (
        "残りが少ない",
        [
            "残りが少ないので、買い物のメモに書いておきます。",
            "残りが少ないものから順に補充しましょう。",
        ],
    ),
];
const PARTIALS: &[&str] = &[
    "thissh",
    "toirettope-pa",
    "tsumeka",
    "kaitash",
    "yob",
    "nokorigasukuna",
];
// These spellings already belong to the converter; no new syllable or alias is added.
const ALIASES: &[(&str, usize)] = &[
    ("texisshu", 0),
    ("telisshu", 0),
    ("toireltutope-pa-", 1),
    ("tumekae", 2),
    ("kaitasi", 3),
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
            .all(|c| { c.source == CandidateSource::Local && (0.0..=100.0).contains(&c.score) })
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
    assert!(index(ime, text, kind) < PAGE_SIZE, "{:?}", ime.candidates());
}
fn authors(ime: &XRTabletImeEngine, which: usize) {
    for sentence in SENTENCES[which].1 {
        first_page(ime, sentence, CandidateKind::Sentence);
    }
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
    let repeated = ime.commit(CommitOptions { force: true });
    assert!(!repeated.ok && repeated.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn restock_fixed_readings_existing_aliases_and_append_windows() {
    assert_eq!(WORDS.len(), 6);
    assert_eq!(SENTENCES.len(), 6);
    assert_eq!(PARTIALS.len(), 6);
    for &(romaji, kana, katakana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
        assert_eq!(romaji_to_hiragana(katakana), katakana);
    }
    for &(seed, which) in ALIASES {
        assert_eq!(romaji_to_hiragana(seed), WORDS[which].1, "{seed}");
    }
    assert_eq!(romaji_to_hiragana("tisshu"), "ちっしゅ");
    assert_ne!(romaji_to_hiragana("tisshu"), WORDS[0].1);
    assert_ne!(romaji_to_hiragana("toirettopepa"), WORDS[1].1);
    // No new reading ends in nasal n: neither fake nn aliases nor ず/づ correction.
    assert_ne!(romaji_to_hiragana("katazuke"), "かたづけ");
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 586 && ja.continuations().count() >= 303);
    for (entry, &(_, kana, _, word)) in ja.readings()[580..586].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(297).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(297)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        593
    );
    assert_eq!(
        ja.continuations()
            .take(303)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        605
    );
}

#[test]
fn restock_complete_spellings_specific_partials_and_aliases_keep_deliberate_commit_choices() {
    for (which, &(romaji, kana, katakana, word)) in WORDS.iter().enumerate() {
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
            authors(&ime, which);
            commit_once(&mut engine(&seed), word, kind);
            for sentence in SENTENCES[which].1 {
                commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
        let ime = engine(katakana);
        draft(&ime, katakana);
        first_page(&ime, katakana, CandidateKind::Literal);
        if katakana == word {
            // These two canonical written words are already Katakana. Their authors are valid.
            authors(&ime, which);
        } else {
            // Do not invent Katakana-to-Kanji aliases for the other four words.
            assert!(ime.candidates().iter().all(|c| {
                !SENTENCES
                    .iter()
                    .flat_map(|(_, ss)| ss)
                    .any(|s| c.text.contains(*s))
            }));
        }
        commit_once(&mut engine(katakana), katakana, CandidateKind::Literal);
        let partial = PARTIALS[which];
        let ime = engine(partial);
        draft(&ime, partial);
        first_page(&ime, word, CandidateKind::Word);
        authors(&ime, which);
        commit_once(&mut engine(partial), word, CandidateKind::Word);
        for sentence in SENTENCES[which].1 {
            commit_once(&mut engine(partial), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(partial), partial, CandidateKind::Literal);
    }
    for &(seed, which) in ALIASES {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, WORDS[which].3, CandidateKind::Word);
        authors(&ime, which);
        commit_once(&mut engine(seed), WORDS[which].3, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn restock_horizontal_gaps_adopted_tail_and_reseed_preserve_raw_spaces() {
    // Split before the complete doubled-consonant chunk, not within an unfinished ss/tt.
    let splits = [
        ("thi", "SSHU"),
        ("toiretto", "PE-PA-"),
        ("tsume", "KAE"),
        ("kai", "TASHI"),
        ("yo", "BI"),
        ("nokori", "GASUKUNAI"),
    ];
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (which, (left, right)) in splits.iter().enumerate() {
            let seed = format!("{separator}{left}{separator}{right}{separator}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, WORDS[which].3, CandidateKind::Word);
            authors(&ime, which);
            commit_once(&mut engine(&seed), WORDS[which].3, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
    for (which, particle) in ["ga", "no", "wo", "ga", "ha", "node"].iter().enumerate() {
        let (reading, _, _, word) = WORDS[which];
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        for separator in ["", " ", "\u{3000}"] {
            let seed = format!("{word}{separator}{particle}");
            ime.seed(&seed);
            draft(&ime, &seed);
            // The author is canonical; only the separate Literal retains reading separators.
            first_page(&ime, SENTENCES[which].1[0], CandidateKind::Sentence);
            commit_once(
                &mut engine(&seed),
                SENTENCES[which].1[0],
                CandidateKind::Sentence,
            );
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
        }
    }
}

#[test]
fn restock_preserves_old_supply_words_authors_and_the_longer_call_reading() {
    for (reading, word, sentences) in [
        (
            "senzai",
            "洗剤",
            ["洗剤はどこにありますか。", "洗剤が少なくなってきました。"],
        ),
        (
            "gomibukuro",
            "ごみ袋",
            [
                "ごみ袋を新しいものに替えます。",
                "ごみ袋が少なくなりました。",
            ],
        ),
    ] {
        let ime = engine(reading);
        draft(&ime, reading);
        first_page(&ime, word, CandidateKind::Word);
        for sentence in sentences {
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(reading), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(reading), word, CandidateKind::Word);
        commit_once(&mut engine(reading), reading, CandidateKind::Literal);
    }
    // These old independent Words do not acquire an invented pair of authors.
    // New 予備 is a shorter reading; the old longer 呼び出し stays available on page one.
    for (seed, word) in [
        ("hamigakiko", "歯磨き粉"),
        ("yobidashi", "呼び出し"),
        ("yobidash", "呼び出し"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn restock_protected_unicode_unknown_split_geminates_and_seven_lines_do_not_borrow_authors() {
    let mut seeds = [
        "https://example.invalid/thisshu",
        "tsumekae@example.invalid",
        "xyztoirettope-pa-",
        "予備dexyz",
        "🙂残りが少ないので",
        "予備\u{200b}ha",
        "ｔｈｉｓｓｈｕ",
        "tisshu",
        "this SHU",
        "toiret TOPE-PA-",
        "katazuke",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("tsume{boundary}kae"));
        seeds.push(format!("ティッシュ{boundary}ga"));
    }
    for tail in ["thisshu", "予備ha", "残りが少ないnode"] {
        let seed = format!("{}{tail}", "前文。".repeat(90));
        assert!(seed.chars().count() > 256);
        seeds.push(seed);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            ime.candidates().iter().all(|c| !SENTENCES
                .iter()
                .flat_map(|(_, ss)| ss)
                .any(|s| c.text.contains(*s))),
            "unexpected restock author for {seed:?}: {:?}",
            ime.candidates()
        );
        if matches!(seed.as_str(), "tisshu" | "this SHU") {
            assert!(ime.candidates().iter().all(|c| c.text != "ティッシュ"));
        }
        if seed == "toiret TOPE-PA-" {
            assert!(
                ime.candidates()
                    .iter()
                    .all(|c| c.text != "トイレットペーパー")
            );
        }
        if seed == "katazuke" {
            assert!(ime.candidates().iter().all(|c| c.text != "片付け"));
        }
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-restock-synthetic-model-error"
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
fn restock_unavailable_timeout_and_empty_models_preserve_each_complete_local_pool() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (which, &(reading, _, _, word)) in WORDS.iter().enumerate() {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
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
            commit_once(&mut ime, SENTENCES[which].1[0], CandidateKind::Sentence);
        }
    }
}
