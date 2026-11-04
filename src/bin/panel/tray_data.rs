//! Presentation-only reports for tray data operations.
//!
//! The shared data APIs and command-line diagnostics deliberately remain unchanged.
//! Their errors can contain paths, configuration keys and platform diagnostics, so
//! only the operation's known outer message is translated; its detail stays opaque.

use suzaku_map::ui::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DataAction {
    Backup,
    OpenIme,
    OpenPanel,
    OpenBackups,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DataFailure {
    DataFolders(String),
    Backup(String),
    OpenFolder(String),
    InvalidSettingsFolder,
    ManagerUnavailable,
    WorkerStart,
    Panicked,
}

impl DataFailure {
    fn message(&self) -> (&'static str, Option<&str>) {
        match self {
            Self::DataFolders(detail) => ("Could not locate data folders: {}", Some(detail)),
            Self::Backup(detail) => ("Settings backup failed: {}", Some(detail)),
            Self::OpenFolder(detail) => ("Could not open data folder: {}", Some(detail)),
            Self::InvalidSettingsFolder => ("Invalid settings folder path", None),
            Self::ManagerUnavailable => ("Data manager unavailable", None),
            Self::WorkerStart => ("Could not start data operation", None),
            Self::Panicked => ("Data operation failed; please retry", None),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DataReport {
    BackupSaved(String),
    FolderOpened(String),
    Failed(DataFailure),
}

impl DataReport {
    pub(crate) fn manager_unavailable() -> Self {
        Self::Failed(DataFailure::ManagerUnavailable)
    }

    pub(crate) fn worker_start_failed() -> Self {
        Self::Failed(DataFailure::WorkerStart)
    }

    pub(crate) fn worker_panicked() -> Self {
        Self::Failed(DataFailure::Panicked)
    }

    /// Render from the typed result whenever the UI language changes, without
    /// re-running the operation or matching the contents of its external detail.
    /// The caller must not pass this already-rendered label through `message()`.
    pub(crate) fn render(&self, ui: UiLanguage) -> String {
        let (key, detail) = match self {
            Self::BackupSaved(file_name) => (
                "Backup saved: {} (see backup folder)",
                Some(file_name.as_str()),
            ),
            Self::FolderOpened(path) => ("Opened: {}", Some(path.as_str())),
            Self::Failed(failure) => failure.message(),
        };
        match detail {
            Some(detail) => ui.tr(key).replace("{}", detail),
            None => ui.tr(key).to_owned(),
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn run(action: DataAction) -> DataReport {
    use suzaku_map::data::{backup, open_directory, paths::DataPaths};

    let paths = match DataPaths::current() {
        Ok(paths) => paths,
        Err(detail) => return DataReport::Failed(DataFailure::DataFolders(detail)),
    };
    let path = match action {
        DataAction::Backup => {
            return match backup::backup_now(&paths) {
                Ok(path) => DataReport::BackupSaved(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                ),
                Err(detail) => DataReport::Failed(DataFailure::Backup(detail)),
            };
        }
        DataAction::OpenIme => paths.ime.parent(),
        DataAction::OpenPanel => paths.panel.parent(),
        DataAction::OpenBackups => Some(paths.backups.as_path()),
    };
    let Some(path) = path else {
        return DataReport::Failed(DataFailure::InvalidSettingsFolder);
    };
    match open_directory(path) {
        Ok(()) => DataReport::FolderOpened(path.display().to_string()),
        Err(detail) => DataReport::Failed(DataFailure::OpenFolder(detail)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: [UiLanguage; 8] = [
        UiLanguage::English,
        UiLanguage::ChineseSimplified,
        UiLanguage::Japanese,
        UiLanguage::Korean,
        UiLanguage::Spanish,
        UiLanguage::French,
        UiLanguage::German,
        UiLanguage::Portuguese,
    ];

    #[test]
    fn data_report_fixed_messages_cover_all_eight_languages() {
        for (report, key) in [
            (
                DataReport::Failed(DataFailure::InvalidSettingsFolder),
                "Invalid settings folder path",
            ),
            (
                DataReport::manager_unavailable(),
                "Data manager unavailable",
            ),
            (
                DataReport::worker_start_failed(),
                "Could not start data operation",
            ),
            (
                DataReport::worker_panicked(),
                "Data operation failed; please retry",
            ),
        ] {
            for language in LANGUAGES {
                assert_eq!(report.render(language), language.tr(key));
                if language != UiLanguage::English {
                    assert_ne!(language.tr(key), key, "missing {language:?}: {key}");
                }
            }
        }
    }

    #[test]
    fn data_report_templates_localize_only_the_fixed_operation_message() {
        for payload in [
            "Settings",
            "Font: custom settings",
            "Opened: /tmp/custom settings",
            "Data manager unavailable",
            "Data operation failed; please retry",
            "/home/example/设置/backup_日_ß:{}",
            "Permission denied (os error 13)\nraw_detail_{}",
            "",
        ] {
            for (report, key) in [
                (
                    DataReport::BackupSaved(payload.into()),
                    "Backup saved: {} (see backup folder)",
                ),
                (DataReport::FolderOpened(payload.into()), "Opened: {}"),
                (
                    DataReport::Failed(DataFailure::DataFolders(payload.into())),
                    "Could not locate data folders: {}",
                ),
                (
                    DataReport::Failed(DataFailure::Backup(payload.into())),
                    "Settings backup failed: {}",
                ),
                (
                    DataReport::Failed(DataFailure::OpenFolder(payload.into())),
                    "Could not open data folder: {}",
                ),
            ] {
                for language in LANGUAGES {
                    assert_eq!(
                        report.render(language),
                        language.tr(key).replace("{}", payload),
                        "{language:?} / {key} / {payload:?}"
                    );
                    if language != UiLanguage::English {
                        assert_ne!(language.tr(key), key, "missing {language:?}: {key}");
                    }
                }
            }
        }
    }

    #[test]
    fn language_switches_preserve_the_typed_report_and_full_raw_detail() {
        let payload = format!(
            "{}\nSettings / 文件_{{}} / Permission denied (os error 13)",
            "backup_日_ß_".repeat(80)
        );
        let report = DataReport::Failed(DataFailure::Backup(payload.clone()));
        let original = report.clone();
        for language in LANGUAGES.into_iter().chain(LANGUAGES.into_iter().rev()) {
            assert_eq!(
                report.render(language),
                language
                    .tr("Settings backup failed: {}")
                    .replace("{}", &payload)
            );
            assert_eq!(report, original, "rendering cannot replace stored detail");
        }
        assert_eq!(
            report,
            DataReport::Failed(DataFailure::Backup(payload)),
            "diagnostic content must never choose another result category"
        );
    }
}
