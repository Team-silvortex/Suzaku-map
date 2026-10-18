//! Deterministic Romaji/Kana baseline; Kanji vocabulary is intentionally a bootstrap set.

use super::ranked_candidates;
use crate::ime::{Candidate, LanguagePlugin};
use crate::lexicon::{EntryKind, Lexicon};
use std::{collections::HashSet, sync::OnceLock};

fn vocabulary() -> &'static Lexicon {
    crate::lexicon::active("ja").expect("Japanese vocabulary is registered")
}

fn kanji() -> impl DoubleEndedIterator<Item = (&'static str, &'static str)> {
    vocabulary()
        .readings()
        .iter()
        .map(|entry| (entry.reading.as_str(), entry.text.as_str()))
}

#[derive(Default)]
pub struct JapaneseLanguagePlugin;

impl LanguagePlugin for JapaneseLanguagePlugin {
    fn id(&self) -> &str {
        "ja"
    }
    fn display_name(&self) -> &str {
        "日本語"
    }
    fn normalize_seed(&self, input: &str) -> String {
        input.to_owned()
    }
    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }
    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        if seed.trim().is_empty() {
            return Vec::new();
        }
        let kana = composition_kana(seed);
        let mut values: Vec<String> = kanji()
            .filter(|(reading, _)| *reading == kana)
            .map(|(_, text)| text.to_string())
            .collect();
        if values.is_empty() {
            let converted = convert_segments(&kana);
            if converted != kana {
                values.push(converted);
            }
        }
        values.extend([kana.clone(), hiragana_to_katakana(&kana), seed.to_string()]);
        ranked_candidates(values, confidence)
    }
    fn direct_candidates(&self, seed: &str, confidence: f32) -> Option<Vec<Candidate>> {
        Some(self.build_candidates(&[], seed, confidence))
    }
    fn commit_separator(&self) -> &str {
        ""
    }
}

pub(crate) fn is_dictionary_word(text: &str) -> bool {
    dictionary_word_index().contains(text)
}

fn dictionary_word_index() -> &'static HashSet<String> {
    static WORDS: OnceLock<HashSet<String>> = OnceLock::new();
    // The active catalog is immutable after startup, including enabled packs.
    // This is membership only: candidate order never comes from set iteration.
    WORDS.get_or_init(|| dictionary_word_forms(vocabulary()))
}

fn dictionary_word_forms(lexicon: &Lexicon) -> HashSet<String> {
    lexicon
        .readings()
        .iter()
        .filter(|entry| entry.kind == EntryKind::Word)
        .flat_map(|entry| {
            [
                entry.text.clone(),
                entry.reading.clone(),
                hiragana_to_katakana(&entry.reading),
            ]
        })
        .collect()
}

#[cfg(test)]
fn is_dictionary_word_linear(lexicon: &Lexicon, text: &str) -> bool {
    lexicon.readings().iter().any(|entry| {
        entry.kind == EntryKind::Word
            && (entry.text == text
                || entry.reading == text
                || hiragana_to_katakana(&entry.reading) == text)
    })
}

#[cfg(test)]
mod dictionary_word_cache_tests {
    use super::*;

    #[test]
    fn cached_membership_matches_linear_for_all_active_forms_and_authored_sentences() {
        let lexicon = vocabulary();
        for entry in lexicon.readings() {
            let katakana = hiragana_to_katakana(&entry.reading);
            for text in [entry.text.as_str(), entry.reading.as_str(), &katakana] {
                assert_eq!(
                    is_dictionary_word(text),
                    is_dictionary_word_linear(lexicon, text),
                    "{text:?} from {entry:?}"
                );
            }
        }
        for (_, sentences) in lexicon.continuations() {
            for text in sentences {
                assert_eq!(
                    is_dictionary_word(text),
                    is_dictionary_word_linear(lexicon, text),
                    "authored sentence {text:?}"
                );
            }
        }
        for text in [
            "",
            " ",
            " 日本語",
            "日本語 ",
            "\n",
            "日本語\n",
            "https://example.invalid/日本語",
            "🙂日本語",
            "ﾆﾎﾝｺﾞ",
            "Ωテスト",
            "İテスト",
            "Σ123",
            "e\u{301}",
            "日本語\u{200b}",
            "ニホンゴ\u{fe0f}",
        ] {
            assert_eq!(
                is_dictionary_word(text),
                is_dictionary_word_linear(lexicon, text),
                "negative or Unicode probe {text:?}"
            );
        }
    }

    #[test]
    fn cached_index_is_reused_bounded_and_contains_only_linear_word_forms() {
        let lexicon = vocabulary();
        let index = dictionary_word_index();
        assert!(std::ptr::eq(index, dictionary_word_index()));
        let word_count = lexicon
            .readings()
            .iter()
            .filter(|entry| entry.kind == EntryKind::Word)
            .count();
        assert!(index.len() <= word_count * 3);
        for text in index {
            assert!(is_dictionary_word_linear(lexicon, text), "{text:?}");
        }
    }

