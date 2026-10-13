use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
use suzaku_map::lexicon::packs::{self, OfflinePack};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-pack-inventory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"));
        command
            .arg("pack")
            // A read-only inventory must work even when registry discovery
            // would fail. Never inspect the real user's installed packages.
            .env("SUZAKU_LEXICON_DIR", "not-an-absolute-registry")
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS");
        command
    }

    fn cli(&self, args: &[&str]) -> Value {
        decode(self.command().args(args).output().unwrap())
    }

    fn inspect(&self, document: &Value) -> Value {
        let path = self.0.join("example.json");
        let bytes = serde_json::to_vec(document).unwrap();
        fs::write(&path, &bytes).unwrap();
        let result = self.cli(&["inspect", path.to_str().unwrap()]);
        assert_eq!(fs::read(path).unwrap(), bytes);
        result
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn decode(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn example(language: &str, lexicon: Value) -> Value {
    json!({
        "format_version": 1,
        "manifest": {
            "id": "example.inventory", "version": "1.0.0",
            "name": "Inventory example", "description": "Authored counting fixture.",
            "language": language, "topics": ["testing"],
            "license": "MIT", "authors": ["Suzaku contributors"]
        },
        "lexicon": lexicon
    })
}

#[test]
fn catalog_details_preserve_default_manifests_order_and_filters() {
    let f = Fixture::new();
    let plain = f.cli(&["catalog"]);
    let details = f.cli(&["catalog", "--details"]);
    let recommended = packs::recommended();
    assert_eq!(plain.as_array().unwrap().len(), 25);
    assert_eq!(details.as_array().unwrap().len(), recommended.len());
    for ((plain, detailed), pack) in plain
        .as_array()
        .unwrap()
        .iter()
        .zip(details.as_array().unwrap())
        .zip(recommended)
    {
        assert_eq!(*plain, serde_json::to_value(&pack.manifest).unwrap());
        assert_eq!(detailed["manifest"], *plain);
        assert_eq!(detailed["stats"]["indexing_units"], pack.units());
        let inspected = f.inspect(&serde_json::to_value(pack).unwrap());
        assert_eq!(inspected["stats"], detailed["stats"]);
        assert_eq!(inspected["manifest"], *plain);
    }
    for args in [
        vec![
            "catalog",
            "--details",
            "--language",
            "ja",
            "--topic",
            "family",
        ],
        vec![
            "catalog",
            "--topic",
            "family",
            "--details",
            "--language",
            "ja",
        ],
        vec![
            "catalog",
            "--language",
            "ja",
            "--topic",
            "family",
            "--details",
        ],
    ] {
        let filtered = f.cli(&args);
        assert_eq!(filtered.as_array().unwrap().len(), 1);
        assert_eq!(filtered[0]["manifest"]["id"], "org.suzaku.ja.family");
    }
    assert_eq!(
        f.cli(&["catalog", "--details", "--topic", "not-shipped"]),
        json!([])
    );
    assert!(!f.0.join("config").exists());
    assert!(!f.0.join("data").exists());
}

#[test]
fn english_stats_count_declared_occurrences_not_unique_or_projected_words() {
    let f = Fixture::new();
    let document = example(
        "en",
        json!({
            "format_version": 1, "language": "en",
            "word_layers": [
                {"id": "first", "words": ["hello", "world"],
                 "next_words": [["hello", ["world", "again"]]],
                 "sentences": ["good morning."]},
                {"id": "second", "words": ["hello"], "sentences": ["good morning."]}
            ],
            "phrase_endings": [["good", ["morning", "night"]], ["stay", ["here"]]]
        }),
    );
    // 3 explicit words + 3 next-word units + 6 sentence-token units + 5 endings.
    assert_eq!(
        OfflinePack::from_json(&document.to_string())
            .unwrap()
            .units(),
        17
    );
    let result = f.inspect(&document);
    assert_eq!(result["format_version"], 1);
    assert_eq!(result["manifest"], document["manifest"]);
    assert_eq!(
        result["stats"],
        json!({
            "word_layers": 2, "word_forms": 3,
            "word_readings": 0, "sentence_readings": 0,
            "next_word_contexts": 1, "next_word_choices": 2,
            "standalone_sentences": 2,
            "continuation_contexts": 0, "continuation_sentences": 0,
            "phrase_contexts": 2, "phrase_endings": 3,
            "indexing_units": 17
        })
    );
}

#[test]
fn cjk_stats_distinguish_word_and_sentence_readings_from_continuations() {
    let f = Fixture::new();
    for (language, reading, text) in [("zh-Hans", "hua", "花"), ("ja", "はな", "花")] {
        let document = example(
            language,
            json!({
                "format_version": 1, "language": language,
                "readings": [
                    {"reading": reading, "text": text, "kind": "word"},
                    {"reading": reading, "text": format!("{text}。"), "kind": "sentence"}
                ],
                "continuations": [[text, [format!("{text}。"), format!("{text}！")]]]
            }),
        );
        assert_eq!(
            f.inspect(&document)["stats"],
            json!({
                "word_layers": 0, "word_forms": 0,
                "word_readings": 1, "sentence_readings": 1,
                "next_word_contexts": 0, "next_word_choices": 0,
                "standalone_sentences": 0,
                "continuation_contexts": 1, "continuation_sentences": 2,
                "phrase_contexts": 0, "phrase_endings": 0,
                "indexing_units": 5
            })
        );
    }
}

#[test]
fn inspect_does_not_read_or_change_an_existing_broken_registry() {
    let f = Fixture::new();
    let registry = f.0.join("packs");
    fs::create_dir(&registry).unwrap();
    let marker = b"preserve this broken registry";
    fs::write(registry.join("registry.json"), marker).unwrap();
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/offline-packs/en-family.json");
    let bytes = fs::read(&source).unwrap();
    let result = decode(
        f.command()
            .env("SUZAKU_LEXICON_DIR", &registry)
            .args(["inspect", source.to_str().unwrap()])
            .output()
            .unwrap(),
    );
    let validation = result["validation"].as_str().unwrap();
    assert!(validation.contains("Schema/decoder only"));
    assert!(validation.contains("not checked"));
    assert_eq!(fs::read(source).unwrap(), bytes);
    assert_eq!(fs::read(registry.join("registry.json")).unwrap(), marker);
    assert_eq!(fs::read_dir(registry).unwrap().count(), 1);
    assert!(!f.0.join("config").exists());
    assert!(!f.0.join("data").exists());
}

#[test]
fn malformed_options_are_rejected_without_reading_or_creating_user_state() {
    let f = Fixture::new();
    for args in [
        vec!["catalog", "--details", "--details"],
        vec!["catalog", "--language"],
        vec!["catalog", "--topic"],
        vec!["catalog", "--language", "ja", "--language", "en"],
        vec!["catalog", "--topic", "family", "--topic", "work"],
        vec!["catalog", "--details", "--language", "fr"],
        vec!["catalog", "--topic", "--details"],
        vec!["catalog", "--details", "true"],
        vec!["catalog", "--unknown", "yes"],
        vec!["inspect"],
        vec!["inspect", "example.json", "--replace"],
    ] {
        let output = f.command().args(&args).output().unwrap();
        assert!(!output.status.success(), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}

#[test]
fn inspect_uses_bounded_schema_validation_and_rejects_non_regular_files() {
    let f = Fixture::new();
    let invalid = f.0.join("invalid.json");
    let mut malformed = serde_json::to_value(packs::recommended().remove(0)).unwrap();
    malformed["script"] = json!("ignored executable fields are not allowed");
    fs::write(&invalid, malformed.to_string()).unwrap();
    let oversized = f.0.join("oversized.json");
    fs::write(&oversized, vec![b' '; packs::MAX_PACK_BYTES + 1]).unwrap();
    for path in [&invalid, &oversized, &f.0.join("missing.json"), &f.0] {
        let output = f
            .command()
            .args(["inspect", path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{}", path.display());
        assert!(output.stdout.is_empty());
    }
    assert_eq!(fs::read_to_string(invalid).unwrap(), malformed.to_string());
    assert_eq!(
        fs::metadata(oversized).unwrap().len(),
        (packs::MAX_PACK_BYTES + 1) as u64
    );
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn inspect_rejects_symlinks_without_following_or_replacing_them() {
    let f = Fixture::new();
    let target = f.0.join("target.json");
    let bytes = serde_json::to_vec(&packs::recommended().remove(0)).unwrap();
    fs::write(&target, &bytes).unwrap();
    let link = f.0.join("link.json");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let output = f
        .command()
        .args(["inspect", link.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_link(link).unwrap(), target);
    assert_eq!(fs::read(target).unwrap(), bytes);
}
