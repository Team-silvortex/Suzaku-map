//! Actual room-comfort dirty baseline: frozen adjacent pools, not clean HEAD.
use std::collections::HashSet;
use suzaku_map::ime::{
    Candidate, CommitOptions, EngineConfig, XRTabletImeEngine,
    candidate_mix::{CandidateKind, CandidateSource},
};
const EN: &str = r###"[["pending",[["pending","pending","word","local",1120403456]]],["lamp",[["lamp","lamp","word","local",1120403456],["lamps","lamps","word","local",1119354880]]],["fan",[["fan","fan","word","local",1120403456],["fans","fans","word","local",1119354880]]],["where did you put the scissors",[["where did you put the scissors","where did you put the scissors","word","local",1120403456],["where did you put the scissors?","where did you put the scissors?","sentence","local",1117650944]]],["where did you put the tape",[["where did you put the tape","where did you put the tape","word","local",1120403456],["where did you put the tape?","where did you put the tape?","sentence","local",1117650944]]],["could i borrow your notebook",[["could i borrow your notebook","could i borrow your notebook","word","local",1120403456],["could i borrow your notebook for","could i borrow your notebook for","word","local",1119354880],["could i borrow your notebook for a moment?","could i borrow your notebook for a moment?","sentence","local",1117650944]]],["may i use your notebook",[["may i use your notebook","may i use your notebook","word","local",1120403456],["may i use your notebook to","may i use your notebook to","word","local",1119354880],["may i use your notebook to take a few notes?","may i use your notebook to take a few notes?","sentence","local",1117650944]]],["could you bring me some paper",[["could you bring me some paper","could you bring me some paper","word","local",1120403456],["could you bring me some paper from","could you bring me some paper from","word","local",1119354880],["could you bring me some paper from the desk?","could you bring me some paper from the desk?","sentence","local",1117650944]]]]"###;
const ZH: &str = r###"[["jie zhi bi",[["借支笔","借支笔","word","local",1120403456],["截止比较","截止比较","word","local",1119354880],["截止并且","截止并且","word","local",1119092736],["截止必须","截止必须","word","local",1118830592],["借支笔填个名字，马上还你。","借支笔填个名字，马上还你。","sentence","local",1117650944],["借支笔写个地址，用完就还给你。","借支笔写个地址，用完就还给你。","sentence","local",1117585408],["截止标准","截止标准","word","local",1118568448],["jie zhi bi","jie zhi bi","literal","local",1113325568]]],["na zhang zhi",[["拿张纸","拿张纸","word","local",1120403456],["na zhang zhi","na zhang zhi","literal","local",1113325568]]],["xiang pi",[["橡皮","橡皮","word","local",1120403456],["想苹果","想苹果","word","local",1119354880],["想拼音","想拼音","word","local",1119092736],["想屏幕","想屏幕","word","local",1118830592],["想平台","想平台","word","local",1118568448],["xiang pi","xiang pi","literal","local",1113325568]]],["qian zi bi",[["签字笔","签字笔","word","local",1120403456],["qian zi bi","qian zi bi","literal","local",1113325568]]],["ding shu ji",[["订书机","订书机","word","local",1120403456],["ding shu ji","ding shu ji","literal","local",1113325568]]],["bian li tie",[["便利贴","便利贴","word","local",1120403456],["bian li tie","bian li tie","literal","local",1113325568]]],["zhi zhang",[["纸张","纸张","word","local",1120403456],["纸张放在打印机旁边了。","纸张放在打印机旁边了。","sentence","local",1117650944],["纸张还有一些，我先拿几张过来。","纸张还有一些，我先拿几张过来。","sentence","local",1117585408],["zhi zhang","zhi zhang","literal","local",1113325568]]],["feng xiang jiao dai",[["封箱胶带","封箱胶带","word","local",1120403456],["feng xiang jiao dai","feng xiang jiao dai","literal","local",1113325568]]]]"###;
const JA: &str = r###"[["pen",[["ぺん","ぺん","unknown","local",1120403456],["ペン","ペン","unknown","local",1119354880],["pen","pen","literal","local",1113325568]]],["memo",[["めも","めも","unknown","local",1120403456],["メモ","メモ","unknown","local",1119354880],["memo","memo","literal","local",1113325568]]],["no-to",[["のーと","のーと","unknown","local",1120403456],["ノート","ノート","unknown","local",1119354880],["no-to","no-to","literal","local",1113325568]]],["senganryo",[["洗顔りょ","洗顔りょ","sentence","local",1120403456],["せんがんりょ","せんがんりょ","unknown","local",1119354880],["洗顔料","洗顔料","word","local",1119158272],["洗顔両方","洗顔両方","word","local",1119092736],["洗顔料を買い物のメモに追加します。","洗顔料を買い物のメモに追加します。","sentence","local",1117323264],["洗顔料の予備はまだあります。","洗顔料の予備はまだあります。","sentence","local",1117257728],["センガンリョ","センガンリョ","unknown","local",1119092736],["洗顔了解","洗顔了解","word","local",1119027200],["洗顔領収書","洗顔領収書","word","local",1118961664],["senganryo","senganryo","literal","local",1113325568]]],["kankise",[["換気せ","換気せ","sentence","local",1120403456],["かんきせ","かんきせ","unknown","local",1119354880],["換気扇","換気扇","word","local",1119158272],["換気世界","換気世界","word","local",1119092736],["換気扇を回して、こもった空気を外に出します。","換気扇を回して、こもった空気を外に出します。","sentence","local",1117323264],["換気扇を止めてから部屋を出ます。","換気扇を止めてから部屋を出ます。","sentence","local",1117257728],["カンキセ","カンキセ","unknown","local",1119092736],["換気選択","換気選択","word","local",1119027200],["換気説明","換気説明","word","local",1118961664],["kankise","kankise","literal","local",1113325568],["換気先週","換気先週","word","local",1113194496],["換気接続","換気接続","word","local",1113063424]]],["kankisen",[["換気扇","換気扇","word","local",1120403456],["かんきせん","かんきせん","word","local",1119354880],["カンキセン","カンキセン","word","local",1119092736],["換気扇を回して、こもった空気を外に出します。","換気扇を回して、こもった空気を外に出します。","sentence","local",1117585408],["換気扇を止めてから部屋を出ます。","換気扇を止めてから部屋を出ます。","sentence","local",1117519872],["kankisen","kankisen","literal","local",1113325568]]]]"###;

fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates().iter().all(|c| c.label == c.text
            && c.source == CandidateSource::Local
            && c.score.is_finite())
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
    ime
}
fn signature(rows: &[Candidate]) -> serde_json::Value {
    serde_json::json!(
        rows.iter()
            .map(|c| (
                &c.text,
                &c.label,
                c.kind.id(),
                c.source.id(),
                c.score.to_bits()
            ))
            .collect::<Vec<_>>()
    )
}
fn frozen(language: &str, raw: &str) {
    let cases: serde_json::Value = serde_json::from_str(raw).unwrap();
    for case in cases.as_array().unwrap() {
        let seed = case[0].as_str().unwrap();
        let ime = engine(language, seed);
        assert_eq!(signature(ime.candidates()), case[1], "{language}/{seed}");
    }
}
fn commit(language: &str, seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(language, seed);
    let at = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()));
    ime.select_candidate(at);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    assert!(!ime.commit(CommitOptions { force: true }).ok);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
#[test]
fn stationery_preserves_eight_actual_english_neighbour_pools() {
    frozen("en", EN);
}
#[test]
fn stationery_preserves_eight_actual_chinese_neighbour_pools() {
    frozen("zh-Hans", ZH);
}
#[test]
fn stationery_preserves_six_actual_japanese_neighbour_pools() {
    frozen("ja", JA);
}
#[test]
fn stationery_old_stems_words_and_exact_raw_remain_usable_after_enrichment() {
    for (language, seed, word) in [
        ("en", "rule", "rule"),
        ("zh-Hans", "bi mao", "笔帽"),
        ("zh-Hans", "cao gao", "草稿"),
        ("zh-Hans", "cao gao", "草稿箱"),
        ("ja", "enpitsu", "鉛筆"),
        ("ja", "keshigomu", "消しゴム"),
    ] {
        commit(language, seed, word, CandidateKind::Word);
        let ime = engine(language, seed);
        let raw = ime.candidates().iter().find(|c| c.text == seed).unwrap();
        let kind = raw.kind;
        assert!(matches!(kind, CandidateKind::Word | CandidateKind::Literal));
        commit(language, seed, seed, kind);
    }
}