    #[test]
    fn sentence_readings_do_not_add_word_forms_but_existing_word_aliases_still_match() {
        let lexicon = Lexicon::from_json(
            r#"{
                "format_version": 1,
                "language": "ja",
                "readings": [
                    {"reading":"かな","text":"仮名","kind":"word"},
                    {"reading":"き","text":"木","kind":"word"},
                    {"reading":"き","text":"気","kind":"word"},
                    {"reading":"ぶんしょう","text":"文章です。","kind":"sentence"},
                    {"reading":"かな","text":"文章かなです。","kind":"sentence"},
                    {"reading":"ひとこと","text":"仮名","kind":"sentence"},
                    {"reading":"こうご","text":"カナ","kind":"sentence"}
                ]
            }"#,
        )
        .unwrap();
        let index = dictionary_word_forms(&lexicon);
        assert_eq!(index.len(), 7);
        for entry in lexicon.readings() {
            let katakana = hiragana_to_katakana(&entry.reading);
            for text in [entry.text.as_str(), entry.reading.as_str(), &katakana] {
                assert_eq!(
                    index.contains(text),
                    is_dictionary_word_linear(&lexicon, text),
                    "{text:?} from {entry:?}"
                );
            }
        }
        for text in ["仮名", "かな", "カナ", "木", "気", "き", "キ"] {
            assert!(index.contains(text), "existing Word form {text:?}");
        }
        for text in [
            "文章です。",
            "ぶんしょう",
            "ブンショウ",
            "文章かなです。",
            "ひとこと",
            "ヒトコト",
            "こうご",
            "コウゴ",
        ] {
            assert!(!index.contains(text), "Sentence-only form {text:?}");
        }
    }
}

/// Greedy dictionary segments plus untouched particles. This is a bounded
/// bootstrap conversion, not a morphological analyzer or a full Japanese IME.
fn convert_segments(kana: &str) -> String {
    let mut rest = kana;
    let mut output = String::new();
    while !rest.is_empty() {
        if is_authored_tail(&output, rest) {
            output.push_str(rest);
            break;
        }
        if let Some((length, word)) = conversion_prefix(rest) {
            output.push_str(word);
            rest = &rest[length..];
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    output
}

/// Preserve a whole authored sentence's Kana progress after recognized words.
/// Newly added short nouns must not split its grammatical tail (e.g. 下 in
/// 勉強したい). This is bounded lexicon matching, not general morphology:
/// unknown, conflicting or line-separated tails still follow normal conversion.
fn is_authored_tail(prefix: &str, rest: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }
    if matches_authored_tail(prefix, rest) {
        return true;
    }
    // Closing a matching grammatical fragment must not turn した back into 下.
    // Ignore only explicit trailing prose marks for this protection comparison;
    // keep them in conversion output and in the later whole-sentence query.
    // Unknown text, opening delimiters, operators and line breaks stay significant.
    let stem = rest.trim_end_matches(|ch| {
        matches!(
            ch,
            '.' | ','
                | '!'
                | '?'
                | '。'
                | '、'
                | '．'
                | '，'
                | '！'
                | '？'
                | '…'
                | '‥'
                | '\''
                | '"'
                | '’'
                | '”'
                | ')'
                | ']'
                | '）'
                | '］'
                | '」'
                | '』'
                | '】'
                | '〉'
                | '》'
        )
    });
    stem.len() != rest.len() && !stem.is_empty() && matches_authored_tail(prefix, stem)
}

fn matches_authored_tail(prefix: &str, rest: &str) -> bool {
    if vocabulary().continuations().any(|(context, sentences)| {
        prefix.starts_with(context)
            && sentences.iter().any(|text| {
                text.strip_prefix(prefix)
                    .is_some_and(|tail| tail.starts_with(rest))
            })
    }) {
        return true;
    }
    // Only the comparison may recognize a Word's existing Katakana alias.
    // Keep the adopted spelling in output and avoid choosing a canonical
    // homophone: compare each alias against the authored sentence itself.
    prefix
        .chars()
        .any(|ch| matches!(ch, '\u{30a1}'..='\u{30f6}'))
        && vocabulary().continuations().any(|(context, sentences)| {
            sentences.iter().any(|text| {
                strip_known_alias_prefix(text, prefix).is_some_and(|tail| {
                    text[..text.len() - tail.len()].starts_with(context) && tail.starts_with(rest)
                })
            })
        })
}

fn strip_known_alias_prefix<'a>(mut text: &'a str, mut prefix: &str) -> Option<&'a str> {
    while !prefix.is_empty() {
        let ch = text.chars().next()?;
        if prefix.starts_with(ch) {
            text = &text[ch.len_utf8()..];
            prefix = &prefix[ch.len_utf8()..];
            continue;
        }
        // Use the same longest recognized written segment as conversion.
        // Unknown Katakana, shorter fragments and line boundaries cannot
        // become aliases or disappear from the comparison.
        let length = known_written_prefix_len(prefix)?;
        let written = &prefix[..length];
        let entry = vocabulary().readings().iter().find(|entry| {
            entry.kind == EntryKind::Word
                && entry.reading.len() == length
                && written.chars().eq(entry.reading.chars().map(katakana_char))
                && text.starts_with(&entry.text)
        })?;
        text = &text[entry.text.len()..];
        prefix = &prefix[length..];
    }
    Some(text)
}

