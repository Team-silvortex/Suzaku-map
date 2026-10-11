//! Independent, authored Japanese location, direction and meeting fixtures.
//! These finite presets do not read personal location or input history, call a
//! live model or promise general Japanese morphology or route planning.
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
    ("basho", "ばしょ", "場所"),
    ("ichi", "いち", "位置"),
    ("houkou", "ほうこう", "方向"),
    ("chizu", "ちず", "地図"),
    ("mejirushi", "めじるし", "目印"),
    ("migi", "みぎ", "右"),
    ("hidari", "ひだり", "左"),
    ("menomae", "めのまえ", "目の前"),
    ("ushiro", "うしろ", "後ろ"),
    ("tonari", "となり", "隣"),
    ("chikaku", "ちかく", "近く"),
    ("mukai", "むかい", "向かい"),
    ("ue", "うえ", "上"),
    ("shita", "した", "下"),
    ("chuuou", "ちゅうおう", "中央"),
    ("iriguchi", "いりぐち", "入口"),
    ("deguchi", "でぐち", "出口"),
    ("kousaten", "こうさてん", "交差点"),
    ("michijun", "みちじゅん", "道順"),
    ("michiannai", "みちあんない", "道案内"),
    ("gouryuu", "ごうりゅう", "合流"),
    ("gouryuuchiten", "ごうりゅうちてん", "合流地点"),
    ("touchaku", "とうちゃく", "到着"),
    ("shuugou", "しゅうごう", "集合"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "場所",
        [
            "場所を地図で確認しましょう。",
            "場所が分かったら教えてください。",
        ],
    ),
    (
        "位置",
        [
            "位置を地図で教えてください。",
            "位置がずれていたら、もう一度確認しましょう。",
        ],
    ),
    (
        "方向",
        [
            "方向が分からなければ、ここで確認しましょう。",
            "方向を確かめてから歩きましょう。",
        ],
    ),
    (
        "地図",
        [
            "地図を見ながら一緒に行きましょう。",
            "地図に待ち合わせの場所を示してください。",
        ],
    ),
    (
        "目印",
        [
            "目印になる建物を教えてください。",
            "目印の前で待っています。",
        ],
    ),
    (
        "右",
        [
            "右に曲がると入口があります。",
            "右の階段を上がってください。",
        ],
    ),
    (
        "左",
        [
            "左に曲がってから、少し進んでください。",
            "左の入口で待っています。",
        ],
    ),
    (
        "目の前",
        [
            "目の前にある入口で待っています。",
            "目の前の交差点を右に曲がってください。",
        ],
    ),
    (
        "後ろ",
        [
            "後ろの入口に回ってください。",
            "後ろを振り返ると目印が見えます。",
        ],
    ),
    (
        "隣",
        [
            "隣の建物の前で待っています。",
            "隣にいるので、声をかけてください。",
        ],
    ),
    (
        "近く",
        [
            "近くまで来たら連絡してください。",
            "近くに目印になる建物はありますか。",
        ],
    ),
    (
        "向かい",
        [
            "向かいの入口で合流しましょう。",
            "向かいに着いたら連絡してください。",
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
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind, first_page: bool) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    if first_page {
        assert!(
            index < PAGE_SIZE,
            "not page one {text:?}, seed={:?}: {:?}",
            ime.snapshot().seed_text,
            ime.candidates()
        );
    }
    index
}
fn commit_once_and_undo(
    ime: &mut XRTabletImeEngine,
    text: &str,
    kind: CandidateKind,
    first_page: bool,
) {
    let selected = index(ime, text, kind, first_page);
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
fn location_preserves_all_226_previous_readings_and_114_contexts_in_full_order() {
    // Captured before this expansion. Includes courtesy, every kind/separator
    // flag and all 227 old sentences; later append-only expansions are allowed.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 226);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(226) {
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
    assert_eq!(readings, 0x2945684641315ca5);
    assert!(ja.continuations().count() >= 114);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut previous_sentence_count = 0;
    for (context, sentences) in ja.continuations().take(114) {
        previous_sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(previous_sentence_count, 227);
    assert_eq!(contexts, 0xfe1e5f944bcc6c83);
}

#[test]
fn location_additions_match_the_independent_authored_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 12);
    assert!(ja.readings().len() >= 250);
    assert!(ja.continuations().count() >= 126);
    assert_eq!(
        ja.continuations()
            .take(126)
            .map(|(_, sentences)| sentences.len())
            .sum::<usize>(),
        251
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(226).take(24).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(114).take(12).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn complete_location_words_support_romaji_uppercase_and_hiragana_on_page_one() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let mut ime = engine(&seed);
            check_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word, true);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal, false);
        }
    }
}

