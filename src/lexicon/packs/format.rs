//! Pack data, validation and declared counts; no registry or startup state.
use super::super::{EntryKind, Lexicon, builtin, check_text};
use super::{MAX_PACK_BYTES, MAX_PACK_UNITS};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub topics: Vec<String>,
    pub license: String,
    pub authors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OfflinePack {
    pub format_version: u32,
    pub manifest: Manifest,
    pub lexicon: Lexicon,
}

/// Counts of declared records, not unique vocabulary or the number of
/// candidates a decoder will display. Repeated priority anchors count again;
/// projected sentence tokens belong only to the conservative indexing budget.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PackStats {
    pub word_layers: usize,
    pub word_forms: usize,
    pub word_readings: usize,
    pub sentence_readings: usize,
    pub next_word_contexts: usize,
    pub next_word_choices: usize,
    pub standalone_sentences: usize,
    pub continuation_contexts: usize,
    pub continuation_sentences: usize,
    pub phrase_contexts: usize,
    pub phrase_endings: usize,
    pub indexing_units: usize,
}

impl OfflinePack {
    pub fn from_json(raw: &str) -> Result<Self, String> {
        if raw.len() > MAX_PACK_BYTES {
            return Err("offline pack exceeds 256 KiB".into());
        }
        let pack: Self = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        pack.validate()?;
        Ok(pack)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != 1 {
            return Err("unsupported offline pack format_version".into());
        }
        let m = &self.manifest;
        identifier(&m.id, 64)?;
        if m.version.split('.').count() != 3
            || !m.version.split('.').all(|part| {
                !part.is_empty()
                    && part.len() <= 6
                    && part.bytes().all(|b| b.is_ascii_digit())
                    && (part == "0" || !part.starts_with('0'))
            })
        {
            return Err("version must be MAJOR.MINOR.PATCH without leading zeroes".into());
        }
        for (value, limit) in [(&m.name, 128), (&m.description, 512), (&m.license, 128)] {
            metadata(value, limit)?;
        }
        if m.authors.is_empty() || m.authors.len() > 8 {
            return Err("provide 1..8 authors".into());
        }
        for author in &m.authors {
            metadata(author, 80)?;
        }
        if m.topics.is_empty() || m.topics.len() > 8 {
            return Err("provide 1..8 topics".into());
        }
        let mut topics = HashSet::new();
        for topic in &m.topics {
            identifier(topic, 32)?;
            if !topics.insert(topic) {
                return Err("duplicate topic".into());
            }
        }
        if builtin(&m.language).is_none() || m.language != self.lexicon.language() {
            return Err(
                "pack/resource language must match a supported decoder: en, zh-Hans, ja".into(),
            );
        }
        self.lexicon.validate()?;
        self.validate_decoder_fields()?;
        if self.lexicon.word_layers.len() > 8 {
            return Err("a pack may contain at most 8 layers".into());
        }
        if !(1..=MAX_PACK_UNITS).contains(&self.units()) {
            return Err("a pack must contain 1..2048 indexing units".into());
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAX_PACK_BYTES {
            return Err("offline pack exceeds 256 KiB".into());
        }
        Ok(())
    }

    // Data remains decoder-independent; these are explicit v1 compatibility
    // profiles so unsupported fields are rejected rather than silently ignored.
    fn validate_decoder_fields(&self) -> Result<(), String> {
        let lex = &self.lexicon;
        if lex.language == "en" {
            if !lex.readings.is_empty() || !lex.continuations.is_empty() {
                return Err(
                    "English uses word_layers and phrase_endings, not readings/continuations"
                        .into(),
                );
            }
            for word in lex.word_layers.iter().flat_map(|layer| {
                layer
                    .words
                    .iter()
                    .chain(layer.next_words.iter().flat_map(|(_, words)| words))
            }) {
                if !word
                    .chars()
                    .all(|ch| ch.is_ascii_alphabetic() || matches!(ch, '\'' | '’'))
                    || !word.chars().any(|ch| ch.is_ascii_alphabetic())
                {
                    return Err(format!("unsupported English word form: {word}"));
                }
            }
        } else {
            if !lex.word_layers.is_empty() || !lex.phrase_endings.is_empty() {
                return Err("Chinese/Japanese use readings and continuations, not word_layers/phrase_endings".into());
            }
            for entry in &lex.readings {
                let valid = if lex.language == "zh-Hans" {
                    entry.reading.split('\'').all(|syllable| {
                        !syllable.is_empty() && syllable.bytes().all(|ch| ch.is_ascii_lowercase())
                    })
                } else {
                    !entry.require_separators
                        && entry
                            .reading
                            .chars()
                            .all(|ch| matches!(ch, 'ぁ'..='ゖ' | 'ー'))
                };
                if !valid {
                    return Err(format!(
                        "invalid {} reading: {}",
                        lex.language, entry.reading
                    ));
                }
            }
        }
        Ok(())
    }

    /// Inspect the pack alone without resolving an installed registry, merging
    /// resources or initializing the active runtime snapshot.
    pub fn stats(&self) -> PackStats {
        let lex = &self.lexicon;
        PackStats {
            word_layers: lex.word_layers.len(),
            word_forms: lex.word_layers.iter().map(|layer| layer.words.len()).sum(),
            word_readings: lex
                .readings
                .iter()
                .filter(|entry| entry.kind == EntryKind::Word)
                .count(),
            sentence_readings: lex
                .readings
                .iter()
                .filter(|entry| entry.kind == EntryKind::Sentence)
                .count(),
            next_word_contexts: lex.next_words().count(),
            next_word_choices: lex.next_words().map(|(_, words)| words.len()).sum(),
            standalone_sentences: lex.sentences().count(),
            continuation_contexts: lex.continuations.len(),
            continuation_sentences: lex
                .continuations
                .iter()
                .map(|(_, values)| values.len())
                .sum(),
            phrase_contexts: lex.phrase_endings.len(),
            phrase_endings: lex
                .phrase_endings
                .iter()
                .map(|(_, values)| values.len())
                .sum(),
            indexing_units: self.units(),
        }
    }

    /// Conservative work budget, including sentence token projections. It is
    /// not a unique-word count and does not change decoder candidate limits.
    pub fn units(&self) -> usize {
        let lex = &self.lexicon;
        lex.readings.len()
            + lex
                .word_layers
                .iter()
                .map(|layer| {
                    layer.words.len()
                        + layer
                            .next_words
                            .iter()
                            .map(|(_, words)| 1 + words.len())
                            .sum::<usize>()
                        + layer
                            .sentences
                            .iter()
                            .map(|s| {
                                1 + s
                                    .split(|ch: char| !ch.is_ascii_alphabetic() && ch != '\'')
                                    .filter(|word| !word.is_empty())
                                    .count()
                            })
                            .sum::<usize>()
                })
                .sum::<usize>()
            + lex
                .continuations
                .iter()
                .chain(&lex.phrase_endings)
                .map(|(_, values)| 1 + values.len())
                .sum::<usize>()
    }
}

fn metadata(value: &str, limit: usize) -> Result<(), String> {
    check_text(value)?;
    if value.chars().count() > limit {
        return Err("oversized package metadata".into());
    }
    Ok(())
}

fn identifier(value: &str, limit: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > limit
        || !value
            .bytes()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase())
        || !value
            .bytes()
            .last()
            .is_some_and(|ch| ch.is_ascii_alphanumeric())
        || !value
            .bytes()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, b'.' | b'-'))
        || value.contains("..")
    {
        return Err("invalid package/topic identifier (lowercase slug required)".into());
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledPack {
    pub enabled: bool,
    pub package: OfflinePack,
}
