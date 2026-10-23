//! Private startup catalogs for stationery fallback; never personal input stores.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_STATIONERY_PACKS_CHILD";
const DONE: &str = "SUZAKU_DAILY_STATIONERY_PACKS_DONE";

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
    let path = std::env::temp_dir().join(format!(
        "suzaku-stationery-packs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
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
    // These specific roots must expose both authored choices on page one.
    // Neither the six/twelve-row budget nor local ranking is changed here.
    for &sentence in sentences {
        assert!(at(&ime, sentence, CandidateKind::Sentence) < PAGE_SIZE);
        commit(language, seed, sentence, CandidateKind::Sentence);
    }
    let raw = ime.candidates().iter().find(|c| c.text == seed).unwrap();
    assert!(matches!(
        raw.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    commit(language, seed, seed, raw.kind);
}

const EN_NEW_FORMS: [&str; 8] = [
    "pen", "pens", "pencil", "pencils", "eraser", "ruler", "sticky", "ink",
];

const EN_CONTEXTS: [&str; 6] = [
    "inside my pencil case i keep",
    "drawing a straight line is easier with a",
    "the ink in this pen has",
    "the sticky note next to my keyboard is",
    "the eraser next to my notebook is",
    "once i've finished writing i'll put the",
];

const EN_WORDS: [[&str; 2]; 6] = [
    ["pens", "pencils"],
    ["ruler", "pencil"],
    ["run", "left"],
    ["for", "from"],
    ["small", "clean"],
    ["pen", "ruler"],
];

const EN_PARTIALS: [[&str; 2]; 6] = [
    ["pen", "pencil"],
    ["rule", "penci"],
    ["ru", "lef"],
    ["fo", "fro"],
    ["smal", "clea"],
    ["pe", "rule"],
];

const EN_SENTENCES: [[&str; 2]; 6] = [
    [
        "inside my pencil case i keep pens for taking notes.",
        "inside my pencil case i keep pencils for drawing small maps.",
    ],
    [
        "drawing a straight line is easier with a ruler on the paper.",
        "drawing a straight line is easier with a pencil than with this pen.",
    ],
    [
        "the ink in this pen has run out, so i'll use a pencil.",
        "the ink in this pen has left a mark on my notebook.",
    ],
    [
        "the sticky note next to my keyboard is for the shopping list.",
        "the sticky note next to my keyboard is from our meeting this morning.",
    ],
    [
        "the eraser next to my notebook is small enough to fit in my pencil case.",
        "the eraser next to my notebook is clean, so it won't leave a mark on the paper.",
    ],
    [
        "once i've finished writing i'll put the pen back in the drawer.",
        "once i've finished writing i'll put the ruler next to my notebook.",
    ],
];

const ZH_WORDS: &[(&str, &str)] = &[
    ("qian'bi'you'dian'dun'le", "铅笔有点钝了"),
    ("zhe'ge'zi'xie'cuo'le", "这个字写错了"),
    ("bang'wo'hua'yi'tiao'xian", "帮我画一条线"),
    ("xian'na'zhang'cao'gao'zhi", "先拿张草稿纸"),
    ("bi'mao'ji'de'gai'hao", "笔帽记得盖好"),
    ("zhe'ji'zhang'zhi'ding'yi'xia", "这几张纸订一下"),
    ("qian'bi", "铅笔"),
    ("chi'zi", "尺子"),
    ("jiao'dai", "胶带"),
    ("bian'qian", "便签"),
    ("cao'gao'zhi", "草稿纸"),
    ("yuan'zhu'bi", "圆珠笔"),
];

const ZH_PARTIALS: &[(&str, &str)] = &[
    ("qian'bi'you'dian'dun'l", "铅笔有点钝了"),
    ("zhe'ge'zi'xie'cuo'l", "这个字写错了"),
    ("bang'wo'hua'yi'tiao'xia", "帮我画一条线"),
    ("xian'na'zhang'cao'gao'zh", "先拿张草稿纸"),
    ("bi'mao'ji'de'gai'ha", "笔帽记得盖好"),
    ("zhe'ji'zhang'zhi'ding'yi'xi", "这几张纸订一下"),
];

const ZH_SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "铅笔有点钝了",
        [
            "铅笔有点钝了，我先削一下再写。",
            "铅笔有点钝了，换一支尖一点的吧。",
        ],
    ),
    (
        "这个字写错了",
        [
            "这个字写错了，我先擦掉再重写。",
            "这个字写错了，在旁边改一下就好。",
        ],
    ),
    (
        "帮我画一条线",
        [
            "帮我画一条线，我想把这两段分开。",
            "帮我画一条线，用尺子画会整齐一点。",
        ],
    ),
    (
        "先拿张草稿纸",
        [
            "先拿张草稿纸，我想算一下这道题。",
            "先拿张草稿纸，这张先别放进文件夹。",
        ],
    ),
    (
        "笔帽记得盖好",
        [
            "笔帽记得盖好，我先收一下桌面。",
            "笔帽记得盖好，用完就收回笔袋吧。",
        ],
    ),
    (
        "这几张纸订一下",
        [
            "这几张纸订一下，我想把它们放在一起。",
            "这几张纸订一下，订好后放进文件夹吧。",
        ],
    ),
];

const JA_WORDS: &[(&str, &str, &str, &str)] = &[
    ("enpitsu", "えんぴつ", "エンピツ", "鉛筆"),
    ("keshigomu", "けしごむ", "ケシゴム", "消しゴム"),
    ("fusen", "ふせん", "フセン", "付箋"),
    ("jougi", "じょうぎ", "ジョウギ", "定規"),
    ("fudebako", "ふでばこ", "フデバコ", "筆箱"),
    ("hochikisu", "ほちきす", "ホチキス", "ホチキス"),
];

const JA_PARTIALS: &[&str] = &["enpit", "keshigom", "fuse", "joug", "fudebak", "hochikis"];

const JA_SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "鉛筆",
        [
            "鉛筆を一本貸してもらえますか。",
            "鉛筆で下書きをしてから、ペンで書きます。",
        ],
    ),
    (
        "消しゴム",
        [
            "消しゴムでこの線を消してもいいですか。",
            "消しゴムのかすを集めて捨てます。",
        ],
    ),
    (
        "付箋",
        [
            "付箋に書いて、机の端に貼っておきます。",
            "付箋の色を変えて、目印にします。",
        ],
    ),
    (
        "定規",
        [
            "定規でまっすぐな線を引きます。",
            "定規を使ったら、筆箱に戻します。",
        ],
    ),
    (
        "筆箱",
        [
            "筆箱に鉛筆と消しゴムを入れます。",
            "筆箱を開けて、必要なものを取り出します。",
        ],
    ),
    (
        "ホチキス",
        [
            "ホチキスでこの紙を留めてもいいですか。",
            "ホチキスの針がなくなったので、補充します。",
        ],
    ),
];