fn is_line_boundary(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

fn composition_kana(seed: &str) -> String {
    // Shift+Space separates romaji words, not words in the converted Japanese text.
    // Line boundaries are different: never join two lines to manufacture a
    // dictionary word or an authored sentence. Preserve CRLF as two scalars too.
    let mut output = String::new();
    let mut start = 0;
    for (offset, ch) in seed.char_indices().filter(|(_, ch)| ch.is_whitespace()) {
        output.push_str(&romaji_to_hiragana(&seed[start..offset]));
        if is_line_boundary(ch) {
            output.push(ch);
        }
        start = offset + ch.len_utf8();
    }
    output.push_str(&romaji_to_hiragana(&seed[start..]));
    output
}

/// Complete the final reading after known dictionary words and particles. Never
/// scan through an arbitrary unknown prefix looking for an unrelated dictionary suffix.
fn word_completions(seed: &str) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    word_completions_with_limit(seed, 4, false)
}

/// Source two existing sentences only when one unambiguous whole-reading Word
/// corrects a greedy split into a different word. Extending an already correct
/// written prefix, competing root words and unfinished consonant probes retain
/// their existing pagination. The mixer must also verify both actual rows.
pub(crate) fn root_correction_continuations(seed: &str) -> Option<Vec<String>> {
    use crate::ime::candidate_mix::CandidateKind;
    if seed.chars().count() > 256 {
        return None;
    }
    let kana = composition_kana(seed);
    if kana.is_empty() || kana.chars().any(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }
    let (_, segmented_word) = conversion_prefix(&kana)?;
    let (word, kind) = word_completions(seed).into_iter().next()?;
    if kind != CandidateKind::Word || word.starts_with(segmented_word) {
        return None;
    }
    let mut roots = vocabulary().readings().iter().filter(|entry| {
        entry.kind == EntryKind::Word && entry.reading != kana && entry.reading.starts_with(&kana)
    });
    if roots.next()?.text != word || roots.any(|entry| entry.text != word) {
        return None;
    }
    let (_, values) = vocabulary()
        .continuations()
        .find(|(context, _)| *context == word)?;
    (values.len() >= 2
        && values
            .iter()
            .take(2)
            .all(|text| text.len() > word.len() && text.starts_with(&word)))
    .then(|| values[..2].to_vec())
}

/// Additional words may fill unused mixed-list slots, without replacing the
/// primary four completions or changing authored sentence weights.
pub(crate) fn word_completion_backfill(
    seed: &str,
) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    word_completions_with_limit(seed, crate::ime::candidate_mix::PAGE_SIZE * 2, true)
}

fn word_completions_with_limit(
    seed: &str,
    limit: usize,
    prefer_authored_probes: bool,
) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    use crate::ime::candidate_mix::CandidateKind;
    if seed.len() < 2 || seed.chars().count() > 256 {
        return Vec::new();
    }
    let kana = composition_kana(seed);
    // A completed Kana reading (including terminal n) keeps dictionary order.
    // Only the spare-slot path may prioritize an unfinished syllable's probes.
    let prefer_authored_probes =
        prefer_authored_probes && kana.ends_with(|ch: char| ch.is_ascii_alphabetic());
    let mut queries = vec![kana.clone()];
    // Trailing reading separators do not end a draft or its unfinished
    // syllable's completions. Probe before them, never across a line boundary.
    let reading = seed.trim_end_matches(|ch: char| ch.is_whitespace() && !is_line_boundary(ch));
    if reading.ends_with(|ch: char| {
        ch.is_ascii_alphabetic() && !"aiueo".contains(ch.to_ascii_lowercase())
    }) {
        queries.extend(
            ['a', 'i', 'u', 'e', 'o']
                .into_iter()
                .map(|vowel| composition_kana(&format!("{reading}{vowel}"))),
        );
    }
    let mut offset = 0;
    let mut prefix = String::new();
    let mut output = Vec::new();
    while offset < kana.len() {
        let rest = &kana[offset..];
        let authored_tail = is_authored_tail(&prefix, rest);
        let earlier_completions = output.len();
        // Resolve at most five probes once per depth, not once per dictionary
        // entry. Prefer the branch compatible with this whole authored prefix;
        // unrelated new words must not crowd out that branch's longer words.
        // Never promote deeper splits or change primary completions/weights.
        let preferred_tails: Vec<_> = if prefer_authored_probes && queries.len() > 1 {
            queries[1..]
                .iter()
                .filter_map(|query| query.strip_prefix(&kana[..offset]))
                .filter(|tail| !tail.is_empty() && is_authored_tail(&prefix, tail))
                .collect()
        } else {
            Vec::new()
        };
        let passes: &[bool] = if preferred_tails.is_empty() {
            &[false]
        } else {
            &[true, false]
        };
        for (reading, word) in passes.iter().flat_map(|preferred| {
            kanji().filter(|(reading, _)| {
                preferred_tails.iter().any(|tail| reading.starts_with(tail)) == *preferred
            })
        }) {
            if (reading != rest || offset > 0)
                && queries.iter().any(|query| {
                    query.strip_prefix(&kana[..offset]).is_some_and(|tail| {
                        !tail.is_empty()
                                && reading.starts_with(tail)
                                // Protect exact short-word substitutions for
                                // vowel probes as well as completed Kana tails.
                                && !(reading == tail
                                    && word != tail
                                    && is_authored_tail(&prefix, tail))
                    })
                })
            {
                let text = format!("{prefix}{word}");
                // A deeper split must not extend an already completable word
                // by reinterpreting its tail (準備中 -> 準備中央). Keep other
                // ambiguities and all homophones at the same reading depth.
                if output[..earlier_completions]
                    .iter()
                    .any(|(existing, kind)| {
                        *kind == CandidateKind::Word
                            && text != *existing
                            && text.starts_with(existing)
                    })
                {
                    continue;
                }
                if !output.iter().any(|(value, _)| value == &text) {
                    output.push((
                        text,
                        if is_dictionary_word(word) {
                            CandidateKind::Word
                        } else {
                            CandidateKind::Sentence
                        },
                    ));
                    if output.len() == limit {
                        return output;
                    }
                }
            }
        }
        if let Some((length, word)) = conversion_prefix(rest) {
            // Stop only an actual dictionary split of the authored tail.
            // Particles and genuine partial-word completions must still work
            // (e.g. 準備ができ -> 準備ができれば alongside the authored sentence).
            if authored_tail {
                break;
            }
            prefix.push_str(word);
            offset += length;
        } else {
            let ch = rest.chars().next().unwrap();
            if !matches!(
                ch,
                'は' | 'を' | 'が' | 'に' | 'で' | 'と' | 'も' | 'へ' | 'の' | '\u{3400}'
                    ..='\u{9fff}'
            ) {
                break;
            }
            prefix.push(ch);
            offset += ch.len_utf8();
        }
    }
    output
}

