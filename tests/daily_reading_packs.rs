//! Reading fallback in a fresh, private sixteen-pack startup catalog.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_READING_PACKS_CHILD";
struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
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
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
    assert!(ime.candidates().iter().any(|c| c.text == seed));
    ime
}

fn commit(language: &str, seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(language, seed);
    let at = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    if matches!(kind, CandidateKind::Word | CandidateKind::Sentence) {
        assert!(at < PAGE_SIZE, "{seed}: {text:?} left page one");
    }
    ime.select_candidate(at);
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

fn choices(language: &str, seed: &str, words: &[&str], sentences: &[&str]) {
    for word in words {
        commit(language, seed, word, CandidateKind::Word);
    }
    for sentence in sentences {
        commit(language, seed, sentence, CandidateKind::Sentence);
    }
    let ime = engine(language, seed);
    let raw_kind = ime
        .candidates()
        .iter()
        .find(|c| c.text == seed)
        .unwrap()
        .kind;
    assert!(matches!(
        raw_kind,
        CandidateKind::Word | CandidateKind::Literal
    ));
    commit(language, seed, seed, raw_kind);
}

#[test]
fn reading_words_sentences_and_old_web_bookmarks_coexist_in_sixteen_private_packs() {
    if std::env::var(CHILD).as_deref() != Ok("enabled") {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "reading_words_sentences_and_old_web_bookmarks_coexist_in_sixteen_private_packs",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "enabled")
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stdout.contains("running 1 test"));
        assert!(stdout.contains("READING_OLD_CONTROLS_OK:16"));
        assert!(stdout.contains("READING_NEW_CHOICES_OK:16"));
        assert!(stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "suzaku-reading-packs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    let owned = Owned(path);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    let selected: Vec<_> = packs::recommended()
        .into_iter()
        .filter(|p| matches!(p.manifest.language.as_str(), "en" | "zh-Hans"))
        .collect();
    assert_eq!(selected.len(), 16);
    for language in ["en", "zh-Hans"] {
        assert_eq!(
            selected
                .iter()
                .filter(|p| p.manifest.language == language)
                .count(),
            8
        );
    }
    let expected: HashSet<_> = selected.iter().map(|p| p.manifest.id.clone()).collect();
    assert_eq!(expected.len(), 16);
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), 16);
    assert_eq!(
        report.loaded.iter().cloned().collect::<HashSet<_>>(),
        expected
    );

    // Check existing paper/notebook and web-reading workflows before new content.
    for (seed, sentence) in [
        (
            "i'd like to bookmark this pa",
            "i'd like to bookmark this page for later.",
        ),
        (
            "can i read this page off",
            "can i read this page offline after saving it?",
        ),
        (
            "i need a clearer sum",
            "i need a clearer summary of this chapter.",
        ),
        (
            "could i borrow your noteb",
            "could i borrow your notebook for a moment?",
        ),
    ] {
        choices("en", seed, &[], &[sentence]);
    }
    choices(
        "zh-Hans",
        "jie zhi bi",
        &["借支笔"],
        &[
            "借支笔填个名字，马上还你。",
            "借支笔写个地址，用完就还给你。",
        ],
    );
    println!("READING_OLD_CONTROLS_OK:16");

    for (seed, words, sentences) in [
        (
            "this bookmark will help me find the ",
            ["page", "chapter"],
            [
                "this bookmark will help me find the page where i stopped.",
                "this bookmark will help me find the chapter i want to read again.",
            ],
        ),
        (
            "these pages are hard to ",
            ["read", "turn"],
            [
                "these pages are hard to read in this light.",
                "these pages are hard to turn with one hand.",
            ],
        ),
        (
            "before closing this book i'll finish the ",
            ["page", "chapter"],
            [
                "before closing this book i'll finish the page i'm reading.",
                "before closing this book i'll finish the chapter and leave a bookmark.",
            ],
        ),
        (
            "the paperback on my desk is ",
            ["open", "closed"],
            [
                "the paperback on my desk is open at the page i need.",
                "the paperback on my desk is closed with a bookmark inside.",
            ],
        ),
        (
            "the hardcover on this shelf is ",
            ["mine", "yours"],
            [
                "the hardcover on this shelf is mine and you can borrow it.",
                "the hardcover on this shelf is yours and i'll bring it back tomorrow.",
            ],
        ),
        (
            "after reading these pages i'll ",
            ["put", "take"],
            [
                "after reading these pages i'll put the book back on the shelf.",
                "after reading these pages i'll take a break before the next chapter.",
            ],
        ),
    ] {
        choices(
            "en",
            seed,
            &words
                .map(|w| format!("{seed}{w}"))
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            &sentences,
        );
    }
    for (seed, word, sentences) in [
        (
            "wo du dao zhe yi ye le",
            "我读到这一页了",
            [
                "我读到这一页了，我先做个记号。",
                "我读到这一页了，明天从这里接着看。",
            ],
        ),
        (
            "shu qian jia zai zhe ye",
            "书签夹在这页",
            [
                "书签夹在这页，我下次就能找到。",
                "书签夹在这页，先别把它拿出来。",
            ],
        ),
        (
            "zhe ye hai mei kan wan",
            "这页还没看完",
            [
                "这页还没看完，我想再看一会儿。",
                "这页还没看完，先不要翻过去。",
            ],
        ),
        (
            "xian fan dao xia yi ye",
            "先翻到下一页",
            [
                "先翻到下一页，我想看看后面的图。",
                "先翻到下一页，这里等会儿再看。",
            ],
        ),
        (
            "zhe duan wo zai du yi bian",
            "这段我再读一遍",
            [
                "这段我再读一遍，我刚才没看明白。",
                "这段我再读一遍，你先别往下翻。",
            ],
        ),
        (
            "zhe ben shu xian bie shou qi lai",
            "这本书先别收起来",
            [
                "这本书先别收起来，我还要看一会儿。",
                "这本书先别收起来，先放在桌边吧。",
            ],
        ),
    ] {
        choices("zh-Hans", seed, &[word], &sentences);
    }
    println!("READING_NEW_CHOICES_OK:16");
}
