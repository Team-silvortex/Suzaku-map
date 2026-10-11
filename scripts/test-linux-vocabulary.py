#!/usr/bin/env python3
"""Offline EN/ZH/JA vocabulary through XTest, real GTK/IBus and saved text.

Use test-linux-apps.sh vocabulary; never run against the personal desktop.
The shared harness enforces private display/bus/configuration and observes both
the real editor buffer and completed save. No companion-seeded draft is used.
"""
from collections import Counter
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("apps", Path(__file__).with_name("test-linux-apps.py"))
apps = importlib.util.module_from_spec(spec)
spec.loader.exec_module(apps)
assert apps.os.environ["SUZAKU_APP_QA_SUITE"] == "vocabulary"
IBus = apps.IBus

# Writing, daily conversation/devices and model-unavailable fallback vocabulary.
CASES = [
    ("en", "please acknowledge rece", "please acknowledge receipt",
     "please acknowledge receipt of this message."),
    ("en", "please keep me po", "please keep me posted",
     "please keep me posted on your progress."),
    ("en", "can we catch up tom", "can we catch up tomorrow", "can we catch up tomorrow?"),
    ("en", "i should've call", "i should've called", "i should've called earlier."),
    ("en", "is this available in a larg", "is this available in a larger",
     "is this available in a larger size?"),
    ("en", "the battery needs rech", "the battery needs recharging",
     "the battery needs recharging."),
    ("en", "could you speak more slo", "could you speak more slowly",
     "could you speak more slowly?"),
    ("en", "please resend the atta", "please resend the attachment",
     "please resend the attachment."),
    ("en", "the network is temp", "the network is temporarily",
     "the network is temporarily unavailable."),
    ("en", "where is the nearest rest", "where is the nearest restroom",
     "where is the nearest restroom?"),
    ("en", "are you available tom", "are you available tomorrow",
     "are you available tomorrow afternoon?"),
    ("en", "please grant me acc", "please grant me access",
     "please grant me access to this document."),
    ("en", "my flight has been del", "my flight has been delayed",
     "my flight has been delayed."),
    ("en", "does this contain dai", "does this contain dairy", "does this contain dairy?"),
    ("en", "would tomorrow mor", "would tomorrow morning", "would tomorrow morning work for you?"),
    ("en", "when does the subscription exp", "when does the subscription expire",
     "when does the subscription expire?"),
    ("en", "i was charged tw", "i was charged twice", "i was charged twice for this order."),
    ("en", "could you send me a rep", "could you send me a replacement",
     "could you send me a replacement?"),
    ("en", "what does this me", "what does this mean", "what does this mean in this context?"),
    ("en", "how do you pron", "how do you pronounce", "how do you pronounce this word?"),
    ("en", "please save a co", "please save a copy", "please save a copy before closing."),
    ("en", "please turn on the cap", "please turn on the captions", "please turn on the captions."),
    ("zh-Hans", "hui yi ji yao", "会议纪要", "会议纪要已经发到群里了。"),
    ("zh-Hans", "gai'qi", "改期", "改期后的时间我再确认一下。"),
    ("zh-Hans", "zao'an", "早安", "早安，今天也要加油。"),
    ("zh-Hans", "yi hui er", "一会儿", "一会儿见。"),
    ("zh-Hans", "gou wu che", "购物车", "购物车里的商品需要再确认一下。"),
    ("zh-Hans", "chong dian bao", "充电宝", "充电宝需要提前充电。"),
    ("zh-Hans", "qing shao deng", "请稍等", "请稍等，我查一下。"),
    ("zh-Hans", "mei shou dao", "没收到", "没收到，可以重新发一下吗？"),
    ("zh-Hans", "li xian mo shi", "离线模式", "离线模式下仍然可以输入。"),
    ("zh-Hans", "bao liu yuan wen", "保留原文", "保留原文，不要自动替换。"),
    ("zh-Hans", "hui yi lian jie", "会议链接", "会议链接已经发到群里了。"),
    ("zh-Hans", "fang wen quan xian", "访问权限", "访问权限不足，麻烦帮我开通。"),
    ("zh-Hans", "sui shen xing li", "随身行李", "随身行李有重量限制吗？"),
    ("zh-Hans", "gou wu qing dan", "购物清单", "购物清单我已经列好了。"),
    ("zh-Hans", "ming tian xia wu", "明天下午", "明天下午可以安排。"),
    ("zh-Hans", "zi dong xu fei", "自动续费", "自动续费可以关闭吗？"),
    ("zh-Hans", "chong fu kou kuan", "重复扣款", "重复扣款了，请帮我核对。"),
    ("zh-Hans", "dian zi fa piao", "电子发票", "电子发票请发到我的邮箱。"),
    ("zh-Hans", "fa yin", "发音", "发音可以再示范一下吗？"),
    ("zh-Hans", "ju ge li zi", "举个例子", "举个例子会更容易理解。"),
    ("zh-Hans", "ping mu jie tu", "屏幕截图", "屏幕截图稍后发给你。"),
    ("zh-Hans", "zi ti da xiao", "字体大小", "字体大小可以在设置里调整。"),
]

