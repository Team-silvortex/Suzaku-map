//! Fresh private pack stores for wash-item fallback; no personal stores or input services.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::english::is_known_english_word,
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_HYGIENE_PACKS_CHILD";
const DONE: &str = "SUZAKU_DAILY_HYGIENE_PACKS_DONE";

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fresh(name: &str, languages: &[(&str, usize)], body: impl FnOnce()) {
    let expected_count: usize = languages.iter().map(|(_, count)| count).sum();
    if std::env::var(CHILD).as_deref() != Ok(name) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(CHILD, name)
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fresh {name} failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("running 1 test"), "{stdout}");
        // The completion marker is printed only after initialization and all checks.
        assert!(
            stdout.contains(&format!("{DONE}:{name}:{expected_count}")),
            "{stdout}"
        );
        assert!(stdout.contains(&format!("test {name} ...")), "{stdout}");
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{stdout}"
        );
        return;
    }
    let path =
        std::env::temp_dir().join(format!("suzaku-daily-hygiene-packs-{}", std::process::id()));
    // Acquire ownership by exclusive creation before arming recursive cleanup.
    fs::create_dir(&path).unwrap();
    let fixture = Fixture(path);
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    let selected: Vec<_> = packs::recommended()
        .into_iter()
        .filter(|pack| {
            languages
                .iter()
                .any(|(language, _)| pack.manifest.language == *language)
        })
        .collect();
    assert_eq!(selected.len(), expected_count);
    for &(language, count) in languages {
        assert_eq!(
            selected
                .iter()
                .filter(|pack| pack.manifest.language == language)
                .count(),
            count,
            "recommended {language} count changed"
        );
    }
    let ids: HashSet<_> = selected
        .iter()
        .map(|pack| pack.manifest.id.clone())
        .collect();
    assert_eq!(ids.len(), expected_count);
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), expected_count);
    assert_eq!(report.loaded.iter().cloned().collect::<HashSet<_>>(), ids);
    body();
    println!("{DONE}:{name}:{expected_count}");
}

fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} {text:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        })
}
fn first_page(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert!(index(ime, text, kind) < PAGE_SIZE, "{:?}", ime.candidates());
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(
        ime.candidates()
            .iter()
            .any(|candidate| candidate.text == seed)
    );
    assert!(ime.candidates().iter().all(|candidate| {
        candidate.source == CandidateSource::Local && (0.0..=100.0).contains(&candidate.score)
    }));
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn commit(language: &str, seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(language, seed);
    draft(&ime, seed);
    ime.select_candidate(index(&ime, text, kind));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn raw(language: &str, seed: &str) {
    let ime = engine(language, seed);
    let kind = ime
        .candidates()
        .iter()
        .find(|candidate| candidate.text == seed)
        .unwrap()
        .kind;
    assert!(matches!(kind, CandidateKind::Literal | CandidateKind::Word));
    commit(language, seed, seed, kind);
}

// Independent frozen expectations, not includes of another executable's tests.
const JAPANESE: &[(&str, &str, &str, &str, [&str; 2])] = &[
    (
        "shanpu-",
        "しゃんぷー",
        "シャンプー",
        "shanpu",
        [
            "シャンプーの詰め替えはありますか。",
            "シャンプーが少なくなったので、買い物のメモに書いておきます。",
        ],
    ),
    (
        "rinsu",
        "りんす",
        "リンス",
        "rins",
        [
            "リンスはどこに置きましたか。",
            "リンスを買う前に、家にある分を確認します。",
        ],
    ),
    (
        "handoso-pu",
        "はんどそーぷ",
        "ハンドソープ",
        "handoso-p",
        [
            "ハンドソープの残りを確認します。",
            "ハンドソープは洗面台の横にあります。",
        ],
    ),
    (
        "heaburashi",
        "へあぶらし",
        "ヘアブラシ",
        "heaburash",
        [
            "ヘアブラシはどこに置きましたか。",
            "ヘアブラシを洗面所に戻しておきます。",
        ],
    ),
    (
        "so-puke-su",
        "そーぷけーす",
        "ソープケース",
        "so-puke-s",
        [
            "ソープケースを洗っておきます。",
            "ソープケースは棚の上にあります。",
        ],
    ),
    (
        "senganryou",
        "せんがんりょう",
        "洗顔料",
        "senganryo",
        [
            "洗顔料を買い物のメモに追加します。",
            "洗顔料の予備はまだあります。",
        ],
    ),
];

#[test]
fn hygiene_private_nine_japanese_packs_keep_new_pairs_and_old_wash_and_travel_choices() {
    const NAME: &str =
        "hygiene_private_nine_japanese_packs_keep_new_pairs_and_old_wash_and_travel_choices";
    fresh(NAME, &[("ja", 9)], || {
        for &(romaji, kana, word, partial, sentences) in JAPANESE {
            for seed in [
                romaji.to_owned(),
                romaji.to_ascii_uppercase(),
                kana.into(),
                partial.into(),
            ] {
                let ime = engine("ja", &seed);
                draft(&ime, &seed);
                first_page(&ime, word, CandidateKind::Word);
                // Optional pack competition may use page two; both authors
                // must remain reachable inside the unchanged twelve-slot pool.
                for sentence in sentences {
                    index(&ime, sentence, CandidateKind::Sentence);
                    commit("ja", &seed, sentence, CandidateKind::Sentence);
                }
                commit("ja", &seed, word, CandidateKind::Word);
                raw("ja", &seed);
            }
            for seed in [word.to_owned(), format!("{word} ")] {
                let ime = engine("ja", &seed);
                draft(&ime, &seed);
                for sentence in sentences {
                    index(&ime, sentence, CandidateKind::Sentence);
                    // Japanese separator spaces do not enter authored payload.
                    commit("ja", &seed, sentence, CandidateKind::Sentence);
                }
                raw("ja", &seed);
            }
        }
        for (seed, word, sentences) in [
            (
                "sengan",
                "洗顔",
                [
                    "洗顔を済ませてから着替えます。",
                    "洗顔が終わったら声をかけます。",
                ],
            ),
            (
                "thisshu",
                "ティッシュ",
                [
                    "ティッシュが残り少なくなりました。",
                    "ティッシュを一箱買い足しておきます。",
                ],
            ),
            (
                "yoyakubangou",
                "予約番号",
                [
                    "予約番号を確認します。",
                    "予約番号はどこに書いてありますか。",
                ],
            ),
        ] {
            let ime = engine("ja", seed);
            draft(&ime, seed);
            first_page(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                index(&ime, sentence, CandidateKind::Sentence);
                commit("ja", seed, sentence, CandidateKind::Sentence);
            }
            commit("ja", seed, word, CandidateKind::Word);
            raw("ja", seed);
        }
        for (seed, word) in [
            ("sekken", "石鹸"),
            ("haburashi", "歯ブラシ"),
            ("hamigakiko", "歯磨き粉"),
        ] {
            first_page(&engine("ja", seed), word, CandidateKind::Word);
            commit("ja", seed, word, CandidateKind::Word);
            raw("ja", seed);
        }
    });
}

const ENGLISH_FORMS: &[&str] = &[
    "paper",
    "soap",
    "shampoo",
    "toothpaste",
    "tissue",
    "tissues",
    "conditioner",
    "mouthwash",
];
const ENGLISH_CASES: &[(&str, &str, &str, &str)] = &[
    (
        "could you bring me some",
        "pape",
        "paper",
        "could you bring me some paper from the desk?",
    ),
    (
        "could you bring me some",
        "tissue",
        "tissues",
        "could you bring me some tissues from the bathroom?",
    ),
    (
        "we've run out of",
        "soa",
        "soap",
        "we've run out of soap by the sink.",
    ),
    (
        "we've run out of",
        "toothpast",
        "toothpaste",
        "we've run out of toothpaste so i'll add it to the list.",
    ),
    (
        "i've left the",
        "shampo",
        "shampoo",
        "i've left the shampoo on the bathroom shelf.",
    ),
    (
        "i've left the",
        "conditione",
        "conditioner",
        "i've left the conditioner next to the shampoo.",
    ),
    (
        "could you hand me a",
        "tissu",
        "tissue",
        "could you hand me a tissue from that box?",
    ),
    (
        "could you hand me a",
        "towe",
        "towel",
        "could you hand me a towel from the shelf?",
    ),
    (
        "there's another bottle of",
        "shampo",
        "shampoo",
        "there's another bottle of shampoo in the cupboard.",
    ),
    (
        "there's another bottle of",
        "mouthwas",
        "mouthwash",
        "there's another bottle of mouthwash on the bathroom shelf.",
    ),
    (
        "please check if we need",
        "toothpast",
        "toothpaste",
        "please check if we need toothpaste before you go out.",
    ),
    (
        "please check if we need",
        "conditione",
        "conditioner",
        "please check if we need conditioner before buying more.",
    ),
];
const CHINESE_WORDS: &[(&str, &str)] = &[
    ("zhi'zhang", "纸张"),
    ("hu'fa'su", "护发素"),
    ("hu'shou'shuang", "护手霜"),
    ("run'chun'gao", "润唇膏"),
    ("mian'jin'zhi", "面巾纸"),
    ("ca'shou'zhi", "擦手纸"),
    ("fei'zao'he", "肥皂盒"),
    ("ti'huan'shua'tou", "替换刷头"),
];
const CHINESE_CASES: &[(&str, &str, &str, [&str; 2])] = &[
    (
        "zhi'zhang",
        "zhi'zhan",
        "纸张",
        ["纸张放在打印机旁边了。", "纸张还有一些，我先拿几张过来。"],
    ),
    (
        "xiang'zao",
        "xiang'za",
        "香皂",
        ["香皂放在洗手台旁边。", "香皂还有一块，先不用再找了。"],
    ),
    (
        "xi'fa'shui",
        "xi'fa'shu",
        "洗发水",
        ["洗发水在浴室的架子上。", "洗发水我已经放进洗漱包了。"],
    ),
    (
        "hu'fa'su",
        "hu'fa's",
        "护发素",
        ["护发素和洗发水分开放着。", "护发素还有一瓶没开封。"],
    ),
    (
        "ya'gao",
        "ya'ga",
        "牙膏",
        ["牙膏我放在牙刷旁边了。", "牙膏快用完了，我先记在清单上。"],
    ),
    (
        "ya'shua",
        "ya'shu",
        "牙刷",
        ["牙刷放在右边的杯子里。", "牙刷我已经装进洗漱包了。"],
    ),
];

#[test]
fn hygiene_private_sixteen_english_chinese_packs_keep_new_terms_and_old_paper_choices() {
    const NAME: &str =
        "hygiene_private_sixteen_english_chinese_packs_keep_new_terms_and_old_paper_choices";
    fresh(NAME, &[("en", 8), ("zh-Hans", 8)], || {
        for word in ENGLISH_FORMS {
            assert!(is_known_english_word(word), "{word}");
            let ime = engine("en", word);
            draft(&ime, word);
            assert_eq!(ime.candidates()[0].text, *word);
            assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
            commit("en", word, word, CandidateKind::Word);
        }
        for &(context, partial, next, sentence) in ENGLISH_CASES {
            let word = format!("{context} {next}");
            for seed in [format!("{context} "), format!("{context} {partial}")] {
                let ime = engine("en", &seed);
                draft(&ime, &seed);
                first_page(&ime, &word, CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
                commit("en", &seed, &word, CandidateKind::Word);
                commit("en", &seed, sentence, CandidateKind::Sentence);
                raw("en", &seed);
            }
        }
        for &(reading, word) in CHINESE_WORDS {
            for seed in [reading.to_owned(), reading.replace('\'', " ")] {
                first_page(&engine("zh-Hans", &seed), word, CandidateKind::Word);
                commit("zh-Hans", &seed, word, CandidateKind::Word);
                raw("zh-Hans", &seed);
            }
        }
        for &(reading, partial, word, sentences) in CHINESE_CASES {
            for seed in [reading, partial] {
                let ime = engine("zh-Hans", seed);
                draft(&ime, seed);
                first_page(&ime, word, CandidateKind::Word);
                for sentence in sentences {
                    index(&ime, sentence, CandidateKind::Sentence);
                    commit("zh-Hans", seed, sentence, CandidateKind::Sentence);
                }
                raw("zh-Hans", seed);
            }
        }
        let ime = engine("zh-Hans", "zhi'jin");
        draft(&ime, "zhi'jin");
        first_page(&ime, "纸巾", CandidateKind::Word);
        for sentence in ["纸巾放在桌上了。", "纸巾快用完了。"] {
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit("zh-Hans", "zhi'jin", sentence, CandidateKind::Sentence);
        }
        commit("zh-Hans", "zhi'jin", "纸巾", CandidateKind::Word);
        raw("zh-Hans", "zhi'jin");
        let seed = "we're nearly out of detergen";
        let word = "we're nearly out of detergent";
        let sentence = "we're nearly out of detergent so i'll add it to the list.";
        let ime = engine("en", seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        first_page(&ime, sentence, CandidateKind::Sentence);
        commit("en", seed, word, CandidateKind::Word);
        commit("en", seed, sentence, CandidateKind::Sentence);
        raw("en", seed);
        for (language, seed) in [
            ("en", "https://example.invalid/shampoo"),
            ("zh-Hans", "https://example.invalid/zhi'zhang"),
        ] {
            let ime = engine(language, seed);
            draft(&ime, seed);
            assert!(ime.candidates().iter().all(|candidate| {
                !ENGLISH_CASES
                    .iter()
                    .any(|(_, _, _, sentence)| candidate.text.contains(*sentence))
                    && !CHINESE_CASES
                        .iter()
                        .flat_map(|(_, _, _, sentences)| sentences)
                        .any(|sentence| candidate.text.contains(*sentence))
            }));
            raw(language, seed);
        }
    });
}
