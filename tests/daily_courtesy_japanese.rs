//! Fixed, project-authored Japanese thanks, apologies and courteous requests.
//! Fixtures are independent of production JSON. No live model, personal input,
//! desktop key injection or general Japanese morphological analysis is used.
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
    ("kansha", "かんしゃ", "感謝"),
    ("orei", "おれい", "お礼"),
    ("kokorodukai", "こころづかい", "心遣い"),
    ("okokorodukai", "おこころづかい", "お心遣い"),
    (
        "moushiwakearimasen",
        "もうしわけありません",
        "申し訳ありません",
    ),
    ("shitsureishimashita", "しつれいしました", "失礼しました"),
    ("otesuu", "おてすう", "お手数"),
    ("onegai", "おねがい", "お願い"),
    ("tetsudatte", "てつだって", "手伝って"),
    ("ryoukai", "りょうかい", "了解"),
    ("shouchishimashita", "しょうちしました", "承知しました"),
    ("okidukainaku", "おきづかいなく", "お気遣いなく"),
    ("goshinsetsu", "ごしんせつ", "ご親切"),
    ("gokyouryoku", "ごきょうりょく", "ご協力"),
    ("omachi", "おまち", "お待ち"),
    ("oshirase", "おしらせ", "お知らせ"),
    ("owabi", "おわび", "お詫び"),
    ("shazai", "しゃざい", "謝罪"),
    ("hairyo", "はいりょ", "配慮"),
    ("yoroshikereba", "よろしければ", "よろしければ"),
    (
        "sashitsukaenakereba",
        "さしつかえなければ",
        "差し支えなければ",
    ),
    (
        "yoroshikuonegaishimasu",
        "よろしくおねがいします",
        "よろしくお願いします",
    ),
    ("douitashimashite", "どういたしまして", "どういたしまして"),
    ("okage", "おかげ", "おかげ"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "感謝",
        [
            "感謝しています。とても助かりました。",
            "感謝の気持ちを伝えたいです。",
        ],
    ),
    (
        "お礼",
        ["お礼を伝えたくて連絡しました。", "お礼を言わせてください。"],
    ),
    (
        "心遣い",
        [
            "心遣いに感謝しています。",
            "心遣いがとてもうれしかったです。",
        ],
    ),
    (
        "お心遣い",
        [
            "お心遣いありがとうございます。",
            "お心遣いに感謝しています。",
        ],
    ),
    (
        "申し訳ありません",
        [
            "申し訳ありません、確認が遅くなりました。",
            "申し訳ありません、もう一度確認します。",
        ],
    ),
    (
        "失礼しました",
        [
            "失礼しました、言い方を間違えました。",
            "失礼しました、こちらで確認し直します。",
        ],
    ),
    (
        "お手数",
        [
            "お手数ですが、もう一度教えてください。",
            "お手数をおかけして申し訳ありません。",
        ],
    ),
    (
        "お願い",
        [
            "お願いがあるのですが、少し時間をいただけますか。",
            "お願いしてもよろしいですか。",
        ],
    ),
    (
        "手伝って",
        ["手伝っていただけると助かります。", "手伝ってもらえますか。"],
    ),
    (
        "了解",
        [
            "了解しました、後で確認します。",
            "了解です、ありがとうございます。",
        ],
    ),
    (
        "承知しました",
        [
            "承知しました、そのように進めます。",
            "承知しました、確認してから連絡します。",
        ],
    ),
    (
        "お気遣いなく",
        [
            "お気遣いなく、こちらで大丈夫です。",
            "お気遣いなく、どうぞ先に進めてください。",
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
fn word_kind(seed: &str, word: &str) -> CandidateKind {
    if seed == word {
        CandidateKind::Literal
    } else {
        CandidateKind::Word
    }
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
fn courtesy_preserves_all_previous_readings_contexts_and_sentence_order() {
    // Full pre-expansion snapshot: retain spelling, type, separator flags,
    // homophone order and all 203 sentences under the original 102 contexts.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 202);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(202) {
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
    assert_eq!(readings, 0x6af75189fa8eb026);
    assert!(ja.continuations().count() >= 102);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut previous_sentence_count = 0;
    for (context, sentences) in ja.continuations().take(102) {
        previous_sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(previous_sentence_count, 203);
    assert_eq!(contexts, 0x3410b42b82a64895);
}

#[test]
fn courtesy_appends_exactly_the_authored_readings_and_independent_contexts() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 12);
    // Keep the courtesy window exact while allowing later independent batches.
    // The current catalog totals have their own strict resource assertions.
    let readings = &ja.readings()[202..226];
    let contexts = ja.continuations().skip(102).take(12).collect::<Vec<_>>();
    assert_eq!(readings.len(), 24);
    assert_eq!(contexts.len(), 12);
    assert_eq!(
        ja.continuations()
            .take(114)
            .map(|(_, sentences)| sentences.len())
            .sum::<usize>(),
        227
    );
    for (entry, &(_, kana, word)) in readings.iter().zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in contexts.into_iter().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn complete_courtesy_words_support_romaji_uppercase_and_hiragana() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let mut ime = engine(&seed);
            check_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, word_kind(&seed, word), true);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal, false);
        }
    }
}