HOME_CASES = [
    ("en", "please close the win", "please close the window", "please close the window before leaving."),
    ("en", "could you lend me a scre", "could you lend me a screwdriver", "could you lend me a screwdriver?"),
    ("zh-Hans", "bei yong yao shi", "备用钥匙", "备用钥匙放在抽屉里。"),
    ("zh-Hans", "shui long tou lou shui", "水龙头漏水", "水龙头漏水了，需要预约维修。"),
]

ERRANDS_CASES = [
    ("en", "please print it on bo", "please print it on both", "please print it on both sides."),
    ("en", "i'd like to renew this bo", "i'd like to renew this book", "i'd like to renew this book."),
    ("zh-Hans", "xu jie tu shu", "续借图书", "续借图书可以在网上办理吗？"),
    ("zh-Hans", "qu jian ma", "取件码", "取件码还没有收到。"),
]
DAILY_CASES = [
    ("en", "that's really imp", "that's really impressive", "that's really impressive."),
    ("en", "i'm almost th", "i'm almost there", "i'm almost there."),
    ("en", "let's take a br", "let's take a break", "let's take a break."),
    ("zh-Hans", "zhen bu cuo", "真不错", "真不错，下次还想再来。"),
    ("zh-Hans", "dao jia le", "到家了", "到家了，给你报个平安。"),
    ("zh-Hans", "xiu xi yi xia", "休息一下", "休息一下，等会儿再继续。"),
]

DAILY_NEEDS_CASES = [
    ("en", "please make it mi", "please make it mild", "please make it mild."),
    ("en", "it's getting co", "it's getting cold", "it's getting cold outside."),
    ("en", "can we change the ti", "can we change the time", "can we change the time?"),
    ("zh-Hans", "shao tang", "少糖", "少糖就好，谢谢。"),
    ("zh-Hans", "dai san", "带伞", "带伞出门，免得淋雨。"),
    ("zh-Hans", "lin shi you shi", "临时有事", "临时有事，可能要晚一点。"),
]

DAILY_COORDINATION_CASES = [
    ("en", "did you get my me", "did you get my message", "did you get my message?"),
    ("en", "could you give me a ha", "could you give me a hand", "could you give me a hand?"),
    ("en", "thanks for getting ba", "thanks for getting back", "thanks for getting back to me."),
    ("zh-Hans", "bang wo kan kan", "帮我看看", "帮我看看有没有漏掉什么。"),
    ("zh-Hans", "fa gei ni le", "发给你了", "发给你了，看看有没有收到。"),
    ("zh-Hans", "xian mang ni de", "先忙你的", "先忙你的，有空再回复就好。"),
]

DAILY_SOCIAL_CASES = [
    ("en", "do you want to grab co", "do you want to grab coffee", "do you want to grab coffee?"),
    ("en", "i hope you're fe", "i hope you're feeling", "i hope you're feeling better."),
    ("en", "i'll have to pass to", "i'll have to pass tonight", "i'll have to pass tonight, but thank you."),
    ("zh-Hans", "zhao gu hao zi ji", "照顾好自己", "照顾好自己，记得按时吃饭。"),
    ("zh-Hans", "wo qu jie ni", "我去接你", "我去接你之前会先给你发消息。"),
    ("zh-Hans", "xie xie yao qing", "谢谢邀请", "谢谢邀请，我看看时间再回复你。"),
]