#[test]
fn complete_location_readings_offer_words_and_both_authored_sentences() {
    for &(word, sentences) in SENTENCES {
        let &(romaji, kana, _) = WORDS.iter().find(|(_, _, text)| *text == word).unwrap();
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            check_draft(&ime, &seed);
            index(&ime, word, CandidateKind::Word, true);
            for sentence in sentences {
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence, true);
            }
        }
    }
}

#[test]
fn specific_partial_location_words_stay_reachable_without_reordering_old_words() {
    // Specific fixtures rather than all one-kana prefixes. In particular the
    // broad う keeps the established 受け取り/受付/運動/歌 slots, not new 上.
    for (seed, word, first_page) in [
        ("bash", "場所", true),
        ("ich", "位置", true),
        ("houko", "方向", true),
        ("chiz", "地図", true),
        ("mejirush", "目印", true),
        ("mig", "右", true),
        ("hidar", "左", true),
        ("menoma", "目の前", true),
        ("ushir", "後ろ", true),
        ("tonar", "隣", true),
        ("chikak", "近く", true),
        ("muka", "向かい", true),
        ("shit", "下", false),
        ("chuuo", "中央", true),
        ("iriguch", "入口", true),
        ("deguch", "出口", true),
        ("kousate", "交差点", true),
        ("michiju", "道順", true),
        ("michianna", "道案内", true),
        ("gouryu", "合流", true),
        ("gouryuuchite", "合流地点", true),
        ("touchak", "到着", true),
        ("shuugo", "集合", true),
        ("ばし", "場所", true),
        ("ほうこ", "方向", true),
        ("めじる", "目印", true),
        ("ひだ", "左", true),
        ("めのま", "目の前", true),
        ("うし", "後ろ", true),
        ("とな", "隣", true),
        ("ちか", "近く", true),
        ("むか", "向かい", true),
        ("いりぐ", "入口", true),
        ("でぐ", "出口", true),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word, first_page);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word, first_page);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn specific_partial_location_readings_keep_both_authored_sentence_branches() {
    for (seed, word, first_page) in [
        ("bash", "場所", true),
        ("ich", "位置", true),
        ("houko", "方向", false),
        ("chiz", "地図", true),
        ("mejirush", "目印", true),
        ("mig", "右", true),
        ("hidar", "左", true),
        ("menoma", "目の前", true),
        ("ushir", "後ろ", true),
        ("tonar", "隣", true),
        ("chikak", "近く", true),
        ("muka", "向かい", true),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word, true);
        let &(_, sentences) = SENTENCES.iter().find(|(text, _)| *text == word).unwrap();
        for sentence in sentences {
            commit_once_and_undo(
                &mut engine(seed),
                sentence,
                CandidateKind::Sentence,
                first_page,
            );
        }
    }
}

