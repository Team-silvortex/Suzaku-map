//! Data-only wallet, coin and receipt locations; not payment processing or financial advice.
use ring::digest::{SHA256, digest};
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    languages::{
        japanese::romaji_to_hiragana,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{
        builtin,
        packs::{self, PackStore},
    },
};

type EnglishCase = (
    &'static str,
    [&'static str; 2],
    [&'static str; 2],
    [&'static str; 2],
);
type ReadingCase = (&'static str, &'static str, &'static str, [&'static str; 2]);
const EN: &[EnglishCase] = &[
    (
        "my wallet is still in the",
        ["jacket", "backpack"],
        ["jacke", "backpac"],
        [
            "my wallet is still in the jacket pocket.",
            "my wallet is still in the backpack by the door.",
        ],
    ),
    (
        "the loose coins are in the",
        ["pouch", "drawer"],
        ["pouc", "drawe"],
        [
            "the loose coins are in the pouch next to my keys.",
            "the loose coins are in the drawer with the spare key.",
        ],
    ),
    (
        "the receipts are together in the",
        ["folder", "basket"],
        ["folde", "baske"],
        [
            "the receipts are together in the folder with the tickets.",
            "the receipts are together in the basket on the desk.",
        ],
    ),
];

const ZH: &[ReadingCase] = &[
    (
        "qian'bao",
        "qian'ba",
        "钱包",
        ["钱包放在外套的内袋里。", "钱包还在背包里，我去拿一下。"],
    ),
    (
        "ling'qian'bao",
        "ling'qian'ba",
        "零钱包",
        ["零钱包在抽屉的小盒子里。", "零钱包和钥匙放在一起了。"],
    ),
    (
        "ka'bao",
        "ka'ba",
        "卡包",
        ["卡包放在桌边的袋子里。", "卡包在背包的侧袋里，你看看。"],
    ),
];

const JA: &[ReadingCase] = &[
    (
        "saifu",
        "saif",
        "財布",
        [
            "財布は上着の内ポケットにあります。",
            "財布を机に置いたままでした。",
        ],
    ),
    (
        "kozeni",
        "kozen",
        "小銭",
        [
            "小銭は財布の小さなポケットに入っています。",
            "小銭を入れた袋は鞄の中にあります。",
        ],
    ),
    (
        "kaban",
        "kaba",
        "鞄",
        [
            "鞄は玄関の椅子に置いてあります。",
            "鞄の内ポケットにレシートを入れてあります。",
        ],
    ),
];

fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local
                && c.label == c.text
                && c.score.is_finite())
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
    if !seed.is_empty() {
        assert!(ime.candidates().iter().any(|c| c.text == seed));
    }
    ime
}
fn at(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing first-page {kind:?} {text:?}: {:?}",
                ime.candidates()
            )
        })
}
fn choices(language: &str, seed: &str, word: &str, authors: &[&str]) {
    let ime = engine(language, seed);
    if seed != word {
        at(&ime, word, CandidateKind::Word);
    }
    for author in authors {
        at(&ime, author, CandidateKind::Sentence);
    }
}
fn commit(language: &str, seed: &str, text: &str) {
    let mut ime = engine(language, seed);
    let pos = ime
        .candidates()
        .iter()
        .position(|c| c.text == text)
        .unwrap();
    ime.select_candidate(pos);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn sha(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn old_resource(language: &str, raw: &str) -> String {
    let mut old = raw.to_owned();
    let first = match language {
        "en" => ",\n    {\n      \"id\": \"daily_pocket_items\",",
        "zh-Hans" => ",\n    [\n      \"钱包\",",
        "ja" => ",\n    [\n      \"財布\",",
        _ => unreachable!(),
    };
    if old.contains(first) {
        let after = if language == "en" {
            "\n  ],\n  \"phrase_endings\": ["
        } else {
            "\n  ]\n}"
        };
        assert_eq!(old.matches(first).count(), 1);
        assert_eq!(old.matches(after).count(), 1);
        let (start, end) = (old.find(first).unwrap(), old.find(after).unwrap());
        assert!(start < end);
        old.replace_range(start..end, "");
    }
    old
}

#[test]
fn pocket_items_preserves_exact_old_resources_readings_flags_rank_sources_and_authors() {
    for (language, raw, expected) in [
        (
            "en",
            include_str!("../data/lexicons/en.json"),
            "cecf7b659558b2c0034a9031f75433d9452d43659a8cb73b07542784ffacbd4f",
        ),
        (
            "zh-Hans",
            include_str!("../data/lexicons/zh-Hans.json"),
            "bff559dfac7fe2ab2d4f1a00f0b79ef43c1e69ae8f418c6c552001a8964846b0",
        ),
        (
            "ja",
            include_str!("../data/lexicons/ja.json"),
            "3d913e6ce519dc66781e990906028c549c8d8fac40258d2f3b62079cd41ac4fd",
        ),
    ] {
        assert_eq!(sha(&old_resource(language, raw)), expected);
    }
}

#[test]
fn pocket_items_adds_only_three_contexts_and_six_authors_per_language_without_new_forms_or_readings()
 {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 61);
    let layer = &en.word_layers()[59];
    assert_eq!(layer.id, "daily_pocket_items");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 3);
    assert_eq!(layer.sentences.len(), 6);
    let known: HashSet<_> = en.word_layers()[..59]
        .iter()
        .flat_map(|l| {
            l.words
                .iter()
                .chain(l.next_words.iter().flat_map(|(_, ws)| ws))
                .chain(&l.sentences)
        })
        .flat_map(|s| s.split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’'))))
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('’', "'").to_ascii_lowercase())
        .collect();
    assert_eq!(known.len(), 6213);
    for text in layer
        .sentences
        .iter()
        .chain(layer.next_words.iter().flat_map(|(_, ws)| ws))
    {
        for token in text.split(|c: char| !(c.is_ascii_alphabetic() || c == '\'')) {
            assert!(
                token.is_empty() || known.contains(token),
                "new form: {token}"
            );
        }
    }
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[i].0, context);
        assert_eq!(layer.next_words[i].1, words);
        assert_eq!(&layer.sentences[2 * i..2 * i + 2], authors);
    }
    for (language, entries, reads, contexts, authors) in
        [("zh-Hans", ZH, 3441, 888, 1774), ("ja", JA, 638, 376, 751)]
    {
        let data = builtin(language).unwrap();
        assert_eq!(data.readings().len(), reads);
        assert_eq!(data.continuations().count(), contexts);
        assert_eq!(
            data.continuations().map(|(_, v)| v.len()).sum::<usize>(),
            authors
        );
        let old_contexts = if language == "zh-Hans" { 882 } else { 370 };
        let tail: Vec<_> = data.continuations().skip(old_contexts).take(3).collect();
        for (i, &(_, _, word, pair)) in entries.iter().enumerate() {
            assert_eq!(tail[i].0, word);
            assert_eq!(tail[i].1, pair);
            assert!(data.readings().iter().any(|r| r.text == word));
        }
    }
}

