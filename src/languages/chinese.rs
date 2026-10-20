//! Curated offline vocabulary, not a replacement for a full Pinyin dictionary.
//! Unknown input is always preserved; an optional LLM can enrich these candidates.

use super::ranked_typed_candidates;
use crate::ime::candidate_mix::CandidateKind;
use crate::ime::{Candidate, LanguagePlugin};
use crate::lexicon::{EntryKind, Lexicon};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

#[derive(Default)]
pub struct ChineseLanguagePlugin;

impl LanguagePlugin for ChineseLanguagePlugin {
    fn id(&self) -> &str {
        "zh-Hans"
    }
    fn display_name(&self) -> &str {
        "简体中文"
    }
    fn normalize_seed(&self, input: &str) -> String {
        // Separators belong to the preedit and the lossless literal candidate.
        input.to_owned()
    }
    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }
    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        ranked_typed_candidates(pinyin_choices(seed), confidence)
    }
    fn direct_candidates(&self, seed: &str, confidence: f32) -> Option<Vec<Candidate>> {
        Some(self.build_candidates(&[], seed, confidence))
    }
    fn commit_separator(&self) -> &str {
        ""
    }
}

// Authored bootstrap vocabulary, with explicit syllables rather than inferred
// splits. Optional typed separators may join these syllables, never split one.
struct PinyinEntry {
    reading: &'static str,
    text: &'static str,
    require_separators: bool,
    kind: EntryKind,
}

impl PinyinEntry {
    #[cfg(test)]
    const fn new(reading: &'static str, text: &'static str) -> Self {
        Self {
            reading,
            text,
            require_separators: false,
            kind: EntryKind::Word,
        }
    }

    fn match_input(&self, input: &str) -> Option<ReadingMatch> {
        // Horizontal padding after an unfinished reading must not hide its
        // completions. Trim only for matching: exact offsets still refer to
        // the original input, whose spacing/literal handling belongs to the
        // decoder. Internal syllable gaps, quotes and line boundaries stay
        // significant; never remove them to manufacture a matching reading.
        let input = input.trim_end_matches(is_pinyin_spacing);
        let mut consumed = 0;
        for expected in self.reading.bytes() {
            if consumed == input.len() {
                // A final apostrophe is a literal closing quote, not evidence
                // for an as-yet untyped next syllable. Exact readings still
                // stop before it so the decoder can retain the quote.
                return (!input.ends_with('\'')).then_some(ReadingMatch::Prefix);
            }
            if expected == b'\'' {
                let separators = separator_bytes(&input[consumed..]);
                if separators > 0 {
                    consumed += separators;
                } else if self.require_separators {
                    return None;
                }
            } else if input.as_bytes()[consumed] == expected {
                consumed += 1;
            } else {
                return None;
            }
        }
        Some(ReadingMatch::Exact(consumed))
    }
}

fn vocabulary() -> &'static Lexicon {
    crate::lexicon::active("zh-Hans").expect("Chinese vocabulary is registered")
}

fn pinyin() -> &'static [PinyinEntry] {
    static ENTRIES: OnceLock<Vec<PinyinEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        vocabulary()
            .readings()
            .iter()
            .map(|entry| PinyinEntry {
                reading: &entry.reading,
                text: &entry.text,
                require_separators: entry.require_separators,
                kind: entry.kind,
            })
            .collect()
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadingMatch {
    Exact(usize),
    Prefix,
}

// Bucket by the first ASCII letter, retaining the authored order within each
// bucket. Expanding the lexicon must not make every beam path scan all entries
// or change how exact readings and homophones are ranked.
fn pinyin_starting_with(input: &str) -> &'static [&'static PinyinEntry] {
    static INDEX: OnceLock<[Vec<&'static PinyinEntry>; 26]> = OnceLock::new();
    let Some(initial) = input.bytes().next().filter(u8::is_ascii_lowercase) else {
        return &[];
    };
    let index = INDEX.get_or_init(|| {
        let mut buckets: [Vec<&'static PinyinEntry>; 26] = std::array::from_fn(|_| Vec::new());
        for entry in pinyin() {
            buckets[usize::from(entry.reading.as_bytes()[0] - b'a')].push(entry);
        }
        buckets
    });
    &index[usize::from(initial - b'a')]
}

fn is_pinyin_spacing(ch: char) -> bool {
    // Horizontal separators can join syllables; line/paragraph boundaries are
    // literal text, even when both neighbouring runs happen to be Pinyin.
    ch == '\t' || (ch.is_whitespace() && !ch.is_control() && !matches!(ch, '\u{2028}' | '\u{2029}'))
}

fn is_pinyin_separator(ch: char) -> bool {
    ch == '\'' || is_pinyin_spacing(ch)
}

fn separator_bytes(text: &str) -> usize {
    text.len() - text.trim_start_matches(is_pinyin_separator).len()
}

fn is_tone_terminator(tail: &[char]) -> bool {
    let Some((&next, rest)) = tail.split_first() else {
        return true;
    };
    if next.is_ascii_alphabetic() || next.is_whitespace() || next == '\'' {
        return true;
    }
    match next {
        // A decimal, clock value, dotted identifier or path is not a sentence
        // boundary. Commas immediately before digits may be numeric grouping.
        '.' | ':' => rest
            .first()
            .is_none_or(|ch| !ch.is_alphanumeric() && !matches!(ch, '/' | '\\' | '_' | '-')),
        ',' => !rest.first().is_some_and(|ch| ch.is_numeric()),
        // An opening aside/quote terminates the preceding syllable just like
        // its closing partner. Chinese em dashes also delimit clauses; keep
        // this explicit so hyphens, en-dash ranges and arithmetic stay literal.
        '!' | '?' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '“' | '”' | '‘' | '’'
        | '，' | '。' | '！' | '？' | '；' | '：' | '、' | '…' | '—' | '（' | '）' | '【'
        | '】' | '〔' | '〕' | '〈' | '〉' | '《' | '》' | '「' | '」' | '『' | '』' => {
            true
        }
        _ => false,
    }
}

fn normalize_pinyin(seed: &str) -> String {
    // Normalize only letters used by this reading system. Unicode lowercasing
    // the entire draft corrupts literal Greek/Cyrillic/accented text. Preserve
    // separators here so the decoder can distinguish readings from literal spans.
    let lowered = seed
        .to_ascii_lowercase()
        .replace("u:", "v")
        .replace(['ü', 'Ü'], "v");
    let chars: Vec<_> = lowered.chars().collect();
    chars
        .iter()
        .enumerate()
        .filter_map(|(index, &ch)| {
            let tone = matches!(ch, '1'..='5')
                && index > 0
                && chars[index - 1].is_ascii_alphabetic()
                && is_tone_terminator(&chars[index + 1..]);
            (!tone).then_some(ch)
        })
        .collect()
}

fn dictionary_words() -> &'static HashSet<&'static str> {
    static WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        pinyin()
            .iter()
            .filter(|word| word.kind == EntryKind::Word)
            .map(|word| word.text)
            .collect()
    })
}

