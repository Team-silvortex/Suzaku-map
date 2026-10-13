//! Authored visit, counter-service and parcel vocabulary; no real errands are performed.
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

// Fixed against the 414-reading baseline before the resource append. In particular,
// 小包 is こづつみ: the existing converter accepts `du` for づ, not `zu` for ず.
const WORDS: &[(&str, &str, &str)] = &[
    ("raikyaku", "らいきゃく", "来客"),
    ("ojamashimasu", "おじゃまします", "お邪魔します"),
    ("ojamashimashita", "おじゃましました", "お邪魔しました"),
    ("temiyage", "てみやげ", "手土産"),
    ("mukae", "むかえ", "迎え"),
    ("miokuri", "みおくり", "見送り"),
    ("youji", "ようじ", "用事"),
    ("madoguchi", "まどぐち", "窓口"),
    ("junban", "じゅんばん", "順番"),
    ("uketsukebangou", "うけつけばんごう", "受付番号"),
    ("shinseisho", "しんせいしょ", "申請書"),
    ("kinyuu", "きにゅう", "記入"),
    ("yuubinkyoku", "ゆうびんきょく", "郵便局"),
    ("saihaitatsu", "さいはいたつ", "再配達"),
    ("haisou", "はいそう", "配送"),
    ("hassou", "はっそう", "発送"),
    ("atena", "あてな", "宛名"),
    ("okurijou", "おくりじょう", "送り状"),
    ("kodutsumi", "こづつみ", "小包"),
    ("fuutou", "ふうとう", "封筒"),
    ("kitte", "きって", "切手"),
    ("yuubinbangou", "ゆうびんばんごう", "郵便番号"),
    ("okihai", "おきはい", "置き配"),
    ("takuhaibokkusu", "たくはいぼっくす", "宅配ボックス"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "来客",
        [
            "来客の予定があるので、少し早く帰ります。",
            "来客があるので、部屋を片付けます。",
        ],
    ),
    (
        "迎え",
        [
            "迎えに来てもらえますか。",
            "迎えに行くので、着いたら連絡してください。",
        ],
    ),
    (
        "手土産",
        [
            "手土産は何がよさそうですか。",
            "手土産を買ってから伺います。",
        ],
    ),
    (
        "用事",
        [
            "用事を済ませてから向かいます。",
            "用事が終わったら連絡します。",
        ],
    ),
    (
        "窓口",
        ["窓口はどこにありますか。", "窓口で手続きをしたいです。"],
    ),
    (
        "順番",
        [
            "順番が来たら教えてください。",
            "順番を待っているので、少し遅れます。",
        ],
    ),
    (
        "郵便局",
        [
            "郵便局に寄ってから帰ります。",
            "郵便局は何時まで開いていますか。",
        ],
    ),
    (
        "再配達",
        [
            "再配達をお願いしたいです。",
            "再配達の時間を変更できますか。",
        ],
    ),
    (
        "発送",
        ["発送はいつになりますか。", "発送が済んだら連絡します。"],
    ),
    (
        "宛名",
        [
            "宛名をここに書けばいいですか。",
            "宛名に間違いがないか確認します。",
        ],
    ),
    (
        "置き配",
        [
            "置き配をお願いできますか。",
            "置き配の場所を指定したいです。",
        ],
    ),
    (
        "宅配ボックス",
        [
            "宅配ボックスに入れてもらえますか。",
            "宅配ボックスがいっぱいなので、手渡しでお願いします。",
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
    assert!(ime.candidates().len() <= 12);
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
fn errands_romaji_inputs_preserve_existing_kana_spelling_rules() {
    for &(romaji, kana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
    }
    assert_eq!(romaji_to_hiragana("kozutsumi"), "こずつみ");
    assert_eq!(romaji_to_hiragana("kodutsumi"), "こづつみ");
}

#[test]
fn errands_preserve_all_414_prior_readings_208_contexts_and_415_sentences() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 414 && ja.continuations().count() >= 208);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(414) {
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
    assert_eq!(readings, 0xdea45e31a624f6fc);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut count = 0;
    for (context, sentences) in ja.continuations().take(208) {
        count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(count, 415);
    assert_eq!(contexts, 0x5187293b74d5595e);
}

#[test]
fn errands_match_fixed_24_readings_and_12_context_append_windows() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 12);
    assert!(ja.readings().len() >= 438 && ja.continuations().count() >= 220);
    for (entry, &(_, kana, word)) in ja.readings()[414..438].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(208).take(12).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(220)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        439
    );
}

#[test]
fn errands_all_full_romaji_case_and_kana_offer_words_sentences_and_literal() {
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
}

