//! Exact entryway vocabulary inverse for one historical, builtin Japanese replay.
//! Real engine output is checked before projection. This does not change runtime
//! candidates, hide dictionary data, replace an old golden or generalize a filter.
use suzaku_map::{
    ime::Candidate,
    lexicon::{EntryKind, builtin},
};

type Row<'a> = (&'a str, &'a str, &'a str, &'a str, u32);
const CURRENT: &[Row<'_>] = &[
    ("靴", "靴", "word", "local", 1120403456),
    ("くつ", "くつ", "word", "local", 1119354880),
    ("靴下", "靴下", "word", "local", 1119158272),
    ("クツ", "クツ", "word", "local", 1119092736),
    (
        "靴を履いたら、すぐに出ます。",
        "靴を履いたら、すぐに出ます。",
        "sentence",
        "local",
        1117323264,
    ),
    (
        "靴は歩きやすいものを選びます。",
        "靴は歩きやすいものを選びます。",
        "sentence",
        "local",
        1117257728,
    ),
    ("靴ひも", "靴ひも", "word", "local", 1119092736),
    ("靴箱", "靴箱", "word", "local", 1119027200),
    ("靴べら", "靴べら", "word", "local", 1118961664),
    (
        "靴ひもを結び直してから歩きます。",
        "靴ひもを結び直してから歩きます。",
        "sentence",
        "local",
        1117192192,
    ),
    (
        "靴ひもがほどけたので、少し待ってください。",
        "靴ひもがほどけたので、少し待ってください。",
        "sentence",
        "local",
        1117126656,
    ),
    ("kutsu", "kutsu", "literal", "local", 1113325568),
];
const BEFORE: &[Row<'_>] = &[
    ("靴", "靴", "word", "local", 1120403456),
    ("くつ", "くつ", "word", "local", 1119354880),
    ("靴下", "靴下", "word", "local", 1119158272),
    ("クツ", "クツ", "word", "local", 1119092736),
    (
        "靴を履いたら、すぐに出ます。",
        "靴を履いたら、すぐに出ます。",
        "sentence",
        "local",
        1117454336,
    ),
    (
        "靴は歩きやすいものを選びます。",
        "靴は歩きやすいものを選びます。",
        "sentence",
        "local",
        1117388800,
    ),
    ("靴ひも", "靴ひも", "word", "local", 1119092736),
    (
        "靴ひもを結び直してから歩きます。",
        "靴ひもを結び直してから歩きます。",
        "sentence",
        "local",
        1117323264,
    ),
    (
        "靴ひもがほどけたので、少し待ってください。",
        "靴ひもがほどけたので、少し待ってください。",
        "sentence",
        "local",
        1117257728,
    ),
    ("kutsu", "kutsu", "literal", "local", 1113325568),
];

fn rows(current: &[Candidate]) -> Vec<Row<'_>> {
    current
        .iter()
        .map(|c| {
            (
                c.text.as_str(),
                c.label.as_str(),
                c.kind.id(),
                c.source.id(),
                c.score.to_bits(),
            )
        })
        .collect()
}

fn hash(current: &[Candidate]) -> u64 {
    let mut h = 0xcbf29ce484222325_u64;
    for c in current {
        for byte in c
            .text
            .bytes()
            .chain([0])
            .chain(c.label.bytes())
            .chain([0])
            .chain(c.kind.id().bytes())
            .chain([0])
            .chain(c.source.id().bytes())
            .chain([0])
            .chain(c.score.to_bits().to_le_bytes())
        {
            h = (h ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    h
}

pub(super) fn before_daily_entryway(
    language: &str,
    seed: &str,
    current: &[Candidate],
    with_packs: bool,
) -> Vec<Candidate> {
    if language != "ja" || seed != "kutsu" || with_packs {
        return current.to_vec();
    }
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 638);
    // Only the exact new entryway append can explain these two extra Words.
    for (position, reading, word) in [(633, "くつばこ", "靴箱"), (635, "くつべら", "靴べら")]
    {
        let entry = &ja.readings()[position];
        assert_eq!(
            (
                &*entry.reading,
                &*entry.text,
                entry.kind,
                entry.require_separators
            ),
            (reading, word, EntryKind::Word, false)
        );
        assert!(
            ja.readings()[..633]
                .iter()
                .all(|old| old.reading != reading && old.text != word)
        );
        assert_eq!(
            ja.readings()
                .iter()
                .filter(|entry| entry.reading == reading && entry.text == word)
                .count(),
            1
        );
    }
    assert_eq!(
        rows(current),
        CURRENT,
        "only this complete measured entryway delta is accepted"
    );
    assert_eq!(hash(current), 0xcff56be25219eac2);

    // Source insertion added exactly two Word positions before the four old
    // authored rows. Undo their observed 2 * 0.5 index-weight shift only here.
    let mut projected: Vec<_> = current
        .iter()
        .enumerate()
        .filter(|(position, _)| ![7, 8].contains(position))
        .map(|(_, candidate)| candidate.clone())
        .collect();
    for candidate in &mut projected {
        candidate.score = match candidate.text.as_str() {
            "靴を履いたら、すぐに出ます。" => {
                assert_eq!(candidate.score.to_bits(), 1117323264);
                f32::from_bits(1117454336)
            }
            "靴は歩きやすいものを選びます。" => {
                assert_eq!(candidate.score.to_bits(), 1117257728);
                f32::from_bits(1117388800)
            }
            "靴ひもを結び直してから歩きます。" => {
                assert_eq!(candidate.score.to_bits(), 1117192192);
                f32::from_bits(1117323264)
            }
            "靴ひもがほどけたので、少し待ってください。" => {
                assert_eq!(candidate.score.to_bits(), 1117126656);
                f32::from_bits(1117257728)
            }
            _ => candidate.score,
        };
    }
    // Actual before-adjacent.jsonl row 33, not a guessed hash-only oracle.
    assert_eq!(
        rows(&projected),
        BEFORE,
        "the exact old ten tuples must be restored"
    );
    assert_eq!(hash(&projected), 0xb846eafefcb16043);
    projected
}
