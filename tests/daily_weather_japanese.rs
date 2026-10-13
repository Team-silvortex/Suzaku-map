//! Authored weather and going-out vocabulary; no real device, forecast or service is used.
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

// Fixed against the 438-reading baseline before appending this batch. Long vowels,
// terminal n and doubled consonants use the existing converter unchanged.
const WORDS: &[(&str, &str, &str)] = &[
    ("tenkiyohou", "てんきよほう", "天気予報"),
    ("hare", "はれ", "晴れ"),
    ("kumori", "くもり", "曇り"),
    ("kosame", "こさめ", "小雨"),
    ("ameagari", "あめあがり", "雨上がり"),
    ("kion", "きおん", "気温"),
    ("hadazamui", "はだざむい", "肌寒い"),
    ("mushiatsui", "むしあつい", "蒸し暑い"),
    ("oritatamigasa", "おりたたみがさ", "折り畳み傘"),
    ("amagu", "あまぐ", "雨具"),
    ("mochiaruki", "もちあるき", "持ち歩き"),
    ("okibasho", "おきばしょ", "置き場所"),
    ("shitsudo", "しつど", "湿度"),
    ("hizashi", "ひざし", "日差し"),
    ("yuudachi", "ゆうだち", "夕立"),
    ("amayadori", "あまやどり", "雨宿り"),
    ("higasa", "ひがさ", "日傘"),
    ("nagagutsu", "ながぐつ", "長靴"),
    ("reinko-to", "れいんこーと", "レインコート"),
    ("bousui", "ぼうすい", "防水"),
    ("komono", "こもの", "小物"),
    ("kagitaba", "かぎたば", "鍵束"),
    ("poketto", "ぽけっと", "ポケット"),
    ("hokan", "ほかん", "保管"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "天気予報",
        [
            "天気予報を見てから出かけます。",
            "天気予報では午後から雨です。",
        ],
    ),
    (
        "晴れ",
        [
            "晴れの日に洗濯物を干します。",
            "晴れたら散歩に行きたいです。",
        ],
    ),
    (
        "曇り",
        [
            "曇りでも外は明るいですね。",
            "曇りの日は家でゆっくり過ごします。",
        ],
    ),
    (
        "小雨",
        [
            "小雨なので傘を持って行きます。",
            "小雨がやむまで少し待ちます。",
        ],
    ),
    (
        "雨上がり",
        [
            "雨上がりの道はまだ濡れています。",
            "雨上がりに公園を歩きました。",
        ],
    ),
    (
        "気温",
        [
            "気温を確認して上着を選びます。",
            "気温が下がってきましたね。",
        ],
    ),
    (
        "肌寒い",
        ["肌寒いので上着を羽織ります。", "肌寒い朝が増えてきました。"],
    ),
    (
        "蒸し暑い",
        [
            "蒸し暑い日は扇風機を使います。",
            "蒸し暑いので少し休みましょう。",
        ],
    ),
    (
        "折り畳み傘",
        [
            "折り畳み傘を鞄に入れました。",
            "折り畳み傘を忘れずに持って行きます。",
        ],
    ),
    (
        "雨具",
        [
            "雨具は玄関に置いてあります。",
            "雨具を乾かしてからしまいます。",
        ],
    ),
    (
        "持ち歩き",
        ["持ち歩きに便利な大きさです。", "持ち歩き用の袋に入れます。"],
    ),
    (
        "置き場所",
        [
            "置き場所を決めておきます。",
            "置き場所が分からなくなりました。",
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
fn weather_romaji_inputs_preserve_existing_kana_spelling_rules() {
    for &(romaji, kana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
    }
    for (seed, kana) in [
        ("kion", "きおん"),
        ("kionn", "きおん"),
        ("hokan", "ほかん"),
        ("hokann", "ほかん"),
        ("poketto", "ぽけっと"),
        ("reinko-to", "れいんこーと"),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    // oo spells おお; it is not the loanword long-vowel mark ー.
    assert_eq!(romaji_to_hiragana("reinkooto"), "れいんこおと");
}

#[test]
fn weather_preserve_all_438_prior_readings_220_contexts_and_439_sentences() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 438 && ja.continuations().count() >= 220);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(438) {
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
    assert_eq!(readings, 0x00285aa0491ca1e5);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut count = 0;
    for (context, sentences) in ja.continuations().take(220) {
        count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(count, 439);
    assert_eq!(contexts, 0x6e87085195448352);
}

#[test]
fn weather_match_fixed_24_readings_and_12_context_append_windows() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 12);
    assert!(ja.readings().len() >= 462 && ja.continuations().count() >= 232);
    for (entry, &(_, kana, word)) in ja.readings()[438..462].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(220).take(12).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(232)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        463
    );
}

#[test]
fn weather_all_full_romaji_case_and_kana_offer_words_sentences_and_literal() {
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
fn weather_specific_partials_keep_selectable_words_and_both_sentences() {
    // Explicit near-complete queries, not arbitrary short prefixes or a universal
    // first-page promise. Incomplete consonants use the existing bounded probes.
    for (seed, word) in [
        ("tenkiyoho", "天気予報"),
        ("har", "晴れ"),
        ("kumor", "曇り"),
        ("kosam", "小雨"),
        ("ameagar", "雨上がり"),
        ("きお", "気温"),
        ("hadazamu", "肌寒い"),
        ("mushiatsu", "蒸し暑い"),
        ("oritatamigas", "折り畳み傘"),
        ("amag", "雨具"),
        ("mochiaruk", "持ち歩き"),
        ("okibash", "置き場所"),
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
    for (seed, word) in [("kionn", "気温"), ("hokann", "保管")] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
    }
}

#[test]
fn weather_horizontal_separators_preserve_whole_readings_and_raw_text() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("tenki", "YOHOU", "天気予報"),
            ("oritatami", "GASA", "折り畳み傘"),
            ("rein", "KO-TO", "レインコート"),
            ("po", "ketto", "ポケット"),
        ] {
            let seed = format!("{separator}{left}{separator}{right}{separator}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, word, CandidateKind::Word);
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                }
            }
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn weather_adopted_context_particles_and_restoration_never_commit_early() {
    for (word, tail, sentence) in [
        ("天気予報", "wo", SENTENCES[0].1[0]),
        ("晴れ", "no", SENTENCES[1].1[0]),
        ("曇り", "NO", SENTENCES[2].1[1]),
        ("小雨", "が", SENTENCES[3].1[1]),
        ("雨上がり", "ni", SENTENCES[4].1[1]),
        ("気温", "GA", SENTENCES[5].1[1]),
        ("肌寒い", "node", SENTENCES[6].1[0]),
        ("蒸し暑い", "ので", SENTENCES[7].1[1]),
        ("折り畳み傘", "wo", SENTENCES[8].1[0]),
        ("雨具", "HA", SENTENCES[9].1[0]),
        ("持ち歩き", "ni", SENTENCES[10].1[0]),
        ("置き場所", "を", SENTENCES[11].1[0]),
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
fn weather_url_unknown_and_line_boundaries_do_not_leak_authored_sentences() {
    for seed in [
        "https://tenkiyohou.example.invalid",
        "kosame@example.invalid",
        "xyzkumori",
        "天気予報dexyz",
        "🙂気温ga",
        "oritatami\ngasa",
        "雨上がり\r\nni",
        "小雨\u{2028}ga",
        "置き場所\u{2029}wo",
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
fn weather_keep_prior_homophones_weather_umbrella_and_grammar() {
    let homophones = engine("kanji");
    assert_eq!(homophones.candidates()[0].text, "漢字");
    assert!(
        index(&homophones, "漢字", CandidateKind::Word)
            < index(&homophones, "感じ", CandidateKind::Word)
    );
    for (seed, word) in [
        ("tenki", "天気"),
        ("kasa", "傘"),
        ("uwagi", "上着"),
        ("mochimono", "持ち物"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, word);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
    }
    let weather = engine("tenki");
    for sentence in ["天気がいいですね。", "天気はどうですか？"] {
        first_page(&weather, sentence, CandidateKind::Sentence);
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
        "ja-weather-synthetic-error"
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
fn weather_all_twelve_contexts_survive_typed_and_empty_model_failures() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
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
