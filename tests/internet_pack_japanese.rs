//! Fixed authored optional Japanese internet workflows. This exercises private
//! engine snapshots, not network actions, physical IBus keys or general morphology.
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::{
        japanese::romaji_to_hiragana,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{
        self, EntryKind,
        packs::{self, OfflinePack, PackStore},
    },
};

const PACK_ID: &str = "org.suzaku.ja.internet";
const CHILD_CASE: &str = "SUZAKU_INTERNET_JAPANESE_CHILD_CASE";

// Independent expectations authored before the optional JSON resource.
const WORDS: &[(&str, &str, &str)] = &[
    ("burauza-", "ぶらうざー", "ブラウザー"),
    ("tabu", "たぶ", "タブ"),
    ("rinku", "りんく", "リンク"),
    ("bukkuma-ku", "ぶっくまーく", "ブックマーク"),
    ("kensaku", "けんさく", "検索"),
    ("kensakukekka", "けんさくけっか", "検索結果"),
    ("ki-wa-do", "きーわーど", "キーワード"),
    ("pe-ji", "ぺーじ", "ページ"),
    ("webupe-ji", "うぇぶぺーじ", "ウェブページ"),
    ("etsuran", "えつらん", "閲覧"),
    ("etsuranrireki", "えつらんりれき", "閲覧履歴"),
    ("saiyomikomi", "さいよみこみ", "再読み込み"),
    ("hozon", "ほぞん", "保存"),
    ("hozonsaki", "ほぞんさき", "保存先"),
    ("daunro-do", "だうんろーど", "ダウンロード"),
    ("appuro-do", "あっぷろーど", "アップロード"),
    ("fairu", "ふぁいる", "ファイル"),
    ("foruda", "ふぉるだ", "フォルダ"),
    ("asshuku", "あっしゅく", "圧縮"),
    ("kaitou", "かいとう", "解凍"),
    ("akaunto", "あかうんと", "アカウント"),
    ("roguin", "ろぐいん", "ログイン"),
    ("roguauto", "ろぐあうと", "ログアウト"),
    ("touroku", "とうろく", "登録"),
    ("settei", "せってい", "設定"),
    ("purofi-ru", "ぷろふぃーる", "プロフィール"),
    ("pasuwa-do", "ぱすわーど", "パスワード"),
    ("ninshou", "にんしょう", "認証"),
    ("hyouji", "ひょうじ", "表示"),
    ("gamen", "がめん", "画面"),
    ("toukou", "とうこう", "投稿"),
    ("komento", "こめんと", "コメント"),
    ("kiji", "きじ", "記事"),
    ("midashi", "みだし", "見出し"),
    ("haishin", "はいしん", "配信"),
    ("douga", "どうが", "動画"),
    ("gazou", "がぞう", "画像"),
    ("saisei", "さいせい", "再生"),
    ("shichou", "しちょう", "視聴"),
    ("jimaku", "じまく", "字幕"),
    ("zengamen", "ぜんがめん", "全画面"),
    ("myu-to", "みゅーと", "ミュート"),
    ("tsuushinryou", "つうしんりょう", "通信量"),
    ("kaisensokudo", "かいせんそくど", "回線速度"),
    ("gashitsu", "がしつ", "画質"),
    ("koukaihan'i", "こうかいはんい", "公開範囲"),
    ("kensakuran", "けんさくらん", "検索欄"),
    ("yomikomichuu", "よみこみちゅう", "読み込み中"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "ブラウザー",
        [
            "ブラウザーを開き直してみます。",
            "ブラウザーの表示がいつもと違います。",
        ],
    ),
    (
        "タブ",
        [
            "タブをもう一つ開きます。",
            "タブを閉じる前に内容を確認します。",
        ],
    ),
    (
        "リンク",
        [
            "リンクを送ってもらえますか。",
            "リンクが開けないので、後でもう一度試します。",
        ],
    ),
    (
        "ブックマーク",
        [
            "ブックマークに追加しておきます。",
            "ブックマークからページを開きました。",
        ],
    ),
    (
        "検索",
        ["検索しても見つかりません。", "検索する言葉を変えてみます。"],
    ),
    (
        "検索結果",
        [
            "検索結果をもう少し絞り込みたいです。",
            "検索結果の上から順に見ています。",
        ],
    ),
    (
        "キーワード",
        [
            "キーワードを一つ追加してみます。",
            "キーワードを変えると結果が変わります。",
        ],
    ),
    (
        "ページ",
        [
            "ページがまだ読み込み中です。",
            "ページの下の方に説明があります。",
        ],
    ),
    (
        "再読み込み",
        [
            "再読み込みしても表示が変わりません。",
            "再読み込みしたら画像が表示されました。",
        ],
    ),
    (
        "保存先",
        [
            "保存先のフォルダを教えてください。",
            "保存先を分かりやすい場所に変えました。",
        ],
    ),
    (
        "ダウンロード",
        [
            "ダウンロードが終わるまで待っています。",
            "ダウンロードしたファイルが見つかりません。",
        ],
    ),
    (
        "アップロード",
        [
            "アップロードが途中で止まりました。",
            "アップロードが終わったら連絡します。",
        ],
    ),
    (
        "ファイル",
        [
            "ファイルが大きいので、少し時間がかかります。",
            "ファイルを開けるか確認してもらえますか。",
        ],
    ),
    (
        "フォルダ",
        [
            "フォルダを分けて保存しています。",
            "フォルダの中に同じ名前のファイルがあります。",
        ],
    ),
    (
        "ログイン",
        [
            "ログインできたので、続きを確認します。",
            "ログインしたままになっていました。",
        ],
    ),
    (
        "ログアウト",
        [
            "ログアウトしてから画面を閉じます。",
            "ログアウトしたか確認しておきます。",
        ],
    ),
    (
        "コメント",
        [
            "コメントに補足を書いておきます。",
            "コメントを送る前に読み直します。",
        ],
    ),
    (
        "動画",
        [
            "動画の再生が途中で止まります。",
            "動画を後でゆっくり見たいです。",
        ],
    ),
    (
        "画像",
        ["画像がまだ表示されません。", "画像を大きくして見たいです。"],
    ),
    (
        "公開範囲",
        [
            "公開範囲を確認してから投稿します。",
            "公開範囲は友達だけにしています。",
        ],
    ),
];

