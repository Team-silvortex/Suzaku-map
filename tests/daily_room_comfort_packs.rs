//! Private startup catalogs for room-comfort fallback; never personal input stores.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_ROOM_COMFORT_PACKS_CHILD";
const DONE: &str = "SUZAKU_DAILY_ROOM_COMFORT_PACKS_DONE";

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
    let path =
        std::env::temp_dir().join(format!("suzaku-room-comfort-packs-{}", std::process::id()));
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
fn room_private_nine_japanese_packs_keep_all_six_pairs_and_old_ventilation() {
    const NAME: &str = "room_private_nine_japanese_packs_keep_all_six_pairs_and_old_ventilation";
    fresh(NAME, &[("ja", 9)], || {
        for seed in [
            "shoumei".to_owned(),
            "shoumei".to_ascii_uppercase(),
            "しょうめい".into(),
            "照明".into(),
            "shoume".into(),
            format!("{} ", "照明"),
        ] {
            check(
                "ja",
                &seed,
                "照明",
                &[
                    "照明を少し暗くしてもいいですか。",
                    "照明をつけると、手元がよく見えます。",
                ],
            );
        }
        for seed in [
            "ka-ten".to_owned(),
            "ka-ten".to_ascii_uppercase(),
            "かーてん".into(),
            "カーテン".into(),
            "ka-te".into(),
            format!("{} ", "カーテン"),
        ] {
            check(
                "ja",
                &seed,
                "カーテン",
                &[
                    "カーテンを開けると、部屋が明るくなります。",
                    "カーテンを閉めてから着替えます。",
                ],
            );
        }
        for seed in [
            "kazetooshi".to_owned(),
            "kazetooshi".to_ascii_uppercase(),
            "かぜとおし".into(),
            "風通し".into(),
            "kazetoosh".into(),
            format!("{} ", "風通し"),
        ] {
            check(
                "ja",
                &seed,
                "風通し",
                &[
                    "風通しをよくするために、窓を少し開けます。",
                    "風通しがよくて、気持ちがいいですね。",
                ],
            );
        }
        for seed in [
            "shitsuon".to_owned(),
            "shitsuon".to_ascii_uppercase(),
            "しつおん".into(),
            "室温".into(),
            "shitsuo".into(),
            format!("{} ", "室温"),
        ] {
            check(
                "ja",
                &seed,
                "室温",
                &[
                    "室温を確かめてから、エアコンを調節します。",
                    "室温がちょうどよくて、過ごしやすいです。",
                ],
            );
        }
        for seed in [
            "eakon".to_owned(),
            "eakon".to_ascii_uppercase(),
            "えあこん".into(),
            "エアコン".into(),
            "eako".into(),
            format!("{} ", "エアコン"),
        ] {
            check(
                "ja",
                &seed,
                "エアコン",
                &[
                    "エアコンの風が直接当たらない席に移ります。",
                    "エアコンを少し弱くしてもいいですか。",
                ],
            );
        }
        for seed in [
            "kankisen".to_owned(),
            "kankisen".to_ascii_uppercase(),
            "かんきせん".into(),
            "換気扇".into(),
            "kankise".into(),
            format!("{} ", "換気扇"),
        ] {
            check(
                "ja",
                &seed,
                "換気扇",
                &[
                    "換気扇を回して、こもった空気を外に出します。",
                    "換気扇を止めてから部屋を出ます。",
                ],
            );
        }
        check(
            "ja",
            "kanki",
            "換気",
            &[
                "換気のために窓を開けます。",
                "換気をして空気を入れ替えましょう。",
            ],
        );
    });
}

