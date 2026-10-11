//! Independent Japanese public-transport and boarding fallback fixtures.
//! These finite presets do not inspect location, query timetables or fares,
//! book tickets, call a live model, or claim general Japanese morphology.
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
    lexicon::{EntryKind, builtin},
};

const WORDS: &[(&str, &str, &str)] = &[
    ("densha", "でんしゃ", "電車"),
    ("basu", "ばす", "バス"),
    ("chikatetsu", "ちかてつ", "地下鉄"),
    ("teiryuujo", "ていりゅうじょ", "停留所"),
    ("noriba", "のりば", "乗り場"),
    ("jousha", "じょうしゃ", "乗車"),
    ("kousha", "こうしゃ", "降車"),
    ("joushaken", "じょうしゃけん", "乗車券"),
    ("unchin", "うんちん", "運賃"),
    ("shuppatsu", "しゅっぱつ", "出発"),
    ("chien", "ちえん", "遅延"),
    ("unkyuu", "うんきゅう", "運休"),
    ("rosen", "ろせん", "路線"),
    ("kaisoku", "かいそく", "快速"),
    ("zaseki", "ざせき", "座席"),
    ("mokutekichi", "もくてきち", "目的地"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "電車",
        ["電車は何時に出発しますか。", "電車を降りたら連絡します。"],
    ),
    (
        "バス",
        ["バスはここに止まりますか。", "バスで駅まで行きたいです。"],
    ),
    (
        "地下鉄",
        [
            "地下鉄の入口はどこですか。",
            "地下鉄で行くと何分かかりますか。",
        ],
    ),
    (
        "乗り場",
        ["乗り場を教えてください。", "乗り場は反対側ですか。"],
    ),
    (
        "乗車券",
        [
            "乗車券はどこで買えますか。",
            "乗車券を確認してから乗りましょう。",
        ],
    ),
    (
        "運賃",
        ["運賃はいくらですか。", "運賃に追加料金は含まれますか。"],
    ),
    ("出発", ["出発は何時ですか。", "出発する前に連絡します。"]),
    (
        "目的地",
        [
            "目的地まで何分かかりますか。",
            "目的地に着いたら教えてください。",
        ],
    ),
];