/// Keep a recognized written word whole before converting the next reading.
/// A longer phonetic match still wins, and ties preserve the dictionary's
/// existing first variant (including pack homophones and pure Kana words).
fn conversion_prefix(rest: &str) -> Option<(usize, &str)> {
    let phonetic = kanji()
        .rev()
        .filter(|(reading, _)| rest.starts_with(reading))
        .max_by_key(|(reading, _)| reading.len());
    if let Some(length) = known_written_prefix_len(rest)
        && phonetic.is_none_or(|(reading, _)| length > reading.len())
    {
        return Some((length, &rest[..length]));
    }
    phonetic.map(|(reading, word)| (reading.len(), word))
}

fn known_written_prefix_len(text: &str) -> Option<usize> {
    vocabulary()
        .readings()
        .iter()
        .filter(|entry| entry.kind == EntryKind::Word)
        .flat_map(|entry| {
            [
                text.starts_with(&entry.text).then_some(entry.text.len()),
                text.get(..entry.reading.len())
                    .filter(|prefix| prefix.chars().eq(entry.reading.chars().map(katakana_char)))
                    .map(str::len),
            ]
        })
        .flatten()
        .max()
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    #[test]
    fn authored_tails_keep_their_spelling_before_closing_punctuation() {
        for (prefix, tail) in [
            ("完了", "した"),
            ("日本語を勉強", "したい"),
            ("カンリョウ", "したら"),
        ] {
            for punctuation in [
                "!", "?", "。", "！？", "、", "，", "…", "。」", "”", ")", "]", "\"", "'",
            ] {
                let rest = format!("{tail}{punctuation}");
                assert!(is_authored_tail(prefix, &rest), "{prefix}{rest}");
                let draft = format!("{prefix}{rest}");
                assert_eq!(convert_segments(&draft), draft);
            }
        }
    }

    #[test]
    fn closing_punctuation_never_hides_unknown_or_control_text() {
        for (prefix, rest) in [
            ("", "した!"),
            ("xyz完了", "した!"),
            ("完了", "した!xyz"),
            ("完了", "した/"),
            ("完了", "した="),
            ("完了", "した_"),
            ("完了", "した()"),
            ("完了", "した\n!"),
            ("完了\n", "した!"),
            ("完了", "!"),
            ("完了", "しt!"),
            ("完了", "したい!"),
        ] {
            assert!(!is_authored_tail(prefix, rest), "{prefix:?}{rest:?}");
        }
        assert_eq!(convert_segments("した!"), "下!");
        assert_eq!(convert_segments("下書き。"), "下書き。");
    }

    #[test]
    fn broad_readings_keep_a_bounded_later_page_in_dictionary_order() {
        let choices = word_completion_backfill("し");
        assert_eq!(
            choices
                .iter()
                .map(|(text, _)| text.as_str())
                .collect::<Vec<_>>(),
            [
                "仕事",
                "失礼します",
                "週末",
                "締め切り",
                "進捗",
                "写真",
                "趣味",
                "植物",
                "質問",
                "詳細",
                "失礼しました",
                "承知しました",
            ]
        );
        assert!(
            choices
                .iter()
                .all(|(_, kind)| { *kind == crate::ime::candidate_mix::CandidateKind::Word })
        );
        assert_eq!(choices.len(), crate::ime::candidate_mix::PAGE_SIZE * 2);
    }

    #[test]
    fn authored_sentences_follow_the_whole_draft_not_only_exact_triggers() {
        for (seed, expected) in [
            ("予定ga", "予定が決まったら連絡します。"),
            ("nihongo wo", "日本語を勉強しています。"),
            ("JYUNBI GADEKI", "準備ができたら連絡します。"),
            ("日本語を勉強して", "日本語を勉強しています。"),
        ] {
            let choices = mixed_candidates(seed);
            assert!(
                choices.iter().any(|(text, _)| text == expected),
                "{seed}: {choices:?}"
            );
        }
        let choices = mixed_candidates("日本語を勉強");
        let study: Vec<_> = choices
            .iter()
            .filter(|(_, kind)| *kind == crate::ime::candidate_mix::CandidateKind::Sentence)
            .map(|(text, _)| text.as_str())
            .collect();
        assert_eq!(
            study,
            ["日本語を勉強しています。", "日本語を勉強したいです。"]
        );
    }

    #[test]
    fn conversion_keeps_line_boundaries_but_joins_reading_separators() {
        assert_eq!(
            composition_kana("nihongo wo benkyou"),
            "にほんごをべんきょう"
        );
        for boundary in [
            "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            let seed = format!("yotei{boundary}ga");
            assert_eq!(composition_kana(&seed), format!("よてい{boundary}が"));
            assert_eq!(
                convert_segments(&composition_kana(&seed)),
                format!("予定{boundary}が")
            );
        }
    }

    #[test]
    fn complete_tail_readings_keep_homophones_after_known_prefixes() {
        for seed in ["私はkanji", "watashihakanji", "私はかんじ"] {
            let choices = word_completions(seed);
            for word in ["私は漢字", "私は感じ"] {
                assert!(
                    choices.iter().any(|(text, kind)| text == word
                        && *kind == crate::ime::candidate_mix::CandidateKind::Word),
                    "{seed}: {choices:?}"
                );
            }
            assert!(choices.len() <= 4);
        }
    }
    #[test]
    fn known_katakana_prefixes_keep_their_spelling_during_completion() {
        for seed in ["ニホンゴwobenky", "ニホンゴ wo benky", "ニホンゴをべんき"] {
            let choices = word_completions(seed);
            assert!(
                choices.iter().any(|(text, _)| text == "ニホンゴを勉強"),
                "{seed}: {choices:?}"
            );
        }
        for seed in [
            "ワカラナイniho",
            "テストniho",
            "ニホンゴxyzniho",
            "ニホンゴ🙂niho",
            "ニホンゴ123niho",
        ] {
            assert!(word_completions(seed).is_empty(), "{seed}");
        }
        assert!(word_completions(&format!("{}benky", "ニ".repeat(252))).is_empty());
    }
    #[test]
    fn incomplete_consonants_and_terminal_n_can_finish_the_last_known_word() {
        for (seed, word) in [
            ("niho", "日本語"),
            ("nihong", "日本語"),
            ("nihon", "日本語"),
            ("watashihanihong", "私は日本語"),
            ("私はniho", "私は日本語"),
            ("nihongo wo benky", "日本語を勉強"),
        ] {
            let choices = word_completions(seed);
            assert!(
                choices.iter().any(|(text, _)| text == word),
                "{seed}: {choices:?}"
            );
            assert!(choices.len() <= 4);
        }
    }
    #[test]
    fn unknown_prefixes_are_not_scanned_for_unrelated_suffix_completions() {
        for seed in ["xyzniho", "https://niho", "foobarniho", "🙂niho"] {
            assert!(word_completions(seed).is_empty(), "{seed}");
        }
        assert!(word_completions(&"あ".repeat(300)).is_empty());
    }
}

