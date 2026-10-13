//! Characterize the public facade while its private implementations move.
//! A fresh process is essential: observing runtime()/active() would itself
//! claim the OnceLock and hide accidental initialization by pure inspection.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use suzaku_map::lexicon::{
    self,
    packs::{
        self, Catalog, InstalledPack, LoadReport, Manifest, OfflinePack, PackStats, PackStore,
    },
};

const CHILD_ROOT: &str = "SUZAKU_PACK_INSPECTION_BOUNDARY_CHILD";
const CHILD_SUCCESS: &str = "PASS: pure pack inspection left explicit startup available";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-pack-inspection-boundary-{}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn pure_inspection_does_not_claim_the_process_startup_snapshot() {
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        inspect_then_initialize(Path::new(&root));
        println!("\n{CHILD_SUCCESS}");
        return;
    }

    let fixture = Fixture::new();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "pure_inspection_does_not_claim_the_process_startup_snapshot",
            "--nocapture",
        ])
        .env(CHILD_ROOT, &fixture.0)
        // Pure operations must not need valid personal registry discovery.
        .env("SUZAKU_LEXICON_DIR", "invalid-relative-registry")
        .env("HOME", &fixture.0)
        .env("XDG_CONFIG_HOME", fixture.0.join("config"))
        .env("XDG_DATA_HOME", fixture.0.join("data"))
        .env("XDG_CACHE_HOME", fixture.0.join("cache"))
        .env("XDG_STATE_HOME", fixture.0.join("state"))
        .env("XDG_RUNTIME_DIR", fixture.0.join("runtime"))
        .env_remove("SUZAKU_IME_CONFIG")
        .env_remove("SUZAKU_LINUX_IME_SOCKET")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| *line == CHILD_SUCCESS)
            .count(),
        1,
        "a successful exit with zero matched child tests is not acceptance"
    );
}

fn inspect_then_initialize(root: &Path) {
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    let recommended = packs::recommended();
    assert_eq!(recommended.len(), 25);
    for pack in &recommended {
        let manifest: &Manifest = &pack.manifest;
        let raw = serde_json::to_string(pack).unwrap();
        let parsed = OfflinePack::from_json(&raw).unwrap();
        parsed.validate().unwrap();
        let stats: PackStats = parsed.stats();
        assert_eq!(stats.indexing_units, parsed.units());
        assert_eq!(parsed.manifest.id, manifest.id);
        assert_eq!(serde_json::to_string(&parsed).unwrap(), raw);
    }
    assert!(OfflinePack::from_json("{}").is_err());

    let empty = Catalog::default();
    for language in ["en", "zh-Hans", "ja"] {
        assert!(std::ptr::eq(
            empty.get(language).unwrap(),
            lexicon::builtin(language).unwrap()
        ));
    }
    assert!(empty.get("unsupported").is_none());

    let entries: Vec<InstalledPack> = recommended[..3]
        .iter()
        .cloned()
        .map(|package| InstalledPack {
            enabled: true,
            package,
        })
        .collect();
    let expected_ids = [
        "org.suzaku.en.outdoors",
        "org.suzaku.zh-hans.outdoors",
        "org.suzaku.ja.rail",
    ];
    let first = Catalog::from_installed(&entries);
    let second = Catalog::from_installed(&entries);
    for catalog in [&first, &second] {
        let report: &LoadReport = &catalog.report;
        assert_eq!(report.loaded, expected_ids);
        assert!(report.disabled.is_empty());
        assert!(report.errors.is_empty());
    }
    for language in ["en", "zh-Hans", "ja"] {
        assert_eq!(
            serde_json::to_value(first.get(language).unwrap()).unwrap(),
            serde_json::to_value(second.get(language).unwrap()).unwrap()
        );
    }
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);

    // Explicit persistence is separate from the pure calls above. Reading an
    // installed generation must not activate it either; only initialize does.
    let store = PackStore::new(root.join("packs")).unwrap();
    store.install(recommended[0].clone(), false).unwrap();
    let before = fs::read(store.path()).unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(store.load().report.loaded, [expected_ids[0]]);
    assert_eq!(fs::read(store.path()).unwrap(), before);

    let report = packs::initialize(&store)
        .expect("inspection and store reads must not freeze ACTIVE before explicit startup");
    assert_eq!(report.loaded, [expected_ids[0]]);
    assert!(report.disabled.is_empty());
    assert!(report.errors.is_empty());
    assert!(std::ptr::eq(report, &packs::runtime().report));
    assert_eq!(fs::read(store.path()).unwrap(), before);
}