DAILY_OBJECTS_CASES = [
    ("en", "have you seen my ke", "have you seen my keys", "have you seen my keys?"),
    ("en", "can i borrow your ch", "can i borrow your charger", "can i borrow your charger?"),
    ("en", "There's a spare ch", "There's a spare charger", "There's a spare charger on the table."),
    ("zh-Hans", "fang hui yuan chu", "放回原处", "放回原处之前，记得擦干净。"),
    ("zh-Hans", "gou'bu'gou", "够不够", "够不够，不够我再拿一点。"),
    ("zh-Hans", "hai sheng duo shao", "还剩多少", "还剩多少，我们先数一下。"),
]

JAPANESE_CASES = [
    ("ja", "sumimasen", "すみません", "すみません、もう一度お願いします。"),
    ("ja", "onegaishimasu", "お願いします", "お願いします。終わったら教えてください。"),
    ("ja", "yotei", "予定", "予定が決まったら連絡します。"),
    ("ja", "JYUNBI", "準備", "準備ができたら連絡します。"),
]

FOLLOWUP_CASES = [
    ("en", "i'm having trouble fi", "i'm having trouble finding",
     "i'm having trouble finding the right place."),
    ("en", "we're almost do", "we're almost done", "we're almost done, just one more step."),
    ("zh-Hans", "wo zhe bian hao le", "我这边好了", "我这边好了，你那边怎么样？"),
    ("zh-Hans", "chong xin lai guo", "重新来过", "重新来过也没关系，我们一起试试。"),
    ("ja", "henshin", "返信", "返信ありがとうございます。"),
    ("ja", "junbichuu", "準備中", "準備中です、少しお待ちください。"),
]

LEISURE_CASES = [
    ("en", "what do you like to re", "what do you like to read",
     "what do you like to read in your free time?"),
    ("en", "this weekend i'm planning to re", "this weekend i'm planning to relax",
     "this weekend i'm planning to relax at home."),
    ("zh-Hans", "zhou mo you kong ma", "周末有空吗", "周末有空吗，一起出去走走吧。"),
    ("zh-Hans", "chu qu zou yi zou", "出去走一走", "出去走一走，换个心情也挺好。"),
    ("ja", "sanpo", "散歩", "散歩に行きませんか。"),
    ("ja", "ryouri", "料理", "料理ができたら呼んでください。"),
]

CLARIFICATION_CASES = [
    ("en", "could you put that in si", "could you put that in simpler",
     "could you put that in simpler words?"),
    ("en", "let me say it an", "let me say it another", "let me say it another way."),
    ("zh-Hans", "wo li jie de shi", "我理解的是", "我理解的是这个意思，你看看对不对。"),
    ("zh-Hans", "wo que ren yi xi", "我确认一下", "我确认一下时间，再回复你。"),
    ("ja", "shitsumon", "質問", "質問があれば教えてください。"),
    ("ja", "hosoku", "補足", "補足があれば教えてください。"),
]

COURTESY_CASES = [
    ("en", "thank you for being so pa", "thank you for being so patient",
     "thank you for being so patient with me."),
    ("en", "sorry to keep you wa", "sorry to keep you waiting", "sorry to keep you waiting."),
    ("zh-Hans", "xie xie ni bang mang", "谢谢你帮忙", "谢谢你帮忙确认时间。"),
    ("zh-Hans", "bao qian lai wan le", "抱歉来晚了", "抱歉来晚了，路上耽搁了一会儿。"),
    ("ja", "kansha", "感謝", "感謝の気持ちを伝えたいです。"),
    ("ja", "otesuu", "お手数", "お手数ですが、もう一度教えてください。"),
]

LOCATION_CASES = [
    ("en", "can we walk across the st", "can we walk across the street",
     "can we walk across the street here?"),
    ("en", "i'll wait on the corne", "i'll wait on the corner",
     "i'll wait on the corner by the cafe."),
    ("zh-Hans", "wo zai lou xia", "我在楼下", "我在楼下入口等你。"),
    ("zh-Hans", "zou guo tou le", "走过头了", "走过头了，我们回刚才的路口看看。"),
    ("ja", "chizu", "地図", "地図を見ながら一緒に行きましょう。"),
    ("ja", "menomae", "目の前", "目の前にある入口で待っています。"),
    ("ja", "junbichu", "準備中", "準備中なので、終わったら連絡します。"),
]