pub(crate) fn is_dictionary_word(text: &str) -> bool {
    // Beam paths repeatedly classify the same authored spellings. This is
    // membership only: decoder/homophone priority still follows pinyin() order.
    // The startup vocabulary is immutable; no typed text enters this index.
    dictionary_words().contains(text)
}

/// A single finished span can precede a whole-word completion after the
/// dictionary grows. Keep that immediate word's own authored pair available;
/// later words behind several ambiguous conversions do not qualify. Inspect
/// original decoder provenance, never a page already reordered by the mixer.
pub(crate) fn immediate_word_continuations(seed: &str) -> Option<Vec<String>> {
    let choices = pinyin_choices(seed);
    let [
        (converted, CandidateKind::Sentence),
        (word, CandidateKind::Word),
        ..,
    ] = choices.as_slice()
    else {
        return None;
    };
    let context_word = word.trim_matches(is_pinyin_spacing);
    if !is_dictionary_word(context_word) {
        return None;
    }
    // Protect only a one-character final completion conflict (e.g. 书 / 睡).
    // Replacing several characters is an existing ambiguous branch and keeps
    // its original pagination, even if a whole-word alternative follows it.
    let converted = converted.trim_matches(is_pinyin_spacing);
    let converted_prefix = &converted[..converted.char_indices().next_back()?.0];
    let word_prefix = &context_word[..context_word.char_indices().next_back()?.0];
    if converted_prefix != word_prefix {
        return None;
    }
    vocabulary()
        .continuations()
        .find(|(context, values)| *context == context_word && values.len() >= 2)
        .and_then(|(_, values)| {
            values
                .iter()
                .map(|text| {
                    text.strip_prefix(context_word)
                        .map(|suffix| format!("{word}{suffix}"))
                })
                .collect()
        })
}

// Reuse the decoder's existing bounded choices. Its first five finished paths
// can precede a correct partial Word; up to four verified completion paths then
// follow. No new reading, beam path or literal interpretation is inferred here.
fn continuation_paths(
    choices: &[(String, CandidateKind)],
    include_completed_words: bool,
) -> impl Iterator<Item = &str> {
    choices
        .iter()
        .enumerate()
        .filter(move |(index, (_, kind))| {
            *index < 5 || (include_completed_words && *kind == CandidateKind::Word)
        })
        .map(|(_, (text, _))| text.as_str())
}

// A second whole-reading homophone can push the fourth root Prefix past the
// old first-five source window. Recover only that already published Word;
// never treat later finished paths or the fifth mixed extra as new sources.
fn displaced_prefix_sources(
    code: &str,
    choices: &[(String, CandidateKind)],
    entries: &[&PinyinEntry],
) -> Vec<&'static str> {
    let code = code.trim_matches(is_pinyin_spacing);
    if code.is_empty() || !code.bytes().all(|ch| ch.is_ascii_lowercase()) {
        return Vec::new();
    }
    let mut exact = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Word && entry.reading == code)
        .map(|entry| entry.text);
    let Some(first) = exact.next() else {
        return Vec::new();
    };
    if !exact.any(|text| text != first) {
        return Vec::new();
    }
    entries
        .iter()
        .filter(|entry| entry.match_input(code) == Some(ReadingMatch::Prefix))
        .take(4)
        .filter(|entry| {
            choices
                .iter()
                .skip(5)
                .any(|(text, kind)| *kind == CandidateKind::Word && text == entry.text)
        })
        .map(|entry| entry.text)
        .collect()
}

// A visible Prefix Word can complete an earlier finished Sentence branch even
// when a dictionary Word is primary. Do not mistake a Sentence-kind Prefix for
// that finished witness: use the decoder's actual pre-completion boundary.
fn displaced_branch_sources(
    choices: &[(String, CandidateKind)],
    finished_count: usize,
) -> impl Iterator<Item = &str> {
    let primary_word = choices.first().is_some_and(|(text, kind)| {
        *kind == CandidateKind::Word && is_dictionary_word(text.trim_matches(is_pinyin_spacing))
    });
    let finished = &choices[..finished_count.min(5).min(choices.len())];
    choices
        .iter()
        .skip(5)
        .filter(move |(text, kind)| {
            primary_word
                && *kind == CandidateKind::Word
                && finished.iter().skip(1).any(|(anchor, kind)| {
                    *kind == CandidateKind::Sentence
                        && !anchor.is_empty()
                        && text.len() > anchor.len()
                        && text.starts_with(anchor)
                })
        })
        .map(|(text, _)| text.as_str())
}

/// Match already converted text without erasing its literal horizontal padding.
/// The optional reading tail is only a routing hint for the long-draft window:
/// emitted sentences must match an actual decoder result, never this hint.
fn authored_sentence_suffix<'a>(
    input: &str,
    sentence: &'a str,
    allow_reading_tail: bool,
) -> Option<&'a str> {
    let mut expected = sentence.char_indices().peekable();
    let mut consumed = 0;
    for (offset, ch) in input.char_indices() {
        if is_pinyin_spacing(ch) {
            // Respect authored spaces when present, but preserve the user's
            // exact spacing in the prefix used to construct the candidate.
            while let Some(&(index, next)) = expected.peek()
                && is_pinyin_spacing(next)
            {
                consumed = index + next.len_utf8();
                expected.next();
            }
            continue;
        }
        let (index, next) = expected.next()?;
        if ch != next {
            if allow_reading_tail && (ch.is_ascii_alphabetic() || matches!(ch, 'ü' | 'Ü')) {
                let reading = normalize_pinyin(&input[offset..]);
                if reading
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || is_pinyin_separator(ch))
                    // This is only a routing hint: keeping more verified Word
                    // progress cannot emit a sentence. Actual emission below
                    // still applies its precise-word/homophone guard.
                    && continuation_paths(&pinyin_choices(&input[offset..]), true).any(
                        |converted| {
                            authored_sentence_suffix(converted, &sentence[index..], false).is_some()
                        },
                    )
                {
                    return Some(&sentence[index..]);
                }
            }
            return None;
        }
        consumed = index + ch.len_utf8();
    }
    (consumed < sentence.len()).then_some(&sentence[consumed..])
}

// Only immutable catalog references and authored positions enter this index.
// Its buckets are lookup aids, never candidate or continuation priority order.
struct ContinuationIndex<'a> {
    contexts: Vec<(&'a str, &'a [String])>,
    by_initial: HashMap<char, Vec<usize>>,
}

