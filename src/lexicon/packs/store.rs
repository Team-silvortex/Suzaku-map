//! Filesystem-backed pack registry and atomic installation transactions.
use super::catalog::{Catalog, LoadReport};
use super::format::{InstalledPack, OfflinePack};
use super::{MAX_PACK_BYTES, MAX_PACKS};
use crate::data::{files, paths};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
};

const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;

impl OfflinePack {
    pub fn read(path: &Path) -> Result<Self, String> {
        let raw = files::read_optional(path, MAX_PACK_BYTES)
            .map_err(|e| e.to_string())?
            .ok_or("offline pack file does not exist")?;
        Self::from_json(&raw)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    format_version: u32,
    // Individual records are decoded separately at startup: one damaged record
    // must not prevent independent valid packs from loading. Keep the original
    // JSON until strict InstalledPack deserialization: Value would silently
    // discard duplicate enabled/package fields and nested package fields.
    packages: Vec<Box<RawValue>>,
}

#[derive(Clone, Debug)]
pub struct PackStore {
    directory: PathBuf,
}

impl PackStore {
    pub fn new(directory: PathBuf) -> Result<Self, String> {
        if !directory.is_absolute() {
            return Err("offline pack directory must be absolute".into());
        }
        Ok(Self { directory })
    }

    pub fn current() -> Result<Self, String> {
        Self::new(match env::var_os("SUZAKU_LEXICON_DIR") {
            Some(path) => PathBuf::from(path),
            None => paths::data_home()
                .ok_or("cannot locate user data directory")?
                .join("suzaku/lexicons"),
        })
    }

    pub fn path(&self) -> PathBuf {
        self.directory.join("registry.json")
    }

    fn registry(&self) -> Result<Registry, String> {
        let raw =
            files::read_optional(&self.path(), MAX_REGISTRY_BYTES).map_err(|e| e.to_string())?;
        let registry: Registry = match raw {
            Some(raw) => serde_json::from_str(&raw)
                .map_err(|e| format!("invalid offline pack registry: {e}"))?,
            None => Registry {
                format_version: 1,
                packages: vec![],
            },
        };
        if registry.format_version != 1 || registry.packages.len() > MAX_PACKS {
            return Err("unsupported registry version or more than 16 installed packs".into());
        }
        Ok(registry)
    }

    /// Read-only: does not create directories, locks or default files.
    pub fn list(&self) -> Result<Vec<InstalledPack>, String> {
        let mut ids = HashSet::new();
        self.registry()?
            .packages
            .into_iter()
            .map(|raw| {
                let entry = decode_entry(&raw)?;
                if !ids.insert(entry.package.manifest.id.clone()) {
                    return Err("duplicate installed package id".into());
                }
                Ok(entry)
            })
            .collect()
    }

    pub fn install(&self, pack: OfflinePack, replace: bool) -> Result<(), String> {
        pack.validate()?;
        self.update(move |entries| {
            if let Some(entry) = entries
                .iter_mut()
                .find(|e| e.package.manifest.id == pack.manifest.id)
            {
                if !replace {
                    return Err("package already installed; use --replace explicitly".into());
                }
                if entry.package.manifest.language != pack.manifest.language {
                    return Err("replacement must keep the package language".into());
                }
                entry.package = pack;
            } else {
                entries.push(InstalledPack {
                    enabled: true,
                    package: pack,
                });
            }
            Ok(())
        })
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        self.update(|entries| {
            entries
                .iter_mut()
                .find(|e| e.package.manifest.id == id)
                .ok_or("package is not installed")?
                .enabled = enabled;
            Ok(())
        })
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        self.update(|entries| {
            let index = entries
                .iter()
                .position(|e| e.package.manifest.id == id)
                .ok_or("package is not installed")?;
            entries.remove(index);
            Ok(())
        })
    }

    fn update(
        &self,
        mutate: impl FnOnce(&mut Vec<InstalledPack>) -> Result<(), String>,
    ) -> Result<(), String> {
        let _lease = files::DataLease::acquire(&self.directory.join(".registry.lock"), true)
            .map_err(|_| "offline pack registry is busy or its lock is inaccessible".to_owned())?;
        let mut entries = self.list()?;
        mutate(&mut entries)?;
        if entries.len() > MAX_PACKS {
            return Err("at most 16 packs may be installed".into());
        }
        let catalog = Catalog::from_entries(entries.iter().cloned().map(Ok));
        if !catalog.report.errors.is_empty() {
            return Err(catalog.report.errors.join("\n"));
        }
        let registry = Registry {
            format_version: 1,
            packages: entries
                .iter()
                .map(serde_json::value::to_raw_value)
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?,
        };
        let bytes = serde_json::to_vec(&registry).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_REGISTRY_BYTES {
            return Err("offline pack registry exceeds 4 MiB".into());
        }
        files::atomic_write(&self.path(), &bytes).map_err(|e| e.to_string())
    }

    pub fn load(&self) -> Catalog {
        match self.registry() {
            Ok(registry) => {
                Catalog::from_entries(registry.packages.iter().map(|raw| decode_entry(raw)))
            }
            Err(error) => Catalog {
                report: LoadReport {
                    errors: vec![error],
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

fn decode_entry(raw: &RawValue) -> Result<InstalledPack, String> {
    let entry: InstalledPack = serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
    entry
        .package
        .validate()
        .map_err(|e| format!("{}: {e}", entry.package.manifest.id))?;
    Ok(entry)
}
