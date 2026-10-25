//! Data-only washroom-location dialogue, not sensing, product advice or device control.
#[path = "support/living_room_reading.rs"]
mod living_room_reading;
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
        "the shampoo is still on the",
        ["shelf", "counter"],
        ["shel", "counte"],
        [
            "the shampoo is still on the shelf next to the conditioner.",
            "the shampoo is still on the counter next to the sink.",
        ],
    ),
    (
        "please put the toothpaste in the",
        ["drawer", "pouch"],
        ["drawe", "pouc"],
        [
            "please put the toothpaste in the drawer with the soap.",
            "please put the toothpaste in the pouch with the shampoo.",
        ],
    ),
    (
        "the clean towels are in the",
        ["basket", "cupboard"],
        ["baske", "cupboar"],
        [
            "the clean towels are in the basket by the bathroom door.",
            "the clean towels are in the cupboard above the sink.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "mao'jin",
        "mao'ji",
        "毛巾",
        ["毛巾挂在门后的架子上。", "毛巾还有一条，放在柜子里。"],
    ),
    (
        "mu'yu'lu",
        "mu'yu'l",
        "沐浴露",
        [
            "沐浴露放在浴室的小架子上。",
            "沐浴露还有一瓶，放在柜子下面。",
        ],
    ),
    (
        "xi'shu'bao",
        "xi'shu'ba",
        "洗漱包",
        ["洗漱包放在行李箱的侧袋里。", "洗漱包里的东西我已经收好了。"],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "haburashi",
        "haburash",
        "歯ブラシ",
        [
            "歯ブラシは洗面台の右側にあります。",
            "歯ブラシを入れるケースはここに置きました。",
        ],
    ),
    (
        "sekken",
        "sekke",
        "石鹸",
        [
            "石鹸は洗面台の左側にあります。",
            "石鹸の予備は棚の小さな箱にあります。",
        ],
    ),
    (
        "taoru",
        "taor",
        "タオル",
        [
            "タオルは洗面所の棚にあります。",
            "タオルの予備はかごにまとめてあります。",
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
    let mut old = living_room_reading::without_living_room_reading(raw);
    let first = match language {
        "en" => ",\n    {\n      \"id\": \"daily_washroom\",",
        "zh-Hans" => ",\n    [\n      \"毛巾\",",
        "ja" => ",\n    [\n      \"歯ブラシ\",",
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
fn washroom_preserves_exact_old_resources_readings_flags_rank_sources_and_authors() {
    for (language, raw, expected) in [
        (
            "en",
            include_str!("../data/lexicons/en.json"),
            "643a76036fa82142288b188459efb70fd068bcb1bc4d738be2dae54ca789584a",
        ),
        (
            "zh-Hans",
            include_str!("../data/lexicons/zh-Hans.json"),
            "3c27c55ce11194fe8a24f1e54b9b0affd6e2eb2bb3089dc2a529609ee9db8db6",
        ),
        (
            "ja",
            include_str!("../data/lexicons/ja.json"),
            "77f1dc39cec5d0b959062bd04afc5656ac0d15c0864301345d4e9bd78ce54486",
        ),
    ] {
        assert_eq!(sha(&old_resource(language, raw)), expected);
    }
}

#[test]
fn washroom_adds_only_three_contexts_and_six_authors_per_language_without_new_forms_or_readings() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    let layer = &en.word_layers()[58];
    assert_eq!(layer.id, "daily_washroom");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 3);
    assert_eq!(layer.sentences.len(), 6);
    let known: HashSet<_> = en.word_layers()[..58]
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
        [("zh-Hans", ZH, 3442, 921, 1840), ("ja", JA, 638, 409, 817)]
    {
        let data = builtin(language).unwrap();
        assert_eq!(data.readings().len(), reads);
        assert_eq!(data.continuations().count(), contexts);
        assert_eq!(
            data.continuations().map(|(_, v)| v.len()).sum::<usize>(),
            authors
        );
        let old_contexts = if language == "zh-Hans" { 879 } else { 367 };
        let tail: Vec<_> = data.continuations().skip(old_contexts).take(3).collect();
        for (i, &(_, _, word, pair)) in entries.iter().enumerate() {
            assert_eq!(tail[i].0, word);
            assert_eq!(tail[i].1, pair);
            assert!(data.readings().iter().any(|r| r.text == word));
        }
    }
}

#[test]
fn washroom_english_full_partials_and_all_six_branches_commit_exactly_once() {
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
fn washroom_chinese_existing_words_joined_separated_case_tones_and_partials_offer_both_authors() {
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
    choices("zh-Hans", "mao2'jin1", "毛巾", &ZH[0].3);
    choices("zh-Hans", "mu4'yu4'lu4", "沐浴露", &ZH[1].3);
    choices("zh-Hans", "xi3'shu4'bao1", "洗漱包", &ZH[2].3);
}
#[test]
fn washroom_japanese_existing_words_romaji_case_kana_and_partials_offer_both_authors() {
    for &(reading, partial, word, authors) in JA {
        let kana = match reading {
            "haburashi" => "はぶらし",
            "sekken" => "せっけん",
            "taoru" => "たおる",
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
fn washroom_horizontal_spaces_and_explicit_word_adoption_reseed_keep_drafts_uncommitted() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("the{gap}clean{gap}towels{gap}are{gap}in{gap}the{gap}baske");
        let word = seed.strip_suffix("baske").unwrap().to_owned() + "basket";
        let author = word.clone() + " by the bathroom door.";
        choices("en", &seed, &word, &[&author]);
        let seed = format!("mu{gap}yu{gap}lu");
        choices("zh-Hans", &seed, "沐浴露", &ZH[1].3);
        for (left, right, which) in [("ha", "burashi", 0), ("se", "kken", 1), ("ta", "oru", 2)] {
            let seed = format!("{left}{gap}{right}");
            choices("ja", &seed, JA[which].2, &JA[which].3);
            commit("ja", &seed, &seed);
        }
    }
    for (language, seed, word, authors) in [
        ("zh-Hans", "mao'ji", "毛巾", ZH[0].3),
        ("ja", "taor", "タオル", JA[2].3),
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
    let seed = format!("{prefix}the clean towels are in the baske");
    let word = format!("{prefix}the clean towels are in the basket");
    let author = format!("{word} by the bathroom door.");
    choices("en", &seed, &word, &[&author]);
    commit("en", &seed, &author);
}
#[test]
fn washroom_literal_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            (
                "en",
                format!("the clean towels are{line}in the baske"),
                EN[2].3[0],
            ),
            ("zh-Hans", format!("mao{line}jin"), ZH[0].3[0]),
            ("ja", format!("ta{line}oru"), JA[2].3[0]),
        ] {
            let ime = engine(language, &seed);
            assert!(!ime.candidates().iter().any(|c| c.text == author));
            commit(language, &seed, &seed);
        }
    }
    for (language, seed) in [
        ("en", "https://example.invalid/the_shampoo/shel"),
        ("en", "clean_towels_baske"),
        ("zh-Hans", "user_mao_jin_42"),
        ("ja", "user_taoru_42"),
    ] {
        commit(language, seed, seed);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "washroom-synthetic-fallback"
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
fn washroom_pending_disabled_five_typed_failures_and_empty_models_keep_the_complete_local_pool() {
    for (language, seed, word, authors) in [
        (
            "en",
            "the shampoo is still on the shel",
            "the shampoo is still on the shelf",
            &EN[0].3[..1],
        ),
        ("zh-Hans", "mu'yu'l", "沐浴露", &ZH[1].3[..]),
        ("ja", "haburash", "歯ブラシ", &JA[0].3[..]),
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
fn washroom_keeps_old_care_hygiene_and_nasal_author_controls() {
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
fn washroom_new_and_old_workflows_coexist_with_each_languages_private_recommended_packs() {
    const NAME: &str =
        "washroom_new_and_old_workflows_coexist_with_each_languages_private_recommended_packs";
    if let Ok(language) = std::env::var("SUZAKU_WASHROOM_TEST_CHILD") {
        assert!(matches!(language.as_str(), "en" | "zh-Hans" | "ja"));
        let dir = std::env::temp_dir().join(format!(
            "suzaku-washroom-packs-{}-{}",
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
        println!("WASHROOM_OLD_CONTROLS_OK:{language}:{expected}");
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
                    // The optional family pack already owns the longer 毛巾放这里
                    // and 毛巾收纳 roots. Preserve their ranking for the short
                    // partial: one new author is on page two, not missing.
                    let ime = engine("zh-Hans", partial);
                    for author in authors {
                        assert!(
                            ime.candidates()
                                .iter()
                                .any(|c| { c.text == author && c.kind == CandidateKind::Sentence })
                        );
                        commit("zh-Hans", partial, author);
                    }
                    if word == "毛巾" {
                        assert_eq!(ime.candidates()[0].text, "毛巾放这里");
                        assert_eq!(ime.candidates()[5].text, authors[0]);
                        assert_eq!(ime.candidates()[PAGE_SIZE].text, authors[1]);
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
        println!("WASHROOM_NEW_CHOICES_OK:{language}:{expected}");
        return;
    }
    for language in ["en", "zh-Hans", "ja"] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
            .env("SUZAKU_WASHROOM_TEST_CHILD", language)
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
            text.contains(&format!("WASHROOM_OLD_CONTROLS_OK:{language}:")),
            "{text}"
        );
        assert!(
            output.status.success(),
            "{text}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            text.contains(&format!("WASHROOM_NEW_CHOICES_OK:{language}:")),
            "{text}"
        );
        assert!(
            text.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{text}"
        );
    }
}