fn internet() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == PACK_ID)
        .expect("the optional Japanese internet pack has its stable catalog ID")
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("suzaku-ja-internet-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
    fn initialize(&self, enabled: bool, coexist: bool) {
        let store = self.store();
        assert!(store.list().unwrap().is_empty());
        let selected = if coexist {
            let ids = [
                "org.suzaku.ja.rail",
                "org.suzaku.ja.family",
                "org.suzaku.ja.internet",
                "org.suzaku.en.internet",
                "org.suzaku.zh-hans.internet",
            ];
            let catalog = packs::recommended();
            ids.into_iter()
                .map(|id| {
                    catalog
                        .iter()
                        .find(|pack| pack.manifest.id == id)
                        .unwrap()
                        .clone()
                })
                .collect::<Vec<_>>()
        } else {
            vec![internet()]
        };
        assert!(selected.len() <= packs::MAX_PACKS);
        for pack in &selected {
            store.install(pack.clone(), false).unwrap();
        }
        if !enabled {
            store.set_enabled(PACK_ID, false).unwrap();
        }
        let report = packs::initialize(&store).unwrap();
        assert!(report.errors.is_empty(), "{report:?}");
        if enabled {
            assert_eq!(report.loaded.len(), selected.len());
            assert!(report.loaded.iter().any(|id| id == PACK_ID));
        } else {
            assert!(report.loaded.is_empty());
            assert_eq!(report.disabled, [PACK_ID]);
            assert!(std::ptr::eq(
                lexicon::active("ja").unwrap(),
                lexicon::builtin("ja").unwrap()
            ));
        }
        let builtin = lexicon::builtin("ja").unwrap();
        let active = lexicon::active("ja").unwrap();
        assert_eq!(
            builtin.readings(),
            &active.readings()[..builtin.readings().len()]
        );
        assert_eq!(
            builtin.continuations().collect::<Vec<_>>(),
            active
                .continuations()
                .take(builtin.continuations().count())
                .collect::<Vec<_>>()
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fresh(name: &str, body: impl FnOnce(&Fixture)) {
    if std::env::var(CHILD_CASE).as_deref() == Ok(name) {
        body(&Fixture::new());
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_CASE, name)
        .env_remove("SUZAKU_LEXICON_DIR")
        .env_remove("SUZAKU_IME_CONFIG")
        .env_remove("SUZAKU_LINUX_IME_SOCKET")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fresh {name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == seed && c.kind == CandidateKind::Literal)
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let position = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    if kind != CandidateKind::Literal {
        assert!(
            position < PAGE_SIZE,
            "{text:?} is not on the first page: {:?}",
            ime.candidates()
        );
    }
    position
}

fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn commit_second_page_once(ime: &mut XRTabletImeEngine, text: &str, expected_index: usize) {
    assert_eq!(expected_index / PAGE_SIZE, 1);
    let chosen = &ime.candidates()[expected_index];
    assert_eq!(chosen.text, text);
    assert_eq!(chosen.kind, CandidateKind::Sentence);
    ime.select_candidate(expected_index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn choices(seed: &str, word: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    let kind = if seed == word {
        CandidateKind::Literal
    } else {
        CandidateKind::Word
    };
    index(&ime, word, kind);
    commit_once(&mut engine(seed), word, kind);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
        for sentence in sentences {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn internet_japanese_has_fixed_novel_readings_and_authored_sentences() {
    let pack = internet();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "ja");
    assert_eq!(pack.manifest.topics, ["internet", "digital"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    assert_eq!(WORDS.len(), 48);
    assert_eq!(SENTENCES.len(), 20);
    assert_eq!(pack.lexicon.readings().len(), WORDS.len());
    assert_eq!(pack.lexicon.continuations().count(), SENTENCES.len());
    for (entry, &(romaji, kana, word)) in pack.lexicon.readings().iter().zip(WORDS) {
        assert_eq!(romaji_to_hiragana(romaji), kana);
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, values), &(word, sentences)) in pack.lexicon.continuations().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            sentences
        );
        assert!(sentences.iter().all(|text| text.starts_with(word)));
    }
    assert_eq!(
        pack.lexicon
            .continuations()
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        40
    );
    let old = packs::recommended();
    for lexicon in std::iter::once(lexicon::builtin("ja").unwrap()).chain(
        old.iter()
            .filter(|other| other.manifest.language == "ja" && other.manifest.id != PACK_ID)
            .map(|other| &other.lexicon),
    ) {
        for &(_, kana, word) in WORDS {
            assert!(
                lexicon
                    .readings()
                    .iter()
                    .all(|entry| entry.reading != kana && entry.text != word),
                "old collision: {kana}/{word}"
            );
        }
        for &(context, sentences) in SENTENCES {
            for (old_context, old_sentences) in lexicon.continuations() {
                assert_ne!(context, old_context);
                assert!(
                    sentences
                        .iter()
                        .all(|s| old_sentences.iter().all(|old| old != s))
                );
            }
        }
    }
}

fn assert_unavailable() {
    for &(romaji, _, word) in WORDS {
        let mut ime = engine(romaji);
        draft(&ime, romaji);
        // Katakana transliteration may already exist as an untyped choice;
        // optional vocabulary must not be mistaken for generic transliteration.
        assert!(
            ime.candidates()
                .iter()
                .all(|c| c.text != word || c.kind != CandidateKind::Word)
        );
        commit_once(&mut ime, romaji, CandidateKind::Literal);
    }
    for &(word, sentences) in SENTENCES {
        let ime = engine(word);
        assert!(
            sentences
                .iter()
                .all(|s| ime.candidates().iter().all(|c| c.text != *s))
        );
    }
}

#[test]
fn internet_japanese_absent_catalog_does_not_install_or_change_builtin() {
    fresh(
        "internet_japanese_absent_catalog_does_not_install_or_change_builtin",
        |fixture| {
            let _pack = internet();
            let store = fixture.store();
            let report = packs::initialize(&store).unwrap();
            assert!(report.loaded.is_empty() && report.errors.is_empty());
            assert!(std::ptr::eq(
                lexicon::active("ja").unwrap(),
                lexicon::builtin("ja").unwrap()
            ));
            assert_unavailable();
            assert!(!store.path().exists());
        },
    );
}

#[test]
fn internet_japanese_disabled_pack_preserves_builtin_choices() {
    fresh(
        "internet_japanese_disabled_pack_preserves_builtin_choices",
        |fixture| {
            fixture.initialize(false, false);
            assert_unavailable();
        },
    );
}

fn complete_workflows() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            choices(&seed, word);
        }
    }
    for &(word, _) in SENTENCES {
        choices(word, word);
    }
}

#[test]
fn internet_japanese_complete_romaji_case_kana_and_words_offer_both_sentences() {
    fresh(
        "internet_japanese_complete_romaji_case_kana_and_words_offer_both_sentences",
        |fixture| {
            fixture.initialize(true, false);
            complete_workflows();
        },
    );
}

#[test]
fn internet_japanese_selected_partials_and_reading_separators_preserve_choices() {
    fresh(
        "internet_japanese_selected_partials_and_reading_separators_preserve_choices",
        |fixture| {
            fixture.initialize(true, false);
            for (seed, word) in [
                ("burauz", "ブラウザー"),
                ("ぶらうざ", "ブラウザー"),
                ("bukkuma-k", "ブックマーク"),
                ("ぶっくまー", "ブックマーク"),
                ("kensakukekk", "検索結果"),
                ("けんさくけっ", "検索結果"),
                ("saiyomikom", "再読み込み"),
                ("さいよみこ", "再読み込み"),
                ("hozonsak", "保存先"),
                ("daunro-d", "ダウンロード"),
                ("だうんろー", "ダウンロード"),
                ("appuro-d", "アップロード"),
                ("あっぷろー", "アップロード"),
                ("roguaut", "ログアウト"),
                ("ろぐあう", "ログアウト"),
                ("komen", "コメント"),
                ("こめん", "コメント"),
                ("koukaihan'", "公開範囲"),
                ("こうかいはん", "公開範囲"),
            ] {
                choices(seed, word);
            }
            // This specific Kana prefix also offers old sa-* words after 保存.
            // The pinned segmented conversion counts toward the first-page
            // sentence quota; the second authored sentence remains on page two.
            // Keep that existing order instead of claiming every partial can
            // fit its two authored sentences on the first page.
            let seed = "ほぞんさ";
            let ime = engine(seed);
            draft(&ime, seed);
            assert_eq!(
                ime.candidates()
                    .iter()
                    .map(|candidate| candidate.text.as_str())
                    .collect::<Vec<_>>(),
                [
                    "保存さ",
                    "保存先",
                    "保存再開",
                    "ホゾンサ",
                    "保存作業",
                    "保存先のフォルダを教えてください。",
                    "保存散歩",
                    "保存先を分かりやすい場所に変えました。",
                    "ほぞんさ",
                    "保存再確認",
                    "保存差し支えなければ",
                    "保存再送",
                ]
            );
            assert_eq!(index(&ime, "保存先", CandidateKind::Word), 1);
            assert_eq!(
                index(
                    &ime,
                    "保存先のフォルダを教えてください。",
                    CandidateKind::Sentence
                ),
                5
            );
            assert_eq!(index(&ime, seed, CandidateKind::Literal), 8);
            commit_once(&mut engine(seed), "保存先", CandidateKind::Word);
            commit_once(
                &mut engine(seed),
                "保存先のフォルダを教えてください。",
                CandidateKind::Sentence,
            );
            commit_second_page_once(
                &mut engine(seed),
                "保存先を分かりやすい場所に変えました。",
                7,
            );
            commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
                for (left, right, word) in [
                    ("KEN", "SAKU", "検索"),
                    ("dau", "nro-do", "ダウンロード"),
                    ("さい", "よみこみ", "再読み込み"),
                    ("rogu", "auto", "ログアウト"),
                ] {
                    choices(&format!("{space}{left}{space}{right}{space}"), word);
                }
            }
            for seed in ["koukaihan’i", "こうかい はんい"] {
                choices(seed, "公開範囲");
            }
        },
    );
}

#[test]
fn internet_japanese_adopted_words_allow_editable_authored_sentence_progress() {
    fresh(
        "internet_japanese_adopted_words_allow_editable_authored_sentence_progress",
        |fixture| {
            fixture.initialize(true, false);
            for (reading, word, tail, sentence) in [
                (
                    "burauza-",
                    "ブラウザー",
                    "wo",
                    "ブラウザーを開き直してみます。",
                ),
                (
                    "rinku",
                    "リンク",
                    "ga",
                    "リンクが開けないので、後でもう一度試します。",
                ),
                (
                    "bukkuma-ku",
                    "ブックマーク",
                    "ni",
                    "ブックマークに追加しておきます。",
                ),
                ("kensaku", "検索", "shitemo", "検索しても見つかりません。"),
                (
                    "kensakukekka",
                    "検索結果",
                    "の",
                    "検索結果の上から順に見ています。",
                ),
                (
                    "saiyomikomi",
                    "再読み込み",
                    "shitara",
                    "再読み込みしたら画像が表示されました。",
                ),
                (
                    "roguin",
                    "ログイン",
                    "dekitanode",
                    "ログインできたので、続きを確認します。",
                ),
                (
                    "komento",
                    "コメント",
                    "NI",
                    "コメントに補足を書いておきます。",
                ),
                ("gazou", "画像", "ga", "画像がまだ表示されません。"),
                (
                    "koukaihan'i",
                    "公開範囲",
                    "ha",
                    "公開範囲は友達だけにしています。",
                ),
            ] {
                let mut ime = engine(reading);
                ime.select_candidate(index(&ime, word, CandidateKind::Word));
                assert_eq!(ime.selected_completion_text(true), Some(word));
                assert!(ime.snapshot().committed_text.is_empty());
                let spaced = format!("{word} ");
                ime.seed(&spaced);
                draft(&ime, &spaced);
                index(&ime, sentence, CandidateKind::Sentence);
                let continued = format!("{word} {tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                commit_once(&mut ime, sentence, CandidateKind::Sentence);
                commit_once(&mut engine(&continued), &continued, CandidateKind::Literal);
                ime.seed(reading);
                draft(&ime, reading);
                index(&ime, word, CandidateKind::Word);
            }
        },
    );
}

#[test]
fn internet_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact() {
    fresh(
        "internet_japanese_protected_literals_unknown_tails_and_long_drafts_remain_exact",
        |fixture| {
            fixture.initialize(true, false);
            let mut seeds: Vec<String> = [
                "https://example.invalid/kensaku?q=rinku",
                "roguin@example.invalid",
                "let kensaku = \"rinku\";",
                "C:/files/daunro-do.txt",
                "xyzkensaku",
                "🙂検索shitemo",
                "画像gaxyz",
                "ログインをが",
                "コメント{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("ken{boundary}saku"));
                seeds.push(format!("検索{boundary}shitemo"));
            }
            for tail in ["kensaku", "ログインdekitanode", "画像ga"] {
                let seed = format!("{}{tail}", "前文。".repeat(90));
                assert!(seed.chars().count() > 256);
                seeds.push(seed);
            }
            for seed in seeds {
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                assert!(
                    ime.candidates().iter().all(|c| !SENTENCES
                        .iter()
                        .flat_map(|(_, ss)| ss)
                        .any(|s| c.text.ends_with(s))),
                    "manufactured authored sentence from {seed:?}: {:?}",
                    ime.candidates()
                );
                commit_once(&mut ime, &seed, CandidateKind::Literal);
            }
        },
    );
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-internet-synthetic-failure"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.0.clone().map_or_else(|| Ok(Vec::new()), Err)
    }
}

#[test]
fn internet_japanese_fallback_survives_typed_and_empty_model_failures() {
    fresh(
        "internet_japanese_fallback_survives_typed_and_empty_model_failures",
        |fixture| {
            fixture.initialize(true, false);
            for error in [
                Some(LlmProviderError::NoLocalModel),
                Some(LlmProviderError::Unavailable),
                Some(LlmProviderError::Timeout),
                Some(LlmProviderError::HttpStatus(503)),
                None,
            ] {
                for (seed, word, sentence) in [
                    ("kensaku", "検索", "検索しても見つかりません。"),
                    (
                        "daunro-do",
                        "ダウンロード",
                        "ダウンロードが終わるまで待っています。",
                    ),
                    (
                        "koukaihan'i",
                        "公開範囲",
                        "公開範囲を確認してから投稿します。",
                    ),
                ] {
                    let mut ime = engine("");
                    ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
                    ime.seed(seed);
                    draft(&ime, seed);
                    index(&ime, word, CandidateKind::Word);
                    index(&ime, sentence, CandidateKind::Sentence);
                    let before = ime.candidates().to_vec();
                    let deadline = Instant::now() + Duration::from_secs(2);
                    while ime.prediction_pending() && Instant::now() < deadline {
                        ime.poll_prediction();
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                    assert_eq!(
                        ime.prediction_error(),
                        Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
                    );
                    assert_eq!(ime.candidates(), before);
                    commit_once(&mut ime, sentence, CandidateKind::Sentence);
                }
            }
        },
    );
}

#[test]
fn internet_japanese_coexists_with_fixed_five_packs_and_preserves_prior_japanese_order() {
    fresh(
        "internet_japanese_coexists_with_fixed_five_packs_and_preserves_prior_japanese_order",
        |fixture| {
            fixture.initialize(true, true);
            complete_workflows();
            let builtin = lexicon::builtin("ja").unwrap();
            let active = lexicon::active("ja").unwrap();
            let mut offset = builtin.readings().len();
            for id in ["org.suzaku.ja.rail", "org.suzaku.ja.family", PACK_ID] {
                let pack = packs::recommended()
                    .into_iter()
                    .find(|pack| pack.manifest.id == id)
                    .unwrap();
                let end = offset + pack.lexicon.readings().len();
                assert_eq!(pack.lexicon.readings(), &active.readings()[offset..end]);
                offset = end;
            }
            assert_eq!(offset, active.readings().len());
            for (seed, first, other) in [("kanji", "漢字", "感じ"), ("sentaku", "選択", "洗濯")]
            {
                let ime = engine(seed);
                assert_eq!(ime.candidates()[0].text, first);
                let first_position = ime
                    .candidates()
                    .iter()
                    .position(|c| c.text == first)
                    .unwrap();
                let other_position = ime
                    .candidates()
                    .iter()
                    .position(|c| c.text == other)
                    .unwrap();
                assert!(first_position < other_position);
            }
            for (seed, word, sentence) in [
                ("kaisatsu", "改札", "改札はどこですか。"),
                ("kazoku", "家族", "家族で一緒に出かけませんか。"),
                ("madowaku", "窓枠", ""),
                ("sentakumono", "洗濯物", "洗濯物を取り込んでおきます。"),
            ] {
                let mut ime = engine(seed);
                draft(&ime, seed);
                index(&ime, word, CandidateKind::Word);
                if !sentence.is_empty() {
                    commit_once(&mut ime, sentence, CandidateKind::Sentence);
                }
            }
        },
    );
}