impl<'a> ContinuationIndex<'a> {
    fn new(contexts: impl IntoIterator<Item = (&'a str, &'a [String])>) -> Self {
        let contexts: Vec<_> = contexts.into_iter().collect();
        let mut by_initial: HashMap<char, Vec<usize>> = HashMap::new();
        for (index, (phrase, _)) in contexts.iter().enumerate() {
            // Validated catalog contexts are nonempty and need not start in Han.
            if let Some(initial) = phrase.chars().next() {
                by_initial.entry(initial).or_default().push(index);
            }
        }
        Self {
            contexts,
            by_initial,
        }
    }

    fn possible_contexts(&self, window: &str) -> Vec<usize> {
        debug_assert!(window.chars().count() <= super::draft::LOCAL_TAIL_CHARS);
        // The caller already supplies the old 256-scalar tail. Keep this local
        // work bound explicit; no draft text is retained after the query.
        let mut initials: Vec<_> = window
            .chars()
            .take(super::draft::LOCAL_TAIL_CHARS)
            .collect();
        initials.sort_unstable();
        initials.dedup();
        let mut possible = Vec::new();
        for initial in initials {
            if let Some(bucket) = self.by_initial.get(&initial) {
                possible.extend_from_slice(bucket);
            }
        }
        // Every context has one initial, and every initial is queried once.
        // Temporary positions are therefore bounded by the active catalog.
        debug_assert!(possible.len() <= self.contexts.len());
        possible.sort_unstable();
        possible
    }
}

fn continuation_index() -> &'static ContinuationIndex<'static> {
    static INDEX: OnceLock<ContinuationIndex<'static>> = OnceLock::new();
    INDEX.get_or_init(|| ContinuationIndex::new(vocabulary().continuations()))
}

/// Prefer the longest matching sentence progress, not merely the last word.
/// Both converted text and raw long-draft routing search the same bounded tail.
fn authored_continuation(
    seed: &str,
    allow_reading_tail: bool,
) -> Option<(&str, &'static [String])> {
    authored_continuation_from_index(seed, allow_reading_tail, continuation_index())
}

fn authored_continuation_from_index<'seed, 'catalog>(
    seed: &'seed str,
    allow_reading_tail: bool,
    index: &ContinuationIndex<'catalog>,
) -> Option<(&'seed str, &'catalog [String])> {
    let window_start = seed
        .char_indices()
        .rev()
        .nth(super::draft::LOCAL_TAIL_CHARS - 1)
        .map_or(0, |(index, _)| index);
    let mut best = None;
    let mut best_key = (0, 0);
    for context_index in index.possible_contexts(&seed[window_start..]) {
        let (phrase, values) = index.contexts[context_index];
        for (offset, _) in seed[window_start..].match_indices(phrase) {
            let start = window_start + offset;
            let tail = &seed[start..];
            let key = (tail.len(), phrase.len());
            if key <= best_key {
                continue;
            }
            let token = seed[..start]
                .rsplit(char::is_whitespace)
                .next()
                .unwrap_or("");
            if token.contains(['/', '\\', '_', '@']) {
                continue;
            }
            if values
                .iter()
                .any(|text| authored_sentence_suffix(tail, text, allow_reading_tail).is_some())
            {
                best_key = key;
                best = Some((tail, values));
            }
        }
    }
    best
}

/// Retain authored Han sentence progress, including an unfinished Pinyin tail.
/// Horizontal padding belongs to the draft; line boundaries and punctuation
/// are not discarded to force a continuation.
pub(crate) fn adopted_continuation_tail(seed: &str) -> Option<&str> {
    authored_continuation(seed, true).map(|(tail, _)| tail)
}

pub(crate) fn mixed_candidates(
    seed: &str,
) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    let code = normalize_pinyin(seed);
    let mut output = Vec::new();
    if code.bytes().filter(u8::is_ascii_lowercase).count() >= 2 {
        for word in pinyin_starting_with(&code)
            .iter()
            .filter(|word| word.match_input(&code) == Some(ReadingMatch::Prefix))
            .take(5)
        {
            output.push((
                word.text.to_owned(),
                if is_dictionary_word(word.text) {
                    CandidateKind::Word
                } else {
                    CandidateKind::Sentence
                },
            ));
        }
    }
    // A whole-reading completion can sit beyond the decoder's first five
    // conversions when shorter ambiguous readings finish first. It is already
    // a verified Word above, so retain it as an authored continuation source.
    // At most five such words are considered, after the original decoder paths.
    // When the primary conversion already is a dictionary word, preserve the
    // original homophone page instead of promoting unrelated longer contexts
    // (e.g. shijian -> 时间 must not evict 世间 for 时间安排's sentences).
    let DecodedPinyin {
        choices: conversions,
        finished_count,
    } = pinyin_choices_with_provenance(seed);
    let include_completed_words = conversions
        .first()
        .is_some_and(|(text, _)| !is_dictionary_word(text.trim_matches(is_pinyin_spacing)));
    let completed_words = if include_completed_words {
        output
            .iter()
            .filter(|(_, kind)| *kind == CandidateKind::Word)
            .map(|(text, _)| text.clone())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    // Already adopted Han prefixes are not whole reading matches above. Their
    // verified partial Word paths must also keep the original authored context.
    for converted in continuation_paths(&conversions, include_completed_words) {
        if let Some((tail, values)) = authored_continuation(converted, false) {
            for text in values {
                if let Some(suffix) = authored_sentence_suffix(tail, text, false) {
                    let continuation = format!("{converted}{suffix}");
                    if !output.iter().any(|(text, _)| *text == continuation) {
                        output.push((continuation, CandidateKind::Sentence));
                    }
                }
            }
        }
    }
    for word in completed_words {
        // Only this whole word's own context is eligible. Do not search inside
        // it for an unrelated shorter context or infer extra reading segments.
        if let Some((_, values)) = vocabulary()
            .continuations()
            .find(|(context, _)| *context == word)
        {
            for text in values {
                if !output.iter().any(|(candidate, _)| candidate == text) {
                    output.push((text.clone(), CandidateKind::Sentence));
                }
            }
        }
    }
    // Extra prefix completions must not outrank an existing shorter dictionary
    // conversion just because the vocabulary grew. Defer only strict textual
    // extensions absent from the decoder's Word choices. Do this after all
    // authored queries so their sources, deduplication and kinds stay intact.
    let mut prioritized = Vec::with_capacity(output.len());
    let mut deferred_words = Vec::new();
    for item in output {
        let (text, kind) = &item;
        let extra_extension = *kind == CandidateKind::Word
            && !conversions
                .iter()
                .any(|(base, kind)| *kind == CandidateKind::Word && base == text)
            && conversions.iter().any(|(base, kind)| {
                *kind == CandidateKind::Word
                    && is_dictionary_word(base)
                    && text.len() > base.len()
                    && text.starts_with(base)
            });
        if extra_extension {
            deferred_words.push(item);
        } else {
            prioritized.push(item);
        }
    }
    prioritized.extend(deferred_words);
    // Preserve the old mixed output and its score indices before appending
    // recovered whole-word contexts. Final page quotas still apply normally.
    for word in displaced_prefix_sources(
        &code,
        &conversions,
        pinyin_starting_with(code.trim_matches(is_pinyin_spacing)),
    ) {
        if let Some((_, values)) = vocabulary()
            .continuations()
            .find(|(context, _)| *context == word)
        {
            for text in values {
                if !prioritized.iter().any(|(candidate, _)| candidate == text) {
                    prioritized.push((text.clone(), CandidateKind::Sentence));
                }
            }
        }
    }
    for converted in displaced_branch_sources(&conversions, finished_count) {
        if let Some((tail, values)) = authored_continuation(converted, false) {
            for text in values {
                if let Some(suffix) = authored_sentence_suffix(tail, text, false) {
                    let continuation = format!("{converted}{suffix}");
                    if !prioritized.iter().any(|(text, _)| *text == continuation) {
                        prioritized.push((continuation, CandidateKind::Sentence));
                    }
                }
            }
        }
    }
    prioritized
}

