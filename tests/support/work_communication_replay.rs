//! The measured workplace-vocabulary delta in the historical Japanese replay.
//! Both actual modes changed only `henk`: two new authored sentences fill slots
//! and displace two backfill Words. Check all current fields before restoring
//! the complete captured historical vector, including scores and evicted tails.
//! This test-only inverse does not claim those tails remain in the short pool.
use suzaku_map::ime::{
    Candidate,
    candidate_mix::{CandidateKind, CandidateSource},
};

pub(super) fn before_work_communication(
    language: &str,
    seed: &str,
    current: &[Candidate],
    _with_packs: bool,
) -> Vec<Candidate> {
    if language != "ja" || seed != "henk" {
        return current.to_vec();
    }
    // Actual builtin and fresh-nine-pack A/B executions had this same vector.
    let expected = [
        ("へんk", "へんk", "unknown", "local", 1120403456),
        ("ヘンk", "ヘンk", "unknown", "local", 1119354880),
        ("変換", "変換", "word", "local", 1119223808),
        ("変更", "変更", "word", "local", 1119158272),
        (
            "変更履歴を見ながら、直した箇所を一緒に確認しましょう。",
            "変更履歴を見ながら、直した箇所を一緒に確認しましょう。",
            "sentence",
            "local",
            1117388800,
        ),
        (
            "変更履歴に理由も残しておくので、引き継ぎのときに確認してください。",
            "変更履歴に理由も残しておくので、引き継ぎのときに確認してください。",
            "sentence",
            "local",
            1117323264,
        ),
        ("変更履歴", "変更履歴", "word", "local", 1119092736),
        ("返却期限", "返却期限", "word", "local", 1119027200),
        (
            "返却期限を教えてもらえますか。",
            "返却期限を教えてもらえますか。",
            "sentence",
            "local",
            1117257728,
        ),
        (
            "返却期限までに持っていきます。",
            "返却期限までに持っていきます。",
            "sentence",
            "local",
            1117192192,
        ),
        ("henk", "henk", "literal", "local", 1113325568),
        ("返却場所", "返却場所", "word", "local", 1113194496),
    ];
    assert_eq!(
        current
            .iter()
            .map(|row| (
                row.text.as_str(),
                row.label.as_str(),
                row.kind.id(),
                row.source.id(),
                row.score.to_bits(),
            ))
            .collect::<Vec<_>>(),
        expected,
        "the accepted work_communication delta must match all twelve current rows and fields"
    );
    [
        ("へんk", "unknown", 1120403456),
        ("ヘンk", "unknown", 1119354880),
        ("変換", "word", 1119223808),
        ("変更", "word", 1119158272),
        ("返却期限を教えてもらえますか。", "sentence", 1117388800),
        ("返却期限までに持っていきます。", "sentence", 1117323264),
        ("変更履歴", "word", 1119092736),
        ("返却期限", "word", 1119027200),
        ("henk", "literal", 1113325568),
        ("返却場所", "word", 1113194496),
        ("返却", "word", 1113063424),
        ("返金", "word", 1112932352),
    ]
    .into_iter()
    .map(|(text, kind, bits)| Candidate {
        text: text.to_owned(),
        label: text.to_owned(),
        score: f32::from_bits(bits),
        kind: CandidateKind::parse(kind).unwrap(),
        source: CandidateSource::Local,
    })
    .collect()
}