// Probe only the final unconverted syllable of the existing conversion. These
// are sentence-prefix queries, never replacement drafts or inferred Kanji.
// Reusing the converted prefix also avoids segmenting the whole draft five times.
fn authored_tail_probes(seed: &str, converted: &str) -> Vec<String> {
    let reading = seed.trim_end_matches(|ch: char| ch.is_whitespace() && !is_line_boundary(ch));
    let raw_prefix = reading.trim_end_matches(|ch: char| ch.is_ascii_alphabetic());
    let converted_prefix = converted.trim_end_matches(|ch: char| ch.is_ascii_alphabetic());
    // Removed reading separators must not join unfinished syllables: s + Space
    // + h leaves an earlier s intact, even though converted ends with "sh".
    // Both lengths describe ASCII suffixes, so this split stays on UTF-8 boundaries.
    let pending_len =
        (reading.len() - raw_prefix.len()).min(converted.len() - converted_prefix.len());
    let (prefix, pending) = converted.split_at(converted.len() - pending_len);
    let (prefix, pending) = if pending.is_empty()
        && reading
            .as_bytes()
            .last()
            .is_some_and(|ch| ch.eq_ignore_ascii_case(&b'n'))
        && !matches!(
            reading
                .as_bytes()
                .iter()
                .rev()
                .nth(1)
                .map(u8::to_ascii_lowercase),
            Some(b'n' | b'x')
        ) {
        // Only a raw single n can still become na/ni/etc. Keep the primary ん
        // unchanged; explicit nn/xn/n'/Kana and dictionary-converted endings
        // cannot be reversed to manufacture a different sentence prefix.
        let Some(prefix) = converted.strip_suffix('ん') else {
            return Vec::new();
        };
        // A complete written dictionary suffix (e.g. すみません) can itself
        // end in ん, including after an adopted prefix. Do not split it again.
        if kanji().any(|(_, word)| converted.ends_with(word)) {
            return Vec::new();
        }
        (prefix, "n")
    } else {
        (prefix, pending)
    };
    // Supported syllables need at most three consonants before their last vowel
    // (e.g. lts + u). Earlier unknown text remains part of every prefix query.
    if prefix.is_empty() || pending.is_empty() || pending.len() > 3 {
        return Vec::new();
    }
    ['a', 'i', 'u', 'e', 'o']
        .into_iter()
        .map(|vowel| romaji_to_hiragana(&format!("{pending}{vowel}")))
        .filter(|tail| !tail.chars().any(|ch| ch.is_ascii_alphabetic()))
        .map(|tail| format!("{prefix}{tail}"))
        .collect()
}

