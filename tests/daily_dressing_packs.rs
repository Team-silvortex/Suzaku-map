//! Private startup catalogs for dressing fallback; never personal input stores.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_DRESSING_PACKS_CHILD";
const DONE: &str = "SUZAKU_DAILY_DRESSING_PACKS_DONE";

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fresh(name: &str, languages: &[(&str, usize)], body: impl FnOnce()) {
    let count: usize = languages.iter().map(|(_, n)| n).sum();
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
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{name}: {out}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(out.contains("running 1 test"));
        assert!(out.contains(&format!("test {name} ...")));
        assert!(out.contains(&format!("{DONE}:{name}:{count}")), "{out}");
        assert!(out.contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
        return;
    }
    let path = std::env::temp_dir().join(format!("suzaku-dressing-packs-{}", std::process::id()));
    // Arm cleanup only after exclusive creation establishes ownership.
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
    assert_eq!(selected.len(), count);
    let ids: HashSet<_> = selected.iter().map(|p| p.manifest.id.clone()).collect();
    assert_eq!(ids.len(), count);
    for &(language, n) in languages {
        assert_eq!(
            selected
                .iter()
                .filter(|p| p.manifest.language == language)
                .count(),
            n
        );
    }
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), count);
    assert_eq!(report.loaded.iter().cloned().collect::<HashSet<_>>(), ids);
    body();
    println!("{DONE}:{name}:{count}");
}
fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local
                && c.label == c.text
                && c.score.is_finite())
    );
    assert_eq!(
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len(),
        ime.candidates().len()
    );
    assert!(ime.candidates().iter().any(|c| c.text == seed));
    ime
}
fn at(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}
fn commit(language: &str, seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(language, seed);
    ime.select_candidate(at(&ime, text, kind));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    assert!(!ime.commit(CommitOptions { force: true }).ok);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn check(language: &str, seed: &str, word: &str, sentences: &[&str]) {
    let ime = engine(language, seed);
    let word_kind = if language == "ja" && seed == word {
        // Canonical Japanese raw text is deduplicated, not duplicated as a
        // second Word. All non-identical spellings remain strict Word checks.
        let raw = ime.candidates().iter().find(|c| c.text == seed).unwrap();
        assert!(matches!(
            raw.kind,
            CandidateKind::Literal | CandidateKind::Word
        ));
        raw.kind
    } else {
        CandidateKind::Word
    };
    assert!(at(&ime, word, word_kind) < PAGE_SIZE);
    commit(language, seed, word, word_kind);
    // Loaded packs can use page two; both authors must remain selectable
    // inside the existing twelve-row limit, without hard-pinning local text.
    for &sentence in sentences {
        at(&ime, sentence, CandidateKind::Sentence);
        commit(language, seed, sentence, CandidateKind::Sentence);
    }
    let raw = ime.candidates().iter().find(|c| c.text == seed).unwrap();
    assert!(matches!(
        raw.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    commit(language, seed, seed, raw.kind);
}

#[test]
fn dressing_private_nine_japanese_packs_keep_new_pairs_and_existing_shoes() {
    const NAME: &str = "dressing_private_nine_japanese_packs_keep_new_pairs_and_existing_shoes";
    fresh(NAME, &[("ja", 9)], || {
        for (full, kana, word, partial, sentences) in [
            (
                "fasuna-",
                "ふぁすなー",
                "ファスナー",
                "fasuna",
                [
                    "ファスナーを最後まで閉めます。",
                    "ファスナーが引っかかったので、ゆっくり動かします。",
                ],
            ),
            (
                "botan",
                "ぼたん",
                "ボタン",
                "bota",
                [
                    "ボタンを留めてから出かけます。",
                    "ボタンが一つ取れてしまいました。",
                ],
            ),
            (
                "sode",
                "そで",
                "袖",
                "sod",
                [
                    "袖を少しまくってから洗い物をします。",
                    "袖が長いので、少し折り返します。",
                ],
            ),
            (
                "kutsuhimo",
                "くつひも",
                "靴ひも",
                "kutsuhim",
                [
                    "靴ひもを結び直してから歩きます。",
                    "靴ひもがほどけたので、少し待ってください。",
                ],
            ),
            (
                "poketto",
                "ぽけっと",
                "ポケット",
                "pokett",
                [
                    "ポケットに鍵を入れておきます。",
                    "ポケットの中を確認してから洗濯します。",
                ],
            ),
            (
                "kitsui",
                "きつい",
                "きつい",
                "kitsu",
                [
                    "きつい服は動きにくいです。",
                    "きついので、一つ大きいサイズを試します。",
                ],
            ),
        ] {
            for seed in [
                full.to_owned(),
                full.to_ascii_uppercase(),
                kana.into(),
                word.into(),
                partial.into(),
                format!("{word} "),
            ] {
                check("ja", &seed, word, &sentences);
            }
        }
        check(
            "ja",
            "kutsu",
            "靴",
            &[
                "靴を履いたら、すぐに出ます。",
                "靴は歩きやすいものを選びます。",
            ],
        );
        check(
            "ja",
            "uwagi",
            "上着",
            &[
                "上着を持っていくので、少し待ってください。",
                "上着は鞄に入れておきます。",
            ],
        );
    });
}

#[test]
fn dressing_private_sixteen_english_chinese_packs_keep_new_pairs_and_old_carry() {
    const NAME: &str =
        "dressing_private_sixteen_english_chinese_packs_keep_new_pairs_and_old_carry";
    fresh(NAME, &[("en", 8), ("zh-Hans", 8)], || {
        for (context, partial, word, sentence) in [
            (
                "my shirt feels",
                "tigh",
                "tight",
                "my shirt feels tight when i sit down.",
            ),
            (
                "my shirt feels",
                "loos",
                "loose",
                "my shirt feels loose, so i'll try a smaller one.",
            ),
            (
                "the sleeves on this shirt are",
                "lon",
                "long",
                "the sleeves on this shirt are long, so i'll fold them back.",
            ),
            (
                "the sleeves on this shirt are",
                "shor",
                "short",
                "the sleeves on this shirt are short, so i'll wear a jacket over it.",
            ),
            (
                "the zipper on my jacket is",
                "stuc",
                "stuck",
                "the zipper on my jacket is stuck near the bottom.",
            ),
            (
                "the zipper on my jacket is",
                "ope",
                "open",
                "the zipper on my jacket is open, so i'll close it before we leave.",
            ),
            (
                "before leaving i'll adjust my",
                "bel",
                "belt",
                "before leaving i'll adjust my belt so it fits better.",
            ),
            (
                "before leaving i'll adjust my",
                "scar",
                "scarf",
                "before leaving i'll adjust my scarf to keep it in place.",
            ),
            (
                "my glasses are inside the",
                "cas",
                "case",
                "my glasses are inside the case on the shelf.",
            ),
            (
                "my glasses are inside the",
                "ba",
                "bag",
                "my glasses are inside the bag next to my notebook.",
            ),
            (
                "i'll change into a",
                "shir",
                "shirt",
                "i'll change into a shirt before we go out.",
            ),
            (
                "i'll change into a",
                "sweate",
                "sweater",
                "i'll change into a sweater because it's cold outside.",
            ),
        ] {
            for seed in [format!("{context} "), format!("{context} {partial}")] {
                check("en", &seed, &format!("{context} {word}"), &[sentence]);
            }
        }
        for (full, partial, word, sentences) in [
            (
                "la'lian'ka'zhu'le",
                "la'lian'ka'zhu'l",
                "拉链卡住了",
                [
                    "拉链卡住了，我先看看有没有夹到衣角。",
                    "拉链卡住了，先别用力拉。",
                ],
            ),
            (
                "niu'kou'diao'le",
                "niu'kou'diao'l",
                "纽扣掉了",
                ["纽扣掉了，我先把它收好。", "纽扣掉了，这件衣服先放一边。"],
            ),
            (
                "xiu'zi'you'dian'duan",
                "xiu'zi'you'dian'dua",
                "袖子有点短",
                [
                    "袖子有点短，我再换一件试试。",
                    "袖子有点短，抬手的时候不太方便。",
                ],
            ),
            (
                "xie'dai'you'song'le",
                "xie'dai'you'song'l",
                "鞋带又松了",
                ["鞋带又松了，我先重新系一下。", "鞋带又松了，等我一下再走。"],
            ),
            (
                "ku'yao'you'dian'jin",
                "ku'yao'you'dian'ji",
                "裤腰有点紧",
                [
                    "裤腰有点紧，我想换一条试试。",
                    "裤腰有点紧，坐下来不太舒服。",
                ],
            ),
            (
                "ling'kou'you'dian'da",
                "ling'kou'you'dian'd",
                "领口有点大",
                [
                    "领口有点大，我换一件比较一下。",
                    "领口有点大，这件先放回去吧。",
                ],
            ),
        ] {
            for reading in [full, partial] {
                for seed in [
                    reading.to_owned(),
                    reading.replace("'", " "),
                    reading.replace("'", ""),
                    reading.replace("'", " ").to_ascii_uppercase(),
                ] {
                    check("zh-Hans", &seed, word, &sentences);
                }
            }
        }
        check(
            "en",
            "please put on your jack",
            "please put on your jacket",
            &["please put on your jacket before we leave."],
        );
        check(
            "zh-Hans",
            "ba la lian la shang",
            "把拉链拉上",
            &[
                "把拉链拉上再出门，外面风有点大。",
                "把拉链拉上，别把衣角夹住了。",
            ],
        );
    });
}
