//! Complete builtin candidate vectors captured from the released 0.9.1 source
//! before any messaging data was appended. No native input or user data is used.
use suzaku_map::ime::{
    EngineConfig, XRTabletImeEngine,
    candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
};

fn unchanged(language: &str, cases: &[(&str, u64)]) {
    for &(seed, expected) in cases {
        let mut engine = XRTabletImeEngine::new(EngineConfig {
            default_language: language.into(),
            ..Default::default()
        });
        engine.enable_ibus_candidate_mix();
        engine.seed(seed);
        assert_eq!(engine.snapshot().seed_text, seed);
        assert!(engine.snapshot().committed_text.is_empty());
        assert!(engine.candidates().len() <= 12);
        // The first after-data run correctly rejected full-pool equality for
        // renraku: the new 連絡漏れ word extends that complete reading. Accept
        // exactly this one addition on page two, not a refreshed old golden.
        // The unchanged mixed-list index formula also lowers the four authored
        // sentence weights by exactly 0.5. Review that bounded effect explicitly;
        // reversing only these deltas must recover every old vector byte.
        let mut candidates = engine.candidates().to_vec();
        // Plant vocabulary fills three spare page-two slots for this exact
        // bounded prefix. Keep both old authors on page one, and accept only
        // the fully reviewed vector before recovering the historical bytes.
        if language == "ja" && seed == "mad" {
            let expected = [
                ("まd", "unknown", 1120403456),
                ("マd", "unknown", 1119354880),
                ("窓枠", "word", 1119223808),
                ("窓口", "word", 1119158272),
                ("窓口はどこにありますか。", "sentence", 1117454336),
                ("窓口で手続きをしたいです。", "sentence", 1117388800),
                ("窓辺", "word", 1119092736),
                ("窓辺に小さな鉢を並べます。", "sentence", 1117323264),
                ("窓辺の植物を眺めながら休みます。", "sentence", 1117257728),
                ("mad", "literal", 1113325568),
            ];
            assert_eq!(
                candidates
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
                "the only accepted plant delta for mad must match in full"
            );
            drop(candidates.drain(6..9));
            for index in [4, 5] {
                candidates[index].score += 0.5;
            }
        }
        if language == "ja" && seed == "renraku" {
            assert_eq!(candidates.len(), 10);
            assert_eq!(PAGE_SIZE, 6);
            let addition = candidates.remove(PAGE_SIZE);
            assert_eq!(addition.text, "連絡漏れ");
            assert_eq!(addition.label, "連絡漏れ");
            assert_eq!(addition.kind, CandidateKind::Word);
            assert_eq!(addition.source, CandidateSource::Local);
            assert_eq!(addition.score.to_bits(), 90.0_f32.to_bits());
            assert!(candidates.iter().all(|c| c.text != addition.text));
            for (index, score) in [4, 5, 6, 7].into_iter().zip([77.5_f32, 77.0, 76.5, 76.0]) {
                let sentence = &mut candidates[index];
                assert_eq!(sentence.kind, CandidateKind::Sentence);
                assert_eq!(sentence.source, CandidateSource::Local);
                assert_eq!(sentence.score.to_bits(), score.to_bits());
                sentence.score += 0.5;
            }
        }
        let mut hash = 0xcbf29ce484222325_u64;
        for candidate in &candidates {
            for byte in candidate
                .text
                .bytes()
                .chain([0])
                .chain(candidate.label.bytes())
                .chain([0])
                .chain(candidate.kind.id().bytes())
                .chain([0])
                .chain(candidate.source.id().bytes())
                .chain([0])
                .chain(candidate.score.to_bits().to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        assert_eq!(
            hash,
            expected,
            "{language} {seed:?}: {:?}",
            engine.candidates()
        );
    }
}

#[test]
fn messaging_preserves_eleven_prior_english_complete_vectors() {
    unchanged(
        "en",
        &[
            ("please", 0x3de602bd958dcaa1),
            ("send", 0xedac6b70c640f467),
            ("can you send", 0x3eb23e56c3e6165f),
            ("let me know", 0x2c0701007bc7dd34),
            ("i will", 0x397e82f057f79ff4),
            ("i found your glas", 0xb4297ac5d0111104),
            ("do you have a spare char", 0x42dbc1ab29ce186c),
            ("could you repeat", 0x3adc3fa7b47b70d0),
            ("check", 0xf7dac6c150db2b96),
            ("reply", 0xb5264003914456ef),
            ("i'll keep it her", 0x85b4858fbf99000d),
        ],
    );
}

#[test]
fn messaging_preserves_sixteen_prior_chinese_complete_vectors() {
    unchanged(
        "zh-Hans",
        &[
            ("hui", 0x9f40febe002c7d3a),
            ("huifu", 0xe0eef19e70ed8d2b),
            ("deng", 0x69a8f8c6352dc0bd),
            ("shou", 0xb36a8aa5a5aefacf),
            ("kan", 0x7faf989e3bfb561b),
            ("xiao", 0xdaf828af192a2f4b),
            ("you", 0xcbca5e25688da3ef),
            ("zhe g", 0xcf9166894dc3b8d2),
            ("ta", 0x65670d69a026d149),
            ("hao", 0x19625ac9d6fa264b),
            ("jie", 0xa7d6f6af2c16488d),
            ("huan", 0xc8c94b70db3cfef1),
            ("na guo lai", 0xc37e4a2218d764e5),
            ("wo zhao dao shu le", 0x3f730e5091d9bf45),
            ("lian", 0x893cb39af5230afb),
            ("duan", 0x372d11b6f19a5bb1),
        ],
    );
}

#[test]
fn messaging_preserves_eleven_japanese_vectors_and_reviews_one_exact_enrichment() {
    unchanged(
        "ja",
        &[
            ("kyouyu", 0x1319e05a844e649e),
            ("bas", 0x1035a9e0c7f8ce5a),
            ("sentakumon", 0x77e657b9fb24c806),
            ("souj", 0x38ddde3a1ab80b7d),
            ("kanji", 0xb797e62a71374a8b),
            ("sentaku", 0xced3c4f331f7a2db),
            ("hoteru", 0x94f3192c31342a01),
            ("renraku", 0xbb39602e6bdd4216),
            ("henji", 0x72362f85c15d6f7f),
            ("mad", 0xce5f0c2206af856d),
            ("kyouka", 0x6c41b51c21a00045),
            ("ichij", 0x77dee4536d36b4c7),
        ],
    );
}