pub fn pinyin_candidates(seed: &str) -> Vec<String> {
    pinyin_choices(seed)
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}

fn pinyin_choices(seed: &str) -> Vec<(String, CandidateKind)> {
    pinyin_choices_with_provenance(seed).choices
}

#[derive(Default)]
struct DecodedPinyin {
    choices: Vec<(String, CandidateKind)>,
    // Number of unique retained finished paths, before Prefix completions.
    finished_count: usize,
}

fn pinyin_choices_with_provenance(seed: &str) -> DecodedPinyin {
    let choices = decode_pinyin_choices(seed);
    if seed.chars().count() > super::draft::LOCAL_TAIL_CHARS
        || choices
            .choices
            .iter()
            .any(|(_, kind)| *kind != CandidateKind::Literal)
    {
        return choices;
    }
    // A literal acronym in a short draft can block its entire reading path.
    // Recover only after the same written boundary used by long drafts, when
    // the full decoder has no conversion. Never normalize the frozen prefix,
    // split at ASCII punctuation or recursively reinterpret the local tail.
    let Some((prefix, tail)) = super::draft::chinese_boundary_tail(seed) else {
        return choices;
    };
    let mut recovered = decode_pinyin_choices(tail);
    if !recovered
        .choices
        .iter()
        .any(|(_, kind)| *kind != CandidateKind::Literal)
    {
        return choices;
    }
    for (text, _) in &mut recovered.choices {
        *text = format!("{prefix}{text}");
    }
    recovered
}

