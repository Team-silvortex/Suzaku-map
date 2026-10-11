//! Independent Japanese daily-routine and everyday-status fallback fixtures.
//! These finite original expressions do not track sleep or health, give medical
//! advice, read a calendar or personal history, or require a live model.
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
    ("hayaoki", "はやおき", "早起き"),
    ("yofukashi", "よふかし", "夜更かし"),
    ("nebusoku", "ねぶそく", "寝不足"),
    ("hiruyasumi", "ひるやすみ", "昼休み"),
    ("hitoyasumi", "ひとやすみ", "ひと休み"),
    ("shigotogaeri", "しごとがえり", "仕事帰り"),
    ("kibun", "きぶん", "気分"),
    ("nemui", "ねむい", "眠い"),
    ("mezamashi", "めざまし", "目覚まし"),
    ("mijitaku", "みじたく", "身支度"),
    ("nikka", "にっか", "日課"),
    ("nebou", "ねぼう", "寝坊"),
    ("inemuri", "いねむり", "居眠り"),
    ("hayane", "はやね", "早寝"),
    ("yorugata", "よるがた", "夜型"),
    ("asagata", "あさがた", "朝型"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "早起き",
        [
            "早起きしたので、少し散歩してきます。",
            "早起きは苦手ですが、明日は早く起きます。",
        ],
    ),
    (
        "夜更かし",
        [
            "夜更かししたので、今日は早めに寝ます。",
            "夜更かしはほどほどにしておきます。",
        ],
    ),
    (
        "寝不足",
        [
            "寝不足なので、今日は早めに帰ります。",
            "寝不足で少し眠いです。",
        ],
    ),
    (
        "昼休み",
        [
            "昼休みに少し外を歩きます。",
            "昼休みが終わったら連絡します。",
        ],
    ),
    (
        "ひと休み",
        [
            "ひと休みしてから続きを進めましょう。",
            "ひと休みしたら、また始めます。",
        ],
    ),
    (
        "仕事帰り",
        [
            "仕事帰りに買い物をしてきます。",
            "仕事帰りに少し寄ってもいいですか。",
        ],
    ),
    (
        "気分",
        [
            "気分を変えて、外を歩きませんか。",
            "気分がいいので、少し散歩してきます。",
        ],
    ),
    (
        "眠い",
        [
            "眠いので、今日は早めに寝ます。",
            "眠いときは、少し休みたいです。",
        ],
    ),
];
const HORIZONTAL: &[&str] = &[" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"];
const BOUNDARIES: &[&str] = &[
    "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
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
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == seed && c.kind == CandidateKind::Literal)
    );
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
fn selectable(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        })
}
fn first_page(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = selectable(ime, text, kind);
    assert!(
        kind == CandidateKind::Literal || index < PAGE_SIZE,
        "not page one {text:?}: {:?}",
        ime.candidates()
    );
    index
}
fn commit_index_once(ime: &mut XRTabletImeEngine, index: usize, text: &str) {
    assert_eq!(ime.candidates()[index].text, text);
    assert_eq!(ime.candidates()[index].source, CandidateSource::Local);
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = first_page(ime, text, kind);
    commit_index_once(ime, index, text);
}
fn word_and_sentences(seed: &str, word: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    first_page(&ime, word, CandidateKind::Word);
    commit_once(&mut engine(seed), word, CandidateKind::Word);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    if let Some(&(_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
        for sentence in sentences {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn routine_preserves_all_314_previous_readings_and_158_contexts_in_full_order() {
    // Independently captured before this extension: every previous reading,
    // kind and separator flag, and every previous authored sentence in order.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 314);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(314) {
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
    assert_eq!(readings, 0xc933b22b252d0006);
    assert!(ja.continuations().count() >= 158);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut previous_sentence_count = 0;
    for (context, sentences) in ja.continuations().take(158) {
        previous_sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(previous_sentence_count, 315);
    assert_eq!(contexts, 0xb814849e4a2fa499);
}

#[test]
fn routine_additions_match_the_independent_authored_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 16);
    assert_eq!(SENTENCES.len(), 8);
    assert!(ja.readings().len() >= 330 && ja.continuations().count() >= 166);
    assert_eq!(
        ja.continuations()
            .take(166)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        331
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(314).take(16).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(158).take(8).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn routine_full_romaji_case_and_kana_offer_first_page_words_and_both_sentence_branches() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            word_and_sentences(&seed, word);
        }
    }
}

#[test]
fn routine_specific_partials_offer_first_page_words_and_both_sentence_branches() {
    for (seed, word) in [
        ("hayaok", "早起き"),
        ("yofukash", "夜更かし"),
        ("nebusok", "寝不足"),
        ("hiruyasum", "昼休み"),
        ("hitoyasum", "ひと休み"),
        ("shigotogaer", "仕事帰り"),
        ("kibu", "気分"),
        ("KIBU", "気分"),
        ("nemu", "眠い"),
        ("mezamash", "目覚まし"),
        ("mijitak", "身支度"),
        ("nikk", "日課"),
        ("nebo", "寝坊"),
        ("inemur", "居眠り"),
        ("hayan", "早寝"),
        ("yorugat", "夜型"),
        ("asagat", "朝型"),
        ("はやお", "早起き"),
        ("よふか", "夜更かし"),
        ("ねぶそ", "寝不足"),
        ("ひるやす", "昼休み"),
        ("ひとやす", "ひと休み"),
        ("きぶ", "気分"),
        ("ねむ", "眠い"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn routine_six_horizontal_separators_preserve_exact_spelling_and_sentence_choices() {
    for &space in HORIZONTAL {
        for (left, right, word) in [
            ("haya", "OKI", "早起き"),
            ("yo", "fukashi", "夜更かし"),
            ("nebu", "soku", "寝不足"),
            ("hiru", "yasumi", "昼休み"),
            ("hito", "yasumi", "ひと休み"),
            ("shigoto", "gaeri", "仕事帰り"),
            ("ki", "bun", "気分"),
            ("ne", "mui", "眠い"),
        ] {
            let seed = format!("{space}{left}{space}{right}{space}");
            word_and_sentences(&seed, word);
        }
    }
    for (seed, word) in [
        ("はや おき", "早起き"),
        ("ひと\u{3000}やすみ", "ひと休み"),
        ("KIBUN’", "気分"),
        ("kibun'", "気分"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn routine_ambiguous_kib_retains_prior_hope_choices_and_later_page_mood_sentences() {
    // The unfinished b also probes bo, so the established 希望 reading and
    // its sentences remain ahead of 気分. Both new sentences stay selectable.
    for seed in ["kib", "KIB"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "きb");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Unspecified);
        assert!(
            first_page(&ime, "希望", CandidateKind::Word)
                < first_page(&ime, "気分", CandidateKind::Word)
        );
        for sentence in [
            "希望があれば教えてください。",
            "希望に近いものを探しましょう。",
        ] {
            first_page(&ime, sentence, CandidateKind::Sentence);
        }
        for sentence in [
            "気分を変えて、外を歩きませんか。",
            "気分がいいので、少し散歩してきます。",
        ] {
            let mut selected = engine(seed);
            let index = selectable(&selected, sentence, CandidateKind::Sentence);
            assert!((PAGE_SIZE..12).contains(&index));
            commit_index_once(&mut selected, index, sentence);
        }
        commit_once(&mut engine(seed), "気分", CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn routine_kana_work_return_partial_keeps_grammar_choices_and_split_sentence_pages() {
    // しごとがえ can continue the existing 仕事 + が + え grammar as
    // well as the longer 仕事帰り reading. Keep all established choices;
    // the two new sentence branches occupy different pages at this prefix.
    let seed = "しごとがえ";
    let ime = engine(seed);
    draft(&ime, seed);
    assert_eq!(ime.candidates()[0].text, "仕事がえ");
    assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
    assert_eq!(ime.candidates()[0].score, 100.0);
    first_page(&ime, "仕事帰り", CandidateKind::Word);
    first_page(&ime, "仕事が選ぶ", CandidateKind::Word);
    first_page(&ime, "仕事が映画", CandidateKind::Word);
    let first_sentence = "仕事帰りに買い物をしてきます。";
    let second_sentence = "仕事帰りに少し寄ってもいいですか。";
    first_page(&ime, first_sentence, CandidateKind::Sentence);
    let later = selectable(&ime, second_sentence, CandidateKind::Sentence);
    assert!((PAGE_SIZE..12).contains(&later));
    commit_once(&mut engine(seed), first_sentence, CandidateKind::Sentence);
    commit_index_once(&mut engine(seed), later, second_sentence);
    commit_once(&mut engine(seed), "仕事帰り", CandidateKind::Word);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
}

#[test]
fn routine_shared_work_prefix_retains_prior_sentences_and_bounded_later_page_choices() {
    // The complete short 仕事 reading remains the primary word and retains
    // both prior sentences. Its longer reading has finite later-page slots.
    for seed in ["shigoto", "SHIGOTO", "しごと"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "仕事");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        assert!(
            first_page(&ime, "仕事", CandidateKind::Word)
                < first_page(&ime, "仕事帰り", CandidateKind::Word)
        );
        for sentence in ["仕事が終わりました。", "仕事について相談したいです。"]
        {
            first_page(&ime, sentence, CandidateKind::Sentence);
        }
        for sentence in [
            "仕事帰りに買い物をしてきます。",
            "仕事帰りに少し寄ってもいいですか。",
        ] {
            let mut selected = engine(seed);
            let index = selectable(&selected, sentence, CandidateKind::Sentence);
            assert!(index < 12);
            commit_index_once(&mut selected, index, sentence);
        }
        let later = selectable(
            &ime,
            "仕事帰りに少し寄ってもいいですか。",
            CandidateKind::Sentence,
        );
        assert!((PAGE_SIZE..12).contains(&later));
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn routine_adoption_restoration_and_particles_stay_editable_until_explicit_commit() {
    for (reading, word, tail, sentence) in [
        (
            "hayaoki",
            "早起き",
            "HA",
            "早起きは苦手ですが、明日は早く起きます。",
        ),
        (
            "yofukashi",
            "夜更かし",
            "ha",
            "夜更かしはほどほどにしておきます。",
        ),
        ("nebusoku", "寝不足", "で", "寝不足で少し眠いです。"),
        ("hiruyasumi", "昼休み", "ni", "昼休みに少し外を歩きます。"),
        (
            "hitoyasumi",
            "ひと休み",
            "shite",
            "ひと休みしてから続きを進めましょう。",
        ),
        (
            "shigotogaeri",
            "仕事帰り",
            "に",
            "仕事帰りに買い物をしてきます。",
        ),
        ("kibun", "気分", "wo", "気分を変えて、外を歩きませんか。"),
        ("nemui", "眠い", "node", "眠いので、今日は早めに寝ます。"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(first_page(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // These seed updates model the host's cumulative editable draft; they
        // do not stand in for native key receipts or physical Backspace QA.
        ime.seed(word);
        draft(&ime, word);
        ime.seed(reading);
        draft(&ime, reading);
        first_page(&ime, word, CandidateKind::Word);
        let spaced = format!("{word} ");
        ime.seed(&spaced);
        draft(&ime, &spaced);
        first_page(&ime, sentence, CandidateKind::Sentence);
        commit_once(&mut engine(&spaced), &spaced, CandidateKind::Literal);
        let continued = format!("{word} {tail}");
        ime.seed(&continued);
        draft(&ime, &continued);
        commit_once(&mut ime, sentence, CandidateKind::Sentence);
        commit_once(&mut engine(&continued), &continued, CandidateKind::Literal);
    }
}

#[test]
fn routine_known_aliases_preserve_adopted_spelling_and_grammar_without_shita_nouns() {
    for (reading, adopted, tail, progress, kind) in [
        (
            "hayaoki",
            "ハヤオキ",
            "shita",
            "ハヤオキした",
            CandidateKind::Unspecified,
        ),
        (
            "yofukashi",
            "ヨフカシ",
            "shit",
            "ヨフカシしt",
            CandidateKind::Unspecified,
        ),
        (
            "hitoyasumi",
            "ヒトヤスミ",
            "SHITARA",
            "ヒトヤスミしたら",
            CandidateKind::Unspecified,
        ),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(first_page(&ime, adopted, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(adopted));
        assert!(ime.snapshot().committed_text.is_empty());
        ime.seed(adopted);
        draft(&ime, adopted);
        for separator in ["", " ", "\t", "\u{3000}"] {
            let continued = format!("{adopted}{separator}{tail}");
            ime.seed(&continued);
            draft(&ime, &continued);
            assert_eq!(ime.candidates()[0].text, progress);
            assert_eq!(ime.candidates()[0].kind, kind);
            assert_eq!(ime.candidates()[0].score, 100.0);
            assert!(ime.candidates().iter().all(|c| !c.text.contains('下')));
            commit_once(&mut ime, progress, kind);
            commit_once(&mut engine(&continued), &continued, CandidateKind::Literal);
        }
    }
    for (seed, progress, sentence) in [
        (
            "早起きshita",
            "早起きした",
            "早起きしたので、少し散歩してきます。",
        ),
        (
            "YOFUKASHI SHITA",
            "夜更かしした",
            "夜更かししたので、今日は早めに寝ます。",
        ),
        (
            "ひと休みshitara",
            "ひと休みしたら",
            "ひと休みしたら、また始めます。",
        ),
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, progress);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
        assert!(ime.candidates().iter().all(|c| !c.text.contains('下')));
        first_page(&ime, sentence, CandidateKind::Sentence);
        commit_once(&mut ime, progress, CandidateKind::Sentence);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
}

#[test]
fn routine_unknown_conflicting_and_line_separated_text_cannot_recover_authored_sentences() {
    let mut seeds = vec![
        "xyzhayaoki".to_owned(),
        "🙂昼休みに".into(),
        "https://yofukashi".into(),
        "kibun@example.invalid".into(),
        "寝不足nixyz".into(),
        "気分をが".into(),
        "ひと休みしたら🙂".into(),
        "テストshita".into(),
    ];
    for boundary in BOUNDARIES {
        for (head, tail) in [("haya", "oki"), ("昼休み", "ni"), ("ハヤオキ", "shita")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
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
            for boundary in BOUNDARIES {
                if seed.contains(boundary) {
                    assert!(candidate.text.contains(boundary));
                }
            }
        }
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
    // Positive controls distinguish valid written-head progress from the
    // protected strings and conflicting tails above.
    for (seed, sentence) in [
        ("昼休みni", "昼休みに少し外を歩きます。"),
        ("気分wo", "気分を変えて、外を歩きませんか。"),
        ("眠いnode", "眠いので、今日は早めに寝ます。"),
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        commit_once(&mut ime, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn routine_keeps_prior_homophone_pages_number_ambiguity_and_standalone_shit() {
    for seed in ["kanji", "KANJI", "かんじ"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "漢字");
        assert!(
            first_page(&ime, "漢字", CandidateKind::Word)
                < first_page(&ime, "感じ", CandidateKind::Word)
        );
    }
    for seed in ["kou", "KOU", "こう"] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        let candidate = first_page(&ime, "候補", CandidateKind::Word);
        let update = first_page(&ime, "更新", CandidateKind::Word);
        let park = selectable(&ime, "公園", CandidateKind::Word);
        let crossing = selectable(&ime, "交差点", CandidateKind::Word);
        assert!(candidate < update && update < park && park < crossing);
        assert!((PAGE_SIZE..12).contains(&park) && (PAGE_SIZE..12).contains(&crossing));
        for sentence in ["公園で少し休みませんか。", "公園まで歩いてみましょう。"]
        {
            first_page(&ime, sentence, CandidateKind::Sentence);
        }
        commit_index_once(&mut ime, crossing, "交差点");
    }
    for seed in ["bango", "BANGO", "ばんご"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "ばんご");
        assert!(
            first_page(&ime, "晩ご飯", CandidateKind::Word)
                < first_page(&ime, "番号", CandidateKind::Word)
        );
        for sentence in [
            "番号をもう一度教えてください。",
            "番号が違っていないか確認します。",
        ] {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
    let mut ime = engine("shit");
    draft(&ime, "shit");
    let down = selectable(&ime, "下", CandidateKind::Word);
    assert!((PAGE_SIZE..12).contains(&down));
    commit_index_once(&mut ime, down, "下");
    commit_once(&mut engine("shit"), "shit", CandidateKind::Literal);
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-routine-synthetic-error"
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
fn routine_local_candidates_survive_typed_missing_unavailable_timeout_http_errors_and_empty_models()
{
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("hayaoki", "早起き", "早起きしたので、少し散歩してきます。"),
            ("hiruyasumi", "昼休み", "昼休みに少し外を歩きます。"),
            ("nemui", "眠い", "眠いので、今日は早めに寝ます。"),
        ] {
            let mut ime = blank_engine();
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
            ime.seed(seed);
            draft(&ime, seed);
            first_page(&ime, word, CandidateKind::Word);
            first_page(&ime, sentence, CandidateKind::Sentence);
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
            draft(&ime, seed);
            commit_once(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}
