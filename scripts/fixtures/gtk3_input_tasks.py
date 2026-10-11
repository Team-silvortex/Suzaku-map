"""Fixed offline input tasks for the private, strict GTK3 driver.

Importing this module does not start a window, host, model or input session.
run(namespace) expects the existing test-linux-gtk3.py driver namespace, not
test-linux-apps.py alone. Every reading, digit, punctuation mark and adoption
uses that driver's focused physical-key helpers. IPC language preparation is
allowed only with an empty draft; live language changes use the real chord.
This is private TextView acceptance, not a personal desktop or model gate.
"""

TASK_NAMES = (
    "English word/sentence undo, literal digits and punctuation",
    "Long physical English draft, word/sentence adoption and undo",
    "Physical EN/ZH language chords, adoption undo and continuation",
    "Adopted Nihongo alias, partial word and authored grammar tail",
    "Adopted Kanryou alias, untyped grammar tail and literal Space",
    "Adopted Benkyou alias and untyped authored grammar tail",
    "Chinese adoption undo, literal time digits and punctuation editing",
    "One document, explicit EN/ZH/JA commits and physical JA-to-EN return",
)


def _frame(qa):
    return qa.qa.watch.latest


def _draft(qa, expected):
    frame = _frame(qa)
    assert qa.target_is_owned(), "input task lost the owned TextView focus"
    assert frame["focused"] and not frame["private"]
    assert frame["seed"] == expected, (expected, frame)
    qa.expect_buffers()
    choices = frame["candidates"]
    assert 1 <= len(choices) <= 12, (expected, choices)
    assert len({item["text"] for item in choices}) == len(choices)
    assert all(item["source"] == "local" for item in choices), choices
    # Known English raw words can be Word, not Literal. Preserve the exact raw
    # choice without inventing a universal classification for every draft.
    assert any(item["text"] == expected for item in choices), (expected, choices)


def _candidate(qa, expected, kind, primary=False):
    choices = _frame(qa)["candidates"]
    matches = [index for index, item in enumerate(choices[:6])
               if item["text"] == expected and item["kind"] == kind
               and item["source"] == "local"]
    assert matches, (expected, kind, choices)
    if primary:
        assert matches[0] == 0, (expected, kind, choices)


def _choose(qa, expected, kind):
    _candidate(qa, expected, kind)
    slot = next(index for index, item in enumerate(_frame(qa)["candidates"][:6])
                if item["text"] == expected and item["kind"] == kind
                and item["source"] == "local")
    # Literal adoption can preserve the same seed. Observe the owned physical
    # press as well, rather than acknowledging an already-matching draft.
    qa.delivered_key(qa.IBus.KEY_1 + slot, expect=expected)
    _draft(qa, expected)


def _undo(qa, expected):
    qa.key(qa.IBus.KEY_BackSpace, expect=expected)
    _draft(qa, expected)


def _start(qa, language):
    assert _frame(qa)["seed"] == "", "preparation must not replace a live draft"
    assert qa.qa.json.loads(qa.qa.command("S"))["settings"]["llm_enabled"] is False
    qa.language(language)
    qa.clear_buffers()
    assert qa.target_is_owned()
    qa.expect_buffers()


def _switch(qa, language, expected):
    before = _frame(qa)
    # The raw GDK press observer is essential when the draft stays unchanged:
    # an already matching seed alone does not establish physical key delivery.
    qa.delivered_key(qa.IBus.KEY_space, qa.IBus.KEY_Control_L,
                     qa.IBus.KEY_Shift_L, expect=expected)
    qa.wait(lambda: _frame(qa)["revision"] > before["revision"]
            and _frame(qa)["context"] != before["context"]
            and _frame(qa)["language"] == language,
            "physical language chord acknowledged in the owned draft")
    assert qa.qa.json.loads(qa.qa.command("S"))["settings"]["language"] == language
    _draft(qa, expected)


def _digits(qa, digits):
    for digit in digits:
        assert digit in "0123456789"
        expected = _frame(qa)["seed"] + digit
        qa.key(ord(digit), qa.IBus.KEY_Alt_L, expect=expected)
        _draft(qa, expected)