pub(crate) fn mixed_candidates(
    seed: &str,
) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    use crate::ime::candidate_mix::CandidateKind;

    let kana = composition_kana(seed);
    let converted = convert_segments(&kana);
    let mut output = Vec::new();
    // Segmented conversion should be available before the unchanged kana forms.
    if converted != kana {
        output.push((
            converted.clone(),
            if is_dictionary_word(&converted) {
                CandidateKind::Word
            } else {
                CandidateKind::Sentence
            },
        ));
    }
    output.extend(word_completions(seed));
    let probes = if seed.chars().count() <= 256 {
        authored_tail_probes(seed, &converted)
    } else {
        Vec::new()
    };
    let mut sentence_words: Vec<_> = output
        .iter()
        .filter(|(_, kind)| *kind == CandidateKind::Word)
        .map(|(text, _)| text.clone())
        .collect();
    if seed.chars().count() <= 256 && kana.ends_with(|ch: char| ch.is_ascii_alphabetic()) {
        // A final vowel can finish a whole dictionary word before it finishes a
        // longer completion (souj -> 掃除, rather than 掃除機). Prioritize only
        // its sentence query, never the existing Word rows or conversion.
        // Compare complete readings rather than converting probes to Kanji.
        // Lift only past adjacent longer words in the same reading branch;
        // bas -> バス must not overtake the unrelated 場所 query. Equal-reading
        // homophones keep their order. Unknown prefixes, line boundaries and
        // separated unfinished syllables cannot disappear.
        let complete_readings = authored_tail_probes(seed, &kana);
        if !complete_readings.is_empty() {
            let mut word_readings = vec![Vec::new(); sentence_words.len()];
            for entry in vocabulary()
                .readings()
                .iter()
                .filter(|entry| entry.kind == EntryKind::Word)
            {
                if let Some(index) = sentence_words.iter().position(|word| word == &entry.text) {
                    word_readings[index].push(entry.reading.as_str());
                }
            }
            for index in 1..sentence_words.len() {
                let mut position = index;
                while position > 0
                    // Packs can give one written word multiple readings. An
                    // already complete query stays ahead even if another one
                    // of its aliases is a longer extension of this reading.
                    && !word_readings[position - 1].iter().any(|previous| {
                        complete_readings.iter().any(|probe| probe.as_str() == *previous)
                    })
                    && word_readings[position].iter().any(|reading| {
                        complete_readings
                            .iter()
                            .any(|probe| probe.as_str() == *reading)
                            && word_readings[position - 1].iter().any(|previous| {
                                previous.len() > reading.len() && previous.starts_with(*reading)
                            })
                    })
                {
                    sentence_words.swap(position - 1, position);
                    word_readings.swap(position - 1, position);
                    position -= 1;
                }
            }
        }
    }
    let words: Vec<_> = std::iter::once(converted)
        .chain(sentence_words)
        // Primary candidates above remain unchanged; only sentence queries can
        // move. Append pending-syllable queries within the same sentence cap.
        .chain(probes)
        .collect();
    for word in words {
        let mut contexts: Vec<_> = vocabulary()
            .continuations()
            .filter(|(prefix, values)| {
                *prefix == word
                    || (word.starts_with(prefix)
                        && values
                            .iter()
                            .any(|text| text.len() > word.len() && text.starts_with(&word)))
            })
            .collect();
        // Exact triggers retain their priority. Otherwise prefer the longest
        // applicable context; ties preserve the lexicon's authored layer order.
        contexts.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
        for (prefix, values) in contexts {
            for text in values {
                // Match the *whole* progress, never discard an unknown prefix,
                // conflicting tail or a line break to recover a trigger. Exact
                // trigger behavior remains compatible with existing packs.
                if prefix != word && (text.len() <= word.len() || !text.starts_with(&word)) {
                    continue;
                }
                if !output.iter().any(|(existing, _)| existing == text) {
                    if output
                        .iter()
                        .filter(|(_, kind)| *kind == CandidateKind::Sentence)
                        .count()
                        >= 4
                    {
                        return output;
                    }
                    output.push((text.clone(), CandidateKind::Sentence));
                }
            }
        }
        if output
            .iter()
            .filter(|(_, kind)| *kind == CandidateKind::Sentence)
            .count()
            >= 4
        {
            break;
        }
    }
    output
}