#[test]
fn stationery_private_nine_japanese_packs_keep_all_six_pairs_and_prior_room_words() {
    const NAME: &str =
        "stationery_private_nine_japanese_packs_keep_all_six_pairs_and_prior_room_words";
    fresh(NAME, &[("ja", 9)], || {
        check(
            "ja",
            "kankise",
            "換気扇",
            &[
                "換気扇を回して、こもった空気を外に出します。",
                "換気扇を止めてから部屋を出ます。",
            ],
        );
        check(
            "ja",
            "senganryo",
            "洗顔料",
            &[
                "洗顔料を買い物のメモに追加します。",
                "洗顔料の予備はまだあります。",
            ],
        );
        println!("old Japanese room and washcare controls passed");
        for (i, &(romaji, kana, _, word)) in JA_WORDS.iter().enumerate() {
            let authors = &JA_SENTENCES[i].1;
            assert_eq!(JA_SENTENCES[i].0, word);
            for seed in [
                romaji.to_owned(),
                romaji.to_ascii_uppercase(),
                kana.into(),
                word.into(),
                JA_PARTIALS[i].into(),
                format!("{word} "),
            ] {
                check("ja", &seed, word, authors);
            }
        }
    });
}
#[test]
fn stationery_private_sixteen_english_chinese_packs_keep_all_branches_and_prior_words() {
    const NAME: &str =
        "stationery_private_sixteen_english_chinese_packs_keep_all_branches_and_prior_words";
    fresh(NAME, &[("en", 8), ("zh-Hans", 8)], || {
        check(
            "en",
            "the air in this room feels ",
            "the air in this room feels fresh",
            &[
                "the air in this room feels fresh with the window open.",
                "the air in this room feels stuffy, so i'll open the window for a while.",
            ],
        );
        check(
            "zh-Hans",
            "jie zhi bi",
            "借支笔",
            &[
                "借支笔填个名字，马上还你。",
                "借支笔写个地址，用完就还给你。",
            ],
        );
        println!("old room and borrowed-pen controls passed");
        for (i, context) in EN_CONTEXTS.iter().enumerate() {
            for j in 0..2 {
                check(
                    "en",
                    &format!("{context} "),
                    &format!("{context} {}", EN_WORDS[i][j]),
                    &EN_SENTENCES[i],
                );
                check(
                    "en",
                    &format!("{context} {}", EN_PARTIALS[i][j]),
                    &format!("{context} {}", EN_WORDS[i][j]),
                    &[EN_SENTENCES[i][j]],
                );
            }
        }
        for word in EN_NEW_FORMS {
            let ime = engine("en", word);
            assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
            commit("en", word, word, CandidateKind::Word);
        }
        for (i, &(word, authors)) in ZH_SENTENCES.iter().enumerate() {
            for reading in [ZH_WORDS[i].0, ZH_PARTIALS[i].0] {
                assert_eq!(ZH_WORDS[i].1, word);
                check("zh-Hans", &reading.replace('\'', " "), word, &authors);
            }
        }
        for &(reading, word) in &ZH_WORDS[6..] {
            let seed = reading.replace('\'', " ");
            let ime = engine("zh-Hans", &seed);
            assert!(at(&ime, word, CandidateKind::Word) < PAGE_SIZE);
            commit("zh-Hans", &seed, word, CandidateKind::Word);
            commit("zh-Hans", &seed, &seed, CandidateKind::Literal);
        }
    });
}
