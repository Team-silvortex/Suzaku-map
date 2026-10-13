//! Optional authored internet expressions, without browsing, account or network access.
//! Each immutable activation snapshot uses a fresh child and owned test store.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{
        self,
        packs::{self, OfflinePack, PackStore},
    },
};

const INTERNET_ID: &str = "org.suzaku.en.internet";
// Fixed authored expectations, not copied from a parsed resource at test time.
const FORMS: &[&str] = &[
    "browser",
    "browsers",
    "browsing",
    "browsed",
    "bookmark",
    "bookmarks",
    "bookmarked",
    "bookmarking",
    "webpage",
    "webpages",
    "website",
    "websites",
    "download",
    "downloads",
    "downloaded",
    "downloading",
    "upload",
    "uploads",
    "uploaded",
    "uploading",
    "reload",
    "reloads",
    "reloaded",
    "reloading",
    "scroll",
    "scrolls",
    "scrolled",
    "scrolling",
    "keyword",
    "keywords",
    "query",
    "queries",
    "thumbnail",
    "thumbnails",
    "hyperlink",
    "hyperlinks",
    "comment",
    "comments",
    "commented",
    "commenting",
    "repost",
    "reposts",
    "reposted",
    "reposting",
    "subscribe",
    "subscribes",
    "subscribed",
    "subscribing",
    "unsubscribe",
    "unsubscribes",
    "unsubscribed",
    "unsubscribing",
    "username",
    "usernames",
    "login",
    "logout",
    "autofill",
    "autocomplete",
    "autosave",
    "autoplay",
    "tab",
    "tabs",
    "toolbar",
    "toolbars",
];
type InternetBranch = (&'static str, &'static str, &'static str);
const INTERNET: &[(&str, [InternetBranch; 2])] = &[
    (
        "could you open this page in a",
        [
            ("new", "ne", "could you open this page in a new tab?"),
            (
                "separate",
                "sep",
                "could you open this page in a separate window?",
            ),
        ],
    ),
    (
        "i'd like to bookmark this",
        [
            ("page", "pa", "i'd like to bookmark this page for later."),
            (
                "article",
                "ar",
                "i'd like to bookmark this article before closing the tab.",
            ),
        ],
    ),
    (
        "the page is taking a long time to",
        [
            (
                "load",
                "loa",
                "the page is taking a long time to load on my phone.",
            ),
            (
                "respond",
                "res",
                "the page is taking a long time to respond after i click.",
            ),
        ],
    ),
    (
        "could you scroll to the",
        [
            ("top", "to", "could you scroll to the top of the page?"),
            (
                "bottom",
                "bo",
                "could you scroll to the bottom to read the comments?",
            ),
        ],
    ),
    (
        "let's search for a more",
        [
            (
                "specific",
                "spec",
                "let's search for a more specific phrase this time.",
            ),
            (
                "recent",
                "rec",
                "let's search for a more recent article on this topic.",
            ),
        ],
    ),
    (
        "try adding another keyword to the",
        [
            (
                "search",
                "sea",
                "try adding another keyword to the search to narrow the results.",
            ),
            (
                "query",
                "que",
                "try adding another keyword to the query before trying again.",
            ),
        ],
    ),
    (
        "the search results look",
        [
            (
                "relevant",
                "rel",
                "the search results look relevant to our question.",
            ),
            (
                "different",
                "dif",
                "the search results look different after changing the keywords.",
            ),
        ],
    ),
    (
        "could you send me the original",
        [
            (
                "link",
                "li",
                "could you send me the original link instead of a screenshot?",
            ),
            (
                "source",
                "sou",
                "could you send me the original source of that quote?",
            ),
        ],
    ),
    (
        "i saved the download in my",
        [
            (
                "downloads",
                "down",
                "i saved the download in my downloads folder.",
            ),
            (
                "documents",
                "doc",
                "i saved the download in my documents folder for later.",
            ),
        ],
    ),
    (
        "this download is still",
        [
            (
                "running",
                "run",
                "this download is still running in the background.",
            ),
            (
                "paused",
                "pau",
                "this download is still paused while i check the connection.",
            ),
        ],
    ),
    (
        "i'll upload the file after",
        [
            (
                "checking",
                "che",
                "i'll upload the file after checking its name and contents.",
            ),
            (
                "renaming",
                "ren",
                "i'll upload the file after renaming it for clarity.",
            ),
        ],
    ),
    (
        "the upload finished without any",
        [
            (
                "errors",
                "err",
                "the upload finished without any errors this time.",
            ),
            (
                "warnings",
                "war",
                "the upload finished without any warnings on the screen.",
            ),
        ],
    ),
    (
        "am i still signed in on this",
        [
            (
                "browser",
                "bro",
                "am i still signed in on this browser after reopening it?",
            ),
            (
                "device",
                "dev",
                "am i still signed in on this device after restarting?",
            ),
        ],
    ),
    (
        "the website keeps asking me to sign",
        [
            ("in", "i", "the website keeps asking me to sign in again."),
            (
                "out",
                "ou",
                "the website keeps asking me to sign out before switching accounts.",
            ),
        ],
    ),
    (
        "i'd like to update my profile",
        [
            (
                "picture",
                "pic",
                "i'd like to update my profile picture before sharing the page.",
            ),
            (
                "description",
                "des",
                "i'd like to update my profile description to make it clearer.",
            ),
        ],
    ),
    (
        "could you check the comment before",
        [
            (
                "posting",
                "pos",
                "could you check the comment before posting it?",
            ),
            (
                "replying",
                "rep",
                "could you check the comment before replying to it?",
            ),
        ],
    ),
    (
        "i'll add a link in the",
        [
            (
                "comments",
                "comm",
                "i'll add a link in the comments for anyone interested.",
            ),
            (
                "description",
                "des",
                "i'll add a link in the description below the video.",
            ),
        ],
    ),
    (
        "please share the post with the",
        [
            (
                "link",
                "li",
                "please share the post with the link to the original page.",
            ),
            (
                "title",
                "ti",
                "please share the post with the title included for context.",
            ),
        ],
    ),
    (
        "i'd like to turn off",
        [
            (
                "autoplay",
                "auto",
                "i'd like to turn off autoplay while reading this page.",
            ),
            (
                "notifications",
                "noti",
                "i'd like to turn off notifications for this website.",
            ),
        ],
    ),
    (
        "can i read this page",
        [
            (
                "offline",
                "off",
                "can i read this page offline after saving it?",
            ),
            (
                "later",
                "lat",
                "can i read this page later without signing in again?",
            ),
        ],
    ),
];

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-internet-en-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
    fn child(&self, mode: &str) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "internet_pack_snapshot_child",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("SUZAKU_INTERNET_EN_QA_MODE", mode)
            .env("SUZAKU_INTERNET_EN_QA_STORE", self.0.join("packs"))
            .env("SUZAKU_LEXICON_DIR", self.0.join("packs"))
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "internet child {mode}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn internet_pack() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == INTERNET_ID)
        .expect("the optional English internet pack must be offered by the catalog")
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig::default());
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.candidates()[0].text, seed);
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local && c.text.starts_with(seed))
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
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
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

