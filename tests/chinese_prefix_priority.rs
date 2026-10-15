//! A longer extra completion must preserve the original short-word first page.
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, XRTabletImeEngine},
};

const EXTENSION: &str = "这个版本是最新的";

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|item| item.text == text && item.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn commit_once(seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(seed);
    ime.select_candidate(index(&ime, text, kind));
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.selected_completion_text(true), Some(text));
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn extra_longer_word_preserves_four_prior_words_and_both_sentence_slots() {
    for seed in [
        "zheg",
        "zhe'g",
        "zhe g",
        "ZHE G",
        "zhe\u{3000}g",
        "zheg ",
        "zhe g\t",
    ] {
        let ime = engine(seed);
        assert_eq!(ime.snapshot().seed_text, seed);
        assert!(ime.snapshot().committed_text.is_empty());
        assert!(ime.candidates().len() <= PAGE_SIZE * 2);
        assert!(
            ime.candidates()
                .iter()
                .all(|item| item.source == CandidateSource::Local)
        );
        for (item, (text, score)) in ime.candidates().iter().zip([
            ("这个词", 100.0),
            ("这个意思", 92.0),
            ("这个更合适", 90.0),
            ("这个", 88.0),
        ]) {
            assert_eq!(
                (item.text.as_str(), item.kind, item.score),
                (text, CandidateKind::Word, score),
                "seed={seed:?}: {:?}",
                ime.candidates()
            );
        }
        for sentence in ["这个词是什么意思？", "这个词应该怎么用？"] {
            assert!(index(&ime, sentence, CandidateKind::Sentence) < PAGE_SIZE);
        }
        let extension = index(&ime, EXTENSION, CandidateKind::Word);
        assert!(
            extension >= PAGE_SIZE,
            "extension displaced the original first page"
        );
        assert!(ime.candidates()[extension].score < 88.0);
        index(&ime, seed, CandidateKind::Literal);
        commit_once(seed, "这个", CandidateKind::Word);
        commit_once(seed, EXTENSION, CandidateKind::Word);
        commit_once(seed, seed, CandidateKind::Literal);
    }
}

#[test]
fn specific_extension_readings_keep_first_page_word_sentence_and_literal() {
    for seed in [
        "zhegebanbenshizuixinde",
        "zhe ge ban ben shi zui xin d",
        "ZHE GE BAN BEN SHI ZUI XIN D",
    ] {
        let ime = engine(seed);
        assert!(index(&ime, EXTENSION, CandidateKind::Word) < PAGE_SIZE);
        for sentence in [
            "这个版本是最新的，后面都用这一份吧。",
            "这个版本是最新的，旧文件可以先留着。",
        ] {
            assert!(index(&ime, sentence, CandidateKind::Sentence) < PAGE_SIZE);
            commit_once(seed, sentence, CandidateKind::Sentence);
        }
        commit_once(seed, EXTENSION, CandidateKind::Word);
        commit_once(seed, seed, CandidateKind::Literal);
    }
}
