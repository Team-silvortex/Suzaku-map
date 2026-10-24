//! Exact vocabulary-only delta for the historical optimization replay.
//! A newline still fences context; the final known token `line` can receive
//! the two new dictionary completions. Assert the entire measured vector before
//! removing only those two tail Words. No runtime result or golden is changed.
use suzaku_map::ime::Candidate;

pub(super) fn before_daily_recycling(
    language: &str,
    seed: &str,
    current: &[Candidate],
    with_packs: bool,
) -> Vec<Candidate> {
    if language != "en" || seed != "one\nline" || with_packs {
        return current.to_vec();
    }
    let expected = [
        ("one\nline", "word", 1120403456),
        ("one\nlinear", "word", 1119354880),
        ("one\nlinen", "word", 1119092736),
        ("one\nliner", "word", 1118830592),
        ("one\nliners", "word", 1118568448),
    ];
    assert_eq!(
        current
            .iter()
            .map(|c| (
                c.text.as_str(),
                c.label.as_str(),
                c.kind.id(),
                c.source.id(),
                c.score.to_bits(),
            ))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|&(text, kind, bits)| (text, text, kind, "local", bits))
            .collect::<Vec<_>>(),
        "the only accepted recycling delta must match every current field"
    );
    current[..3].to_vec()
}
