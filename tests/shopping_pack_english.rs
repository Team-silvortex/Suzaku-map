//! Optional authored shopping expressions, not store policy or transaction access.
//! Each immutable activation snapshot uses a fresh child and owned test store.
#[path = "support/offline_pack_sets.rs"]
mod offline_pack_sets;

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

const SHOPPING_ID: &str = "org.suzaku.en.shopping";
// Fixed authored expectations, not copied from a parsed resource at test time.
const FORMS: &[&str] = &[
    "refund",
    "refunds",
    "refundable",
    "nonrefundable",
    "exchange",
    "exchanges",
    "exchanged",
    "exchanging",
    "return",
    "returns",
    "returned",
    "returning",
    "purchase",
    "purchases",
    "purchased",
    "purchasing",
    "cashier",
    "cashiers",
    "checkout",
    "checkouts",
    "receipt",
    "receipts",
    "barcode",
    "barcodes",
    "aisle",
    "aisles",
    "basket",
    "baskets",
    "trolley",
    "trolleys",
    "stockroom",
    "stockrooms",
    "display",
    "displays",
    "hem",
    "hems",
    "seam",
    "seams",
    "zipper",
    "zippers",
    "waistband",
    "waistbands",
    "insole",
    "insoles",
    "shoelace",
    "shoelaces",
    "fitting",
    "sizing",
];
type ShoppingBranch = (&'static str, &'static str, &'static str);
const SHOPPING: &[(&str, [ShoppingBranch; 2])] = &[
    (
        "where can i find the",
        [
            (
                "checkout",
                "che",
                "where can i find the checkout for these items?",
            ),
            (
                "cashier",
                "cas",
                "where can i find the cashier to ask about this receipt?",
            ),
        ],
    ),
    (
        "could you show me the fitting",
        [
            (
                "room",
                "ro",
                "could you show me the fitting room before i choose a size?",
            ),
            (
                "area",
                "ar",
                "could you show me the fitting area for these jackets?",
            ),
        ],
    ),
    (
        "this waistband feels",
        [
            ("tight", "ti", "this waistband feels tight when i sit down."),
            (
                "loose",
                "loo",
                "this waistband feels loose even with a belt.",
            ),
        ],
    ),
    (
        "could i try the next size",
        [
            ("up", "u", "could i try the next size up before deciding?"),
            (
                "down",
                "do",
                "could i try the next size down for comparison?",
            ),
        ],
    ),
    (
        "could i see a different",
        [
            (
                "color",
                "col",
                "could i see a different color in the same size?",
            ),
            (
                "style",
                "sty",
                "could i see a different style with a wider fit?",
            ),
        ],
    ),
    (
        "is this item available in a",
        [
            (
                "smaller",
                "sm",
                "is this item available in a smaller size to try on?",
            ),
            (
                "larger",
                "lar",
                "is this item available in a larger size in this store?",
            ),
        ],
    ),
    (
        "the shelf label shows a different",
        [
            (
                "price",
                "pri",
                "the shelf label shows a different price from the tag.",
            ),
            (
                "size",
                "si",
                "the shelf label shows a different size from this box.",
            ),
        ],
    ),
    (
        "could you check the barcode",
        [
            (
                "again",
                "ag",
                "could you check the barcode again before i pay?",
            ),
            ("here", "he", "could you check the barcode here on the box?"),
        ],
    ),
    (
        "could i get a printed",
        [
            (
                "receipt",
                "rec",
                "could i get a printed receipt for this purchase?",
            ),
            ("copy", "cop", "could i get a printed copy of the receipt?"),
        ],
    ),
    (
        "do i need the original",
        [
            (
                "receipt",
                "rec",
                "do i need the original receipt to ask for an exchange?",
            ),
            (
                "packaging",
                "pac",
                "do i need the original packaging to bring this back?",
            ),
        ],
    ),
    (
        "could you explain the exchange",
        [
            (
                "policy",
                "pol",
                "could you explain the exchange policy before i buy this?",
            ),
            (
                "process",
                "pro",
                "could you explain the exchange process for a different size?",
            ),
        ],
    ),
    (
        "i'd like to compare these two",
        [
            (
                "sizes",
                "si",
                "i'd like to compare these two sizes before deciding.",
            ),
            (
                "colors",
                "col",
                "i'd like to compare these two colors in the light.",
            ),
        ],
    ),
];

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-shopping-en-{}-{}",
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
                "shopping_pack_snapshot_child",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("SUZAKU_SHOPPING_EN_QA_MODE", mode)
            .env("SUZAKU_SHOPPING_EN_QA_STORE", self.0.join("packs"))
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
            "shopping child {mode}: {}\n{}",
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
fn shopping_pack() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == SHOPPING_ID)
        .expect("the optional English shopping pack must be offered by the catalog")
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
        "shopping-pack-synthetic-error"
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
        ("waistban", "waistband"),
        ("waistban", "waistbands"),
        ("insol", "insole"),
        ("insol", "insoles"),
        ("shoelac", "shoelace"),
        ("shoelac", "shoelaces"),
        ("stockroo", "stockroom"),
        ("nonrefun", "nonrefundable"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_raw_once(&mut engine(seed), seed);
    }
    for (context, branches) in SHOPPING {
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
            "Could i get a printed REC",
            "Could i get a printed RECEIPT",
            "Could i get a printed RECEIPT for this purchase?",
        ),
        (
            "I’d like to compare these two col",
            "I’d like to compare these two colors",
            "I’d like to compare these two colors in the light.",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for pad in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let seed = format!("{pad}could{pad}i{pad}get{pad}a{pad}printed{pad}rec");
        let word = format!("{pad}could{pad}i{pad}get{pad}a{pad}printed{pad}receipt");
        let ime = engine(&seed);
        draft(&ime, &seed);
        index(&ime, &word, CandidateKind::Word);
        commit_once(
            &mut engine(&seed),
            &format!("{word} for this purchase?"),
            CandidateKind::Sentence,
        );
    }
    for prefix in [
        String::new(),
        "Earlier note. ".repeat(40),
        "前文😀。  ".repeat(100),
    ] {
        let seed = format!("{prefix}do i need the original rec");
        let word = format!("{prefix}do i need the original receipt");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        ime.select_candidate(index(&ime, &word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
        assert!(ime.snapshot().committed_text.is_empty());
        // Host-like draft replacement, not physical keyboard integration.
        for tail in ["  ", "  to", "  to as"] {
            let continued = format!("{word}{tail}");
            ime.seed(&continued);
            draft(&ime, &continued);
            index(
                &ime,
                &format!("{word}  to ask for an exchange?"),
                CandidateKind::Sentence,
            );
        }
        commit_once(
            &mut ime,
            &format!("{word}  to ask for an exchange?"),
            CandidateKind::Sentence,
        );
        commit_raw_once(&mut engine(&seed), &seed);
    }
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("could i get a{boundary}printed rec");
        let forbidden = format!("could i get a{boundary}printed receipt for this purchase?");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in SHOPPING {
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
fn shopping_pack_has_independent_fixed_records_without_context_or_sentence_collisions() {
    let pack = shopping_pack();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "en");
    assert_eq!(pack.manifest.topics, ["shopping", "errands"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    let layers = pack.lexicon.word_layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "in-store-shopping");
    assert_eq!(FORMS.len(), 48);
    assert_eq!(layers[0].words, FORMS);
    assert_eq!(SHOPPING.len(), 12);
    assert_eq!(layers[0].next_words.len(), 12);
    assert_eq!(layers[0].sentences.len(), 24);
    assert_eq!(pack.units(), 342);
    for ((context, values), (expected, branches)) in layers[0].next_words.iter().zip(SHOPPING) {
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
        SHOPPING
            .iter()
            .flat_map(|(_, bs)| bs.iter().map(|(_, _, s)| *s))
            .collect::<Vec<_>>()
    );
    let builtin = lexicon::builtin("en").unwrap();
    let other_packs = packs::recommended()
        .into_iter()
        .filter(|p| p.manifest.language == "en" && p.manifest.id != SHOPPING_ID)
        .collect::<Vec<_>>();
    for lexicon in std::iter::once(builtin).chain(other_packs.iter().map(|p| &p.lexicon)) {
        for (context, branches) in SHOPPING {
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
fn shopping_pack_enabled_disabled_and_absent_snapshots_are_isolated() {
    let fixture = Fixture::new();
    fixture.child("absent");
    fixture.store().install(shopping_pack(), false).unwrap();
    fixture.store().set_enabled(SHOPPING_ID, false).unwrap();
    fixture.child("disabled");
    fixture.store().set_enabled(SHOPPING_ID, true).unwrap();
    fixture.child("enabled");
    fixture.store().set_enabled(SHOPPING_ID, false).unwrap();
    fixture.child("disabled");
}

#[test]
fn shopping_pack_workflows_coexist_with_historical_sixteen_packages() {
    let fixture = Fixture::new();
    for pack in offline_pack_sets::historical_sixteen() {
        fixture.store().install(pack, false).unwrap();
    }
    fixture.child("quality");
}

#[test]
#[ignore = "subprocess-only immutable pack snapshot; invoked by the owning parent tests"]
fn shopping_pack_snapshot_child() {
    let mode = std::env::var("SUZAKU_SHOPPING_EN_QA_MODE").unwrap();
    let path = PathBuf::from(std::env::var_os("SUZAKU_SHOPPING_EN_QA_STORE").unwrap());
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
                    "waistbands"
                ));
                let seed = "this waistband feels ti";
                let ime = engine(seed);
                draft(&ime, seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|c| c.text == "this waistband feels tight when i sit down.")
                );
                if mode == "disabled" {
                    // Registry mutation is private and cannot hot-swap this initialized process.
                    store.set_enabled(SHOPPING_ID, true).unwrap();
                }
            }
        }
        "enabled" => {
            assert_eq!(report.loaded, [SHOPPING_ID]);
            quality_workflows();
        }
        "quality" => {
            assert_eq!(
                report.loaded.len(),
                offline_pack_sets::historical_sixteen().len()
            );
            assert!(report.loaded.iter().any(|id| id == SHOPPING_ID));
            quality_workflows();
        }
        _ => panic!("unknown shopping snapshot case {mode}"),
    }
}
