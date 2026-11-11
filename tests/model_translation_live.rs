//! Explicit local-only smoke check. Never reads a draft or permits cloud fallback.
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};
use suzaku_map::{
    ime::settings::ImeSettings,
    languages::{
        model::{HttpModelProvider, ModelScope},
        translation::{TranslationLanguage, TranslationProvider, TranslationRequest},
    },
};

fn isolated_configuration_path(
    explicit: Option<&Path>,
    temporary_root: &Path,
) -> Result<PathBuf, &'static str> {
    let path = explicit
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or("explicit private QA configuration required")?
        .canonicalize()
        .map_err(|_| "existing QA configuration required")?;
    let temporary_root = temporary_root
        .canonicalize()
        .map_err(|_| "existing temporary directory required")?;
    if !path.starts_with(temporary_root) {
        return Err("use an isolated temporary configuration, not personal settings");
    }
    if !path.is_file() {
        return Err("QA configuration must be a regular file");
    }
    Ok(path)
}

fn open_isolated_configuration(
    explicit: Option<&Path>,
    temporary_root: &Path,
) -> Result<File, String> {
    let path = isolated_configuration_path(explicit, temporary_root)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|_| "cannot open the validated QA configuration")?;
    let metadata = file
        .metadata()
        .map_err(|_| "cannot inspect the opened QA configuration")?;
    if !metadata.is_file() {
        return Err("QA configuration must be a regular file".into());
    }
    if metadata.len() > 65_536 {
        return Err("QA configuration exceeds 65536 bytes".into());
    }
    Ok(file)
}

fn read_configuration(file: File) -> Result<ImeSettings, String> {
    let mut raw = String::new();
    file.take(65_537)
        .read_to_string(&mut raw)
        .map_err(|_| "cannot read the opened QA configuration")?;
    if raw.len() > 65_536 {
        return Err("QA configuration exceeds 65536 bytes".into());
    }
    ImeSettings::from_json(&raw)
}

fn load_isolated_configuration(
    explicit: Option<&Path>,
    temporary_root: &Path,
) -> Result<ImeSettings, String> {
    read_configuration(open_isolated_configuration(explicit, temporary_root)?)
}

#[test]
#[ignore = "opt-in SUZAKU_TRANSLATION_LOCAL_QA=1; uses the configured local model with synthetic text"]
fn configured_local_model_translates_eight_synthetic_targets() {
    assert_eq!(
        std::env::var("SUZAKU_TRANSLATION_LOCAL_QA").as_deref(),
        Ok("1")
    );
    let explicit = std::env::var_os("SUZAKU_IME_CONFIG").map(PathBuf::from);
    let config = load_isolated_configuration(explicit.as_deref(), &std::env::temp_dir())
        .expect("isolated local model configuration")
        .provider;
    assert_eq!(
        config.scope,
        ModelScope::Local,
        "live QA must never use a cloud model"
    );
    let provider = HttpModelProvider::new(config);
    for target in TranslationLanguage::ALL {
        let text = provider
            .translate(&TranslationRequest {
                text: "Hello, how are you? Thank you for your help.".into(),
                source: Some(TranslationLanguage::English),
                target,
            })
            .unwrap_or_else(|error| panic!("{}: {error}", target.name()));
        assert!(!text.trim().is_empty());
        println!("{}: {}", target.name(), text);
    }
}

#[cfg(test)]
mod isolation_tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::SystemTime,
    };

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        temporary_root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "suzaku-translation-isolation-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            // Model a temporary root and an outside file inside our own fixture.
            let temporary_root = root.join("temporary");
            fs::create_dir(&temporary_root).unwrap();
            Self {
                root,
                temporary_root,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn explicit_configuration_is_required() {
        let temporary_root = std::env::temp_dir();
        assert!(isolated_configuration_path(None, &temporary_root).is_err());
        assert!(isolated_configuration_path(Some(Path::new("")), &temporary_root).is_err());
    }

    #[test]
    fn configuration_outside_temporary_root_is_rejected() {
        let fixture = Fixture::new();
        let path = fixture.root.join("outside-settings.json");
        fs::write(&path, b"{}").unwrap();
        assert!(isolated_configuration_path(Some(&path), &fixture.temporary_root).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn temporary_symlink_to_outside_configuration_is_rejected() {
        let fixture = Fixture::new();
        let outside = fixture.root.join("outside-settings.json");
        fs::write(&outside, b"{}").unwrap();
        let path = fixture.temporary_root.join("settings.json");
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        assert!(isolated_configuration_path(Some(&path), &fixture.temporary_root).is_err());
    }

    #[test]
    fn existing_regular_temporary_configuration_is_accepted() {
        let fixture = Fixture::new();
        let path = fixture.temporary_root.join("settings.json");
        fs::write(&path, b"{}").unwrap();
        assert_eq!(
            isolated_configuration_path(Some(&path), &fixture.temporary_root),
            Ok(path.canonicalize().unwrap())
        );
    }

    #[test]
    fn missing_file_and_directory_are_rejected() {
        let fixture = Fixture::new();
        let missing = fixture.temporary_root.join("missing-settings.json");
        assert!(isolated_configuration_path(Some(&missing), &fixture.temporary_root).is_err());
        assert!(
            isolated_configuration_path(Some(&fixture.temporary_root), &fixture.temporary_root)
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn original_symlink_retargeting_cannot_change_an_opened_configuration() {
        let fixture = Fixture::new();
        let target = fixture.temporary_root.join("validated-settings.json");
        fs::write(&target, br#"{"llm_model":"validated-model"}"#).unwrap();
        let outside = fixture.root.join("outside-settings.json");
        fs::write(&outside, br#"{"llm_model":"outside-model"}"#).unwrap();
        let original = fixture.temporary_root.join("settings.json");
        std::os::unix::fs::symlink(&target, &original).unwrap();
        let file = open_isolated_configuration(Some(&original), &fixture.temporary_root).unwrap();
        fs::remove_file(&original).unwrap();
        std::os::unix::fs::symlink(&outside, &original).unwrap();
        assert_eq!(
            read_configuration(file).unwrap().provider.model,
            "validated-model"
        );
        assert!(load_isolated_configuration(Some(&original), &fixture.temporary_root).is_err());
    }

    #[test]
    fn configuration_reads_preserve_the_65536_byte_limit() {
        let fixture = Fixture::new();
        let path = fixture.temporary_root.join("settings.json");
        let mut raw = b"{}".to_vec();
        raw.resize(65_536, b' ');
        fs::write(&path, &raw).unwrap();
        assert!(load_isolated_configuration(Some(&path), &fixture.temporary_root).is_ok());
        let file = open_isolated_configuration(Some(&path), &fixture.temporary_root).unwrap();
        raw.push(b' ');
        fs::write(&path, &raw).unwrap();
        // The file may grow after its metadata was checked: bound the same handle's read.
        assert!(read_configuration(file).is_err());
        assert!(load_isolated_configuration(Some(&path), &fixture.temporary_root).is_err());
    }
}