fn blank_engine() -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = blank_engine();
    ime.seed(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    ime
}
fn check_draft(ime: &XRTabletImeEngine, seed: &str) {
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
            .all(|candidate| { candidate.source == CandidateSource::Local })
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
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = selectable_index(ime, text, kind);
    assert!(
        kind == CandidateKind::Literal || index < PAGE_SIZE,
        "not page one {text:?}, seed={:?}: {:?}",
        ime.snapshot().seed_text,
        ime.candidates()
    );
    index
}
fn selectable_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        })
}
fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let selected = index(ime, text, kind);
    commit_index_once_and_undo(ime, selected, text);
}
fn commit_index_once_and_undo(ime: &mut XRTabletImeEngine, selected: usize, text: &str) {
    assert_eq!(ime.candidates()[selected].text, text);
    assert_eq!(ime.candidates()[selected].source, CandidateSource::Local);
    ime.select_candidate(selected);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok);
    assert!(duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn transit_preserves_all_282_previous_readings_and_142_contexts_in_full_order() {
    // Captured before transit: every old flag, complete context order and all
    // 283 authored sentences, including the shopping expansion.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 282);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(282) {
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
            readings = (readings ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(readings, 0x5c10ab53e67a277e);
    assert!(ja.continuations().count() >= 142);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut previous_sentence_count = 0;
    for (context, sentences) in ja.continuations().take(142) {
        previous_sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(previous_sentence_count, 283);
    assert_eq!(contexts, 0x6183526ffb56af69);
}

#[test]
fn transit_additions_match_the_independent_authored_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 16);
    assert_eq!(SENTENCES.len(), 8);
    assert!(ja.readings().len() >= 298);
    assert!(ja.continuations().count() >= 150);
    assert_eq!(
        ja.continuations()
            .take(150)
            .map(|(_, sentences)| sentences.len())
            .sum::<usize>(),
        299
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(282).take(16).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(142).take(8).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn complete_transit_readings_offer_first_page_words_sentences_and_exact_single_commits() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            check_draft(&ime, &seed);
            commit_once_and_undo(&mut engine(&seed), word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            if let Some(&(_, sentences)) = SENTENCES.iter().find(|(text, _)| *text == word) {
                for sentence in sentences {
                    commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn specific_transit_partials_and_ambiguous_branches_remain_reachable() {
    // Precise independent prefixes. Broad one-kana requests keep their old
    // ranks and page limits, not a promise to surface every matching new word.
    for (seed, word) in [
        ("densh", "電車"),
        ("chikatets", "地下鉄"),
        ("teiryuuj", "停留所"),
        ("norib", "乗り場"),
        ("joush", "乗車"),
        ("koush", "降車"),
        ("unchi", "運賃"),
        ("shuppats", "出発"),
        ("chie", "遅延"),
        ("unkyu", "運休"),
        ("rose", "路線"),
        ("kaisok", "快速"),
        ("zase", "座席"),
        ("mokutekich", "目的地"),
        ("でんし", "電車"),
        ("ちかて", "地下鉄"),
        ("のり", "乗り場"),
        ("うんち", "運賃"),
        ("しゅっぱ", "出発"),
        ("もくてき", "目的地"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
        if let Some(&(_, sentences)) = SENTENCES.iter().find(|(text, _)| *text == word) {
            for sentence in sentences {
                commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
            }
        }
    }
    // An unfinished s probes both old ばしょ and new ばす. Keep the
    // unconverted default, old word order and old page-one sentences; the
    // two new bus sentences remain selectable on page two. Full basu has
    // no such ambiguity and is covered by the first-page complete test.
    for seed in ["bas", "BAS"] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "ばs");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Unspecified);
        assert!(
            index(&ime, "場所", CandidateKind::Word) < index(&ime, "バス", CandidateKind::Word)
        );
        for sentence in [
            "場所を地図で確認しましょう。",
            "場所が分かったら教えてください。",
        ] {
            index(&ime, sentence, CandidateKind::Sentence);
        }
        for sentence in ["バスはここに止まりますか。", "バスで駅まで行きたいです。"]
        {
            let mut selected = engine(seed);
            let page_two = selectable_index(&selected, sentence, CandidateKind::Sentence);
            assert!((PAGE_SIZE..2 * PAGE_SIZE).contains(&page_two));
            commit_index_once_and_undo(&mut selected, page_two, sentence);
            // Restore the exact original editable spelling, not the converted
            // base or the selected sentence; this is a pure-engine update.
            selected.seed(seed);
            check_draft(&selected, seed);
            commit_once_and_undo(&mut selected, seed, CandidateKind::Literal);
        }
    }
    // 乗車 is complete before the unfinished け. Retain that converted
    // progress and the old け noun alternatives rather than removing them
    // to promote both ticket sentences. Full joushaken still has both
    // sentences on page one in the independent complete-reading test.
    for seed in ["joushake", "JOUSHAKE", "じょうしゃけ"] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "乗車け");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
        let ticket = index(&ime, "乗車券", CandidateKind::Word);
        let scenery = selectable_index(&ime, "乗車景色", CandidateKind::Word);
        let conclusion = selectable_index(&ime, "乗車結論", CandidateKind::Word);
        let morning = selectable_index(&ime, "乗車今朝", CandidateKind::Word);
        assert!(ticket < scenery && scenery < conclusion && conclusion < morning);
        commit_once_and_undo(&mut engine(seed), "乗車券", CandidateKind::Word);
        commit_once_and_undo(
            &mut engine(seed),
            "乗車券はどこで買えますか。",
            CandidateKind::Sentence,
        );
        let mut selected = engine(seed);
        let second = "乗車券を確認してから乗りましょう。";
        let page_two = selectable_index(&selected, second, CandidateKind::Sentence);
        assert!((PAGE_SIZE..2 * PAGE_SIZE).contains(&page_two));
        commit_index_once_and_undo(&mut selected, page_two, second);
        selected.seed(seed);
        check_draft(&selected, seed);
        commit_once_and_undo(&mut selected, seed, CandidateKind::Literal);
    }
}

#[test]
fn transit_preserves_old_homophone_order_and_kou_later_page_selection() {
    // New こうしゃ must not displace any of the four existing こう word
    // completions or promote page-two choices by changing the six-row budget.
    for seed in ["kanji", "KANJI", "かんじ"] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "漢字");
        assert!(
            index(&ime, "漢字", CandidateKind::Word) < index(&ime, "感じ", CandidateKind::Word)
        );
    }
    for seed in ["kou", "KOU", "こう"] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        let candidate = index(&ime, "候補", CandidateKind::Word);
        let update = index(&ime, "更新", CandidateKind::Word);
        let park = selectable_index(&ime, "公園", CandidateKind::Word);
        let crossing = selectable_index(&ime, "交差点", CandidateKind::Word);
        assert!(candidate < update && update < park && park < crossing);
        assert!((PAGE_SIZE..12).contains(&park));
        assert!((PAGE_SIZE..12).contains(&crossing));
        for sentence in ["公園で少し休みませんか。", "公園まで歩いてみましょう。"]
        {
            index(&ime, sentence, CandidateKind::Sentence);
        }
        commit_index_once_and_undo(&mut ime, crossing, "交差点");
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn transit_horizontal_separators_and_n_apostrophes_preserve_literal_spelling() {
    for (seed, word, sentence) in [
        ("den sha", "電車", "電車は何時に出発しますか。"),
        ("BA\u{3000}SU", "バス", "バスはここに止まりますか。"),
        ("ちか てつ", "地下鉄", "地下鉄の入口はどこですか。"),
        ("\tnori ba\t", "乗り場", "乗り場を教えてください。"),
        ("  JOU SHA KEN", "乗車券", "乗車券はどこで買えますか。"),
        ("UN’CHIN", "運賃", "運賃はいくらですか。"),
        ("しゅっ ぱつ", "出発", "出発は何時ですか。"),
        ("mokute kichi", "目的地", "目的地まで何分かかりますか。"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for (seed, word) in [
        ("den'sha", "電車"),
        ("UN’KYUU", "運休"),
        ("joushaken'", "乗車券"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn transit_adoption_space_restore_and_particles_remain_an_editable_draft() {
    for (reading, word, tail, sentence) in [
        ("densha", "電車", "HA", "電車は何時に出発しますか。"),
        ("basu", "バス", "de", "バスで駅まで行きたいです。"),
        ("chikatetsu", "地下鉄", "の", "地下鉄の入口はどこですか。"),
        ("noriba", "乗り場", "wo", "乗り場を教えてください。"),
        (
            "joushaken",
            "乗車券",
            "wo",
            "乗車券を確認してから乗りましょう。",
        ),
        ("unchin", "運賃", "HA", "運賃はいくらですか。"),
        ("shuppatsu", "出発", "suru", "出発する前に連絡します。"),
        (
            "mokutekichi",
            "目的地",
            "に",
            "目的地に着いたら教えてください。",
        ),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Pure-engine adoption equivalent, not a physical host-key receipt.
        let spaced = format!("{word} ");
        ime.seed(&spaced);
        check_draft(&ime, &spaced);
        index(&ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&spaced), &spaced, CandidateKind::Literal);
        ime.seed(reading);
        check_draft(&ime, reading);
        index(&ime, word, CandidateKind::Word);
        let continued = format!("{word} {tail}");
        ime.seed(&continued);
        check_draft(&ime, &continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&continued), &continued, CandidateKind::Literal);
    }
}

#[test]
fn transit_keeps_unknown_text_and_never_joins_control_boundaries() {
    let mut seeds = vec![
        "xyzdensha".to_owned(),
        "🙂電車は".into(),
        "https://joushaken".into(),
        "地下鉄nixyz".into(),
        "バスをが".into(),
        "目的地に🙂".into(),
    ];
    let boundaries = [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ];
    for boundary in boundaries {
        for (head, tail) in [("den'", "sha"), ("電車", "ha"), ("jou", "shaken")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        check_draft(&ime, &seed);
        for candidate in ime.candidates() {
            for &(_, sentences) in SENTENCES {
                for sentence in sentences {
                    assert!(
                        !candidate.text.ends_with(sentence),
                        "manufactured {sentence:?} from {seed:?}: {:?}",
                        ime.candidates()
                    );
                }
            }
            for boundary in boundaries {
                if seed.contains(boundary) {
                    assert!(candidate.text.contains(boundary));
                }
            }
        }
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-transit-synthetic-error"
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
fn transit_local_candidates_survive_typed_synthetic_failures_and_empty_replies() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("densha", "電車", "電車は何時に出発しますか。"),
            ("joushaken", "乗車券", "乗車券はどこで買えますか。"),
            ("mokutekichi", "目的地", "目的地まで何分かかりますか。"),
        ] {
            let mut ime = blank_engine();
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
            ime.seed(seed);
            check_draft(&ime, seed);
            index(&ime, word, CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
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
            check_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}