// Standard five-vowel rows plus common alternative spellings. The conversion algorithm,
// including terminal n and doubled consonants, is separate from language/LLM dispatch.
const ROWS: &[(&str, &str)] = &[
    ("", "あいうえお"),
    ("k", "かきくけこ"),
    ("g", "がぎぐげご"),
    ("s", "さしすせそ"),
    ("z", "ざじずぜぞ"),
    ("t", "たちつてと"),
    ("d", "だぢづでど"),
    ("n", "なにぬねの"),
    ("h", "はひふへほ"),
    ("b", "ばびぶべぼ"),
    ("p", "ぱぴぷぺぽ"),
    ("m", "まみむめも"),
    ("r", "らりるれろ"),
    ("x", "ぁぃぅぇぉ"),
    ("l", "ぁぃぅぇぉ"),
];
const SPECIAL: &[(&str, &str)] = &[
    ("ltsu", "っ"),
    ("xtsu", "っ"),
    ("ltu", "っ"),
    ("xtu", "っ"),
    ("shi", "し"),
    ("chi", "ち"),
    ("tsu", "つ"),
    ("fu", "ふ"),
    ("ji", "じ"),
    ("sha", "しゃ"),
    ("shu", "しゅ"),
    ("sho", "しょ"),
    ("she", "しぇ"),
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("cho", "ちょ"),
    ("che", "ちぇ"),
    ("thi", "てぃ"),
    ("ja", "じゃ"),
    ("ju", "じゅ"),
    ("jo", "じょ"),
    ("je", "じぇ"),
    ("jya", "じゃ"),
    ("jyu", "じゅ"),
    ("jyo", "じょ"),
    ("fa", "ふぁ"),
    ("fi", "ふぃ"),
    ("fe", "ふぇ"),
    ("fo", "ふぉ"),
    ("va", "ゔぁ"),
    ("vi", "ゔぃ"),
    ("vu", "ゔ"),
    ("ve", "ゔぇ"),
    ("vo", "ゔぉ"),
    ("ya", "や"),
    ("yu", "ゆ"),
    ("yo", "よ"),
    ("wa", "わ"),
    ("wo", "を"),
    ("wi", "うぃ"),
    ("we", "うぇ"),
    ("ye", "いぇ"),
    ("xn", "ん"),
    ("xya", "ゃ"),
    ("xyu", "ゅ"),
    ("xyo", "ょ"),
    ("lya", "ゃ"),
    ("lyu", "ゅ"),
    ("lyo", "ょ"),
];

pub fn romaji_to_hiragana(input: &str) -> String {
    let input = input.to_ascii_lowercase();
    let mut rest = input.as_str();
    let mut output = String::new();
    while !rest.is_empty() {
        let bytes = rest.as_bytes();
        if let Some(after) = rest.strip_prefix("n'").or_else(|| rest.strip_prefix("n’")) {
            output.push('ん');
            rest = after;
            continue;
        }
        if rest.starts_with('n')
            && (bytes.len() == 1 || (bytes.get(1).is_some_and(|ch| !b"aiueoy".contains(ch))))
        {
            output.push('ん');
            // A completed nn stays one ん when a following consonant starts a
            // new syllable, just as at a text boundary. Reserve the second n
            // for live na/ni/nya/etc. syllables, repeated n, and explicit n'.
            let consume = if rest.strip_prefix("nn").is_some_and(|tail| {
                tail.chars().next().is_none_or(|ch| {
                    !matches!(ch, 'a' | 'i' | 'u' | 'e' | 'o' | 'n' | 'y' | '\'' | '’')
                })
            }) {
                2
            } else {
                1
            };
            rest = &rest[consume..];
            continue;
        }
        if bytes.len() >= 2
            && bytes[0].is_ascii_lowercase()
            && !b"aiueon".contains(&bytes[0])
            && (bytes[0] == bytes[1] || rest.starts_with("tch"))
        {
            output.push('っ');
            rest = &rest[1..];
            continue;
        }
        if let Some((roman, kana)) = SPECIAL
            .iter()
            .filter(|(roman, _)| rest.starts_with(roman))
            .max_by_key(|(roman, _)| roman.len())
        {
            output.push_str(kana);
            rest = &rest[roman.len()..];
            continue;
        }
        if bytes.len() >= 3
            && bytes[1] == b'y'
            && b"auo".contains(&bytes[2])
            && let Some((_, row)) = ROWS
                .iter()
                .find(|(prefix, _)| prefix.as_bytes() == &bytes[..1])
        {
            output.push(row.chars().nth(1).unwrap());
            output.push(match bytes[2] {
                b'a' => 'ゃ',
                b'u' => 'ゅ',
                _ => 'ょ',
            });
            rest = &rest[3..];
            continue;
        }
        let mut matched = false;
        for (prefix, kana) in ROWS {
            if let Some(after) = rest.strip_prefix(prefix)
                && let Some(index) = after
                    .as_bytes()
                    .first()
                    .and_then(|vowel| b"aiueo".iter().position(|ch| ch == vowel))
            {
                output.push(kana.chars().nth(index).unwrap());
                rest = &rest[prefix.len() + 1..];
                matched = true;
                break;
            }
        }
        if !matched {
            let ch = rest.chars().next().unwrap();
            output.push(match ch {
                '-' => 'ー',
                _ => ch,
            });
            rest = &rest[ch.len_utf8()..];
        }
    }
    output
}

fn katakana_char(ch: char) -> char {
    if ('ぁ'..='ゖ').contains(&ch) {
        char::from_u32(ch as u32 + 0x60).unwrap_or(ch)
    } else {
        ch
    }
}

