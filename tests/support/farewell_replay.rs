//! One measured vocabulary-only delta for the historical English replay.
//! The new authored `i should` branch fills one spare second-page Word slot.
//! Verify every current field before removing only that row; no runtime data,
//! score, old row, corpus or historical golden is changed.
use suzaku_map::ime::Candidate;

pub(super) fn before_farewell(
    language: &str,
    seed: &str,
    current: &[Candidate],
    with_packs: bool,
) -> Vec<Candidate> {
    if language != "en" || seed != "i s" || with_packs {
        return current.to_vec();
    }
    let expected = [
        ("i s", "i s", "literal", "local", 1120403456),
        ("i should've", "i should've", "word", "local", 1119354880),
        ("i see", "i see", "word", "local", 1119092736),
        ("i still", "i still", "word", "local", 1118830592),
        (
            "i should've called earlier.",
            "i should've called earlier.",
            "sentence",
            "local",
            1117650944,
        ),
        (
            "i should've checked the time.",
            "i should've checked the time.",
            "sentence",
            "local",
            1117585408,
        ),
        ("i should", "i should", "word", "local", 1118568448),
        (
            "i see what you mean.",
            "i see what you mean.",
            "sentence",
            "local",
            1117519872,
        ),
        (
            "i see what you need now.",
            "i see what you need now.",
            "sentence",
            "local",
            1117454336,
        ),
        (
            "i still have the receipt if you need to see it.",
            "i still have the receipt if you need to see it.",
            "sentence",
            "local",
            1117388800,
        ),
        (
            "i still have the box and the original packaging.",
            "i still have the box and the original packaging.",
            "sentence",
            "local",
            1117323264,
        ),
    ];
    assert_eq!(
        current
            .iter()
            .map(|candidate| (
                candidate.text.as_str(),
                candidate.label.as_str(),
                candidate.kind.id(),
                candidate.source.id(),
                candidate.score.to_bits(),
            ))
            .collect::<Vec<_>>(),
        expected,
        "the only accepted farewell delta must match all eleven current rows and fields"
    );
    let mut before = current.to_vec();
    before.remove(6);
    before
}