#[test]
fn complete_courtesy_readings_offer_words_and_both_authored_sentences() {
    for &(word, sentences) in SENTENCES {
        let &(romaji, kana, _) = WORDS.iter().find(|(_, _, text)| *text == word).unwrap();
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            check_draft(&ime, &seed);
            index(&ime, word, word_kind(&seed, word), true);
            for sentence in sentences {
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence, true);
            }
        }
    }
}

#[test]
fn specific_partial_courtesy_readings_keep_word_and_sentence_choices() {
    // Fixed specific prefixes. おね is separately tested with the older
    // お願いします context, without promising every broad prefix page one.
    for (seed, word) in [
        ("kansh", "感謝"),
        ("ore", "お礼"),
        ("kokoroduka", "心遣い"),
        ("okokoroduka", "お心遣い"),
        ("moushiwakearimase", "申し訳ありません"),
        ("shitsureishimashit", "失礼しました"),
        ("otesu", "お手数"),
        ("tetsudatt", "手伝って"),
        ("ryouka", "了解"),
        ("shouchishimashit", "承知しました"),
        ("okidukainak", "お気遣いなく"),
        ("かんし", "感謝"),
        ("おれ", "お礼"),
        ("こころづか", "心遣い"),
        ("おこころづか", "お心遣い"),
        ("もうしわけありませ", "申し訳ありません"),
        ("しつれいしまし", "失礼しました"),
        ("おてす", "お手数"),
        ("てつだっ", "手伝って"),
        ("りょうか", "了解"),
        ("しょうちしまし", "承知しました"),
        ("おきづかいな", "お気遣いなく"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word, true);
        let &(_, sentences) = SENTENCES.iter().find(|(text, _)| *text == word).unwrap();
        for sentence in sentences {
            commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence, true);
        }
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn courtesy_horizontal_separators_preserve_raw_spelling_and_choices() {
    for (seed, word, sentence) in [
        ("kan sha", "感謝", "感謝の気持ちを伝えたいです。"),
        ("O\u{3000}REI", "お礼", "お礼を言わせてください。"),
        ("こころ づかい", "心遣い", "心遣いに感謝しています。"),
        (
            "  o te suu\t",
            "お手数",
            "お手数ですが、もう一度教えてください。",
        ),
        (
            "\tmoushi\twake\u{3000}arimasen",
            "申し訳ありません",
            "申し訳ありません、もう一度確認します。",
        ),
        (
            "MOUSHIWAKEARIMASEN’",
            "申し訳ありません",
            "申し訳ありません、確認が遅くなりました。",
        ),
        (
            "moushiwakearimasen'",
            "申し訳ありません",
            "申し訳ありません、もう一度確認します。",
        ),
        ("tetsu datte", "手伝って", "手伝ってもらえますか。"),
        ("りょう かい", "了解", "了解です、ありがとうございます。"),
        (
            "shou chi shimashita",
            "承知しました",
            "承知しました、そのように進めます。",
        ),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word, true);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence, true);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn courtesy_preserves_existing_homophones_and_broad_request_priority() {
    for seed in ["kanji", "KANJI", "かんじ"] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "漢字");
        assert!(
            index(&ime, "漢字", CandidateKind::Word, true)
                < index(&ime, "感じ", CandidateKind::Word, true)
        );
    }
    for seed in ["oneg", "ONEG", "おね"] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        let old = index(&ime, "お願いします", CandidateKind::Word, true);
        let new = index(&ime, "お願い", CandidateKind::Word, true);
        assert!(old < new);
        for sentence in [
            "お願いします。終わったら教えてください。",
            "お願いします。準備ができたら連絡してください。",
        ] {
            index(&ime, sentence, CandidateKind::Sentence, true);
        }
        for sentence in [
            "お願いがあるのですが、少し時間をいただけますか。",
            "お願いしてもよろしいですか。",
        ] {
            let later = index(&ime, sentence, CandidateKind::Sentence, false);
            assert!((PAGE_SIZE..12).contains(&later));
            commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence, false);
        }
        commit_once_and_undo(&mut ime, "お願い", CandidateKind::Word, true);
    }
}