fn hiragana_to_katakana(input: &str) -> String {
    input.chars().map(katakana_char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sentence_probes_only_complete_the_final_pending_syllable() {
        for (converted, expected) in [
            (
                "引用s",
                vec!["引用さ", "引用し", "引用す", "引用せ", "引用そ"],
            ),
            (
                "引用sh",
                vec!["引用しゃ", "引用し", "引用しゅ", "引用しぇ", "引用しょ"],
            ),
            (
                "引用しt",
                vec!["引用した", "引用しち", "引用しつ", "引用して", "引用しと"],
            ),
            ("アメニth", vec!["アメニてぃ"]),
            ("引用lts", vec!["引用っ"]),
        ] {
            assert_eq!(
                authored_tail_probes(converted, converted),
                expected,
                "{converted}"
            );
        }
    }
    #[test]
    fn sentence_probes_leave_complete_readings_and_unknown_tails_alone() {
        for converted in ["", "sh", "引用", "引用ん", "引用shxyz", "引用sh\n"] {
            assert!(
                authored_tail_probes(converted, converted).is_empty(),
                "{converted:?}"
            );
        }
        // Prefix matching will reject these queries; probing must not erase a
        // line break, an unknown written prefix, or a user's Katakana spelling.
        for prefix in ["引用\n", "🙂引用", "カンリョウ"] {
            let converted = format!("{prefix}sh");
            let probes = authored_tail_probes(&converted, &converted);
            assert_eq!(probes.len(), 5);
            assert!(probes.iter().all(|query| query.starts_with(prefix)));
        }
    }
    #[test]
    fn sentence_probes_never_join_separately_typed_pending_syllables() {
        for separator in [" ", "\t", "\u{a0}", "\u{2003}", "\u{202f}", "\u{3000}"] {
            let seed = format!("引用s{separator}h{separator}");
            assert_eq!(
                authored_tail_probes(&seed, "引用sh"),
                ["引用sは", "引用sひ", "引用sふ", "引用sへ", "引用sほ"],
                "{seed:?}"
            );
            let seed = format!("引用shi{separator}t{separator}");
            assert_eq!(
                authored_tail_probes(&seed, "引用しt"),
                ["引用した", "引用しち", "引用しつ", "引用して", "引用しと"],
                "{seed:?}"
            );
        }
    }
    #[test]
    fn sentence_probes_distinguish_pending_n_from_confirmed_nasal_text() {
        for seed in ["引用n", "引用N", "引用 n\t", "in'youn", "IN’YOUN\u{3000}"] {
            assert_eq!(
                authored_tail_probes(seed, "引用ん"),
                ["引用な", "引用に", "引用ぬ", "引用ね", "引用の"],
                "{seed:?}"
            );
        }
        for seed in [
            "引用nn",
            "引用NN",
            "引用xn",
            "引用XN",
            "引用n'",
            "引用n’",
            "引用ん",
            "引用ン",
        ] {
            assert!(authored_tail_probes(seed, "引用ん").is_empty(), "{seed:?}");
        }
        // A dictionary conversion or a preserved line boundary is not a pending n.
        for (seed, converted) in [("hon", "本"), ("引用n\n", "引用ん\n"), ("n", "ん")] {
            assert!(authored_tail_probes(seed, converted).is_empty(), "{seed:?}");
        }
        // These complete dictionary forms end in Hiragana ん, not Kanji.
        // A written suffix can be recognized after another adopted word too.
        for (seed, converted) in [
            ("sumimasen", "すみません"),
            ("wakarimasen", "分かりません"),
            ("moushiwakearimasen", "申し訳ありません"),
            ("引用sumimasen", "引用すみません"),
        ] {
            assert!(authored_tail_probes(seed, converted).is_empty(), "{seed:?}");
        }
    }
    #[test]
    fn terminal_n_boundaries_aliases_and_non_roman_text_are_preserved() {
        for (input, expected) in [
            ("nn。", "ん。"),
            ("konn!", "こん!"),
            ("shuudennha", "しゅうでんは"),
            ("shuudenn", "しゅうでん"),
            ("shuudennh", "しゅうでんh"),
            ("dennwawo", "でんわを"),
            ("konnbanha", "こんばんは"),
            ("konnkaino", "こんかいの"),
            ("nn🙂", "ん🙂"),
            ("nn\u{3000}", "ん\u{3000}"),
            ("shin’you", "しんよう"),
            ("nn'ya", "んんや"),
            ("nn’ya", "んんや"),
            ("nna", "んな"),
            ("nni", "んに"),
            ("nnya", "んにゃ"),
            ("nnna", "んんな"),
            ("shinnyuu", "しんにゅう"),
            ("nnn", "んん"),
            ("nnk", "んk"),
            ("konnichiha", "こんにちは"),
            ("nya", "にゃ"),
            ("jya", "じゃ"),
            ("jyu", "じゅ"),
            ("jyo", "じょ"),
            ("jyuu", "じゅう"),
            ("jyunbi", "じゅんび"),
            ("Ωテスト", "Ωテスト"),
            ("Σ123", "Σ123"),
            ("İテスト", "İテスト"),
            ("NIHONGO", "にほんご"),
        ] {
            assert_eq!(romaji_to_hiragana(input), expected, "{input}");
        }
    }
    #[test]
    fn handles_romanization_small_kana_geminates_and_syllabic_n() {
        for (input, expected) in [
            ("konnichiha", "こんにちは"),
            ("gakkou", "がっこう"),
            ("shin'you", "しんよう"),
            ("nya", "にゃ"),
            ("kan", "かん"),
            ("nn", "ん"),
            ("kya", "きゃ"),
            ("tcha", "っちゃ"),
            ("nihongo", "にほんご"),
            ("ky", "ky"),
            ("テスト123", "テスト123"),
        ] {
            assert_eq!(romaji_to_hiragana(input), expected, "{input}");
        }
    }
    #[test]
    fn offers_kanji_kana_katakana_and_original_input() {
        let candidates = JapaneseLanguagePlugin.build_candidates(&[], "nihongo", 1.0);
        let texts: Vec<_> = candidates.iter().map(|item| item.text.as_str()).collect();
        assert_eq!(texts, ["日本語", "にほんご", "ニホンゴ", "nihongo"]);
    }
}