def _symbol(qa, symbol, *modifiers):
    expected = _frame(qa)["seed"] + symbol
    key = {"!": qa.IBus.KEY_1, ":": qa.IBus.KEY_semicolon}.get(symbol, ord(symbol))
    qa.key(key, *modifiers, expect=expected)
    _draft(qa, expected)


def _grammar(qa, raw, progress, kind):
    _draft(qa, raw)
    _candidate(qa, progress, kind, primary=True)
    for forbidden in ("勉強下", "ベンキョウ下", "カンリョウ下", "ベンキョウになりま下"):
        assert all(forbidden not in item["text"] for item in _frame(qa)["candidates"]), (
            raw, forbidden, _frame(qa)["candidates"])


def _english_symbols(qa):
    _start(qa, "en")
    qa.type_text("please sen")
    _choose(qa, "please send", "word")
    _undo(qa, "please sen")
    _choose(qa, "please send", "word")
    qa.type_text(" ")
    _draft(qa, "please send ")
    sentence = "please send me the details."
    _choose(qa, sentence, "sentence")
    _undo(qa, "please send ")
    _choose(qa, sentence, "sentence")
    qa.type_text(" version ")
    _digits(qa, "0")
    _symbol(qa, ".")
    _digits(qa, "8")
    _symbol(qa, ".")
    _digits(qa, "6")
    _symbol(qa, "!", qa.IBus.KEY_Shift_L)
    final = "please send me the details. version 0.8.6!"
    _draft(qa, final)
    # Real editing retires the old sentence-adoption undo; delete one scalar.
    _undo(qa, "please send me the details. version 0.8.6")
    _symbol(qa, "!", qa.IBus.KEY_Shift_L)
    _choose(qa, final, "literal")
    qa.commit(final)


def _long_english(qa):
    _start(qa, "en")
    prefix = "note " * 52
    raw = prefix + "please sen"
    word = prefix + "please send"
    sentence = prefix + "please send me the details."
    assert len(raw) > 256
    # No companion replacement: even the long prefix is physically typed.
    qa.type_text(raw)
    _draft(qa, raw)
    _choose(qa, word, "word")
    _undo(qa, raw)
    _choose(qa, word, "word")
    qa.type_text(" ")
    _draft(qa, word + " ")
    _choose(qa, sentence, "sentence")
    _undo(qa, word + " ")
    _choose(qa, sentence, "sentence")
    qa.commit(sentence)


def _bilingual_shortcuts(qa):
    _start(qa, "en")
    qa.type_text("nihao")
    _switch(qa, "zh-Hans", "nihao")
    _choose(qa, "你好", "word")
    _undo(qa, "nihao")
    _choose(qa, "你好", "word")
    qa.type_text(" de")
    _draft(qa, "你好 de")
    _choose(qa, "你好 的", "sentence")
    qa.commit("你好 的")
    # Preserve the preceding commit in this same document, then switch back
    # while the new raw reading is live. No IPC change replaces that draft.
    qa.type_text("hel")
    _switch(qa, "en", "hel")
    _choose(qa, "hello", "word")
    _undo(qa, "hel")
    _choose(qa, "hello", "word")
    qa.type_text(" world")
    _draft(qa, "hello world")
    qa.commit("hello world")
    assert qa.buffers()[qa.active] == "你好 的hello world"


def _nihongo_tail(qa):
    _start(qa, "ja")
    qa.type_text("nihongo")
    _choose(qa, "ニホンゴ", "word")
    _undo(qa, "nihongo")
    _choose(qa, "ニホンゴ", "word")
    qa.type_text("wobenky")
    _choose(qa, "ニホンゴを勉強", "word")
    _undo(qa, "ニホンゴwobenky")
    _choose(qa, "ニホンゴを勉強", "word")
    qa.type_text("shit")
    _grammar(qa, "ニホンゴを勉強shit", "ニホンゴを勉強しt", "unknown")
    qa.type_text("ai")
    raw, progress = "ニホンゴを勉強shitai", "ニホンゴを勉強したい"
    _grammar(qa, raw, progress, "sentence")
    _choose(qa, progress, "sentence")
    _undo(qa, raw)
    _choose(qa, progress, "sentence")
    qa.commit(progress)