#[test]
fn broad_ryou_readings_keep_old_page_one_and_new_word_on_a_later_page() {
    for seed in ["ryou", "RYOU", "りょう"] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        let both = index(&ime, "両方", CandidateKind::Word, true);
        let cooking = index(&ime, "料理", CandidateKind::Word, true);
        let understood = index(&ime, "了解", CandidateKind::Word, false);
        assert!(both < cooking && cooking < understood);
        assert!((PAGE_SIZE..12).contains(&understood));
        for sentence in [
            "料理を一緒に作りませんか。",
            "料理ができたら呼んでください。",
        ] {
            index(&ime, sentence, CandidateKind::Sentence, true);
        }
        commit_once_and_undo(&mut ime, "了解", CandidateKind::Word, false);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn courtesy_adoption_space_restore_and_continued_input_stay_editable() {
    for (reading, word, tail, sentence) in [
        (
            "kansha",
            "感謝",
            "shite",
            "感謝しています。とても助かりました。",
        ),
        ("orei", "お礼", "wo", "お礼を伝えたくて連絡しました。"),
        ("kokorodukai", "心遣い", "ni", "心遣いに感謝しています。"),
        (
            "okokorodukai",
            "お心遣い",
            "NI",
            "お心遣いに感謝しています。",
        ),
        (
            "moushiwakearimasen",
            "申し訳ありません",
            "、",
            "申し訳ありません、確認が遅くなりました。",
        ),
        (
            "shitsureishimashita",
            "失礼しました",
            "、",
            "失礼しました、言い方を間違えました。",
        ),
        (
            "otesuu",
            "お手数",
            "desuga",
            "お手数ですが、もう一度教えてください。",
        ),
        (
            "onegai",
            "お願い",
            "ga",
            "お願いがあるのですが、少し時間をいただけますか。",
        ),
        (
            "tetsudatte",
            "手伝って",
            "itadake",
            "手伝っていただけると助かります。",
        ),
        (
            "ryoukai",
            "了解",
            "desu",
            "了解です、ありがとうございます。",
        ),
        (
            "shouchishimashita",
            "承知しました",
            "、",
            "承知しました、そのように進めます。",
        ),
        (
            "okidukainaku",
            "お気遣いなく",
            "、",
            "お気遣いなく、こちらで大丈夫です。",
        ),
    ] {
        let mut ime = engine(reading);
        let selected = index(&ime, word, CandidateKind::Word, true);
        ime.select_candidate(selected);
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Engine-level draft replacement mirrors adoption. No host key event
        // is sent; a reading separator never implicitly commits the draft.
        let spaced = format!("{word} ");
        ime.seed(&spaced);
        check_draft(&ime, &spaced);
        index(&ime, sentence, CandidateKind::Sentence, true);
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
fn courtesy_keeps_unknown_text_and_never_joins_control_boundaries() {
    let mut seeds = vec![
        "xyzkansha".to_owned(),
        "🙂お礼を".into(),
        "https://otesuu".into(),
        "感謝shiteXYZ".into(),
        "お願いをが".into(),
        "申し訳ありません🙂".into(),
    ];
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (head, tail) in [("kan", "sha"), ("お礼", "wo"), ("moushi", "wakearimasen")] {
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
        "ja-courtesy-synthetic-error"
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
fn courtesy_candidates_survive_typed_synthetic_provider_failures() {
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            ("kansha", "感謝", "感謝の気持ちを伝えたいです。"),
            (
                "moushiwakearimasen",
                "申し訳ありません",
                "申し訳ありません、もう一度確認します。",
            ),
            ("onegai", "お願い", "お願いしてもよろしいですか。"),
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