TIME_CASES = [
    ("en", "can we move this to to", "can we move this to tomorrow",
     "can we move this to tomorrow morning?"),
    ("en", "please give me another mi", "please give me another minute",
     "please give me another minute to get ready."),
    ("zh-Hans", "nenggaidaomingtianma", "能改到明天吗", "能改到明天吗？今天有点赶。"),
    ("zh-Hans", "daoshihoutixingwo", "到时候提醒我", "到时候提醒我带伞，免得又忘了。"),
    ("ja", "gozen", "午前", "午前の予定を確認しましょう。"),
    ("ja", "kon'ya", "今夜", "今夜の待ち合わせは何時ですか。"),
]
SHOPPING_CASES = [
    ("en", "does this price include ta", "does this price include tax",
     "does this price include tax?"),
    ("en", "can i pay with my ca", "can i pay with my card", "can i pay with my card?"),
    ("zh-Hans", "hai neng pian yi yi dian ma", "还能便宜一点吗", "还能便宜一点吗？可以的话我就买这一件。"),
    ("zh-Hans", "ke yi sao ma ma", "可以扫码吗", "可以扫码吗？我现在付款。"),
    ("ja", "nedan", "値段", "値段を教えてください。"),
    ("ja", "shiharai", "支払い", "支払いはカードでできますか。"),
]
TRANSIT_CASES = [
    ("en", "which platform should we us", "which platform should we use", "which platform should we use?"),
    ("en", "can i buy my ticket on", "can i buy my ticket online", "can i buy my ticket online?"),
    ("zh-Hans", "yao huan cheng ma", "要换乘吗", "要换乘吗？我想先确认路线。"),
    ("zh-Hans", "che piao zen me mai", "车票怎么买", "车票怎么买比较方便？能在手机上买吗？"),
    ("ja", "densha", "電車", "電車は何時に出発しますか。"),
    ("ja", "joushaken", "乗車券", "乗車券はどこで買えますか。"),
]
CONTACT_CASES = [
    ("en", "can you hear me cl", "can you hear me clearly", "can you hear me clearly?"),
    ("en", "please send me your nu", "please send me your number", "please send me your number."),
    ("zh-Hans", "ni neng ting qing ma", "你能听清吗", "你能听清吗？我把声音调大一点。"),
    ("zh-Hans", "huan ge wang luo shi shi", "换个网络试试", "换个网络试试，看看能不能连上。"),
    ("ja", "tsuuwa", "通話", "通話は今できますか。"),
    ("ja", "setsuzoku", "接続", "接続を確認してから送ります。"),
]
# Fixed partial-reading previews are independent of the watched candidate list.
# Full CJK fixtures retain their existing word-first preview expectation.
PARTIAL_PREVIEW_EXPECTATIONS = {
    ("ja", "junbichu"): ("準備ちゅ", "sentence"),
}

PACK_CASES = [
    ("en", "please annotate this para", "please annotate this paragraph",
     "please annotate this paragraph before our discussion."),
    ("zh-Hans", "yue du pi zhu", "阅读批注", "阅读批注请写在页边空白处。"),
    ("en", "please pass me the col", "please pass me the colander",
     "please pass me the colander from the cupboard."),
    ("zh-Hans", "shi cai qing dan", "食材清单", "食材清单先按菜谱整理一下。"),
    ("en", "could you store our lugg", "could you store our luggage",
     "could you store our luggage until this afternoon?"),
    ("zh-Hans", "xing li ji cun", "行李寄存", "行李寄存可以到下午吗？"),
    ("en", "please update the project road", "please update the project roadmap",
     "please update the project roadmap before our next meeting."),
    ("zh-Hans", "xiang mu lu xian tu", "项目路线图", "项目路线图请在下次会议前更新。"),
    ("en", "our grandparents are coming ov", "our grandparents are coming over",
     "our grandparents are coming over this afternoon."),
    ("zh-Hans", "jia ren tuan ju", "家人团聚", "家人团聚的时间先问问大家。"),
    ("ja", "souji", "掃除", "掃除が終わったら休みましょう。"),
] + COURTESY_CASES + LOCATION_CASES + TIME_CASES + SHOPPING_CASES + TRANSIT_CASES + CONTACT_CASES
# A focused diagnostic never replaces the default complete CI gate. Reject
# misspellings rather than silently running no cases or claiming a full pass.
scope = apps.os.environ.get("SUZAKU_VOCABULARY_QA_SCOPE", "all")
if scope not in {"all", "home", "daily", "japanese", "followup", "leisure", "clarification", "courtesy", "location", "time", "shopping", "transit", "contact", "packs", "part-1", "part-2", "part-3"}:
    raise ValueError("SUZAKU_VOCABULARY_QA_SCOPE must be all, home, daily, japanese, followup, leisure, clarification, courtesy, location, time, shopping, transit, contact, packs, part-1, part-2 or part-3")