#[test]
fn pocket_items_english_full_partials_and_all_six_branches_commit_exactly_once() {
    for &(context, words, partials, authors) in EN {
        for i in 0..2 {
            let word = format!("{context} {}", words[i]);
            for seed in [
                format!("{context} "),
                format!("{context} {}", partials[i]),
                word.clone(),
            ] {
                choices("en", &seed, &word, &[authors[i]]);
                for text in [&*word, authors[i], &*seed] {
                    commit("en", &seed, text);
                }
            }
        }
    }
}
#[test]
fn pocket_items_chinese_existing_words_joined_separated_case_tones_and_partials_offer_both_authors()
{
    for &(reading, partial, word, authors) in ZH {
        for seed in [
            reading.into(),
            reading.replace('\'', ""),
            reading.replace('\'', " ").to_ascii_uppercase(),
            partial.into(),
        ] {
            choices("zh-Hans", &seed, word, &authors);
            for text in [word, authors[0], authors[1], &seed] {
                commit("zh-Hans", &seed, text);
            }
        }
    }
    choices("zh-Hans", "qian2'bao1", "钱包", &ZH[0].3);
    choices("zh-Hans", "ling2'qian2'bao1", "零钱包", &ZH[1].3);
    choices("zh-Hans", "ka3'bao1", "卡包", &ZH[2].3);
}
#[test]
fn pocket_items_japanese_existing_words_romaji_case_kana_and_partials_offer_both_authors() {
    for &(reading, partial, word, authors) in JA {
        let kana = match reading {
            "saifu" => "さいふ",
            "kozeni" => "こぜに",
            "kaban" => "かばん",
            _ => unreachable!(),
        };
        assert_eq!(romaji_to_hiragana(reading), kana);
        for seed in [
            reading.into(),
            reading.to_ascii_uppercase(),
            kana.into(),
            partial.into(),
            word.into(),
        ] {
            choices("ja", &seed, word, &authors);
            for text in [word, authors[0], authors[1], &seed] {
                commit("ja", &seed, text);
            }
        }
    }
}
#[test]
fn pocket_items_horizontal_spaces_and_explicit_word_adoption_reseed_keep_drafts_uncommitted() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("the{gap}receipts{gap}are{gap}together{gap}in{gap}the{gap}folde");
        let word = seed.strip_suffix("folde").unwrap().to_owned() + "folder";
        let author = word.clone() + " with the tickets.";
        choices("en", &seed, &word, &[&author]);
        let seed = format!("ling{gap}qian{gap}bao");
        choices("zh-Hans", &seed, "零钱包", &ZH[1].3);
        for (left, right, which) in [("sa", "ifu", 0), ("ko", "zeni", 1), ("ka", "ban", 2)] {
            let seed = format!("{left}{gap}{right}");
            choices("ja", &seed, JA[which].2, &JA[which].3);
            commit("ja", &seed, &seed);
        }
    }
    for (language, seed, word, authors) in [
        ("zh-Hans", "qian'ba", "钱包", ZH[0].3),
        ("ja", "kaba", "鞄", JA[2].3),
    ] {
        let mut ime = engine(language, seed);
        ime.select_candidate(at(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert_eq!(ime.snapshot().seed_text, seed);
        assert!(ime.snapshot().committed_text.is_empty());
        ime.seed(word); // Same explicit adoption/reseed boundary used by the input UI.
        for author in authors {
            at(&ime, author, CandidateKind::Sentence);
        }
        assert!(ime.snapshot().committed_text.is_empty());
    }
    let prefix = "之前的记录🙂，不改写已有内容。".repeat(30);
    let seed = format!("{prefix}the receipts are together in the folde");
    let word = format!("{prefix}the receipts are together in the folder");
    let author = format!("{word} with the tickets.");
    choices("en", &seed, &word, &[&author]);
    commit("en", &seed, &author);
}
#[test]
fn pocket_items_literal_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            (
                "en",
                format!("the receipts are together{line}in the folde"),
                EN[2].3[0],
            ),
            ("zh-Hans", format!("qian{line}bao"), ZH[0].3[0]),
            ("ja", format!("ka{line}ban"), JA[2].3[0]),
        ] {
            let ime = engine(language, &seed);
            assert!(!ime.candidates().iter().any(|c| c.text == author));
            commit(language, &seed, &seed);
        }
    }
    for (language, seed) in [
        ("en", "https://example.invalid/my_wallet/jacke"),
        ("en", "receipts_together_folde"),
        ("zh-Hans", "user_qian_bao_42"),
        ("ja", "user_kaban_42"),
    ] {
        commit(language, seed, seed);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "pocket_items-synthetic-fallback"
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
fn pocket_items_pending_disabled_five_typed_failures_and_empty_models_keep_the_complete_local_pool()
{
    for (language, seed, word, authors) in [
        (
            "en",
            "my wallet is still in the jacke",
            "my wallet is still in the jacket",
            &EN[0].3[..1],
        ),
        ("zh-Hans", "ling'qian'ba", "零钱包", &ZH[1].3[..]),
        ("ja", "saif", "財布", &JA[0].3[..]),
    ] {
        choices(language, seed, word, authors);
        let baseline = engine(language, seed).candidates().to_vec();
        for error in [
            Some(LlmProviderError::NoLocalModel),
            Some(LlmProviderError::Unavailable),
            Some(LlmProviderError::Timeout),
            Some(LlmProviderError::HttpStatus(503)),
            Some(LlmProviderError::InvalidResponse),
            None,
        ] {
            let mut ime = engine(language, "");
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(seed);
            assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
            assert_eq!(ime.candidates(), baseline);
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                assert_eq!(ime.candidates(), baseline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(!ime.prediction_pending());
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(
                ime.prediction_error(),
                Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
            );
            assert_eq!(ime.candidates(), baseline);
            ime.configure_prediction(None);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
        }
    }
}
fn old_controls(language: &str) {
    match language {
        "en" => choices(
            "en",
            "please leave the towel he",
            "please leave the towel here",
            &["please leave the towel here by the sink."],
        ),
        "zh-Hans" => choices(
            "zh-Hans",
            "ya'gao",
            "牙膏",
            &["牙膏我放在牙刷旁边了。", "牙膏快用完了，我先记在清单上。"],
        ),
        "ja" => choices(
            "ja",
            "hamigaki",
            "歯磨き",
            &[
                "歯磨きを済ませてから出かけます。",
                "歯磨きはもう終わりました。",
            ],
        ),
        _ => unreachable!(),
    }
}
#[test]
fn pocket_items_keeps_old_care_hygiene_and_nasal_author_controls() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}
struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn pocket_items_new_and_old_workflows_coexist_with_each_languages_private_recommended_packs() {
    const NAME: &str =
        "pocket_items_new_and_old_workflows_coexist_with_each_languages_private_recommended_packs";
    if let Ok(language) = std::env::var("SUZAKU_POCKET_ITEMS_TEST_CHILD") {
        assert!(matches!(language.as_str(), "en" | "zh-Hans" | "ja"));
        let dir = std::env::temp_dir().join(format!(
            "suzaku-pocket_items-packs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let owned = Owned(dir);
        let store = PackStore::new(owned.0.join("packs")).unwrap();
        for pack in packs::recommended()
            .into_iter()
            .filter(|p| p.manifest.language == language)
        {
            store.install(pack, false).unwrap();
        }
        let registry = fs::read(store.path()).unwrap();
        let expected = if language == "ja" { 9 } else { 8 };
        let report = packs::initialize(&store).unwrap();
        assert_eq!(report.loaded.len(), expected);
        assert!(report.errors.is_empty() && report.disabled.is_empty());
        old_controls(&language);
        println!("POCKET_ITEMS_OLD_CONTROLS_OK:{language}:{expected}");
        match language.as_str() {
            "en" => {
                for &(ctx, words, partials, authors) in EN {
                    for i in 0..2 {
                        choices(
                            "en",
                            &format!("{ctx} {}", partials[i]),
                            &format!("{ctx} {}", words[i]),
                            &[authors[i]],
                        );
                    }
                }
            }
            "zh-Hans" => {
                for &(reading, partial, word, authors) in ZH {
                    choices("zh-Hans", reading, word, &authors);
                    let ime = engine("zh-Hans", partial);
                    for author in authors {
                        assert!(
                            ime.candidates()
                                .iter()
                                .any(|c| { c.text == author && c.kind == CandidateKind::Sentence })
                        );
                        commit("zh-Hans", partial, author);
                    }
                }
            }
            "ja" => {
                for &(_, partial, word, authors) in JA {
                    choices("ja", partial, word, &authors);
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(fs::read(store.path()).unwrap(), registry);
        println!("POCKET_ITEMS_NEW_CHOICES_OK:{language}:{expected}");
        return;
    }
    for language in ["en", "zh-Hans", "ja"] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
            .env("SUZAKU_POCKET_ITEMS_TEST_CHILD", language)
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("IBUS_ADDRESS")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("running 1 test"), "{text}");
        assert!(
            text.contains(&format!("POCKET_ITEMS_OLD_CONTROLS_OK:{language}:")),
            "{text}"
        );
        assert!(
            output.status.success(),
            "{text}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            text.contains(&format!("POCKET_ITEMS_NEW_CHOICES_OK:{language}:")),
            "{text}"
        );
        assert!(
            text.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{text}"
        );
    }
}