#[test]
fn horizontal_location_separators_and_n_boundaries_preserve_raw_spelling() {
    for (seed, word, sentence) in [
        ("ba sho", "場所", "場所を地図で確認しましょう。"),
        (
            "CHI\u{3000}ZU",
            "地図",
            "地図を見ながら一緒に行きましょう。",
        ),
        ("めじ るし", "目印", "目印になる建物を教えてください。"),
        ("  hi dari\t", "左", "左の入口で待っています。"),
        (
            "\tme\tno\u{3000}mae",
            "目の前",
            "目の前にある入口で待っています。",
        ),
        ("u shiro", "後ろ", "後ろを振り返ると目印が見えます。"),
        ("ち かく", "近く", "近くまで来たら連絡してください。"),
        ("mu kai", "向かい", "向かいの入口で合流しましょう。"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word, true);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence, true);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
    for (seed, word) in [
        ("michi an'nai", "道案内"),
        ("MICHIAN’NAI", "道案内"),
        ("こう さてん", "交差点"),
        ("KOUSATENN", "交差点"),
        ("kousaten'", "交差点"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word, true);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn location_preserves_old_homophones_and_kou_words_with_a_new_later_page_word() {
    for seed in ["kanji", "KANJI", "かんじ"] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "漢字");
        assert!(
            index(&ime, "漢字", CandidateKind::Word, true)
                < index(&ime, "感じ", CandidateKind::Word, true)
        );
    }
    for seed in ["kou", "KOU", "こう"] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        let candidate = index(&ime, "候補", CandidateKind::Word, true);
        let update = index(&ime, "更新", CandidateKind::Word, true);
        let park = index(&ime, "公園", CandidateKind::Word, false);
        let crossing = index(&ime, "交差点", CandidateKind::Word, false);
        assert!(candidate < update && update < park && park < crossing);
        assert!((PAGE_SIZE..12).contains(&park));
        assert!((PAGE_SIZE..12).contains(&crossing));
        for sentence in ["公園で少し休みませんか。", "公園まで歩いてみましょう。"]
        {
            index(&ime, sentence, CandidateKind::Sentence, true);
        }
        commit_once_and_undo(&mut ime, "交差点", CandidateKind::Word, false);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn location_adoption_space_restore_and_particles_remain_an_editable_draft() {
    for (reading, word, tail, sentence) in [
        ("basho", "場所", "wo", "場所を地図で確認しましょう。"),
        (
            "ichi",
            "位置",
            "GA",
            "位置がずれていたら、もう一度確認しましょう。",
        ),
        ("houkou", "方向", "を", "方向を確かめてから歩きましょう。"),
        (
            "chizu",
            "地図",
            "ni",
            "地図に待ち合わせの場所を示してください。",
        ),
        (
            "mejirushi",
            "目印",
            "ni",
            "目印になる建物を教えてください。",
        ),
        ("migi", "右", "NI", "右に曲がると入口があります。"),
        ("hidari", "左", "no", "左の入口で待っています。"),
        (
            "menomae",
            "目の前",
            "ni",
            "目の前にある入口で待っています。",
        ),
        ("ushiro", "後ろ", "を", "後ろを振り返ると目印が見えます。"),
        ("tonari", "隣", "ni", "隣にいるので、声をかけてください。"),
        (
            "chikaku",
            "近く",
            "made",
            "近くまで来たら連絡してください。",
        ),
        (
            "mukai",
            "向かい",
            "に",
            "向かいに着いたら連絡してください。",
        ),
    ] {
        let mut ime = engine(reading);
        let selected = index(&ime, word, CandidateKind::Word, true);
        ime.select_candidate(selected);
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Pure-engine adoption equivalent, not physical host key delivery.
        let spaced = format!("{word} ");
        ime.seed(&spaced);
        check_draft(&ime, &spaced);
        index(&ime, sentence, CandidateKind::Sentence, true);
        commit_once_and_undo(&mut engine(&spaced), &spaced, CandidateKind::Literal, false);
        ime.seed(reading);
        check_draft(&ime, reading);
        index(&ime, word, CandidateKind::Word, true);
        let continued = format!("{word} {tail}");
        ime.seed(&continued);
        check_draft(&ime, &continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence, true);
        commit_once_and_undo(
            &mut engine(&continued),
            &continued,
            CandidateKind::Literal,
            false,
        );
    }
}

#[test]
fn location_keeps_unknown_text_and_never_joins_control_boundaries() {
    let mut seeds = vec![
        "xyzchizu".to_owned(),
        "🙂場所を".into(),
        "https://mejirushi".into(),
        "右nixyz".into(),
        "左をが".into(),
        "向かいに🙂".into(),
    ];
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (head, tail) in [("chi", "zu"), ("場所", "wo"), ("meno", "mae")] {
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
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                if seed.contains(boundary) {
                    assert!(candidate.text.contains(boundary));
                }
            }
        }
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal, false);
    }
}

struct Unavailable(LlmProviderError);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-location-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Err(self.0.clone())
    }
}
#[test]
fn location_local_candidates_survive_typed_synthetic_provider_failures() {
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            ("basho", "場所", "場所を地図で確認しましょう。"),
            ("chizu", "地図", "地図を見ながら一緒に行きましょう。"),
            ("menomae", "目の前", "目の前にある入口で待っています。"),
        ] {
            let mut ime = blank_engine();
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
            ime.seed(seed);
            check_draft(&ime, seed);
            index(&ime, word, CandidateKind::Word, true);
            index(&ime, sentence, CandidateKind::Sentence, true);
            let local = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(ime.prediction_error(), Some(&error));
            assert_eq!(ime.candidates(), local);
            check_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence, true);
        }
    }
}
