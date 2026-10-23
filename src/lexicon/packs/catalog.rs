//! In-memory pack merging and ordered load reports; no filesystem access.
use super::super::{Lexicon, MAX_SOURCE_WORD_LAYERS, builtin};
use super::format::{InstalledPack, OfflinePack};
use super::{MAX_LANGUAGE_UNITS, MAX_PACKS};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Default, Serialize)]
pub struct LoadReport {
    pub loaded: Vec<String>,
    pub disabled: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Default)]
pub struct Catalog {
    pub(super) lexicons: BTreeMap<String, Lexicon>,
    pub report: LoadReport,
}

impl Catalog {
    /// Inspect one already-read registry generation, without filesystem access.
    pub fn from_installed(entries: &[InstalledPack]) -> Self {
        Self::from_entries(entries.iter().map(|entry| {
            entry.package.validate()?;
            Ok(entry.clone())
        }))
    }

    pub fn get(&self, language: &str) -> Option<&Lexicon> {
        self.lexicons.get(language).or_else(|| builtin(language))
    }

    pub(super) fn from_entries(
        entries: impl IntoIterator<Item = Result<InstalledPack, String>>,
    ) -> Self {
        let mut catalog = Self::default();
        let mut ids = HashSet::new();
        let mut budgets = BTreeMap::<String, usize>::new();
        for (index, entry) in entries.into_iter().enumerate() {
            // Disabled and malformed entries still occupy installed slots.
            // Stop at the first excess entry, including for direct API callers.
            if index >= MAX_PACKS {
                catalog
                    .report
                    .errors
                    .push("at most 16 packs may be installed".into());
                break;
            }
            let result = entry.and_then(|entry| {
                let pack = entry.package;
                let id = &pack.manifest.id;
                if !ids.insert(id.clone()) {
                    return Err(format!("{id}: duplicate installed package id"));
                }
                if !entry.enabled {
                    catalog.report.disabled.push(id.clone());
                    return Ok(());
                }
                let language = &pack.manifest.language;
                let budget = budgets.entry(language.clone()).or_default();
                if *budget + pack.units() > MAX_LANGUAGE_UNITS {
                    return Err(format!(
                        "{id}: enabled packs exceed 4096 indexing units for {language}"
                    ));
                }
                let mut merged = catalog.get(language).ok_or("unsupported language")?.clone();
                append(&mut merged, &pack).map_err(|e| format!("{id}: {e}"))?;
                *budget += pack.units();
                catalog.lexicons.insert(language.clone(), merged);
                catalog.report.loaded.push(id.clone());
                Ok(())
            });
            if let Err(error) = result {
                catalog.report.errors.push(error);
            }
        }
        catalog
    }
}

fn append(target: &mut Lexicon, pack: &OfflinePack) -> Result<(), String> {
    for layer in &pack.lexicon.word_layers {
        let mut layer = layer.clone();
        layer.id = format!("{}:{}", pack.manifest.id, layer.id);
        layer.next_words = novel_pairs(target.next_words(), &layer.next_words)?;
        target.word_layers.push(layer);
    }
    for entry in &pack.lexicon.readings {
        if let Some(existing) = target
            .readings
            .iter()
            .find(|e| e.reading == entry.reading && e.text == entry.text)
        {
            if existing != entry {
                return Err(format!("conflicting reading flags/type: {}", entry.reading));
            }
        } else {
            target.readings.push(entry.clone());
        }
    }
    target.continuations.extend(novel_pairs(
        target.continuations(),
        &pack.lexicon.continuations,
    )?);
    target.phrase_endings.extend(novel_pairs(
        target.phrase_endings(),
        &pack.lexicon.phrase_endings,
    )?);
    // Each pack was separately validated to at most eight layers. The runtime
    // aggregate is bounded by one <=64-layer source plus <=16 such packs, not
    // by the single-source cap (which must not erode pack capacity as it grows).
    target.validate_with_layer_limit(MAX_SOURCE_WORD_LAYERS + MAX_PACKS * 8)
}

fn novel_pairs(
    existing: impl Iterator<Item = (impl AsRef<str>, impl AsRef<[String]>)>,
    incoming: &[(String, Vec<String>)],
) -> Result<Vec<(String, Vec<String>)>, String> {
    let existing: BTreeMap<String, Vec<String>> = existing
        .map(|(k, v)| (k.as_ref().into(), v.as_ref().into()))
        .collect();
    let mut output = Vec::new();
    for (key, values) in incoming {
        match existing.get(key) {
            Some(previous) if previous != values => {
                return Err(format!(
                    "context already exists with different values: {key}; use a more specific context"
                ));
            }
            Some(_) => {}
            None => output.push((key.clone(), values.clone())),
        }
    }
    Ok(output)
}