#[test]
fn errands_specific_partials_keep_selectable_words_and_both_sentences() {
    // Fixed near-complete prefixes, not arbitrary short-query/page-one promises.
    // むか still shares the earlier 向かい reading; use its bounded later page.
    for (seed, word) in [
        ("raikyak", "来客"),
        ("むか", "迎え"),
        ("temiyag", "手土産"),
        ("youj", "用事"),
        ("madoguch", "窓口"),
        ("junba", "順番"),
        ("yuubinkyok", "郵便局"),
        ("saihaitats", "再配達"),
        ("はっそ", "発送"),
        ("あて", "宛名"),
        ("okiha", "置き配"),
        ("takuhaibokkus", "宅配ボックス"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        for sentence in SENTENCES
            .iter()
            .find(|(context, _)| *context == word)
            .unwrap()
            .1
        {
            index(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn errands_horizontal_separators_preserve_whole_readings_and_raw_text() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("yuubin", "KYOKU", "郵便局"),
            ("uketsuke", "BANGOU", "受付番号"),
            ("takuHAI", "bokkusu", "宅配ボックス"),
            ("ko", "dutsumi", "小包"),
        ] {
            let seed = format!("{separator}{left}{separator}{right}{separator}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, word, CandidateKind::Word);
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn errands_adopted_context_particles_and_restoration_never_commit_early() {
    for (word, tail, sentence) in [
        ("来客", "ga", SENTENCES[0].1[1]),
        ("迎え", "に", SENTENCES[1].1[0]),
        ("手土産", "WO", SENTENCES[2].1[1]),
        ("用事", "が", SENTENCES[3].1[1]),
        ("窓口", "de", SENTENCES[4].1[1]),
        ("順番", "を", SENTENCES[5].1[1]),
        ("郵便局", "ni", SENTENCES[6].1[0]),
        ("再配達", "の", SENTENCES[7].1[1]),
        ("発送", "ha", SENTENCES[8].1[0]),
        ("宛名", "を", SENTENCES[9].1[0]),
        ("置き配", "NO", SENTENCES[10].1[1]),
        ("宅配ボックス", "が", SENTENCES[11].1[1]),
    ] {
        let reading = WORDS.iter().find(|(_, _, text)| *text == word).unwrap().0;
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        // Selection plus cumulative reseeding exercises core adoption/restoration,
        // not real native-window keys, timing or focus.
        for seed in [
            word.to_owned(),
            format!("{word} {tail}"),
            format!("{word}\u{3000}{tail}"),
        ] {
            ime.seed(&seed);
            draft(&ime, &seed);
            first_page(&ime, sentence, CandidateKind::Sentence);
            ime.seed(reading);
            draft(&ime, reading);
            index(&ime, word, CandidateKind::Word);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn errands_url_unknown_and_line_boundaries_do_not_leak_authored_sentences() {
    for seed in [
        "https://yuubinkyoku.example.invalid",
        "saihaitatsu@example.invalid",
        "xyzraikyaku",
        "窓口dexyz",
        "🙂発送ga",
        "taku\nhai bokkusu",
        "郵便局\r\nni",
        "来客\u{2028}ga",
        "宛名\u{2029}wo",
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

#[test]
fn errands_keep_prior_homophones_reception_receipt_absence_and_direction() {
    let homophones = engine("kanji");
    assert_eq!(homophones.candidates()[0].text, "漢字");
    assert!(
        index(&homophones, "漢字", CandidateKind::Word)
            < index(&homophones, "感じ", CandidateKind::Word)
    );
    for (seed, word) in [
        ("uketsuke", "受付"),
        ("uketori", "受け取り"),
        ("fuzai", "不在"),
        ("mukai", "向かい"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, word);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
    }
    for (seed, sentences) in [
        (
            "uketori",
            [
                "受け取りました、ありがとうございます。",
                "受け取りの確認をお願いします。",
            ],
        ),
        (
            "mukai",
            [
                "向かいの入口で合流しましょう。",
                "向かいに着いたら連絡してください。",
            ],
        ),
    ] {
        let ime = engine(seed);
        for sentence in sentences {
            first_page(&ime, sentence, CandidateKind::Sentence);
        }
    }
    let grammar = engine("日本語を勉強shitai");
    assert_eq!(grammar.candidates()[0].text, "日本語を勉強したい");
    first_page(
        &grammar,
        "日本語を勉強したいです。",
        CandidateKind::Sentence,
    );
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-errands-synthetic-error"
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
fn errands_all_twelve_contexts_survive_typed_and_empty_model_failures() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        Some(LlmProviderError::InvalidResponse),
        None,
    ] {
        for &(word, sentences) in SENTENCES {
            let reading = WORDS.iter().find(|(_, _, text)| *text == word).unwrap().0;
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
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