ALL_CASES = (CASES + HOME_CASES + ERRANDS_CASES + DAILY_CASES + DAILY_NEEDS_CASES
             + DAILY_COORDINATION_CASES + DAILY_SOCIAL_CASES + DAILY_OBJECTS_CASES + JAPANESE_CASES
             + FOLLOWUP_CASES + LEISURE_CASES + CLARIFICATION_CASES + COURTESY_CASES + LOCATION_CASES + TIME_CASES + SHOPPING_CASES + TRANSIT_CASES + CONTACT_CASES)
PART_CASES = {"part-1": ALL_CASES[::3], "part-2": ALL_CASES[1::3], "part-3": ALL_CASES[2::3]}
# Compare whole case tuples, including repeated entries if ever intentional:
# all three partitions preserve the exact full gate, without omissions.
assert Counter(PART_CASES["part-1"] + PART_CASES["part-2"] + PART_CASES["part-3"]) == Counter(ALL_CASES)
assert all(cases and {case[0] for case in cases} == {"en", "zh-Hans", "ja"}
           for cases in PART_CASES.values()), "each vocabulary part must cover all three languages"
CASES = {"home": HOME_CASES,
         "daily": (DAILY_CASES + DAILY_NEEDS_CASES + DAILY_COORDINATION_CASES
                   + DAILY_SOCIAL_CASES + DAILY_OBJECTS_CASES),
         "japanese": JAPANESE_CASES, "followup": FOLLOWUP_CASES,
         "leisure": LEISURE_CASES,
         "clarification": CLARIFICATION_CASES,
         "courtesy": COURTESY_CASES,
         "location": LOCATION_CASES,
         "time": TIME_CASES,
         "shopping": SHOPPING_CASES,
         "transit": TRANSIT_CASES,
         "contact": CONTACT_CASES,
         "packs": PACK_CASES, **PART_CASES}.get(scope, ALL_CASES)


def prepare_packs():
    """Exercise all recommended packs, only in the harness's private data directory."""
    assert apps.os.environ["XDG_DATA_HOME"] == str(apps.root / "data")
    assert not apps.os.environ.get("SUZAKU_LEXICON_DIR")
    tool = apps.bins / "suzaku_tool"
    expected = ["org.suzaku.en.outdoors", "org.suzaku.zh-hans.outdoors", "org.suzaku.ja.rail"]
    expected += [f"org.suzaku.{language}.{topic}" for topic in ("study", "cooking", "travel", "work")
                 for language in ("en", "zh-hans")]
    expected += [f"org.suzaku.{language}.family" for language in ("en", "zh-hans", "ja")]
    for identifier in expected:
        destination = apps.root / (identifier + ".json")
        for arguments in [("export", identifier, str(destination)), ("install", str(destination))]:
            apps.subprocess.run([str(tool), "pack", *arguments], check=True,
                                capture_output=True, text=True, timeout=10)
    result = apps.subprocess.run([str(tool), "pack", "list"], check=True,
                                 capture_output=True, text=True, timeout=10)
    report = json.loads(result.stdout)
    assert not report["next_startup"]["errors"]
    assert [p["manifest"]["id"] for p in report["packages"] if p["enabled"]] == expected
    assert len(report["next_startup"]["loaded"]) == len(expected)
    assert len(expected) == 14
    print("PASS: installed CLI exported and installed all 14 recommended EN/ZH/JA packs into private QA data", flush=True)


