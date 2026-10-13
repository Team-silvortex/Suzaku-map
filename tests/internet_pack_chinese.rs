//! Fixed authored examples for the optional Chinese internet pack, not a corpus
//! or real network/model/physical-key benchmark. Every runtime snapshot is private.
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
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{
        self, EntryKind,
        packs::{self, OfflinePack, PackStore},
    },
};

const PACK_ID: &str = "org.suzaku.zh-hans.internet";
const CHILD_CASE: &str = "SUZAKU_INTERNET_CHINESE_CHILD_CASE";

const WORDS: &[(&str, &str)] = &[
    ("liu'lan'qi'biao'qian'ye", "浏览器标签页"),
    ("da'kai'xin'biao'qian'ye", "打开新标签页"),
    ("guan'bi'zhe'ge'biao'qian'ye", "关闭这个标签页"),
    ("qie'hui'shang'yi'ge'ye'mian", "切回上一个页面"),
    ("zhe'ge'wang'ye'da'bu'kai", "这个网页打不开"),
    ("ye'mian'yi'zhi'zai'jia'zai", "页面一直在加载"),
    ("shua'xin'hou'hai'shi'kong'bai", "刷新后还是空白"),
    ("wang'ye'xian'shi'bu'wan'zheng", "网页显示不完整"),
    ("ba'lian'jie'fa'gei'wo", "把链接发给我"),
    ("fu'zhi'wang'ye'di'zhi", "复制网页地址"),
    ("lian'jie'hao'xiang'shi'xiao'le", "链接好像失效了"),
    ("zhe'ge'lian'jie'dian'bu'kai", "这个链接点不开"),
    ("sou'suo'jie'guo'tai'duo'le", "搜索结果太多了"),
    ("huan'ge'guan'jian'ci'shi'shi", "换个关键词试试"),
    ("zhi'kan'zui'jin'de'jie'guo", "只看最近的结果"),
    ("zhao'dao'le'xiang'guan'ye'mian", "找到了相关页面"),
    ("ba'zhe'ge'ye'mian'shou'cang", "把这个页面收藏"),
    ("shou'cang'jia'li'zhao'yi'xia", "收藏夹里找一下"),
    ("qing'li'yi'xia'liu'lan'ji'lu", "清理一下浏览记录"),
    ("xia'zai'dao'na'ge'wen'jian'jia", "下载到哪个文件夹"),
    ("xia'zai'jin'du'bu'dong'le", "下载进度不动了"),
    ("zhe'ge'wen'jian'mei'xia'wan", "这个文件没下完"),
    ("chong'xin'xia'zai'yi'bian", "重新下载一遍"),
    ("shang'chuan'yi'zhi'mei'wan'cheng", "上传一直没完成"),
    ("wen'jian'tai'da'fa'bu'chu'qu", "文件太大发不出去"),
    ("xian'ya'suo'zai'shang'chuan", "先压缩再上传"),
    ("shang'chuan'hou'kan'bu'dao'wen'jian", "上传后看不到文件"),
    ("fu'jian'mei'you'chuan'shang'qu", "附件没有传上去"),
    ("deng'lu'zhuang'tai'guo'qi'le", "登录状态过期了"),
    ("wo'hao'xiang'tui'chu'deng'lu'le", "我好像退出登录了"),
    ("zhe'ge'zhang'hao'hai'zai'xian", "这个账号还在线"),
    ("huan'ge'fang'shi'deng'lu", "换个方式登录"),
    ("yan'zheng'ma'yi'zhi'mei'shou'dao", "验证码一直没收到"),
    ("xian'bu'yao'ji'zhu'mi'ma", "先不要记住密码"),
    ("ping'lun'qu'zai'na'li", "评论区在哪里"),
    ("zhe'tiao'ping'lun'fa'bu'chu'qu", "这条评论发不出去"),
    ("wo'xiang'bu'chong'yi'ju", "我想补充一句"),
    ("xian'kan'kan'da'jia'de'ping'lun", "先看看大家的评论"),
    ("fen'xiang'gei'ji'ge'peng'you", "分享给几个朋友"),
    ("zhe'ge'tie'zi'shou'cang'le", "这个帖子收藏了"),
    ("xiao'xi'ti'xing'tai'duo'le", "消息提醒太多了"),
    ("xian'guan'diao'zhe'ge'ti'xing", "先关掉这个提醒"),
    ("tu'pian'jia'zai'bu'chu'lai", "图片加载不出来"),
    ("shi'pin'yi'zhi'zai'zhuan'quan", "视频一直在转圈"),
    ("sheng'yin'he'hua'mian'dui'bu'shang", "声音和画面对不上"),
    ("qie'dao'quan'ping'kan'kan", "切到全屏看看"),
    ("wang'su'you'dian'man", "网速有点慢"),
    ("deng'wang'luo'hui'fu'zai'shi", "等网络恢复再试"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "打开新标签页",
        [
            "打开新标签页再看看搜索结果。",
            "打开新标签页后，把原来的页面留着。",
        ],
    ),
    (
        "这个网页打不开",
        [
            "这个网页打不开，我晚一点再试。",
            "这个网页打不开，你那边能打开吗？",
        ],
    ),
    (
        "页面一直在加载",
        [
            "页面一直在加载，还没有显示内容。",
            "页面一直在加载，我先看看网络状态。",
        ],
    ),
    (
        "把链接发给我",
        [
            "把链接发给我，我想看看具体内容。",
            "把链接发给我，我稍后用浏览器打开。",
        ],
    ),
    (
        "链接好像失效了",
        [
            "链接好像失效了，你能发个新的地址吗？",
            "链接好像失效了，我这里显示页面不存在。",
        ],
    ),
    (
        "搜索结果太多了",
        [
            "搜索结果太多了，我再加一个关键词。",
            "搜索结果太多了，先看看标题相关的。",
        ],
    ),
    (
        "换个关键词试试",
        [
            "换个关键词试试，也许能找到更合适的结果。",
            "换个关键词试试，这次用完整名称搜索。",
        ],
    ),
    (
        "把这个页面收藏",
        [
            "把这个页面收藏起来，下次方便找。",
            "把这个页面收藏之后，我再关掉标签页。",
        ],
    ),
    (
        "下载到哪个文件夹",
        [
            "下载到哪个文件夹了？我在默认位置没找到。",
            "下载到哪个文件夹比较方便？我想单独放一下。",
        ],
    ),
    (
        "下载进度不动了",
        [
            "下载进度不动了，我先等一会儿。",
            "下载进度不动了，不知道是不是网络断了。",
        ],
    ),
    (
        "这个文件没下完",
        [
            "这个文件没下完，先别关掉浏览器。",
            "这个文件没下完，我稍后再确认进度。",
        ],
    ),
    (
        "上传一直没完成",
        [
            "上传一直没完成，我先保留本地文件。",
            "上传一直没完成，页面还在显示进度。",
        ],
    ),
    (
        "文件太大发不出去",
        [
            "文件太大发不出去，我试试压缩一下。",
            "文件太大发不出去，可以换个分享方式吗？",
        ],
    ),
    (
        "附件没有传上去",
        [
            "附件没有传上去，我再检查一下文件。",
            "附件没有传上去，这条消息先别发。",
        ],
    ),
    (
        "登录状态过期了",
        [
            "登录状态过期了，我需要重新登录。",
            "登录状态过期了，刚才的页面先留着。",
        ],
    ),
    (
        "验证码一直没收到",
        [
            "验证码一直没收到，我先确认接收方式。",
            "验证码一直没收到，过一会儿再看看。",
        ],
    ),
    (
        "这条评论发不出去",
        [
            "这条评论发不出去，我先把文字保存下来。",
            "这条评论发不出去，不知道是不是网络不稳定。",
        ],
    ),
    (
        "我想补充一句",
        [
            "我想补充一句，刚才那个链接已经更新了。",
            "我想补充一句，这个方法在我这里可用。",
        ],
    ),
    (
        "图片加载不出来",
        [
            "图片加载不出来，我只看到了文字。",
            "图片加载不出来，可以再发一次吗？",
        ],
    ),
    (
        "等网络恢复再试",
        [
            "等网络恢复再试，现在先保留草稿。",
            "等网络恢复再试，别急着重复提交。",
        ],
    ),
];

