//! Authored file/photo sharing vocabulary; no files are sent or accounts contacted.
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

// Frozen against the 486-reading baseline before this batch is appended.
// Compound words retain the established Kana/Romaji rules and optional base words.
const WORDS: &[(&str, &str, &str)] = &[
    ("fairumei", "ふぁいるめい", "ファイル名"),
    ("tenpufairu", "てんぷふぁいる", "添付ファイル"),
    ("kyouyuurinku", "きょうゆうりんく", "共有リンク"),
    ("kakuchoushi", "かくちょうし", "拡張子"),
    ("gamenkyouyuu", "がめんきょうゆう", "画面共有"),
    ("saisoushin", "さいそうしん", "再送信"),
    ("akusesuken", "あくせすけん", "アクセス権"),
    ("hozonkeishiki", "ほぞんけいしき", "保存形式"),
    ("henshuurireki", "へんしゅうりれき", "編集履歴"),
    ("henkourireki", "へんこうりれき", "変更履歴"),
    ("gazousaizu", "がぞうさいず", "画像サイズ"),
    ("kaizoudo", "かいぞうど", "解像度"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "拡張子",
        [
            "拡張子を含めたファイル名を教えてください。",
            "拡張子が違うので、別の形式で送ってもらえますか。",
        ],
    ),
    (
        "画面共有",
        ["画面共有を始めてもいいですか。", "画面共有を一度止めます。"],
    ),
    (
        "アクセス権",
        [
            "アクセス権を確認してもらえますか。",
            "アクセス権がないのでファイルを開けません。",
        ],
    ),
    (
        "保存形式",
        [
            "保存形式を選んでから送ります。",
            "保存形式はどれにすればいいですか。",
        ],
    ),
    (
        "編集履歴",
        [
            "編集履歴から変更前の内容を見られますか。",
            "編集履歴に変更した箇所が残っています。",
        ],
    ),
    (
        "解像度",
        [
            "解像度を下げてから送ります。",
            "解像度が低いので、文字が読みにくいです。",
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
fn sharing_fixed_readings_romaji_rules_and_authored_append_windows() {
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    for &(romaji, kana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
    }
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 498 && ja.continuations().count() >= 250);
    for (entry, &(_, kana, word)) in ja.readings()[486..498].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(244).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(244)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        487
    );
    assert_eq!(
        ja.continuations()
            .take(250)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        499
    );
}

#[test]
fn sharing_full_romaji_uppercase_and_kana_commit_words_sentences_or_raw_once() {
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
fn sharing_six_specific_partials_and_existing_sharing_resend_photo_flows() {
    for (seed, word) in [
        ("kakuchoush", "拡張子"),
        ("gamenkyouyu", "画面共有"),
        ("akusesuke", "アクセス権"),
        ("hozonkeishik", "保存形式"),
        ("henshuurirek", "編集履歴"),
        ("kaizoud", "解像度"),
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
        ("kyouyuu", "共有"),
        ("kyouyu", "共有"),
        ("saisou", "再送"),
        ("saiso", "再送"),
        ("shashin", "写真"),
        ("shashi", "写真"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        let sentences: &[&str] = match word {
            "共有" => &["共有ありがとうございます。", "共有する前に確認しましょう。"],
            "写真" => &["写真を撮ってもいいですか。", "写真を後で送ります。"],
            _ => &[],
        };
        for &sentence in sentences {
            // The existing kyouyu segmentation keeps 今日ゆ first; its second
            // 共有 sentence has always been selectable beyond the first page.
            if seed != "kyouyu" || sentence != "共有する前に確認しましょう。" {
                first_page(&ime, sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
        if seed == "kyouyu" {
            assert_eq!(ime.candidates()[0].text, "今日ゆ");
        }
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn sharing_horizontal_readings_and_adopted_particle_progress_remain_editable() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("kaku", "CHOUSHI", "拡張子"),
            ("gamen", "KYOUYUU", "画面共有"),
            ("akusesu", "KEN", "アクセス権"),
            ("hozon", "KEISHIKI", "保存形式"),
            ("henshuu", "RIREKI", "編集履歴"),
            ("kai", "ZOUDO", "解像度"),
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
        ("拡張子", "wo", SENTENCES[0].1[0]),
        ("画面共有", "WO", SENTENCES[1].1[1]),
        ("アクセス権", "ga", SENTENCES[2].1[1]),
        ("保存形式", "は", SENTENCES[3].1[1]),
        ("編集履歴", "kara", SENTENCES[4].1[0]),
        ("解像度", "が", SENTENCES[5].1[1]),
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
fn sharing_unknown_urls_and_line_boundaries_keep_raw_text_without_authored_leaks() {
    for seed in [
        "https://kaizoudo.example.invalid",
        "henshuurireki@example.invalid",
        "xyzkakuchoushi",
        "保存形式dexyz",
        "🙂画面共有wo",
        "gamen\nkyouyuu",
        "アクセス権\r\nwo",
        "拡張子\u{2028}ga",
        "解像度\u{2029}wo",
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

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-sharing-synthetic-model-error"
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
fn sharing_model_unavailable_timeout_and_empty_preserve_all_six_local_contexts() {
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