def check_vocabulary(bus, x):
    document = apps.root / "suzaku-vocabulary-qa.txt"
    document.write_text("")
    apps.spawn(["gnome-text-editor", "--standalone", str(document)])
    apps.wait(lambda: x.window(document.name), "vocabulary editor window")
    editor_window = x.window(document.name)
    x.focus(editor_window)
    x.click(200, 100)
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    apps.settle_input(bus, x, editor_window)
    apps.prepare_editor(x, document)
    print(f"READY: offline vocabulary scope={scope} in the real GTK editor using {apps.bins}", flush=True)
    panel = apps.spawn([str(apps.bins / "panel")]) if scope == "packs" else None

    for language, reading, word, sentence in CASES:
        apps.clear_document(x, document)
        revision = apps.watch.latest["revision"]
        assert json.loads(apps.command("L" + language))["ok"]
        apps.wait(lambda: apps.watch.latest["revision"] > revision
                  and apps.watch.latest["language"] == language, "vocabulary language applied")
        x.type(reading)
        apps.wait(lambda: apps.seed_is(reading), "physical vocabulary spelling")
        if panel is not None:
            # The companion previews the selected Chinese/Japanese conversion,
            # while the host retains the exact physical Pinyin/Romaji seed.
            # English's first choice is the literal editable reading.
            preview = reading
            if language in {"zh-Hans", "ja"}:
                selected = apps.watch.latest["candidates"][apps.watch.latest["selected"]]
                expected_preview, expected_kind = PARTIAL_PREVIEW_EXPECTATIONS.get(
                    (language, reading), (word, "word"))
                assert selected["text"] == expected_preview and selected["kind"] == expected_kind, (
                    reading, expected_preview, selected)
                assert selected["kind"] != "literal" and selected["source"] == "local"
                preview = selected["text"]
            apps.wait(lambda: apps.companion_frame().get("runtime_font")
                      and apps.companion_frame().get("draft") == preview,
                      "installed companion renders the physical pack draft", timeout=30)
            assert panel.poll() is None and x.focused() == editor_window
        candidates = apps.watch.latest["candidates"][:6]
        for text, kind, label in [(word, "word", "ᵂ"), (sentence, "sentence", "ˢ")]:
            assert any(c["text"] == text and c["kind"] == kind and c["source"] == "local"
                       and label in c["ibus_label"] for c in candidates), (reading, kind, candidates)
            apps.choose_number(x, text)
            apps.save_document(x, document, "")
            x.key(IBus.KEY_BackSpace)
            apps.wait(lambda: apps.seed_is(reading), "exact spelling restored after adoption")

        apps.choose_number(x, word)
        x.key(IBus.KEY_space)
        apps.wait(lambda: apps.seed_is(word + " "), "Space keeps the adopted draft editable")
        if language == "ja":
            # Japanese consumes reading separators in conversion, but keeps the
            # exact spaced literal draft selectable and does not submit on Space.
            assert any(c["text"] == word + " " for c in apps.watch.latest["candidates"])
            assert any(c["text"] == sentence and c["kind"] == "sentence"
                       for c in apps.watch.latest["candidates"][:6]), (reading, apps.watch.latest)
        elif language == "zh-Hans" or sentence.startswith(word + " "):
            remainder = sentence[len(word):]
            if language == "en":
                remainder = remainder.lstrip()
            expected_sentence = word + " " + remainder
            assert any(c["text"] == expected_sentence and c["kind"] == "sentence"
                       and c["source"] == "local" for c in apps.watch.latest["candidates"][:6]), (
                           reading, expected_sentence, apps.watch.latest)
        suffix = {"en": "today", "zh-Hans": "de", "ja": "ga"}[language]
        x.type(suffix)
        apps.wait(lambda: apps.seed_is(word + " " + suffix), "Space continues adopted vocabulary")
        continued = word + {"en": " today", "zh-Hans": " 的", "ja": "が"}[language]
        japanese_progress = language == "ja" and sentence.startswith(continued)
        if japanese_progress:
            assert any(c["text"] == sentence and c["kind"] == "sentence"
                       and c["source"] == "local" for c in apps.watch.latest["candidates"][:6]), (
                           "sentence lost after Japanese particle", reading, apps.watch.latest)
            apps.choose_number(x, sentence)
            apps.save_document(x, document, "")
            x.key(IBus.KEY_BackSpace)
            apps.wait(lambda: apps.seed_is(word + " " + suffix),
                      "Japanese sentence undo preserves the adopted word, space and romaji tail")
        if language != "en":
            apps.choose_number(x, continued)
        apps.save_document(x, document, "")
        apps.commit(x)
        apps.save_document(x, document, continued + "\n")

        # Exercise actual sentence submission too, not only its presence/undo.
        apps.clear_document(x, document)
        x.type(reading)
        apps.wait(lambda: apps.seed_is(reading), "physical spelling before sentence commit")
        if japanese_progress:
            apps.choose_number(x, word)
            x.type(" " + suffix)
            apps.wait(lambda: apps.seed_is(word + " " + suffix),
                      "Japanese physical progress before sentence commit")
        apps.choose_number(x, sentence)
        apps.save_document(x, document, "")
        apps.commit(x)
        apps.save_document(x, document, sentence + "\n")
        print(f"PASS: {language} {reading!r}: word/sentence labels, adoption, undo, Space, exact saved commits",
              flush=True)

    print(f"PASS: all {len(CASES)} offline vocabulary workflows (scope={scope})", flush=True)

    # N58: keep a finite authored sentence through multiple physical word adoptions.
    # No companion-injected draft, actual editor contents observed on every save.
    progress_cases = [
        ("fa yin", "发音", "ke yi", "可以", "zai", "再示范一下吗？"),
        ("shu ru fa", "输入法", "zhi chi", "支持", "", "多种语言。"),
        ("bang wo kan kan", "帮我看看", "you mei you", "有没有", "", "漏掉什么。"),
    ] if scope not in {"part-2", "part-3", "japanese"} else []
    for reading, word, next_reading, next_word, last_reading, remainder in progress_cases:
        apps.clear_document(x, document)
        revision = apps.watch.latest["revision"]
        assert json.loads(apps.command("Lzh-Hans"))["ok"]
        apps.wait(lambda: apps.watch.latest["revision"] > revision, "sentence progress language")
        x.type(reading)
        apps.wait(lambda: apps.seed_is(reading), "sentence progress physical reading")
        apps.choose_number(x, word)
        x.type(" " + next_reading)
        raw = word + " " + next_reading
        apps.wait(lambda: apps.seed_is(raw), "sentence progress second reading")
        converted = word + " " + next_word
        assert any(c["text"] == converted + remainder and c["kind"] == "sentence"
                   for c in apps.watch.latest["candidates"][:6]), apps.watch.latest
        apps.choose_number(x, converted)
        apps.save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        apps.wait(lambda: apps.seed_is(raw), "second-word adoption undo keeps its original spelling")
        apps.choose_number(x, converted)
        x.key(IBus.KEY_space)
        draft = converted + " "
        apps.wait(lambda: apps.seed_is(draft), "sentence progress literal Space")
        sentence = draft + remainder
        assert any(c["text"] == sentence and c["kind"] == "sentence"
                   for c in apps.watch.latest["candidates"][:6]), apps.watch.latest
        if last_reading:
            x.type(last_reading)
            draft += last_reading
            apps.wait(lambda: apps.seed_is(draft), "sentence progress homophone reading")
        apps.choose_number(x, sentence)
        apps.save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        apps.wait(lambda: apps.seed_is(draft), "sentence progress sentence undo")
        apps.choose_number(x, sentence)
        apps.commit(x)
        apps.save_document(x, document, sentence + "\n")
        print(f"PASS: sentence progress {reading!r}: consecutive words, Space, homophones, undo and exact saved sentence",
              flush=True)
    japanese_progress_cases = [
        ("kanryou", "完了", "shitara", "完了したら", "完了したら連絡してください。"),
        ("nihongo", "日本語", "wo benkyoushitai", "日本語を勉強したい",
         "日本語を勉強したいです。"),
    ] if scope in {"all", "part-1", "location", "packs", "japanese"} else []
    for reading, word, tail, converted, sentence in japanese_progress_cases:
        apps.clear_document(x, document)
        revision = apps.watch.latest["revision"]
        assert json.loads(apps.command("Lja"))["ok"]
        apps.wait(lambda: apps.watch.latest["revision"] > revision
                  and apps.watch.latest["language"] == "ja", "Japanese progress language")
        x.type(reading)
        apps.wait(lambda: apps.seed_is(reading), "Japanese progress physical reading")
        apps.choose_number(x, word)
        apps.save_document(x, document, "")
        x.type(tail)
        raw = word + tail
        apps.wait(lambda: apps.seed_is(raw), "Japanese progress preserves the physical romaji tail")
        for attempt in range(2):
            choices = apps.watch.latest["candidates"]
            assert choices[0]["text"] == converted and choices[0]["kind"] == "sentence" \
                and choices[0]["source"] == "local", (
                reading, raw, choices)
            assert any(c["text"] == raw and c["kind"] == "literal" for c in choices)
            assert any(c["text"] == sentence and c["kind"] == "sentence"
                       and c["source"] == "local" and "ˢ" in c["ibus_label"]
                       for c in choices[:6]), (reading, raw, choices)
            apps.choose_number(x, sentence)
            apps.save_document(x, document, "")
            if attempt == 0:
                x.key(IBus.KEY_BackSpace)
                apps.wait(lambda: apps.seed_is(raw), "Japanese sentence undo restores the entire original tail")
                apps.save_document(x, document, "")
        apps.commit(x)
        apps.save_document(x, document, sentence + "\n")
        print(f"PASS: Japanese sentence progress {reading!r}: physical romaji tail, first choice, page-one sentence, exact undo and saved commit",
              flush=True)
    paging_cases = 0
    if scope in {"all", "part-1", "clarification"}:
        # The broad unfinished tail keeps old 这个词 suggestions on page one.
        # Select the authored full-context sentence from page two using only
        # physical PageDown/number keys, then restore its exact raw spelling.
        apps.clear_document(x, document)
        revision = apps.watch.latest["revision"]
        assert json.loads(apps.command("Lzh-Hans"))["ok"]
        apps.wait(lambda: apps.watch.latest["revision"] > revision, "clarification paging language")
        x.type("wo li jie de shi")
        apps.wait(lambda: apps.seed_is("wo li jie de shi"), "clarification physical phrase")
        apps.choose_number(x, "我理解的是")
        x.type("zheg")
        raw = "我理解的是zheg"
        sentence = "我理解的是这个意思，你看看对不对。"
        apps.wait(lambda: apps.seed_is(raw), "clarification physical unfinished tail")
        for attempt in range(2):
            choices = apps.watch.latest["candidates"]
            assert len(choices) == 10 and choices[0]["text"] == "我理解的是这个词"
            assert choices[6]["text"] == sentence and choices[6]["kind"] == "sentence"
            assert choices[6]["source"] == "local" and "ˢ" in choices[6]["ibus_label"]
            assert apps.watch.latest["selected"] == 0
            x.key(IBus.KEY_Page_Down)
            apps.wait(lambda: apps.watch.latest["selected"] == 6, "physical page two selection")
            assert apps.seed_is(raw) and apps.watch.latest["candidates"] == choices
            apps.save_document(x, document, "")
            x.key(IBus.KEY_1)
            apps.wait(lambda: apps.seed_is(sentence), "page two number adopts exact sentence")
            apps.save_document(x, document, "")
            if attempt == 0:
                x.key(IBus.KEY_BackSpace)
                apps.wait(lambda: apps.seed_is(raw), "page two sentence undo restores unfinished tail")
                x.key(IBus.KEY_Page_Up)
                apps.wait(lambda: apps.watch.latest["selected"] == 0, "restore first candidate page")
        apps.commit(x)
        apps.save_document(x, document, sentence + "\n")
        paging_cases = 1
        print("PASS: clarification unfinished-tail page two: old priority, physical paging/number, exact undo and saved sentence", flush=True)
    print(f"PASS: all {len(CASES) + len(progress_cases) + len(japanese_progress_cases) + paging_cases} vocabulary and sentence-progress workflows (scope={scope})", flush=True)


if __name__ == "__main__":
    try:
        if scope == "packs":
            prepare_packs()
        bus, x = apps.start()
        check_vocabulary(bus, x)
    except Exception:
        apps.diagnose()
        raise
    finally:
        apps.close()
