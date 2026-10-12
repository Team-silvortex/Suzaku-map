//! Independent basic-word and adopted-progress expectations, before data changes.
//! Numeric adoption remains editable; these are engine, not physical-key checks.
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, XRTabletImeEngine},
    lexicon::{self, EntryKind},
};

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn choice(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

#[test]
fn common_still_and_after_words_are_explicit_independent_data() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    // This historical two-word addition did not add contexts. Later layers
    // have their own fixed windows; retain this released prefix unchanged.
    assert_eq!(vocabulary.continuations().take(687).count(), 687);
    assert_eq!(
        vocabulary
            .continuations()
            .take(687)
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1372
    );
    for (reading, text) in [("hai", "还"), ("hou", "后")] {
        let entry = vocabulary
            .readings()
            .iter()
            .find(|entry| entry.reading == reading && entry.text == text)
            .unwrap_or_else(|| panic!("missing basic independent reading {reading} -> {text}"));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
}

#[test]
fn basic_words_are_selectable_without_a_model_and_keep_literal_input() {
    for (reading, text) in [("hai", "还"), ("hou", "后")] {
        for seed in [
            reading.to_owned(),
            reading.to_ascii_uppercase(),
            format!("{reading}  "),
        ] {
            let mut ime = engine(&seed);
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
            let index = choice(&ime, text, CandidateKind::Word);
            assert!(index < PAGE_SIZE);
            choice(&ime, &seed, CandidateKind::Literal);
            ime.select_candidate(index);
            assert_eq!(ime.selected_completion_text(true), Some(text));
            assert!(ime.snapshot().committed_text.is_empty());
            assert_eq!(
                ime.commit(CommitOptions { force: true }).text.as_deref(),
                Some(text)
            );
            assert!(!ime.commit(CommitOptions { force: true }).ok);
            assert!(ime.undo().unwrap().committed_text.is_empty());
            assert!(ime.undo().is_none());
        }
    }
}

#[test]
fn adopted_han_progress_is_a_sentence_until_a_partial_word_is_completed() {
    for (start, middle, adopted, tail, word, sentence) in [
        (
            "优惠券",
            "hai zai",
            "优惠券还在",
            "youxi",
            "优惠券还在有效期",
            "优惠券还在有效期内。",
        ),
        (
            "登录失败",
            "hou ke yi",
            "登录失败后可以",
            "zaishiyi",
            "登录失败后可以再试一次",
            "登录失败后可以再试一次。",
        ),
    ] {
        let raw = format!("{start}{middle}");
        let mut ime = engine(&raw);
        choice(&ime, &raw, CandidateKind::Literal);
        let index = choice(&ime, adopted, CandidateKind::Sentence);
        assert!(index < PAGE_SIZE);
        ime.select_candidate(index);
        assert_eq!(ime.selected_completion_text(true), Some(adopted));
        assert!(ime.snapshot().committed_text.is_empty());

        let raw = format!("{adopted}{tail}");
        ime.seed(&raw);
        choice(&ime, &raw, CandidateKind::Literal);
        choice(&ime, word, CandidateKind::Word);
        let index = choice(&ime, sentence, CandidateKind::Sentence);
        ime.select_candidate(index);
        assert_eq!(ime.selected_completion_text(true), Some(sentence));
        assert!(ime.snapshot().committed_text.is_empty());
        assert_eq!(
            ime.commit(CommitOptions { force: true }).text.as_deref(),
            Some(sentence)
        );
        assert!(!ime.commit(CommitOptions { force: true }).ok);
        assert!(ime.undo().unwrap().committed_text.is_empty());
    }
}

#[test]
fn function_words_append_only_after_the_frozen_care_vocabulary() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(vocabulary.language(), "zh-Hans");
    assert!(vocabulary.word_layers().is_empty());
    assert_eq!(vocabulary.phrase_endings().count(), 0);
    assert!(vocabulary.readings().len() >= 2922);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2920) {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for byte in entry
            .reading
            .bytes()
            .chain([0])
            .chain(entry.text.bytes())
            .chain([0])
            .chain(kind.bytes())
            .chain([u8::from(entry.require_separators)])
        {
            reading_hash = (reading_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    // Independent fingerprints of the frozen zh-before.json resource, not
    // regenerated expectations from the now-appended embedded vocabulary.
    assert_eq!(reading_hash, 0x725df5a269c5adef);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut context_count = 0;
    let mut sentence_count = 0;
    for (context, values) in vocabulary.continuations().take(687) {
        context_count += 1;
        sentence_count += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!((context_count, sentence_count), (687, 1372));
    assert_eq!(context_hash, 0x06df73a06ff2be69);
    for (entry, (reading, text)) in vocabulary.readings()[2920..2922]
        .iter()
        .zip([("hai", "还"), ("hou", "后")])
    {
        assert_eq!(entry.reading, reading);
        assert_eq!(entry.text, text);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
}

#[test]
fn six_existing_whole_words_keep_their_precise_primary_over_new_single_char_paths() {
    for (reading, text) in [
        ("hai'mei", "还没"),
        ("hai'shi", "还是"),
        ("hai'xian", "海鲜"),
        ("hou'xuan", "候选"),
        ("hou'tian", "后天"),
        ("hou'mian", "后面"),
    ] {
        for seed in [
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.to_owned(),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            let ime = engine(&seed);
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
            assert_eq!(choice(&ime, text, CandidateKind::Word), 0);
            assert_eq!(ime.candidates()[0].score, 100.0);
            choice(&ime, &seed, CandidateKind::Literal);
        }
    }
    // New one-syllable words are appended behind existing short-prefix
    // completion slots. Do not reinterpret ha / ho as exact hai / hou.
    for (seed, primary) in [("ha", "好"), ("ho", "候选")] {
        let ime = engine(seed);
        assert_eq!(choice(&ime, primary, CandidateKind::Word), 0);
        choice(&ime, seed, CandidateKind::Literal);
    }
    for (seed, old_words) in [
        ("hai", ["还没", "还是", "海鲜", "还可以"]),
        ("hou", ["候选", "后天", "候选词", "候机"]),
    ] {
        let ime = engine(seed);
        for old in old_words {
            choice(&ime, old, CandidateKind::Word);
        }
    }
}

#[test]
fn unrelated_homophone_and_partial_continuation_page_guards_do_not_move() {
    let ime = engine("shijian");
    assert_eq!(choice(&ime, "时间", CandidateKind::Word), 0);
    assert_eq!(ime.candidates()[0].score, 100.0);
    for text in ["事件", "实践", "世间"] {
        assert!(choice(&ime, text, CandidateKind::Word) < PAGE_SIZE);
    }
    assert!(!ime.candidates().iter().any(|candidate| {
        candidate.kind == CandidateKind::Sentence && candidate.text.starts_with("时间安排")
    }));
    choice(&ime, "shijian", CandidateKind::Literal);

    let ime = engine("zaishiyi");
    assert_eq!(ime.candidates()[0].text, "在释义");
    assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
    assert_eq!(ime.candidates()[0].score, 100.0);
    assert!(choice(&ime, "再试一次", CandidateKind::Word) < PAGE_SIZE);
    for sentence in ["再试一次看看能否连接。", "再试一次，不行的话稍后再处理。"]
    {
        choice(&ime, sentence, CandidateKind::Sentence);
    }
    choice(&ime, "zaishiyi", CandidateKind::Literal);

    let ime = engine("chumenqianshoushiyixi");
    assert_eq!(ime.candidates()[0].text, "出门签收释义西");
    assert_eq!(ime.candidates()[0].score, 100.0);
    for (word, score) in [
        ("出门前收拾一下", 84.0),
        ("出门签收释义谢谢", 82.0),
        ("出门签收释义现在", 80.0),
        ("出门签收释义喜欢", 78.0),
    ] {
        let index = choice(&ime, word, CandidateKind::Word);
        assert!(index < PAGE_SIZE);
        assert_eq!(ime.candidates()[index].score, score);
    }
    assert!(
        choice(
            &ime,
            "出门前收拾一下，别把东西落在桌上。",
            CandidateKind::Sentence,
        ) < PAGE_SIZE
    );
    let index = choice(
        &ime,
        "出门前收拾一下，回来就不用再找了。",
        CandidateKind::Sentence,
    );
    assert!((PAGE_SIZE..2 * PAGE_SIZE).contains(&index));
    choice(&ime, "chumenqianshoushiyixi", CandidateKind::Literal);
}

#[test]
fn single_function_syllables_do_not_erase_wrong_splits_lines_or_protected_tokens() {
    for (reading, word, splits) in [
        ("hai", "还", ["h a i", "ha i"]),
        ("hou", "后", ["h o u", "ho u"]),
    ] {
        for seed in splits {
            let ime = engine(seed);
            choice(&ime, seed, CandidateKind::Literal);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == word)
            );
        }
        for line in [
            "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
        ] {
            let split = format!("{}{line}{}", &reading[..2], &reading[2..]);
            let ime = engine(&split);
            choice(&ime, &split, CandidateKind::Literal);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == word)
            );
            let trailing = format!("{reading}{line}");
            let ime = engine(&trailing);
            choice(&ime, &trailing, CandidateKind::Literal);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == word)
            );
        }
        for prefix in [
            "https://example.invalid/",
            "mail@example.invalid/",
            "file_name_",
            "folder\\",
        ] {
            let seed = format!("{prefix}{reading}");
            let ime = engine(&seed);
            choice(&ime, &seed, CandidateKind::Literal);
            assert!(!ime.candidates().iter().any(|candidate| {
                candidate.text == word || candidate.text == format!("{prefix}{word}")
            }));
        }
    }
}

#[test]
fn adopted_function_word_progress_preserves_literal_han_and_control_boundaries() {
    for (raw, frozen, target) in [
        ("优惠卷hai zai", "优惠卷", "优惠券还在有效期内。"),
        ("登陆失败hou ke yi", "登陆失败", "登录失败后可以再试一次。"),
        (
            "登录失败后可以在shiyi",
            "登录失败后可以在",
            "登录失败后可以再试一次。",
        ),
    ] {
        let ime = engine(raw);
        choice(&ime, raw, CandidateKind::Literal);
        assert!(
            ime.candidates()
                .iter()
                .all(|candidate| candidate.text.starts_with(frozen))
        );
        assert!(
            !ime.candidates()
                .iter()
                .any(|candidate| candidate.text == target)
        );
    }
    for (prefix, tail, target) in [
        ("优惠券还在", "youxi", "优惠券还在有效期内。"),
        ("登录失败后可以", "zaishiyi", "登录失败后可以再试一次。"),
    ] {
        for separator in ["\n", "\u{2028}", "?", "@", "_"] {
            let seed = format!("{prefix}{separator}{tail}");
            let ime = engine(&seed);
            choice(&ime, &seed, CandidateKind::Literal);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == target)
            );
        }
    }
}