struct Fixture(PathBuf);

impl Fixture {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-internet-chinese-{}-{case}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn in_fresh_process(name: &str, test: impl FnOnce(&Fixture)) {
    if std::env::var(CHILD_CASE).as_deref() == Ok(name) {
        test(&Fixture::new(name));
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_CASE, name)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Chinese internet case {name:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn internet_pack() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == PACK_ID)
        .expect("Chinese internet pack must be available in the optional catalog")
}

fn initialize(fixture: &Fixture, enabled: bool, all_packs: bool) {
    let base = lexicon::builtin("zh-Hans").unwrap();
    let original = serde_json::to_value(base).unwrap();
    let store = fixture.store();
    assert!(store.list().unwrap().is_empty());
    assert!(!store.path().parent().unwrap().exists());
    let expected = if all_packs {
        let selected = packs::recommended()
            .into_iter()
            .filter(|pack| {
                pack.manifest.language == "zh-Hans"
                    || matches!(
                        pack.manifest.id.as_str(),
                        "org.suzaku.ja.rail" | "org.suzaku.ja.family"
                    )
                    || pack.manifest.id == "org.suzaku.en.internet"
            })
            .collect::<Vec<_>>();
        // Eight Chinese packs, two Japanese packs and the English internet pack;
        // Preserve this historical eleven-pack cohort as the catalog grows;
        // the catalog must never silently raise the 16-install limit.
        assert_eq!(selected.len(), 11);
        assert!(selected.len() <= packs::MAX_PACKS);
        selected
    } else {
        vec![internet_pack()]
    };
    for pack in &expected {
        store.install(pack.clone(), false).unwrap();
    }
    if !enabled {
        store.set_enabled(PACK_ID, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    if enabled {
        assert_eq!(report.loaded.len(), expected.len());
        assert!(report.loaded.iter().any(|id| id == PACK_ID));
        assert!(report.disabled.is_empty());
    } else {
        assert!(report.loaded.is_empty());
        assert_eq!(report.disabled, [PACK_ID]);
        assert!(std::ptr::eq(base, lexicon::active("zh-Hans").unwrap()));
    }
    let active = lexicon::active("zh-Hans").unwrap();
    assert_eq!(base.readings(), &active.readings()[..base.readings().len()]);
    assert_eq!(
        base.continuations().collect::<Vec<_>>(),
        active
            .continuations()
            .take(base.continuations().count())
            .collect::<Vec<_>>()
    );
    assert_eq!(serde_json::to_value(base).unwrap(), original);
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.to_owned(),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}

fn reading_for(word: &str) -> &'static str {
    WORDS.iter().find(|(_, text)| *text == word).unwrap().0
}

fn assert_draft(ime: &XRTabletImeEngine, seed: &str) {
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

fn first_page_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = if kind == CandidateKind::Literal {
        ime.candidates()
            .iter()
            .position(|c| c.text == text && c.kind == kind)
            .unwrap()
    } else {
        first_page_index(ime, text, kind)
    };
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert_eq!(second.text, None);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn internet_catalog_has_fixed_novel_readings_and_contexts() {
    let pack = internet_pack();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "zh-Hans");
    assert_eq!(pack.manifest.topics, ["internet", "digital"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    assert_eq!(WORDS.len(), 48);
    assert_eq!(SENTENCES.len(), 20);
    assert_eq!(pack.lexicon.readings().len(), WORDS.len());
    assert_eq!(pack.lexicon.continuations().count(), SENTENCES.len());
    for (entry, &(reading, word)) in pack.lexicon.readings().iter().zip(WORDS) {
        assert_eq!(entry.reading, reading);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, values), &(word, sentences)) in pack.lexicon.continuations().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            sentences
        );
        assert!(sentences.iter().all(|sentence| sentence.starts_with(word)));
    }
    assert_eq!(
        pack.lexicon
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        40
    );
    let base = lexicon::builtin("zh-Hans").unwrap();
    let all = packs::recommended();
    let old = std::iter::once(base).chain(
        all.iter()
            .filter(|other| other.manifest.id != PACK_ID && other.manifest.language == "zh-Hans")
            .map(|other| &other.lexicon),
    );
    for lexicon in old {
        for &(reading, word) in WORDS {
            assert!(
                lexicon
                    .readings()
                    .iter()
                    .all(|entry| entry.reading != reading && entry.text != word)
            );
        }
        for &(context, sentences) in SENTENCES {
            for (old_context, old_sentences) in lexicon.continuations() {
                assert_ne!(context, old_context);
                assert!(
                    sentences
                        .iter()
                        .all(|sentence| old_sentences.iter().all(|old| old != sentence))
                );
            }
        }
    }
}

#[test]
fn disabled_internet_pack_leaves_builtin_and_literal_choices_unchanged() {
    in_fresh_process(
        "disabled_internet_pack_leaves_builtin_and_literal_choices_unchanged",
        |fixture| {
            initialize(fixture, false, false);
            for &(reading, word) in WORDS {
                let seed = reading.replace('\'', " ");
                let mut ime = engine(&seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                assert_draft(&ime, &seed);
                assert!(
                    ime.candidates()
                        .iter()
                        .all(|c| c.text != word || c.kind != CandidateKind::Word)
                );
                commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
            }
            for &(word, sentences) in SENTENCES {
                let ime = engine(word);
                assert!(
                    sentences
                        .iter()
                        .all(|sentence| ime.candidates().iter().all(|c| c.text != *sentence))
                );
            }
        },
    );
}

fn check_word_and_sentence_workflows() {
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
    for &(word, sentences) in SENTENCES {
        for seed in spellings(reading_for(word)) {
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page_index(&ime, sentence, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn internet_full_spellings_offer_word_and_both_sentence_choices() {
    in_fresh_process(
        "internet_full_spellings_offer_word_and_both_sentence_choices",
        |fixture| {
            initialize(fixture, true, false);
            check_word_and_sentence_workflows();
        },
    );
}

fn check_unfinished_readings() {
    // These 48 near-complete prefixes were checked against built-ins and every
    // older pack; this does not promise a new word for short ambiguous prefixes.
    for &(reading, word) in WORDS {
        for seed in spellings(&reading[..reading.len() - 1]) {
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            first_page_index(&ime, word, CandidateKind::Word);
        }
    }
    for &(word, sentences) in SENTENCES {
        let reading = reading_for(word);
        for seed in spellings(&reading[..reading.len() - 1]) {
            let ime = engine(&seed);
            first_page_index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page_index(&ime, sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn internet_near_complete_readings_keep_word_and_sentence_choices() {
    in_fresh_process(
        "internet_near_complete_readings_keep_word_and_sentence_choices",
        |fixture| {
            initialize(fixture, true, false);
            check_unfinished_readings();
        },
    );
}

#[test]
fn adopted_internet_contexts_keep_spacing_and_long_drafts_without_committing() {
    in_fresh_process(
        "adopted_internet_contexts_keep_spacing_and_long_drafts_without_committing",
        |fixture| {
            initialize(fixture, true, false);
            for &(word, sentences) in SENTENCES {
                let mut ime = engine(reading_for(word));
                ime.select_candidate(first_page_index(&ime, word, CandidateKind::Word));
                let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                assert_eq!(adopted, word);
                assert!(ime.snapshot().committed_text.is_empty());
                for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
                    for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
                        let seed = format!("{prefix}{adopted}{padding}");
                        for sentence in sentences {
                            ime.seed(&seed);
                            assert_draft(&ime, &seed);
                            let expected =
                                format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                            commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                        }
                    }
                }
            }
        },
    );
}

#[test]
fn adopted_internet_sentences_continue_with_more_pinyin_or_han_text() {
    in_fresh_process(
        "adopted_internet_sentences_continue_with_more_pinyin_or_han_text",
        |fixture| {
            initialize(fixture, true, false);
            for (word, reading, tail, sentence) in [
                (
                    "把这个页面收藏",
                    None,
                    "起来",
                    "把这个页面收藏起来，下次方便找。",
                ),
                (
                    "下载到哪个文件夹",
                    Some("le"),
                    "了",
                    "下载到哪个文件夹了？我在默认位置没找到。",
                ),
                (
                    "打开新标签页",
                    Some("zai'kan'kan"),
                    "再看看",
                    "打开新标签页再看看搜索结果。",
                ),
            ] {
                for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
                    for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
                        let mut adopted_ime = engine(reading_for(word));
                        adopted_ime.select_candidate(first_page_index(
                            &adopted_ime,
                            word,
                            CandidateKind::Word,
                        ));
                        let adopted = adopted_ime
                            .selected_completion_text(true)
                            .unwrap()
                            .to_owned();
                        assert_eq!(adopted, word);
                        assert!(adopted_ime.snapshot().committed_text.is_empty());
                        for suffix in std::iter::once(tail.to_owned())
                            .chain(reading.into_iter().flat_map(spellings))
                        {
                            let seed = format!("{prefix}{adopted}{padding}{suffix}");
                            let expected = format!(
                                "{prefix}{adopted}{padding}{}",
                                sentence.strip_prefix(word).unwrap()
                            );
                            let mut ime = engine(&seed);
                            assert_draft(&ime, &seed);
                            commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                        }
                    }
                }
            }
        },
    );
}

struct UnavailableProvider(LlmProviderError);

impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "internet-chinese-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Err(self.0.clone())
    }
}

fn check_provider_failures() {
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            (
                "ye mian yi zhi zai jia zai",
                "页面一直在加载",
                "页面一直在加载，还没有显示内容。",
            ),
            (
                "xia zai jin du bu dong l",
                "下载进度不动了",
                "下载进度不动了，我先等一会儿。",
            ),
        ] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(seed);
            assert_draft(&ime, seed);
            first_page_index(&ime, word, CandidateKind::Word);
            first_page_index(&ime, sentence, CandidateKind::Sentence);
            let before = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(ime.prediction_error(), Some(&error));
            assert_eq!(ime.candidates(), before);
            assert_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn internet_choices_survive_typed_provider_failures() {
    in_fresh_process(
        "internet_choices_survive_typed_provider_failures",
        |fixture| {
            initialize(fixture, true, false);
            check_provider_failures();
        },
    );
}

#[test]
fn internet_workflows_coexist_with_eleven_selected_recommended_packs() {
    in_fresh_process(
        "internet_workflows_coexist_with_eleven_selected_recommended_packs",
        |fixture| {
            initialize(fixture, true, true);
            check_word_and_sentence_workflows();
            check_unfinished_readings();
            check_provider_failures();
        },
    );
}

#[test]
fn internet_urls_mail_punctuation_and_split_syllables_preserve_raw_input() {
    in_fresh_process(
        "internet_urls_mail_punctuation_and_split_syllables_preserve_raw_input",
        |fixture| {
            initialize(fixture, true, false);
            for seed in [
                "https://example.invalid/xiazaijindubudongle?keep=1&tag=café",
                "xiazaijindubudongle@example.invalid",
                "https://example.invalid/下载进度不动了jiu",
            ] {
                let mut ime = engine(seed);
                assert_draft(&ime, seed);
                assert_eq!(
                    ime.candidates()
                        .iter()
                        .map(|candidate| candidate.text.as_str())
                        .collect::<Vec<_>>(),
                    [seed],
                    "protected address must not gain a rewritten candidate"
                );
                commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
            }
            for seed in [
                "“raw”  123\t???",
                "xia za i jin du bu dong le",
                "ye mian yi zhi zai ji a zai",
                "da kai xin biao qia n ye",
            ] {
                let mut ime = engine(seed);
                assert_draft(&ime, seed);
                assert!(ime.candidates().iter().all(|candidate| {
                    !SENTENCES
                        .iter()
                        .any(|(_, values)| values.contains(&candidate.text.as_str()))
                }));
                commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
            }
            for boundary in ["\n", "\r", "\u{2028}", "\u{2029}", "，", "。", "！", "？"] {
                let seed = format!("打开新标签页{boundary}zai kan kan");
                let mut ime = engine(&seed);
                assert_draft(&ime, &seed);
                let crossed = format!("打开新标签页{boundary}再看看搜索结果。");
                assert!(
                    ime.candidates()
                        .iter()
                        .all(|candidate| candidate.text != crossed)
                );
                commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
            }
        },
    );
}