#[test]
fn room_private_sixteen_english_chinese_packs_keep_all_branches_and_old_dressing() {
    const NAME: &str =
        "room_private_sixteen_english_chinese_packs_keep_all_branches_and_old_dressing";
    fresh(NAME, &[("en", 8), ("zh-Hans", 8)], || {
        check(
            "en",
            "the curtains by my desk are ",
            "the curtains by my desk are open",
            &[
                "the curtains by my desk are open to let the sunlight in.",
                "the curtains by my desk are closed to keep the room cool.",
            ],
        );
        check(
            "en",
            "the curtains by my desk are ope",
            "the curtains by my desk are open",
            &["the curtains by my desk are open to let the sunlight in."],
        );
        check(
            "en",
            "the curtains by my desk are ",
            "the curtains by my desk are closed",
            &[
                "the curtains by my desk are open to let the sunlight in.",
                "the curtains by my desk are closed to keep the room cool.",
            ],
        );
        check(
            "en",
            "the curtains by my desk are close",
            "the curtains by my desk are closed",
            &["the curtains by my desk are closed to keep the room cool."],
        );
        check(
            "en",
            "the glare on my screen makes ",
            "the glare on my screen makes reading",
            &[
                "the glare on my screen makes reading difficult at this desk.",
                "the glare on my screen makes work difficult in the afternoon.",
            ],
        );
        check(
            "en",
            "the glare on my screen makes readin",
            "the glare on my screen makes reading",
            &["the glare on my screen makes reading difficult at this desk."],
        );
        check(
            "en",
            "the glare on my screen makes ",
            "the glare on my screen makes work",
            &[
                "the glare on my screen makes reading difficult at this desk.",
                "the glare on my screen makes work difficult in the afternoon.",
            ],
        );
        check(
            "en",
            "the glare on my screen makes wor",
            "the glare on my screen makes work",
            &["the glare on my screen makes work difficult in the afternoon."],
        );
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
            "en",
            "the air in this room feels fres",
            "the air in this room feels fresh",
            &["the air in this room feels fresh with the window open."],
        );
        check(
            "en",
            "the air in this room feels ",
            "the air in this room feels stuffy",
            &[
                "the air in this room feels fresh with the window open.",
                "the air in this room feels stuffy, so i'll open the window for a while.",
            ],
        );
        check(
            "en",
            "the air in this room feels stuff",
            "the air in this room feels stuffy",
            &["the air in this room feels stuffy, so i'll open the window for a while."],
        );
        check(
            "en",
            "this part of the room feels ",
            "this part of the room feels warm",
            &[
                "this part of the room feels warm even with the fan on.",
                "this part of the room feels cool enough to sit here for a while.",
            ],
        );
        check(
            "en",
            "this part of the room feels war",
            "this part of the room feels warm",
            &["this part of the room feels warm even with the fan on."],
        );
        check(
            "en",
            "this part of the room feels ",
            "this part of the room feels cool",
            &[
                "this part of the room feels warm even with the fan on.",
                "this part of the room feels cool enough to sit here for a while.",
            ],
        );
        check(
            "en",
            "this part of the room feels coo",
            "this part of the room feels cool",
            &["this part of the room feels cool enough to sit here for a while."],
        );
        check(
            "en",
            "without more light i can't ",
            "without more light i can't read",
            &[
                "without more light i can't read the small print under this lamp.",
                "without more light i can't see where i put my keys.",
            ],
        );
        check(
            "en",
            "without more light i can't rea",
            "without more light i can't read",
            &["without more light i can't read the small print under this lamp."],
        );
        check(
            "en",
            "without more light i can't ",
            "without more light i can't see",
            &[
                "without more light i can't read the small print under this lamp.",
                "without more light i can't see where i put my keys.",
            ],
        );
        check(
            "en",
            "without more light i can't se",
            "without more light i can't see",
            &["without more light i can't see where i put my keys."],
        );
        check(
            "en",
            "the lighting near my chair is ",
            "the lighting near my chair is bright",
            &[
                "the lighting near my chair is bright enough for reading.",
                "the lighting near my chair is dim, so i'll sit next to the window.",
            ],
        );
        check(
            "en",
            "the lighting near my chair is brigh",
            "the lighting near my chair is bright",
            &["the lighting near my chair is bright enough for reading."],
        );
        check(
            "en",
            "the lighting near my chair is ",
            "the lighting near my chair is dim",
            &[
                "the lighting near my chair is bright enough for reading.",
                "the lighting near my chair is dim, so i'll sit next to the window.",
            ],
        );
        check(
            "en",
            "the lighting near my chair is di",
            "the lighting near my chair is dim",
            &["the lighting near my chair is dim, so i'll sit next to the window."],
        );
        check("en", "curtains", "curtains", &[]);
        check("en", "lamp", "lamp", &[]);
        check("en", "lamps", "lamps", &[]);
        check("en", "fan", "fan", &[]);
        check("en", "fans", "fans", &[]);
        check("en", "sunlight", "sunlight", &[]);
        check("en", "lighting", "lighting", &[]);
        check("en", "stuffy", "stuffy", &[]);
        check(
            "zh-Hans",
            "fang'jian'you'dian'men",
            "房间有点闷",
            &[
                "房间有点闷，我想开一会儿窗。",
                "房间有点闷，我们换个位置坐吧。",
            ],
        );
        check(
            "zh-Hans",
            "fang jian you dian men",
            "房间有点闷",
            &[
                "房间有点闷，我想开一会儿窗。",
                "房间有点闷，我们换个位置坐吧。",
            ],
        );
        check(
            "zh-Hans",
            "fangjianyoudianmen",
            "房间有点闷",
            &[
                "房间有点闷，我想开一会儿窗。",
                "房间有点闷，我们换个位置坐吧。",
            ],
        );
        check(
            "zh-Hans",
            "fang jian you dian me",
            "房间有点闷",
            &[
                "房间有点闷，我想开一会儿窗。",
                "房间有点闷，我们换个位置坐吧。",
            ],
        );
        check(
            "zh-Hans",
            "kai'chuang'tong'tong'feng",
            "开窗通通风",
            &[
                "开窗通通风，我先把桌上的纸压好。",
                "开窗通通风，等会儿再关上。",
            ],
        );
        check(
            "zh-Hans",
            "kai chuang tong tong feng",
            "开窗通通风",
            &[
                "开窗通通风，我先把桌上的纸压好。",
                "开窗通通风，等会儿再关上。",
            ],
        );
        check(
            "zh-Hans",
            "kaichuangtongtongfeng",
            "开窗通通风",
            &[
                "开窗通通风，我先把桌上的纸压好。",
                "开窗通通风，等会儿再关上。",
            ],
        );
        check(
            "zh-Hans",
            "kai chuang tong tong fen",
            "开窗通通风",
            &[
                "开窗通通风，我先把桌上的纸压好。",
                "开窗通通风，等会儿再关上。",
            ],
        );
        check(
            "zh-Hans",
            "ba'chuang'lian'la'kai",
            "把窗帘拉开",
            &[
                "把窗帘拉开，我想让房间亮一点。",
                "把窗帘拉开，看看外面的天气。",
            ],
        );
        check(
            "zh-Hans",
            "ba chuang lian la kai",
            "把窗帘拉开",
            &[
                "把窗帘拉开，我想让房间亮一点。",
                "把窗帘拉开，看看外面的天气。",
            ],
        );
        check(
            "zh-Hans",
            "bachuanglianlakai",
            "把窗帘拉开",
            &[
                "把窗帘拉开，我想让房间亮一点。",
                "把窗帘拉开，看看外面的天气。",
            ],
        );
        check(
            "zh-Hans",
            "ba chuang lian la ka",
            "把窗帘拉开",
            &[
                "把窗帘拉开，我想让房间亮一点。",
                "把窗帘拉开，看看外面的天气。",
            ],
        );
        check(
            "zh-Hans",
            "deng'guang'you'dian'ci'yan",
            "灯光有点刺眼",
            &[
                "灯光有点刺眼，我想换个位置坐。",
                "灯光有点刺眼，能不能调暗一点？",
            ],
        );
        check(
            "zh-Hans",
            "deng guang you dian ci yan",
            "灯光有点刺眼",
            &[
                "灯光有点刺眼，我想换个位置坐。",
                "灯光有点刺眼，能不能调暗一点？",
            ],
        );
        check(
            "zh-Hans",
            "dengguangyoudianciyan",
            "灯光有点刺眼",
            &[
                "灯光有点刺眼，我想换个位置坐。",
                "灯光有点刺眼，能不能调暗一点？",
            ],
        );
        check(
            "zh-Hans",
            "deng guang you dian ci ya",
            "灯光有点刺眼",
            &[
                "灯光有点刺眼，我想换个位置坐。",
                "灯光有点刺眼，能不能调暗一点？",
            ],
        );
        check(
            "zh-Hans",
            "feng'shan'tiao'xiao'yi'dian",
            "风扇调小一点",
            &[
                "风扇调小一点，我这边风有点大。",
                "风扇调小一点，别对着桌上的纸吹。",
            ],
        );
        check(
            "zh-Hans",
            "feng shan tiao xiao yi dian",
            "风扇调小一点",
            &[
                "风扇调小一点，我这边风有点大。",
                "风扇调小一点，别对着桌上的纸吹。",
            ],
        );
        check(
            "zh-Hans",
            "fengshantiaoxiaoyidian",
            "风扇调小一点",
            &[
                "风扇调小一点，我这边风有点大。",
                "风扇调小一点，别对着桌上的纸吹。",
            ],
        );
        check(
            "zh-Hans",
            "feng shan tiao xiao yi dia",
            "风扇调小一点",
            &[
                "风扇调小一点，我这边风有点大。",
                "风扇调小一点，别对着桌上的纸吹。",
            ],
        );
        check(
            "zh-Hans",
            "kong'tiao'wen'du'tiao'gao'yi'dian",
            "空调温度调高一点",
            &[
                "空调温度调高一点，我觉得有点冷。",
                "空调温度调高一点，先这样试一会儿吧。",
            ],
        );
        check(
            "zh-Hans",
            "kong tiao wen du tiao gao yi dian",
            "空调温度调高一点",
            &[
                "空调温度调高一点，我觉得有点冷。",
                "空调温度调高一点，先这样试一会儿吧。",
            ],
        );
        check(
            "zh-Hans",
            "kongtiaowendutiaogaoyidian",
            "空调温度调高一点",
            &[
                "空调温度调高一点，我觉得有点冷。",
                "空调温度调高一点，先这样试一会儿吧。",
            ],
        );
        check(
            "zh-Hans",
            "kong tiao wen du tiao gao yi dia",
            "空调温度调高一点",
            &[
                "空调温度调高一点，我觉得有点冷。",
                "空调温度调高一点，先这样试一会儿吧。",
            ],
        );
        check("zh-Hans", "guang xian", "光线", &[]);
        check("zh-Hans", "tong feng", "通风", &[]);
        check("zh-Hans", "tai deng", "台灯", &[]);
        check("zh-Hans", "feng shan", "风扇", &[]);
        check("zh-Hans", "kong tiao", "空调", &[]);
        check("zh-Hans", "wen du", "温度", &[]);
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
            &["把拉链拉上再出门，外面风有点大。"],
        );
    });
}