fn commit_raw_once(ime: &mut XRTabletImeEngine, seed: &str) {
    let mut matching = ime.candidates().iter().filter(|c| c.text == seed);
    let candidate = matching.next().expect("missing exact raw draft candidate");
    assert!(matching.next().is_none(), "duplicate raw draft candidate");
    assert_eq!(candidate.source, CandidateSource::Local);
    // Existing whole words (for example `we`) share the raw candidate after
    // deduplication; its kind may be Word without changing the committed text.
    assert!(matches!(
        candidate.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    let kind = candidate.kind;
    commit_once(ime, seed, kind);
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "internet-pack-synthetic-error"
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

fn quality_workflows() {
    for word in FORMS {
        assert!(
            suzaku_map::languages::english::is_known_english_word(word),
            "unindexed form {word}"
        );
    }
    for (seed, word) in [
        ("webpag", "webpage"),
        ("webpag", "webpages"),
        ("keywor", "keyword"),
        ("keywor", "keywords"),
        ("browsi", "browsing"),
        ("reposti", "reposting"),
        ("downloade", "downloaded"),
        ("unsubscr", "unsubscribe"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_raw_once(&mut engine(seed), seed);
    }
    for (context, branches) in INTERNET {
        for (next, partial, sentence) in branches {
            let word = format!("{context} {next}");
            for seed in [
                format!("{context} "),
                format!("{context} {partial}"),
                word.clone(),
            ] {
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                index(&ime, &word, CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
            }
            let seed = format!("{context} {partial}");
            commit_once(&mut engine(&seed), &word, CandidateKind::Word);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            commit_raw_once(&mut engine(&seed), &seed);
        }
    }
    for (seed, word, sentence) in [
        (
            "Could you open this page in a NEW",
            "Could you open this page in a NEW",
            "Could you open this page in a NEW tab?",
        ),
        (
            "I’d like to bookmark this pa",
            "I’d like to bookmark this page",
            "I’d like to bookmark this page for later.",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for pad in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let seed = format!("{pad}i'd{pad}like{pad}to{pad}bookmark{pad}this{pad}pa");
        let word = format!("{pad}i'd{pad}like{pad}to{pad}bookmark{pad}this{pad}page");
        let ime = engine(&seed);
        draft(&ime, &seed);
        index(&ime, &word, CandidateKind::Word);
        commit_once(
            &mut engine(&seed),
            &format!("{word} for later."),
            CandidateKind::Sentence,
        );
    }
    for prefix in [
        String::new(),
        "Earlier note. ".repeat(40),
        "前文😀。  ".repeat(100),
    ] {
        let seed = format!("{prefix}i'd like to bookmark this pa");
        let word = format!("{prefix}i'd like to bookmark this page");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        ime.select_candidate(index(&ime, &word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
        assert!(ime.snapshot().committed_text.is_empty());
        // Host-like draft replacement, not physical keyboard integration.
        for tail in ["  ", "  fo", "  for la"] {
            let continued = format!("{word}{tail}");
            ime.seed(&continued);
            draft(&ime, &continued);
            index(
                &ime,
                &format!("{word}  for later."),
                CandidateKind::Sentence,
            );
        }
        commit_once(
            &mut ime,
            &format!("{word}  for later."),
            CandidateKind::Sentence,
        );
        commit_raw_once(&mut engine(&seed), &seed);
    }
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("i'd like to{boundary}bookmark this pa");
        let forbidden = format!("i'd like to{boundary}bookmark this page for later.");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }

    for seed in [
        "https://example.test/bookmar",
        "https://example.test/?query=keywor",
        "reader@bookmar",
        "user_bookmar",
        "src/bookmar",
        "bookmar()",
        "const_value",
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(
            ime.candidates()
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            [seed],
            "protected token {seed:?} must remain literal"
        );
        commit_raw_once(&mut engine(seed), seed);
    }
    for token in ["https://example.test/", "reader@", "src/", "user_"] {
        let seed = format!("{token}{}，bookmar", "前文".repeat(150));
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert_eq!(
            ime.candidates()
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            [seed.as_str()],
            "long prefix must not hide protected syntax"
        );
        commit_raw_once(&mut engine(&seed), &seed);
    }

    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in INTERNET {
            for (next, partial, sentence) in branches {
                let seed = format!("{context} {partial}");
                let mut ime = engine(&seed);
                ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
                ime.seed(&seed);
                draft(&ime, &seed);
                index(&ime, &format!("{context} {next}"), CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
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
                commit_once(&mut ime, sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn internet_pack_has_independent_fixed_records_without_context_or_sentence_collisions() {
    let pack = internet_pack();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "en");
    assert_eq!(pack.manifest.topics, ["internet", "digital"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    let layers = pack.lexicon.word_layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "everyday-internet");
    assert_eq!(FORMS.len(), 64);
    assert_eq!(layers[0].words, FORMS);
    assert_eq!(INTERNET.len(), 20);
    assert_eq!(layers[0].next_words.len(), 20);
    assert_eq!(layers[0].sentences.len(), 40);
    assert_eq!(pack.units(), 552);
    for ((context, values), (expected, branches)) in layers[0].next_words.iter().zip(INTERNET) {
        assert_eq!(context, expected);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            branches.map(|(word, _, _)| word)
        );
    }
    assert_eq!(
        layers[0]
            .sentences
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        INTERNET
            .iter()
            .flat_map(|(_, bs)| bs.iter().map(|(_, _, s)| *s))
            .collect::<Vec<_>>()
    );
    let builtin = lexicon::builtin("en").unwrap();
    let other_packs = packs::recommended()
        .into_iter()
        .filter(|p| p.manifest.language == "en" && p.manifest.id != INTERNET_ID)
        .collect::<Vec<_>>();
    for lexicon in std::iter::once(builtin).chain(other_packs.iter().map(|p| &p.lexicon)) {
        for (context, branches) in INTERNET {
            assert!(
                !lexicon.next_words().any(|(key, _)| key == *context),
                "context collision: {context}"
            );
            for (_, _, sentence) in branches {
                assert!(
                    !lexicon.sentences().any(|old| old == *sentence),
                    "sentence duplicate: {sentence}"
                );
            }
        }
    }
    let fixture = Fixture::new();
    assert!(fixture.store().list().unwrap().is_empty());
    assert!(!fixture.store().path().exists());
}

#[test]
fn internet_pack_enabled_disabled_and_absent_snapshots_are_isolated() {
    let fixture = Fixture::new();
    fixture.child("absent");
    fixture.store().install(internet_pack(), false).unwrap();
    fixture.store().set_enabled(INTERNET_ID, false).unwrap();
    fixture.child("disabled");
    fixture.store().set_enabled(INTERNET_ID, true).unwrap();
    fixture.child("enabled");
    fixture.store().set_enabled(INTERNET_ID, false).unwrap();
    fixture.child("disabled");
}

// A catalog may offer more packs than one installation can activate. Exercise
// every English pack plus the original two Japanese packs and Chinese internet,
// never all catalog entries indiscriminately.
fn coexistence_packs() -> Vec<OfflinePack> {
    let packs = packs::recommended()
        .into_iter()
        .filter(|pack| {
            pack.manifest.language == "en"
                || matches!(
                    pack.manifest.id.as_str(),
                    "org.suzaku.ja.rail" | "org.suzaku.ja.family"
                )
                || pack.manifest.id == "org.suzaku.zh-hans.internet"
        })
        .collect::<Vec<_>>();
    assert_eq!(
        packs.iter().filter(|p| p.manifest.language == "en").count(),
        8
    );
    assert_eq!(
        packs.iter().filter(|p| p.manifest.language == "ja").count(),
        2
    );
    assert_eq!(
        packs
            .iter()
            .filter(|p| p.manifest.id == "org.suzaku.zh-hans.internet")
            .count(),
        1
    );
    assert_eq!(packs.len(), 11);
    assert!(packs.len() <= packs::MAX_PACKS);
    packs
}

#[test]
fn internet_pack_workflows_coexist_with_all_english_and_selected_other_language_packs() {
    let fixture = Fixture::new();
    for pack in coexistence_packs() {
        fixture.store().install(pack, false).unwrap();
    }
    fixture.child("quality");
}

#[test]
#[ignore = "subprocess-only immutable pack snapshot; invoked by the owning parent tests"]
fn internet_pack_snapshot_child() {
    let mode = std::env::var("SUZAKU_INTERNET_EN_QA_MODE").unwrap();
    let path = PathBuf::from(std::env::var_os("SUZAKU_INTERNET_EN_QA_STORE").unwrap());
    assert!(path.is_absolute());
    assert_ne!(path, Path::new("/"));
    let store = PackStore::new(path).unwrap();
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    match mode.as_str() {
        "absent" | "disabled" => {
            assert!(report.loaded.is_empty());
            assert_eq!(
                serde_json::to_value(lexicon::active("en").unwrap().word_layers()).unwrap(),
                serde_json::to_value(lexicon::builtin("en").unwrap().word_layers()).unwrap()
            );
            for _ in 0..2 {
                assert!(!suzaku_map::languages::english::is_known_english_word(
                    "reposting"
                ));
                let seed = "i'd like to bookmark this pa";
                let ime = engine(seed);
                draft(&ime, seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|c| c.text == "i'd like to bookmark this page for later.")
                );
                if mode == "disabled" {
                    // Registry mutation is private and cannot hot-swap this initialized process.
                    store.set_enabled(INTERNET_ID, true).unwrap();
                }
            }
        }
        "enabled" => {
            assert_eq!(report.loaded, [INTERNET_ID]);
            quality_workflows();
        }
        "quality" => {
            let expected = coexistence_packs()
                .into_iter()
                .map(|p| p.manifest.id)
                .collect::<HashSet<_>>();
            assert_eq!(
                report.loaded.iter().cloned().collect::<HashSet<_>>(),
                expected
            );
            assert_eq!(report.loaded.len(), 11);
            assert!(report.loaded.iter().any(|id| id == INTERNET_ID));
            quality_workflows();
        }
        _ => panic!("unknown internet snapshot case {mode}"),
    }
}
