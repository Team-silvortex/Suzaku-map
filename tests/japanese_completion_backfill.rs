//! Later Japanese words fill unused rows without evicting legacy sentences.
use suzaku_map::{
    ime::LanguagePlugin,
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, offline},
    ime::{CommitOptions, EngineConfig, XRTabletImeEngine},
    languages::japanese::JapaneseLanguagePlugin,
};

const LEGACY_SHI: [&str; 10] = [
    "し",
    "シ",
    "仕事",
    "失礼します",
    "仕事が終わりました。",
    "仕事について相談したいです。",
    "週末",
    "締め切り",
    "週末は何をする予定ですか。",
    "週末に一緒に出かけませんか。",
];

#[test]
fn later_words_fill_second_page_without_changing_any_legacy_choice_or_weight() {
    let plugin = JapaneseLanguagePlugin;
    let base = plugin.direct_candidates("し", 1.0).unwrap();
    let old = offline("ja", "し", "", base.clone(), 10);
    assert_eq!(
        old.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
        LEGACY_SHI
    );
    let expanded = offline("ja", "し", "", base, 12);
    assert_eq!(expanded.len(), 12);
    assert_eq!(&expanded[..10], old);
    assert_eq!(expanded[10].text, "進捗");
    assert_eq!(expanded[11].text, "写真");
    assert!(expanded[10..].iter().all(|c| c.kind == CandidateKind::Word
        && c.source == CandidateSource::Local
        && c.score < old.iter().map(|c| c.score).fold(f32::INFINITY, f32::min)));
    for limit in 1..=10 {
        let candidates = offline(
            "ja",
            "し",
            "",
            plugin.direct_candidates("し", 1.0).unwrap(),
            limit,
        );
        assert_eq!(candidates.len(), limit);
        assert!(
            candidates
                .iter()
                .all(|c| !matches!(c.text.as_str(), "進捗" | "写真"))
        );
        assert!(candidates.iter().any(|c| c.text == "し"));
    }
}

#[test]
fn backfilled_word_selection_commits_once_and_undoes_without_changing_raw_draft() {
    for word in ["進捗", "写真"] {
        let mut ime = XRTabletImeEngine::new(EngineConfig {
            default_language: "ja".into(),
            ..Default::default()
        });
        ime.enable_ibus_candidate_mix();
        ime.seed("し");
        let index = ime
            .candidates()
            .iter()
            .position(|c| c.text == word)
            .unwrap();
        assert!((PAGE_SIZE..12).contains(&index));
        ime.select_candidate(index);
        assert_eq!(ime.snapshot().seed_text, "し");
        assert!(ime.snapshot().committed_text.is_empty());
        assert_eq!(ime.selected_completion_text(true), Some(word));
        let committed = ime.commit(CommitOptions { force: true });
        assert!(committed.ok);
        assert_eq!(committed.text.as_deref(), Some(word));
        let duplicate = ime.commit(CommitOptions { force: true });
        assert!(!duplicate.ok && duplicate.text.is_none());
        assert_eq!(ime.snapshot().committed_text, word);
        assert!(ime.undo().unwrap().committed_text.is_empty());
        assert!(ime.undo().is_none());
    }
}