fn decode_pinyin_choices(seed: &str) -> DecodedPinyin {
    if seed.trim().is_empty() {
        return DecodedPinyin::default();
    }
    let input_chars = seed.chars().count();
    if input_chars > super::draft::LOCAL_TAIL_CHARS {
        return DecodedPinyin {
            choices: vec![(seed.into(), CandidateKind::Literal)],
            finished_count: 0,
        };
    }
    let normalized = normalize_pinyin(seed);
    let mut paths = vec![(0usize, String::new(), 0usize)];
    // Bounded beam search: never exponential in the number of ambiguous syllables.
    let mut finished = Vec::new();
    let mut completions = Vec::new();
    // Every live path consumes at least one Unicode scalar per round. Allow
    // the already bounded input to finish, plus one round to collect its end;
    // a fixed 128 rounds loses Pinyin after long adopted/literal prefixes.
    // Normalization never increases the input's scalar count, and the beam
    // width remains capped below, independently of the number of syllables.
    for _ in 0..=input_chars {
        let mut next = Vec::new();
        for (offset, text, segments) in paths {
            let rest = &normalized[offset..];
            if rest.is_empty() {
                finished.push((text, segments));
                continue;
            }
            let separators = separator_bytes(rest);
            if separators > 0 {
                // Only separators within a decoded phonetic span may disappear.
                // A trailing space can still end a reading; a closing quote or
                // spacing before literal Han text must not disappear with it.
                let after_reading = normalized[..offset]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_ascii_lowercase());
                let tail = &rest[separators..];
                let within_readings = tail.starts_with(|ch: char| ch.is_ascii_lowercase());
                let trailing_spacing = tail.is_empty() && rest.chars().all(is_pinyin_spacing);
                let text = if after_reading && (within_readings || trailing_spacing) {
                    text
                } else {
                    format!("{text}{}", &rest[..separators])
                };
                next.push((offset + separators, text, segments));
                continue;
            }
            if let Some(ch) = rest.chars().next().filter(|ch| !ch.is_ascii_alphabetic()) {
                next.push((offset + ch.len_utf8(), format!("{text}{ch}"), segments));
                continue;
            }
            let can_complete = rest.bytes().filter(u8::is_ascii_lowercase).count() >= 2;
            let mut completed = 0;
            for word in pinyin_starting_with(rest) {
                match word.match_input(rest) {
                    Some(ReadingMatch::Exact(bytes)) => {
                        next.push((offset + bytes, format!("{text}{}", word.text), segments + 1))
                    }
                    // Complete only the final reading, including explicitly
                    // separated syllables. A separator inside a syllable fails
                    // the match; it is never blindly stripped to make a word.
                    Some(ReadingMatch::Prefix) if can_complete && completed < 4 => {
                        completed += 1;
                        completions.push((
                            format!("{text}{}", word.text),
                            segments + 1,
                            if is_dictionary_word(word.text) {
                                CandidateKind::Word
                            } else {
                                CandidateKind::Sentence
                            },
                        ));
                    }
                    _ => {}
                }
            }
        }
        next.sort_by(|left, right| right.0.cmp(&left.0).then(left.2.cmp(&right.2)));
        next.truncate(24);
        completions.sort_by_key(|(_, segments, _)| *segments);
        completions.truncate(64);
        if next.is_empty() {
            break;
        }
        paths = next;
    }
    finished.sort_by_key(|(_, segments)| *segments);
    let mut seen = std::collections::HashSet::new();
    let mut output: Vec<_> = finished
        .into_iter()
        .filter(|(text, _)| seen.insert(text.clone()))
        .take(5)
        .map(|(text, _)| {
            let kind = if text == seed {
                CandidateKind::Literal
            } else if is_dictionary_word(text.trim_matches(is_pinyin_spacing)) {
                CandidateKind::Word
            } else {
                CandidateKind::Sentence
            };
            (text, kind)
        })
        .collect();
    let finished_count = output.len();
    output.extend(
        completions
            .into_iter()
            .filter(|(text, _, _)| seen.insert(text.clone()))
            .take(4)
            .map(|(text, _, kind)| (text, kind)),
    );
    if seen.insert(seed.to_owned()) {
        output.push((seed.into(), CandidateKind::Literal));
    }
    DecodedPinyin {
        choices: output,
        finished_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_authored_continuation<'seed, 'catalog>(
        seed: &'seed str,
        allow_reading_tail: bool,
        contexts: impl IntoIterator<Item = (&'catalog str, &'catalog [String])>,
    ) -> Option<(&'seed str, &'catalog [String])> {
        let window_start = seed
            .char_indices()
            .rev()
            .nth(super::super::draft::LOCAL_TAIL_CHARS - 1)
            .map_or(0, |(index, _)| index);
        let mut best = None;
        let mut best_key = (0, 0);
        for (phrase, values) in contexts {
            for (offset, _) in seed[window_start..].match_indices(phrase) {
                let start = window_start + offset;
                let tail = &seed[start..];
                let key = (tail.len(), phrase.len());
                if key <= best_key {
                    continue;
                }
                let token = seed[..start]
                    .rsplit(char::is_whitespace)
                    .next()
                    .unwrap_or("");
                if token.contains(['/', '\\', '_', '@']) {
                    continue;
                }
                if values
                    .iter()
                    .any(|text| authored_sentence_suffix(tail, text, allow_reading_tail).is_some())
                {
                    best_key = key;
                    best = Some((tail, values));
                }
            }
        }
        best
    }

    fn assert_continuation_index_reference(seed: &str, index: &ContinuationIndex<'_>) {
        for allow_reading_tail in [false, true] {
            assert_eq!(
                authored_continuation_from_index(seed, allow_reading_tail, index),
                linear_authored_continuation(
                    seed,
                    allow_reading_tail,
                    index.contexts.iter().copied(),
                ),
                "seed={seed:?}, allow_reading_tail={allow_reading_tail}",
            );
        }
    }

    #[test]
    fn continuation_index_matches_linear_reference_for_all_contexts_and_authored_sentences() {
        let index = continuation_index();
        for (phrase, values) in vocabulary().continuations() {
            assert_continuation_index_reference(phrase, index);
            for sentence in values {
                let ends: Vec<_> = sentence
                    .char_indices()
                    .map(|(offset, ch)| offset + ch.len_utf8())
                    .collect();
                let mut prefix_ends = vec![
                    sentence.len(),
                    ends[ends.len() / 2],
                    sentence.char_indices().next_back().unwrap().0,
                ];
                if let Some(&end) = ends.iter().find(|&&end| end > phrase.len()) {
                    prefix_ends.push(end);
                }
                prefix_ends.sort_unstable();
                prefix_ends.dedup();
                for end in prefix_ends {
                    assert_continuation_index_reference(&sentence[..end], index);
                }
            }
        }
    }

    #[test]
    fn continuation_index_preserves_padding_lines_tokens_repeated_occurrences_and_raw_text() {
        let index = continuation_index();
        for phrase in [
            "你好",
            "还没睡",
            "我这边看不到画面",
            "纸巾只剩一包了",
            "先腾点地方出来",
        ] {
            for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
                for seed in [
                    format!("{phrase}{gap}"),
                    format!("{gap}{phrase}{gap}"),
                    format!("{phrase}{gap}，wo"),
                    format!("前文 café 😀。{gap}{phrase}{gap}，wo"),
                ] {
                    assert_continuation_index_reference(&seed, index);
                }
            }
            for boundary in [
                "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                assert_continuation_index_reference(&format!("{phrase}{boundary}，我"), index);
                assert_continuation_index_reference(&format!("{phrase}{boundary}{phrase}"), index);
            }
            for prefix in [
                "https://",
                "dir\\",
                "user_",
                "person@",
                "literal'",
                "literal\"",
            ] {
                assert_continuation_index_reference(&format!("{prefix}{phrase}"), index);
            }
            assert_continuation_index_reference(&format!("{phrase}，{phrase}"), index);
            assert_continuation_index_reference(&format!("{phrase}{phrase}"), index);
        }
        for seed in [
            "",
            "zzqvjy",
            "API",
            "café",
            "αБ😀",
            "你好'",
            "你好3.5",
            "你好3:30",
            "我想学习zhongw",
            "发音 ke y",
            "登录失败后可以 ZAI SHI YI",
            "你那边有回声",
        ] {
            assert_continuation_index_reference(seed, index);
        }
        for length in [255, 256, 257, 1024] {
            let seed = format!("{}你好", "前".repeat(length - 2));
            assert_eq!(seed.chars().count(), length);
            assert_continuation_index_reference(&seed, index);
        }
        assert_continuation_index_reference(
            &format!("{}纸巾只剩一包了，gui zi", "前文 café 😀。  ".repeat(40)),
            index,
        );
    }

    #[test]
    fn continuation_index_supports_non_han_initials_and_retains_original_tie_order() {
        // Duplicate synthetic contexts exercise the old <= tie rule directly;
        // they do not authorize loading a duplicate context in a real pack.
        let contexts = [
            ("API", vec!["API 已确认。".to_owned()]),
            ("café", vec!["café 可以继续。".to_owned()]),
            ("😀", vec!["😀 我先看看。".to_owned()]),
            ("α", vec!["α 保留原文。".to_owned()]),
            ("Ａ", vec!["Ａ 全角字母。".to_owned()]),
            ("甲", vec!["甲先选。".to_owned(), "甲 甲继续。".to_owned()]),
            ("甲", vec!["甲后选。".to_owned()]),
            ("甲乙", vec!["甲乙继续。".to_owned()]),
        ];
        let index = ContinuationIndex::new(
            contexts
                .iter()
                .map(|(phrase, values)| (*phrase, values.as_slice())),
        );
        for (phrase, values) in &contexts {
            assert_continuation_index_reference(phrase, &index);
            for sentence in values {
                assert_continuation_index_reference(sentence, &index);
            }
        }
        for seed in [
            "API ",
            "API 已",
            "café 可",
            "😀 我",
            "α 保",
            "Ａ 全",
            "甲",
            "甲乙",
            "甲 甲",
            "https://API",
            "name@café",
            "dir_😀",
            "α\n保",
            "😀\u{3000}我",
        ] {
            assert_continuation_index_reference(seed, &index);
        }
        assert_eq!(index.possible_contexts("甲乙"), vec![5, 6, 7]);
        let (_, values) = authored_continuation_from_index("甲", false, &index).unwrap();
        assert_eq!(values, contexts[5].1.as_slice());
        assert!(std::ptr::eq(values.as_ptr(), contexts[5].1.as_ptr()));
    }

    #[test]
    fn continuation_index_reuses_catalog_references_and_bounds_ordered_query_work() {
        let index = continuation_index();
        assert!(std::ptr::eq(index, continuation_index()));
        assert_eq!(index.contexts.len(), vocabulary().continuations().count());
        let mut positions = Vec::new();
        for (initial, bucket) in &index.by_initial {
            assert!(bucket.windows(2).all(|pair| pair[0] < pair[1]));
            for &position in bucket {
                assert_eq!(index.contexts[position].0.chars().next(), Some(*initial));
                positions.push(position);
            }
        }
        positions.sort_unstable();
        assert_eq!(positions, (0..index.contexts.len()).collect::<Vec<_>>());
        for (actual, expected) in index.contexts.iter().zip(vocabulary().continuations()) {
            assert!(std::ptr::eq(actual.0.as_ptr(), expected.0.as_ptr()));
            assert!(std::ptr::eq(actual.1.as_ptr(), expected.1.as_ptr()));
        }
        for window in [
            "你好".to_owned(),
            "收纳".to_owned(),
            "我".repeat(256),
            "z".repeat(256),
        ] {
            let actual = index.possible_contexts(&window);
            let expected: Vec<_> = index
                .contexts
                .iter()
                .enumerate()
                .filter_map(|(position, (phrase, _))| {
                    window
                        .contains(phrase.chars().next().unwrap())
                        .then_some(position)
                })
                .collect();
            assert_eq!(actual, expected);
            assert!(actual.len() <= index.contexts.len());
            assert!(actual.windows(2).all(|pair| pair[0] < pair[1]));
        }
        assert!(index.possible_contexts("").is_empty());
        assert!(index.possible_contexts("你好").len() < index.contexts.len());
    }

    #[test]
    fn displaced_branch_sources_require_finished_sentence_provenance_and_visible_word() {
        let mut choices: Vec<_> = [
            ("拿过来", CandidateKind::Word),
            ("拿国来", CandidateKind::Sentence),
            ("那国来", CandidateKind::Sentence),
            ("那过来", CandidateKind::Sentence),
            ("拿国来不及了", CandidateKind::Word),
            ("拿国来得及吗", CandidateKind::Word),
            ("拿过来不及了", CandidateKind::Word),
            ("na guo lai", CandidateKind::Literal),
        ]
        .into_iter()
        .map(|(text, kind)| (text.into(), kind))
        .collect();
        assert_eq!(
            displaced_branch_sources(&choices, 4).collect::<Vec<_>>(),
            ["拿国来得及吗"]
        );
        // A Sentence-kind Prefix must not be mistaken for a finished anchor.
        assert!(displaced_branch_sources(&choices, 1).next().is_none());
        assert!(displaced_branch_sources(&choices, 0).next().is_none());
        for kind in [CandidateKind::Word, CandidateKind::Literal] {
            choices[1].1 = kind;
            assert!(displaced_branch_sources(&choices, 2).next().is_none());
        }
        choices[1].1 = CandidateKind::Sentence;
        for kind in [CandidateKind::Sentence, CandidateKind::Literal] {
            choices[5].1 = kind;
            assert!(displaced_branch_sources(&choices, 2).next().is_none());
        }
        choices[5].1 = CandidateKind::Word;
        for (text, kind) in [
            ("拿过来", CandidateKind::Sentence),
            ("拿过来", CandidateKind::Literal),
            ("未收录主词", CandidateKind::Word),
        ] {
            choices[0] = (text.into(), kind);
            assert!(displaced_branch_sources(&choices, 2).next().is_none());
        }
    }

    #[test]
    fn displaced_branch_sources_keep_original_window_and_do_not_use_primary_or_substrings() {
        let choices: Vec<_> = [
            ("拿过来", CandidateKind::Word),
            ("拿国来", CandidateKind::Sentence),
            ("拿国来得及吗", CandidateKind::Word),
            ("无关", CandidateKind::Sentence),
            ("另一项", CandidateKind::Word),
            ("拿过来不及了", CandidateKind::Word),
            ("前文拿国来得及吗", CandidateKind::Word),
            ("拿国来", CandidateKind::Word),
        ]
        .into_iter()
        .map(|(text, kind)| (text.into(), kind))
        .collect();
        assert!(displaced_branch_sources(&choices, 2).next().is_none());
        assert!(displaced_branch_sources(&[], 5).next().is_none());
    }

    #[test]
    fn decoded_finished_boundary_survives_literal_prefix_recovery_without_changing_choices() {
        for tail in ["na guo lai", "ni hao", "shijian", "shi jian"] {
            let decoded = pinyin_choices_with_provenance(tail);
            assert!((1..=5).contains(&decoded.finished_count));
            assert_eq!(decoded.choices, pinyin_choices(tail));
            let prefix = "API。";
            let recovered = pinyin_choices_with_provenance(&format!("{prefix}{tail}"));
            assert_eq!(recovered.finished_count, decoded.finished_count);
            assert_eq!(
                recovered.choices,
                decoded
                    .choices
                    .into_iter()
                    .map(|(text, kind)| (format!("{prefix}{text}"), kind))
                    .collect::<Vec<_>>()
            );
        }
        assert!(pinyin_choices_with_provenance("").choices.is_empty());
        let seed = "字".repeat(super::super::draft::LOCAL_TAIL_CHARS + 1);
        let decoded = pinyin_choices_with_provenance(&seed);
        assert_eq!(decoded.finished_count, 0);
        assert_eq!(decoded.choices, [(seed, CandidateKind::Literal)]);
    }

    #[test]
    fn displaced_prefix_source_window_requires_distinct_single_reading_homophones() {
        let entries = [
            PinyinEntry::new("ka", "甲"),
            PinyinEntry::new("ka", "乙"),
            PinyinEntry::new("ka'ba", "第一"),
            PinyinEntry::new("ka'ca", "第二"),
            PinyinEntry::new("ka'da", "第三"),
            PinyinEntry::new("ka'fa", "第四"),
            PinyinEntry::new("ka'ga", "第五"),
        ];
        let refs: Vec<_> = entries.iter().collect();
        let choices: Vec<_> = ["甲", "乙", "第一", "第二", "第三", "第四", "第五"]
            .into_iter()
            .map(|text| (text.to_owned(), CandidateKind::Word))
            .collect();
        assert_eq!(displaced_prefix_sources("ka", &choices, &refs), ["第四"]);
        assert_eq!(
            displaced_prefix_sources("ka \t\u{3000}", &choices, &refs),
            ["第四"]
        );
        for code in ["", "ka'", "ka\n", "ka4.5", "ka2:30", "k a", "甲ka"] {
            assert!(
                displaced_prefix_sources(code, &choices, &refs).is_empty(),
                "{code:?}"
            );
        }
        let duplicate_refs = [
            &entries[0],
            &entries[0],
            &entries[2],
            &entries[3],
            &entries[4],
            &entries[5],
        ];
        assert!(displaced_prefix_sources("ka", &choices, &duplicate_refs).is_empty());
        let single_refs = [
            &entries[0],
            &entries[2],
            &entries[3],
            &entries[4],
            &entries[5],
        ];
        assert!(displaced_prefix_sources("ka", &choices, &single_refs).is_empty());
    }

    #[test]
    fn displaced_prefix_source_requires_visible_word_and_does_not_reset_four_prefix_limit() {
        let entries = [
            PinyinEntry::new("ka", "甲"),
            PinyinEntry::new("ka", "乙"),
            PinyinEntry::new("ka'ba", "第一"),
            PinyinEntry::new("ka'ca", "第二"),
            PinyinEntry::new("ka'da", "第三"),
            PinyinEntry::new("ka'fa", "第四"),
            PinyinEntry::new("ka'ga", "第五"),
        ];
        let refs: Vec<_> = entries.iter().collect();
        for kind in [CandidateKind::Sentence, CandidateKind::Literal] {
            let mut choices = vec![("甲".to_owned(), CandidateKind::Word); 5];
            choices.push(("第四".to_owned(), kind));
            choices.push(("第五".to_owned(), CandidateKind::Word));
            assert!(displaced_prefix_sources("ka", &choices, &refs).is_empty());
        }
        let early: Vec<_> = ["甲", "乙", "第一", "第二", "第四", "第五"]
            .into_iter()
            .map(|text| (text.to_owned(), CandidateKind::Word))
            .collect();
        assert!(displaced_prefix_sources("ka", &early, &refs).is_empty());
        let later_finished: Vec<_> = ["甲", "乙", "甲", "甲", "甲", "未有前缀来源"]
            .into_iter()
            .map(|text| (text.to_owned(), CandidateKind::Word))
            .collect();
        assert!(displaced_prefix_sources("ka", &later_finished, &refs).is_empty());
    }

    #[test]
    fn displaced_whole_contexts_append_after_the_original_mixed_extra_without_duplicates() {
        for (seed, original, pair) in [
            (
                "ta",
                ["他们", "讨论", "堂食", "太好了", "太棒了"],
                ["太好了，终于等到你了。", "太好了，我们一起去吧。"],
            ),
            (
                "hao",
                ["好吃", "好喝", "好的", "好久不见", "好厉害"],
                ["好久不见，最近怎么样？", "好久不见，有空一起吃饭吧。"],
            ),
            (
                "you",
                ["游泳", "邮件", "优先", "优先级", "优惠"],
                ["优先级请再确认一下。", "优先级较高的事项先处理。"],
            ),
        ] {
            let choices = mixed_candidates(seed);
            assert_eq!(
                choices[..5],
                original.map(|text| (text.into(), CandidateKind::Word))
            );
            assert_eq!(
                choices[5..],
                pair.map(|text| (text.into(), CandidateKind::Sentence))
            );
        }
    }

    #[test]
    fn unfinished_reading_cannot_consume_a_final_apostrophe_to_invent_a_syllable() {
        let entry = PinyinEntry::new("zai'shi'yi'ci", "再试一次");
        for input in ["zaishiyi", "zai'shi'yi", "zai'shi'yi'c", "zai shi yi c "] {
            assert!(matches!(
                entry.match_input(input),
                Some(ReadingMatch::Prefix)
            ));
        }
        for input in ["zaishiyi'", "zai'shi'yi'", "zai'shi'yi'  ", "zai shi yi'\t"] {
            assert!(entry.match_input(input).is_none(), "{input:?}");
        }
        let input = "zai'shi'yi'ci'";
        assert!(matches!(
            entry.match_input(input),
            Some(ReadingMatch::Exact(bytes)) if bytes == input.len() - 1
        ));
    }

    #[test]
    fn word_membership_index_matches_linear_reference_for_the_entire_vocabulary() {
        let linear = |text: &str| {
            pinyin()
                .iter()
                .any(|entry| entry.text == text && entry.kind == EntryKind::Word)
        };
        let mut probes = HashSet::new();
        for entry in pinyin() {
            probes.insert(entry.text.to_owned());
            probes.insert(entry.reading.to_owned());
            // Membership is exact; callers, not the index, own whitespace policy.
            probes.insert(format!(" {}", entry.text));
            probes.insert(format!("{}\u{3000}", entry.text));
        }
        for (prefix, continuations) in vocabulary().continuations() {
            probes.insert(prefix.to_owned());
            probes.extend(continuations.iter().cloned());
        }
        probes.extend([
            String::new(),
            "unknown😀".into(),
            "不在词库的合成测试词".into(),
        ]);
        for text in probes {
            assert_eq!(is_dictionary_word(&text), linear(&text), "{text:?}");
        }
        let expected: HashSet<_> = pinyin()
            .iter()
            .filter(|entry| entry.kind == EntryKind::Word)
            .map(|entry| entry.text)
            .collect();
        assert_eq!(dictionary_words(), &expected);
        assert!(dictionary_words().len() <= pinyin().len());
    }

    #[test]
    fn trailing_spacing_does_not_supply_a_required_internal_separator() {
        let entry = PinyinEntry {
            require_separators: true,
            ..PinyinEntry::new("xi'an", "西安")
        };
        for input in ["xia ", "xian\t", "xia\u{3000}"] {
            assert!(entry.match_input(input).is_none(), "{input:?}");
        }
        for input in ["xi a ", "xi'a\t", "xi\u{3000}a\u{a0}"] {
            assert!(
                entry.match_input(input) == Some(ReadingMatch::Prefix),
                "{input:?}"
            );
        }
        assert!(entry.match_input("xi an ") == Some(ReadingMatch::Exact(5)));
        assert_eq!(pinyin_candidates("xian ")[0], "先");
        assert_eq!(pinyin_candidates("xi an ")[0], "西安");
    }

    #[test]
    fn reading_separators_use_utf8_boundaries_without_consuming_newlines() {
        let entry = PinyinEntry::new("shu'ru'fa", "输入法");
        for input in [
            "shu ru fa",
            "shu\tru\tfa",
            "shu\u{3000}ru\u{a0}fa",
            "shu''ru'fa",
        ] {
            assert!(entry.match_input(input) == Some(ReadingMatch::Exact(input.len())));
        }
        for input in [
            "shu\nru fa",
            "shu\r\nru fa",
            "shu\u{2028}ru fa",
            "sh u ru fa",
        ] {
            assert!(entry.match_input(input).is_none(), "{input:?}");
        }
        assert_eq!(pinyin_candidates("ni hao ")[0], "你好");
        assert_eq!(pinyin_candidates("nÜ")[0], "女");
        assert_eq!(pinyin_candidates("NU:")[0], "女");
    }

    #[test]
    fn authored_syllables_match_joined_and_separated_readings_without_inferred_splits() {
        let mut entries = std::collections::HashSet::new();
        for entry in pinyin() {
            assert!(entries.insert((entry.reading, entry.text)));
            assert_eq!(
                entry.reading.split('\'').count(),
                entry.text.chars().count()
            );
            assert!(
                entry
                    .reading
                    .split('\'')
                    .all(|part| !part.is_empty() && part.bytes().all(|ch| ch.is_ascii_lowercase()))
            );
            assert!(
                entry.match_input(entry.reading) == Some(ReadingMatch::Exact(entry.reading.len()))
            );
            if !entry.require_separators {
                let joined = entry.reading.replace('\'', "");
                assert!(entry.match_input(&joined) == Some(ReadingMatch::Exact(joined.len())));
            }
            let repeated = entry.reading.replace('\'', "''");
            assert!(entry.match_input(&repeated) == Some(ReadingMatch::Exact(repeated.len())));
        }
        for (seed, expected) in [
            ("shu1 ru4 fa3", "输入法"),
            ("shu1ru4fa3", "输入法"),
            ("WO3  XI3  HUAN1  BEI3  JING1", "我喜欢北京"),
            ("我想xue2 xi2 zhong1 wen2", "我想学习中文"),
            ("xi'an zai", "西安在"),
            ("xian zai", "现在"),
        ] {
            assert_eq!(pinyin_candidates(seed)[0], expected, "{seed}");
            assert!(pinyin_candidates(seed).iter().any(|value| value == seed));
        }
    }

    #[test]
    fn prefix_index_retains_every_entry_and_its_authored_rank() {
        let mut count = 0;
        for initial in b'a'..=b'z' {
            let prefix = char::from(initial).to_string();
            let indexed = pinyin_starting_with(&prefix);
            let exhaustive: Vec<_> = pinyin()
                .iter()
                .filter(|entry| entry.reading.starts_with(char::from(initial)))
                .collect();
            assert_eq!(indexed.len(), exhaustive.len());
            for (actual, expected) in indexed.iter().zip(exhaustive) {
                // Ordering concerns entry contents, not storage addresses.
                assert_eq!(
                    (actual.reading, actual.text, actual.require_separators),
                    (expected.reading, expected.text, expected.require_separators)
                );
            }
            count += indexed.len();
        }
        assert_eq!(count, pinyin().len());
        for input in ["", "A", "1", "'", " ", "中", "ü"] {
            assert!(pinyin_starting_with(input).is_empty(), "{input}");
        }
    }

    #[test]
    fn every_authored_entry_reaches_the_decoder_with_valid_syllable_boundaries() {
        for entry in pinyin() {
            let mut spellings = vec![entry.reading.to_owned(), entry.reading.replace('\'', " ")];
            if !entry.require_separators {
                spellings.push(entry.reading.replace('\'', ""));
            }
            for seed in spellings {
                let candidates = pinyin_candidates(&seed);
                assert!(
                    candidates.iter().any(|text| text == entry.text),
                    "{seed} -> {}: {candidates:?}",
                    entry.text
                );
                assert!(candidates.contains(&seed));
            }
        }
    }

    #[test]
    fn authored_continuations_are_unique_complete_and_preserve_padding() {
        assert_eq!(pinyin().len(), 3330);
        let mut phrases = std::collections::HashSet::new();
        let mut sentences = std::collections::HashSet::new();
        for (phrase, values) in vocabulary().continuations() {
            assert!(phrases.insert(phrase), "duplicate phrase: {phrase}");
            assert!(!values.is_empty());
            for sentence in values {
                assert!(sentences.insert(sentence), "duplicate sentence: {sentence}");
                assert!(sentence.starts_with(phrase));
                assert!(sentence.ends_with(['。', '？', '！']));
            }
            for padding in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{phrase}{padding}");
                let candidates = mixed_candidates(&seed);
                for sentence in values {
                    let expected = format!("{seed}{}", &sentence[phrase.len()..]);
                    assert!(
                        candidates.iter().any(|(text, kind)| {
                            *text == expected && *kind == CandidateKind::Sentence
                        }),
                        "missing {expected:?}: {candidates:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn converts_common_pinyin_and_tone_numbers() {
        for seed in ["nihao", "ni hao", "ni3 hao3", "NIHAO"] {
            assert_eq!(pinyin_candidates(seed)[0], "你好");
        }
        assert_eq!(pinyin_candidates("woaizhongguo")[0], "我爱中国");
    }

    #[test]
    fn tone_terminators_preserve_punctuation_and_numeric_literal_boundaries() {
        for punctuation in [
            ".", ":", ",", "!", "?", ";", ")", "]", "}", "\"", "”", "’", "，", "。", "！", "？",
            "；", "：", "、", "…", "）", "】", "》", "」", "』", "?!", "...", ".)",
        ] {
            let seed = format!("ni3hao3{punctuation}");
            assert_eq!(normalize_pinyin(&seed), format!("nihao{punctuation}"));
            assert_eq!(pinyin_candidates(&seed)[0], format!("你好{punctuation}"));
            assert!(pinyin_candidates(&seed).contains(&seed));
        }
        for suffix in [
            "3.5",
            "3:30",
            "3,000",
            "3,４００",
            "30",
            "36",
            "0!",
            "6!",
            "3/4",
            "3-4",
            "3_4",
            "3.rs",
            "3://",
            "3@home",
            "3%",
            "3+4",
            "3=4",
        ] {
            let seed = format!("hao{suffix}");
            assert_eq!(normalize_pinyin(&seed), seed, "{seed:?}");
        }
    }
    #[test]
    fn apostrophe_keeps_syllable_boundaries_and_unknown_input_is_lossless() {
        assert_eq!(pinyin_candidates("xi'an")[0], "西安");
        assert_eq!(pinyin_candidates("xian")[0], "先");
        assert_eq!(pinyin_candidates("Rust2026"), ["Rust2026"]);
        assert!(pinyin_candidates("nihao").contains(&"nihao".into()));
        assert_eq!(pinyin_candidates("nü")[0], "女");
    }

    #[test]
    fn spaces_and_apostrophes_keep_pinyin_syllable_boundaries() {
        for seed in ["xi an", "xi  an", "XI AN", "xi1 an1", "xi'an"] {
            let candidates = pinyin_candidates(seed);
            assert_eq!(candidates[0], "西安", "{seed}: {candidates:?}");
            assert!(candidates.iter().any(|text| text == seed));
        }
        assert_eq!(pinyin_candidates("xian")[0], "先");
        assert_eq!(pinyin_candidates("xi'an")[0], "西安");
    }

    #[test]
    fn partial_last_syllables_and_typed_han_prefixes_have_bounded_lossless_completions() {
        for (seed, expected) in [
            ("woxihuanbeij", "我喜欢北京"),
            ("我喜欢bei", "我喜欢北京"),
            ("wo xihuan bei", "我喜欢北京"),
            ("我想学习zhongw", "我想学习中文"),
        ] {
            let candidates = pinyin_choices(seed);
            assert!(
                candidates
                    .iter()
                    .any(|(text, kind)| text == expected && *kind == CandidateKind::Word),
                "{seed}: {candidates:?}"
            );
            assert!(candidates.iter().any(|(text, _)| text == seed));
            assert!(candidates.len() <= 10);
        }
        for seed in [
            "unknownbei",
            "https://bei",
            "Rust2026",
            "字".repeat(300).as_str(),
        ] {
            assert_eq!(pinyin_candidates(seed), [seed]);
        }
    }
}