def _kanryou_tail(qa):
    _start(qa, "ja")
    qa.type_text("kanryou")
    _choose(qa, "カンリョウ", "word")
    _undo(qa, "kanryou")
    _choose(qa, "カンリョウ", "word")
    qa.type_text("shit")
    _grammar(qa, "カンリョウshit", "カンリョウしt", "unknown")
    qa.type_text("ara")
    raw, progress = "カンリョウshitara", "カンリョウしたら"
    # Pure Kana remains Unspecified (wire kind "unknown"), not a Sentence.
    _grammar(qa, raw, progress, "unknown")
    _choose(qa, progress, "unknown")
    _undo(qa, raw)
    _choose(qa, progress, "unknown")
    qa.type_text(" ")
    _draft(qa, progress + " ")
    # Conversion consumes reading spaces. Choose the exact raw padding before
    # Enter, rather than pretending its default converted choice retains it.
    _choose(qa, "カンリョウしたら ", "literal")
    qa.commit("カンリョウしたら ")


def _benkyou_tail(qa):
    _start(qa, "ja")
    qa.type_text("benkyou")
    _choose(qa, "ベンキョウ", "word")
    _undo(qa, "benkyou")
    _choose(qa, "ベンキョウ", "word")
    qa.type_text("ninarimashita")
    # The old authored canonical control is 勉強になりました。; only its
    # grammar is protected here. Do not request a newly canonicalized sentence.
    raw, progress = "ベンキョウninarimashita", "ベンキョウになりました"
    _grammar(qa, raw, progress, "unknown")
    _choose(qa, progress, "unknown")
    _undo(qa, raw)
    _choose(qa, progress, "unknown")
    qa.commit(progress)


def _chinese_numbers(qa):
    _start(qa, "zh-Hans")
    qa.type_text("hui yi")
    _choose(qa, "会议", "word")
    _undo(qa, "hui yi")
    _choose(qa, "会议", "word")
    qa.type_text(" ")
    _digits(qa, "12")
    _symbol(qa, ":", qa.IBus.KEY_Shift_L)
    _digits(qa, "30")
    _symbol(qa, "!", qa.IBus.KEY_Shift_L)
    _draft(qa, "会议 12:30!")
    _undo(qa, "会议 12:30")
    _symbol(qa, "!", qa.IBus.KEY_Shift_L)
    _choose(qa, "会议 12:30!", "literal")
    qa.commit("会议 12:30!")


def _three_language_document(qa):
    _start(qa, "en")
    qa.type_text("please sen")
    english = "please send me the details."
    _choose(qa, english, "sentence")
    qa.commit(english)
    # Every IPC language change occurs after a completed explicit commit. The
    # document is deliberately NOT cleared between these three language tasks.
    qa.language("zh-Hans")
    qa.type_text("hui yi")
    chinese = "会议什么时候开始？"
    _choose(qa, chinese, "sentence")
    qa.commit(chinese)
    qa.language("ja")
    qa.type_text("nihongo")
    _choose(qa, "日本語", "word")
    qa.type_text("wo benkyoushitai")
    raw = "日本語wo benkyoushitai"
    _grammar(qa, raw, "日本語を勉強したい", "sentence")
    japanese = "日本語を勉強したいです。"
    _choose(qa, japanese, "sentence")
    _undo(qa, raw)
    _choose(qa, japanese, "sentence")
    qa.commit(japanese)
    qa.type_text("hel")
    _switch(qa, "en", "hel")
    _choose(qa, "hello", "word")
    _undo(qa, "hel")
    _choose(qa, "hello", "word")
    qa.commit("hello")
    assert qa.buffers()[qa.active] == english + chinese + japanese + "hello"


def run(qa):
    """Run eight fixed tasks after the driver's unchanged 24 baseline tasks."""
    tasks = (_english_symbols, _long_english, _bilingual_shortcuts, _nihongo_tail,
             _kanryou_tail, _benkyou_tail, _chinese_numbers, _three_language_document)
    assert len(tasks) == len(TASK_NAMES) == 8
    for label, task in zip(TASK_NAMES, tasks):
        task(qa)
        assert _frame(qa)["seed"] == ""
        assert qa.target_is_owned()
        qa.expect_buffers()
        qa.passed_case("offline input task: " + label)
    assert qa.qa.json.loads(qa.qa.command("S"))["settings"]["llm_enabled"] is False
    return len(tasks)
