# Suzaku Map · 朱雀

**Published baseline: [0.9.6 / build 46](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.9.6), exact source `c5604aaff8bc6121f318ca846da6f058866c4d25`.** All eight gates of [CI 40056999646](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/40056999646) passed; accepted artifact `12546279679` and all four uploaded Linux assets were independently validated and re-downloaded for byte comparison before publication. Linux Alpha prerelease, not latest stable. The source-preparation pending and earlier uncommitted labels below are historical. The personal installation is unchanged and the cancelled automation remains cancelled.

<!-- release-0.9.7:start -->
**New source candidate: 0.9.7 / build 47 — Everyday dialogue fallback.** Six batches add 57 trilingual contexts / 114 authored sentences, three English forms and one Chinese Word reading. One independent versioned full Rust run passes 2,805 tests / zero failures / 32 original ignores in 301 unfiltered groups including Doc-tests; all 70 new cases occur once. Separately, 19 isolated UI gates and strict all-target/all-feature locked-offline Clippy with warnings denied pass (61.25s main process), not added to the Rust total. Only six data strings were withdrawn to restore old Chinese pools; production algorithms, model protocols, 25 packs/SDK and candidate budgets remain unchanged. Exact-commit remote CI and accepted-package validation remain pending; no 0.9.7 tag, download or publication is claimed. The six uncommitted batch records below remain historical. The personal installation and cancelled automation remain unchanged. See [0.9.7 preparation](docs/releases/0.9.7.md).
<!-- release-0.9.7:end -->

**当前未提交：日常进度反馈（2026-10-24）**。复用既有词，新增三语9语境、18作者句，零新词形/读音；EN6216/915/1794（63层），ZH3442/906/1810，JA638/394/787。独立单次有界回归1194过、0失败、2原ignore，83组零筛除（完整lib+82指定集成目标），11新项只计一次；严格all-target/all-feature Clippy-Dwarnings实际通过（59.61s main process）。旧数据、模型协议、生产算法、25包/SDK、分页配额和0.9.6/46版本不变。仅有限进度表达，不执行任务、发送消息或自动催办；非本轮全仓/Doc-tests/UI/原生/实模/性能/安装/远端CI验收，未提交、不在已发布0.9.6包内，不恢复已取消自动跟进。

**当前未提交：日常澄清与确认（2026-10-24）**。复用既有词，新增三语9语境、18作者句，零新词形/读音；EN6216/912/1788（63层），ZH3442/903/1804，JA638/391/781。独立单次有界回归1319过、0失败、2原ignore，99组零筛除（完整lib+98指定集成目标），11新项只计一次；严格all-target/all-feature Clippy-Dwarnings实际通过（62.53s main process）。旧数据、模型协议、生产算法、25包/SDK、分页配额和0.9.6/46版本不变。仅有限澄清表达，不执行预约、任务或通用问答；非本轮全仓/Doc-tests/UI/原生/实模/性能/安装/远端CI验收，未提交、不在已发布0.9.6包内，不恢复已取消自动跟进。

**当前未提交：日常沟通与安排（2026-10-24）**。复用既有词，新增三语9语境、18作者句，零新词形/读音；EN6216/909/1782（63层），ZH3442/900/1798，JA638/388/775。独立单次有界回归1215过、0失败、2原ignore，83组零筛除（完整lib+82指定集成目标），10新项只计一次；严格all-target/all-feature Clippy-Dwarnings实际通过（71.93s main process）。旧数据、模型协议、生产算法、25包/SDK、分页配额和0.9.6/46版本不变。仅日常沟通表达，不实际发送消息或安排预约；非本轮全仓/Doc-tests/UI/原生/实模/性能/安装/远端CI验收，未提交、不在已发布0.9.6包内，不恢复已取消自动跟进。

**Uncommitted follow-up — small everyday essentials (2026-10-24).** Before 0.10.0, high-frequency authored EN/ZH/JA fallback remains the focus. This batch adds three English forms, `comb`, `handkerchief` and `hairbrush`, plus nine trilingual contexts / 18 authored sentences; no new Chinese/Japanese readings. The existing author-token index registers the three forms, while layer63 `daily_essentials_dialogue.words` stays empty. Totals: EN 6,216 / 906 / 1,776 (63 layers); ZH 3,442 / 897 / 1,792; JA 638 / 385 / 769. One independent bounded run passes 1,311 tests, zero failures and two original explicit ignores in 93 unfiltered groups (complete library plus 92 selected integration targets); the 11 new cases are included once. Strict all-target/all-feature Clippy with warnings denied passes. The retained first bounded run failed one old Japanese replay; only the new sock-author context was replaced by an existing raincoat word before the independent final rerun. Old 6,213 first-seen ranks, production algorithms, model protocols, 25 packs/SDK and version 0.9.6/46 remain unchanged. This is finite location dialogue, not item sensing, advice or device control. No new full-repository, Doc-tests, native/UI, real-model, installation, performance or remote-CI acceptance. The batch is uncommitted and absent from the published 0.9.6 assets.

**Uncommitted follow-up — personal items and everyday accessories (2026-10-24).** Before 0.10.0, authored EN/ZH/JA fallback coverage remains the main development focus. This batch adds 12 contexts / 24 authored sentences, with no new word forms or readings: EN 6,213 forms / 63 layers / 903 contexts / 1,770 sentences; ZH 3,442 / 894 / 1,786; JA 638 / 382 / 763. One independent bounded run passes 1,135 tests, zero failures and two original explicit ignores in 70 unfiltered groups (complete library plus 69 selected integration targets); the 11 new cases are included once. Strict all-target/all-feature Clippy with warnings denied passes. Production algorithms, model protocols, 25 packs/SDK and version 0.9.6/46 are unchanged. This is finite text, not item sensing or accessory advice. No new full-repository, Doc-tests, native/UI, real-model, installation, performance or remote-CI acceptance. The batch is uncommitted and absent from the published 0.9.6 assets.

**Uncommitted follow-up — living-room small items (2026-10-24).** Adds one Chinese Word reading (`yao'kong'qi → 遥控器`) and nine contexts / 18 authored sentences across EN/ZH/JA. English reuses all 6,213 forms: 62 layers / 897 contexts / 1,758 sentences; Chinese 3,442 readings / 891 contexts / 1,780 sentences; Japanese 638 readings / 379 contexts / 757 continuations. One independent bounded run passes 1,093 tests, zero failures and two original explicit ignores in 66 unfiltered groups (complete library plus 65 selected integration targets). The 12 new cases are included once; strict all-target/all-feature Clippy with warnings denied passes. Old data, historical windows/hashes, production algorithms, model protocols, 25 packs/SDK and version 0.9.6/46 remain unchanged except the explicit data append and current test-total maintenance. This is finite text, not item sensing or device/battery advice. No new full-repository, Doc-tests, native/UI, real-model, installation, performance or remote-CI acceptance. The batch is not in the published 0.9.6 assets.

<!-- release-0.9.6:start -->
**New source candidate: 0.9.6 / build 46 — Everyday fallback and candidate continuity.** This includes six vocabulary batches (20 English builtin forms, 31 Chinese and 16 Japanese Word readings, 81 contexts and 162 authored sentences) plus three bounded Chinese/Japanese candidate-compatibility repairs. The independent versioned full Rust run passes 2,735 tests / zero failures / 32 original ignores in 294 unfiltered groups including Doc-tests. Separately, 19 exact isolated UI gates and strict all-target/all-feature Clippy with warnings denied (58.69s) pass. Exact-commit remote CI and accepted-package validation remain pending; no 0.9.6 tag, download or publication is claimed yet. The cancelled automation stays cancelled and the personal installation is unchanged. Earlier source/uncommitted/pending records below remain historical. See [0.9.6 release preparation](docs/releases/0.9.6.md).
<!-- release-0.9.6:end -->

A local-first, continuous-writing input method with word and sentence candidates,
optional language models, and a compact floating companion panel.

**[0.9.5 · Linux / IBus Alpha prerelease](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.9.5), not latest stable.** Current validation targets **Ubuntu 24.04,
amd64, IBus, X11 or GNOME with XWayland**. Keep your usual input method available
as a fallback. Other platforms remain experimental.

**Published baseline: 0.9.5 / build 45, source `de42d39`.** The exact-commit
[CI run 39894489515](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/39894489515)
passed all eight required gates. Accepted artifact `12476299897` supplied the Linux
`.tar.gz`, `.deb` and their two `.sha256` files; the four release assets were independently
validated and re-downloaded for byte comparison before publication. This is a non-latest
Linux Alpha prerelease; the personal installation was not changed and the cancelled
publication automation was not restored. Preparation-time pending statuses below are historical,
not the current publication status. See [0.9.5 preparation scope](docs/releases/0.9.5.md).

**Uncommitted follow-up — thermos and table-item locations (2026-10-24).** Adds nine contexts and 18 authored sentences across EN/ZH/JA, with no new word forms/readings. Totals: EN 6,213 / 894 / 1,752 (61 layers); ZH 3,441 / 888 / 1,774; JA 638 / 376 / 751. One independent bounded run passes 1021 tests, zero failures and two original ignores, in 60 unfiltered groups (complete library plus 59 integration targets). Strict all-target/all-feature Clippy passes with warnings denied (1m 07s). New tests share a test-only helper; old suites are not migrated. Production algorithms, candidate budgets, models/protocols, 25 packs/SDK and versions stay unchanged. These are finite location expressions, not item sensing/control. Not full-repository, native, UI, real-model, installation, performance or CI acceptance. Earlier records remain historical.

**Uncommitted follow-up — wallet, coins and receipt locations (2026-10-24).** Adds nine contexts and 18 authored sentences across English, Chinese and Japanese without new word forms/readings. Totals: EN 6,213 / 891 / 1,746 (60 layers); ZH 3,441 / 885 / 1,768; JA 638 / 373 / 745. The independent bounded run passes 983 tests, zero failures and two original ignores, across 55 unfiltered groups (complete library plus 54 integration targets). Strict all-target/all-feature Clippy passes with warnings denied (1m 06s). This is finite find-item text, not payment processing or financial advice. Production algorithms, model protocols, candidate budgets and 25 packs/SDK stay unchanged. No new full-repository, Doc-tests, native, real-model, installation or CI acceptance. Earlier snapshots below remain historical.

**Uncommitted follow-up — washroom item locations (2026-10-24).** Adds nine contexts and 18 authored sentences across English, Chinese and Japanese, reusing existing words/readings. Totals: EN 6,213 forms / 888 contexts / 1,740 sentences (59 layers); ZH 3,441 / 882 / 1,762; JA 638 / 370 / 739. The independent bounded run passes 929 tests, with zero failures and two original ignores, across 48 unfiltered groups (complete library plus 47 integration targets). Strict all-target/all-feature Clippy passes with warnings denied (18.67s). This is not a new full-repository, Doc-tests, physical-keyboard, real-model or installation acceptance. No production algorithm, model protocol, pack/SDK byte or version changes. Earlier snapshots below remain historical.

<!-- entryway-validation-closeout:start -->

**Validation closeout — 2026-10-24 (uncommitted).** The fixed compatibility matrix passes **805 tests / zero failures / two original ignores**, across 20 unfiltered groups. All 44 new checks are included once. Chinese recovery excludes already-reachable authors and unsupported readings; Japanese nasal-parent preservation also handles literal Kana forms, only at the default twelve-row capacity. The earlier 22-failure complete run and exit-143 interruption remain recorded, not acceptance. The independent complete run passes 2,705 tests, zero failures and 32 original ignores, across 291 unique unfiltered groups including Doc-tests. Strict all-target/all-feature Clippy passes with warnings denied (55.99s). No version, commit, push, personal installation or automation change. The preparation snapshots below retain their earlier validation scope.

<!-- entryway-validation-closeout:end -->

**Current uncommitted follow-up: entryway and leaving home.** Adds six English forms, nine Chinese/five Japanese Word readings,
18 contexts and36 authored sentences from the accessory-inclusive970-file dirty snapshot, not clean HEAD.
Totals: **EN6,213/885/1,734 (58 layers), ZH3,441/879/1,756, JA638/367/733**; existing carry vocabulary and optional-pack words are not counted as globally new.
Japanese “鍵の置き場所” composes old Words; `teikiire` retains both adjacent vowels. This is finite find-item dialogue, not lock/weather/location control.
Two narrow Chinese repairs preserve verified adopted-syllable authors and a uniquely sourced terminal-`ng` pair; they do not add arbitrary prediction or polyphone inference.
Real failing before/after/repair runs remain saved; the four original entryway specifications and author wording are unchanged.
Focused acceptance passes **647 tests / zero failures / two original ignores**, across7 unfiltered groups (library plus6 targets); all35 new checks are included once.
Fresh independent complete regression and strict Clippy are still pending; older2642/full or735/bounded receipts are not this batch's final acceptance.
Version0.9.5/45 stays uncommitted/outside published packages; no installation, CI, automation, native/model or performance acceptance is added.
See the [entryway work record](DEVELOPMENT.md#daily-entryway-and-leaving-home--2026-10-23).

The accessory block below retains the preceding snapshot's scope and receipts, not the latest entryway totals or acceptance.

**Current uncommitted follow-up: desk accessories and cable storage.** Adds six English forms,
10 Chinese and five Japanese Word readings,18 contexts and36 authored sentences from the recycling-inclusive dirty snapshot.
Totals: **EN6,207/879/1,722 (57 layers), ZH3,432/873/1,744, JA633/361/721**.
Existing chargers/adapters/cases are reused; Japanese “充電器の置き場所” is a composed context, not a new dictionary Word.
This is finite location/packing dialogue, not electrical operations or advice; runtime/model logic, budgets and25 packs/SDK stay fixed.
The real before run found missing candidates; the first after run has18passes/one English failure at the broad `connec` root.
Only two new test tuples narrow that probe to `connecto`; old ranks/quotas and the failed log are retained.
The new bounded run passes **735 tests / zero failures / three original ignores**, across23 unfiltered groups: complete library plus22 integration targets.
Strict Clippy (58.85s), formatting,83 Python guards,250 links,tensor and preservation checks pass; this is not a new full-workspace run.
Version0.9.5/45 remains uncommitted/outside published packages; no installation, CI, automation, UI/native/model or performance acceptance is added.
See the [accessory work record](DEVELOPMENT.md#daily-desk-accessories-and-cable-storage--2026-10-23).

The recycling block below retains the previous snapshot's totals and validation, not the latest accessory batch's results.

**Current uncommitted follow-up: recycling and household coordination.** This finite
data-only batch adds **eight builtin English forms, 12 Chinese and six Japanese Word
readings, 18 contexts and 36 trilingual authored sentences**. `bins` already existed in
the optional English family pack and is counted only as newly builtin. Examples include
`cardboard / bin liners`, “纸箱先压平／回收日是哪天” and “段ボール／収集日／ペットボトル”.
Current totals are **EN 6,201 / 873 / 1,710 (56 layers)**, **ZH 3,422 / 867 / 1,732**,
**JA 628 / 355 / 709** (forms/readings, contexts, sentences). Source version/build remains
0.9.5/45; this follow-up is not in the published packages. Model/runtime algorithms, budgets,
25 recommended packs and SDK are unchanged. These are input expressions, not a collection
service, local sorting rules or a promise that all short-root candidate pools are unchanged.
The tests-first run recorded missing candidates in all three languages and the private-pack
workflow. The first focused run passed all 19 new tests but also exposed one stale current-total
assertion; that failure is retained, not labeled an all-green run. The first complete run then
found two historical English replay mismatches: the final token of `one\nline` now also
completes to `liner / liners`. A test-only projection checks the entire measured five-row
vector before removing those two added tail rows; the old corpus/goldens and runtime behavior
are unchanged. The fresh independent full run now passes **2,642 tests / zero failures /
32 original ignores**, across279 unfiltered groups including Doc-tests, with all19 new tests
included once. Strict all-target/all-feature Clippy with warnings denied (1m01s), formatting,
83 Python guards,250 audit links and source preservation checks pass. The earlier failed runs
remain recorded; these results do not add new UI/native-keyboard, real-model, installation or
performance acceptance. No installation, CI run or automation is started for this batch.
See the [recycling work record](DEVELOPMENT.md#daily-recycling-and-household-coordination--2026-10-23).

### Historical release and source-preparation snapshots

The following baseline/candidate and uncommitted labels retain their original development-time
scope. The kitchen and bedding batches are now included in published 0.9.5; the newer recycling
batch above is not. Their old local counts are not new-batch or remote-release acceptance.

**Published baseline: 0.9.3 / build 43.** It includes five everyday-vocabulary batches,
immutable-catalog query indexes and Japanese sentence-slot repairs. Its Linux release
and four package/checksum assets are published; this does not update the personal installation.
The **2,456 Rust tests**, **19 isolated UI checks** and strict Clippy recorded during
preparation remain historical local evidence, separate from exact-commit release acceptance.
See [0.9.3 preparation and release gates](docs/releases/0.9.3.md). Earlier
"uncommitted/unreleased" paragraphs retain their development-time scope; those five
follow-ups are now in 0.9.3. Its assets do not contain the newer batches below.

**New source candidate: 0.9.5 / build 45 — Kitchen and bedtime fallback.**
It includes the two kitchen/bedding batches after the0.9.4 source baseline `d2b0a1a`:
**14 builtin English forms,28 CJK Word readings,30 contexts and60 authored sentences**.
Current totals are **EN6,193/867/1,698 (55 layers)**, **ZH3,410/861/1,720** and
**JA622/349/697**. Runtime/model logic, earlier ranks/data,25 packs/SDK and all budgets
are unchanged; bounded short-root candidate pools may still change as vocabulary grows.
The new independent versioned full run passes **2,623 tests /zero failures /32 original ignores**,
across275 unfiltered groups including Doc-tests;83 Python guards,250 links and preservation checks pass.
Separately, **19 exact isolated UI gates** and strict all-target/all-feature Clippy
(1m03s, warnings denied) pass. Exact-commit CI/accepted-four-package checks remain pending;
the19 UI checks are not added to the full total.
Only after acceptance may the four Linux assets be published as an Ubuntu24.04 amd64/glibc2.39
Alpha prerelease, not latest. This does not restore the cancelled automation or update the
personal installation. See [0.9.5 scope and gates](docs/releases/0.9.5.md).
The source-candidate headings and uncommitted statuses below retain their historical scope;
the earlier2623/52.51s/419 receipts are not this new version's acceptance.

**Current source candidate: 0.9.4 / build 44 — Everyday spaces and candidate continuity.**
The dressing, room-comfort, stationery, reading and plants batches are now included in this
candidate: **28 English forms, 82 CJK Word readings, 90 contexts and 180 authored sentences**.
It also includes bounded Japanese nasal-progress/spare-word repairs and the offline catalog's
per-source versus merged-layer budget correction. The 25 recommended packs, SDK, model contract,
six-row page size and default twelve-row candidate list are unchanged.
The versioned complete Rust run passes **2,585 tests /zero failures /32 original ignores**,
across267 unfiltered groups. Separately, **19/19 isolated UI checks**, strict Clippy and source
preservation checks pass. The exact new commit must still pass every required CI
and accepted-package check before its four Linux assets can be published as a non-latest Alpha
prerelease. There is no new 0.9.4 download or personal-installation update yet.
See [0.9.4 scope and release gates](docs/releases/0.9.4.md).

**Current uncommitted follow-up: bedding and bedtime preparation.**
After the kitchen-inclusive working snapshot, this finite batch adds **six builtin English
forms, eight Chinese and three Japanese Word readings, 12 contexts and 24 trilingual authored
sentences**. Examples include `sheet / pillows / bedside`, “枕头／被子／床单换好了吗” and
“シーツ／毛布／パジャマ”; the existing 目覚まし Word is reused, not counted again.
Current totals are **EN 6,193 / 867 / 1,698 (55 layers)**, **ZH 3,410 / 861 / 1,720**,
**JA 622 / 349 / 697** (forms/readings, contexts, sentences). This is everyday input vocabulary,
not medical advice, sleep-effect guidance, alarm control or appliance actions. Version0.9.4/44,
runtime logic, model contracts,25 packs/SDK and all budgets are unchanged. No commit, installation,
CI query or resumed publication automation is part of this work. This batch's own tests-first
evidence and validation are in the [bedding work record](DEVELOPMENT.md#daily-bedding-and-bedtime-preparation--2026-10-23),
separate from the completed kitchen and earlier snapshots below.
The independent complete all-features run passes **2,623 tests /zero failures /32 original
ignores**, across275 unfiltered groups including Doc-tests; all19 new checks are included once.
Strict all-target/all-feature Clippy passes with warnings denied (52.51s), alongside formatting,
83 Python guards,250 audit links and preservation checks. This does not imply every short-prefix
pool is unchanged or new UI/native/model/install/performance acceptance.

**Uncommitted follow-up after `d2b0a1a`: kitchen preparation, tableware and clearing up.**
The finite batch adds **eight builtin English forms, 12 Chinese and five Japanese Word
readings, 18 contexts and 36 trilingual authored sentences**. Examples include `bowls`,
`cutting`, “我先把菜洗好／菜板／筷架” and “スプーン／フライ返し／水切りかご”; existing
筷子 and 箸 are retained, not counted again. Current totals are **EN 6,187 / 863 / 1,690
(54 layers)**, **ZH 3,402 / 857 / 1,712**, **JA 619 / 345 / 689** (forms/readings,
contexts, sentences). These are everyday input expressions, not recipes, health advice or
actions on kitchen appliances. Version/build stays0.9.4/44; runtime logic, model contracts,
25 packs/SDK and budgets are unchanged. No commit, installation or resumed release follow-up
is part of this work. See the [kitchen work record](DEVELOPMENT.md#daily-kitchen-preparation-tableware-and-clearing-up--2026-10-23)
for this batch's own tests-first evidence and validation, not the older results below.
The independent complete all-features run passes **2,604 tests /zero failures /32 original
ignores**, across271 unfiltered groups; all19 new checks are included once. Strict
all-target/all-feature Clippy passes with warnings denied (50.42s). This is not new
UI/native-keyboard, real-model, performance-timing or installation acceptance.

### Development-time batch snapshots included in 0.9.4

The following “uncommitted” statuses and 0.9.3/43 checks retain their original snapshot scope;
they are not versioned 0.9.4, remote CI, package or installation acceptance.

**Current uncommitted follow-up: balcony plants and everyday plant-related dialogue.**
Adds **eight builtin English forms, 12 Chinese and five Japanese Word readings, 18 contexts
and 36 trilingual authored sentences**. Examples include “花盆／叶子／帮我拿一下喷壶” and
“植木鉢／水やり／窓辺”. Current totals are **EN 6,179 / 857 / 1,678 (53 layers)**,
**ZH 3,390 / 851 / 1,700**, **JA 614 / 339 / 677** (forms/readings, contexts, sentences).
These are finite input expressions, not horticultural guidance, plant sensing or automatic watering.
Existing watering/balcony words are retained, not counted again; new entries can change bounded
short-prefix choices. The 25 packs/SDK, model contract and budgets are unchanged. A narrow Japanese
default-twelve-row spare-slot rule restores a proven parent Word without changing earlier rows,
raw input or slot metadata; smaller/larger limits retain their old behavior.
Version/build remains 0.9.3/43, with no commit, installation or package change.
The independent complete all-features rerun passes **2,585 tests /zero failures /32 original
explicit ignores**, across267 unfiltered groups including Doc-tests; all24 new checks are included
once. Strict all-target/all-feature Clippy passes (51.34s), as do formatting,83 Python guards,
250 audit links and preservation checks. The two failed earlier runs remain recorded; this is not
new native/UI/model-quality, latency or installation acceptance.
See the [plants work record](DEVELOPMENT.md#daily-plants-and-balcony-dialogue--2026-10-20)
for this batch's own validation; the reading/stationery results below retain their earlier scope.

**Current uncommitted follow-up: reading, book pages and bookmarks.**
Adds **four English forms, 12 Chinese and four Japanese Word readings, 18 contexts
and 36 trilingual authored sentences**. Examples include `bookmark`, “书签／这页还没看完”
and “栞／表紙／本棚”. Current totals are **EN 6,171 / 851 / 1,666 (52 layers)**,
**ZH 3,378 / 845 / 1,688**, **JA 609 / 333 / 665** (forms/readings, contexts, sentences).
This is finite fallback vocabulary, not book content, reading-state tracking or file actions.
Old reading order/flags and English ranks, 25 packs/SDK, model contracts and budgets are unchanged;
new compound entries can change bounded short-prefix choices. Version/build remains 0.9.3/43,
without commit, installation or package changes. The independent complete all-features run passes
**2,561 tests /zero failures /32 existing explicit ignores**, across262 unfiltered groups including
Doc-tests; all19 new checks are included once. Strict all-target/all-feature Clippy passes
with warnings denied (50.58s); formatting,83 Python guards,250 audit links and preservation checks pass.
Detailed validation is recorded separately
in the [reading work record](DEVELOPMENT.md#daily-reading-pages-and-bookmarks--2026-10-20);
the completed stationery/room results below are historical, not this batch's acceptance.

**Current uncommitted follow-up: paper, stationery and everyday writing.**
Adds **eight English forms, 12 Chinese and four Japanese Word readings, 18 contexts
and 36 trilingual authored sentences**. Examples include `inside my pencil case i keep`,
“铅笔有点钝了／这个字写错了／先拿张草稿纸” and “付箋／定規／筆箱”.
Current totals are **EN 6,167 / 845 / 1,654 (51 layers)**, **ZH 3,366 / 839 / 1,676**,
**JA 605 / 327 / 653** (forms/readings, contexts, sentences). Existing pen, paper and writing
entries are not counted as new; bounded short-prefix competition can replace old generated choices.
This is finite fallback data, not handwriting recognition,
open-ended composition or actions on notes/files. Model contracts,25 packs/SDK and all budgets
are unchanged; version/build remains0.9.3/43, with no commit, release or installation change.
After the documented new-test oracle corrections, an independent complete all-features run
passes **2,542 tests / zero failures / 32 existing explicit ignores**, across258 unfiltered
top-level groups including Doc-tests; all24 new checks are counted once. This is not the first
23/1 focused run plus six English checks combined. Strict all-target/all-feature Clippy passes
(49.91s), as do formatting,83 Python guards,250 audit links and source-schema checks; these are
not new native/UI/model/performance/installation tests. Details are in the
[stationery work record](DEVELOPMENT.md#daily-stationery-and-writing--2026-10-20).
The room/dressing totals and results below retain their earlier snapshot scope, not this batch's acceptance.

**Current uncommitted follow-up: indoor light, ventilation and room comfort.**
After the dressing snapshot, it adds **eight English forms, 12 Chinese and five Japanese
Word readings, 18 contexts and 36 trilingual authored sentences**. Examples include
`the air in this room feels stuffy`, “开窗通通风／把窗帘拉开／灯光有点刺眼” and
“照明／カーテン／風通し”. Totals are **EN 6,159 / 839 / 1,642 (50 layers)**,
**ZH 3,354 / 833 / 1,664**, **JA 601 / 321 / 641** (forms/readings, contexts, sentences).
These are finite input expressions, not ventilation advice, sensing or device controls.
The original forms/readings, 25 optional packs/SDK, model contract and list budgets remain protected.
The first five-target run records **23 passes and one actual Japanese first-page failure**:
`kankise` reaches 換気扇, but its second authored sentence is at index eight. A narrow
authored-stem proof fixes it while preserving older ticket pagination. A separate bounded
merge fix retains all16 pack slots without raising single-source or per-pack limits.
The independent complete all-features rerun passes **2,518 tests / zero failures / 32 existing
explicit ignores**, across253 unfiltered top-level groups, including all38 new checks once.
Both surfaced old regressions pass; focused93 and captured child output are not added again.
Strict all-target/all-feature Clippy passes (49.21s), as do formatting,83 Python guards,
250 source-audit links and exact preservation checks; these are not new native/UI/model/install tests.
This data is not in published 0.9.3 or the personal installation. Version/build is unchanged;
see the [current work and retained failure](DEVELOPMENT.md#daily-room-comfort-and-terminal-nasal-completion--2026-10-20).
Earlier batch totals and test counts below describe their own snapshots, not this current batch.

**Uncommitted follow-up after `6edfa3e`: daily dressing, fit and accessories.**
Adds **12 Chinese and four Japanese Word readings, 18 contexts and 36 trilingual
authored sentences**, reusing all 6,151 English forms without changing their ranks.
Examples include `my shirt feels tight`, “拉链卡住了／鞋带又松了／裤腰有点紧” and
`fasuna-` → “ファスナー”. Totals are now **EN 6,151 / 833 / 1,630 (49 layers)**,
**ZH 3,342 / 827 / 1,652**, **JA 596 / 315 / 629** (forms/readings, contexts, sentences).
These are finite input suggestions, not clothing recommendations or open-ended generation.
The 25 optional packs/SDK, model contract and candidate budgets are unchanged.
The new complete all-features regression passes **2,480 tests / zero failures / 32 existing
explicit ignores**, across245 unfiltered top-level groups. This total includes the24 new tests.
Strict all-target/all-feature Clippy, formatting and83 Python guards also pass; no new UI/native,
real-model, installation or performance-timing acceptance is claimed.
This data is not in the pinned 0.9.3 acceptance commit or the personal installation;
see [current validation and limitations](DEVELOPMENT.md#daily-dressing-fit-and-accessories--2026-10-20).

朱雀是以 Linux 为核心的连续写作输入法：用字母、拼音或罗马音起笔，同时获得词与句子候选。
这是供试用和反馈的 Alpha，并非已覆盖所有桌面和应用的稳定版。

## What works today

- English-first completion, Chinese Pinyin and Japanese Romaji/Kana with small offline vocabularies.
  Word and phrase/sentence candidates appear together when available; English continuations use
  the preceding phrase and preserve straight/curly apostrophes.
- The [local vocabulary](data/lexicons/README.md) is an immediate fallback when optional models
  are disabled, missing or failing. It covers common words and authored short sentences, not
  open-ended generation; local choices remain usable while model requests are pending.
- Optional data-only [offline packs and a small authoring SDK](docs/linux-packaging-data.md#离线词库包与制包-sdk),
  grouped by language/topic. CLI installation and enable/disable apply at the next host/panel startup;
  no executable plugins, live reload, automatic downloads or settings-panel package manager yet.
- Continuous drafts: spaces and punctuation keep writing open; Enter or a candidate click submits.
  Labels distinguish candidate type, model origin and ranking weight, not confidence.
- Linux dead-key/Compose sequences produce editable accents and symbols using the system Compose
  table, without changing the keyboard layout or Caps Lock state.
- Optional local or explicitly authorized HTTPS cloud models through Ollama and compatible chat APIs.
  Local discovery prefers an already installed LLaMA; no model is bundled or automatically downloaded.
- A floating panel/orb, screen keyboard, tray activation/restoration, guardian themes, sharper text,
  compact searchable settings and optional system title bars.
- Eight interface and translation languages: English, Simplified Chinese, Japanese, Korean, Spanish,
  French, German and Portuguese. Translation needs a configured model and an explicit action;
  these are **not** eight native input dictionaries.
- Linux configuration backup and preview-first restore. Packages do not collect personal data.

The earlier **basic washcare-and-paper vocabulary** (now in the 0.9.3 source candidate) adds eight English forms
(`paper`, `soap`, `shampoo`, `toothpaste`, `tissue`, `tissues`, `conditioner`, `mouthwash`),
eight Chinese Word readings, six Japanese Word readings and **36 authored sentences**.
Current totals: **6,151 English forms / 827 contexts / 1,618 sentences (48 layers)**;
**3,330 Chinese readings / 821 contexts / 1,640 sentences**;
**592 Japanese readings / 309 contexts / 617 continuations**.
It also fixes first-page sentence slots for a verified unique Japanese long-vowel completion
such as `senganryo` → `洗顔料`, without altering its greedy conversion or Word weights.
The scoped rerun passes **1,872 tests, zero failures, five existing explicit ignores**;
26 new tests are included, not added again. This is not full-repository, native/UI, real-model,
installation or performance-timing acceptance. The 25 optional packs/SDK, model contract,
old reading order and first 6,143 English ranks remain unchanged. Not committed or installed;
see the [current vocabulary and validation record](DEVELOPMENT.md).

An **uncommitted Japanese sentence-slot repair** keeps both authored continuations on the first
page when a verified conversion is still in progress, including `汚れ ga`. The conversion stays
first at its original score; existing Words, literal text, model competition and 0–12 limits are
preserved. At six slots, literal text no longer displaces the second continuation. Frozen new
regressions and the unchanged old cleaning target pass. The expanded rerun records **1,846 passes,
zero failures and five existing explicit ignores**, plus strict Clippy; not a native/UI/install run.
See the [current repair record](DEVELOPMENT.md). Not committed, published or installed.

The preceding **uncommitted candidate-computation follow-up** replaces repeated English context scans,
Chinese continuation scans and Japanese written-form conversions with immutable startup-catalog
indexes. Five paired warm release replays improve mean candidate time by about **20% English,
83% Chinese, 29% Japanese and 40% Japanese with nine packs**; English has substantial run-to-run
variation. All **419 complete before/after candidate vectors** remain identical, with unchanged
vocabulary, ranking, budgets and model contracts. This is not a UI, native-keyboard, cold-start or
real-model speed claim. See [method, limits and current validation](DEVELOPMENT.md).
These changes are not in the published packages or the personal installation.
Broader scoped regression records **1,837 passes, one pre-existing Japanese first-page failure,
and five existing explicit ignores**; the saved before library reproduces the same failure.
That performance-only pass left the issue unfixed; the subsequent sentence-slot repair above addresses
it. Its failed result is retained, not rewritten as all-green. Strict Clippy passed for that snapshot;
the timing percentages precede the ranking repair, not a new measurement of current UI/model latency.

The earlier **uncommitted household-restocking vocabulary follow-up** adds **18 Chinese and 6 Japanese
Word readings, plus 36 trilingual authored sentences** for remaining supplies, refill packs,
checking spares and shopping-list reminders. Its working-tree totals at the time were
**6,143 English forms / 821 contexts / 1,606 sentences (47 layers)**,
**3,322 Chinese readings / 815 contexts / 1,628 sentences**, and
**586 Japanese readings / 303 contexts / 605 continuations**. These finite authored phrases
reuse existing English tokens such as `detergent`; the 6,143 indexed forms and their first-occurrence
order, model-independent fallback, 25 optional packs and SDK are unchanged. Basic English index
gaps remained at that point for `paper`, `soap`, `shampoo`, `toothpaste`, `tissue` and `tissues`; this does not complete
the household-supplies dictionary. The phrases do not track inventory, update a shopping list
or buy anything. Version/build remains **0.9.2/42**; this batch is uncommitted and not included
in the four published 0.9.2 assets or the personal installation. This batch's scoped regression
passes **1,027 checks / zero failures / two pre-existing opt-in model ignores**, plus strict Clippy;
this is not full-repository, native-keyboard, real-model or installation acceptance.
See the [lexicon record](data/lexicons/README.md).

The preceding **uncommitted storage-and-moving vocabulary follow-up** adds **18 Chinese and 6 Japanese
Word readings, plus 36 trilingual authored sentences** for making room, keeping things separate,
moving objects and returning them to their place. That batch's totals were
**6,143 English forms / 815 contexts / 1,594 sentences (46 layers)**,
**3,304 Chinese readings / 809 contexts / 1,616 sentences**, and
**580 Japanese readings / 297 contexts / 593 continuations**. This finite local fallback remains
model-independent; the English indexed forms and their first-occurrence order, 25 optional packs
and SDK are unchanged. These are input suggestions, not actions on objects or files. The new
data is not in the published 0.9.2 packages or the existing personal installation. Local scoped
regression passes **1,004 checks / zero failures / two pre-existing opt-in model ignores**, plus strict
Clippy; this is not full-repository, native-keyboard or installation acceptance. See the
[lexicon record](data/lexicons/README.md) for the exact scope and reviewed candidate changes.

The preceding **uncommitted call-recovery vocabulary follow-up** adds 24 Chinese/Japanese Word readings
and 36 trilingual authored sentences for frozen video, audio/video delay, changing headphones
and rejoining a call. These are input suggestions, not call/device controls. That batch's totals were
**6,143 English forms / 809 contexts / 1,582 sentences (45 layers)**,
**3,286 Chinese readings / 803 contexts / 1,604 sentences**, and
**574 Japanese readings / 291 contexts / 581 continuations**. See the
[lexicon record](data/lexicons/README.md) and actual scoped acceptance in
[development notes](DEVELOPMENT.md). Packages and the personal installation are unchanged.

The preceding **uncommitted route-coordination follow-up after 0.9.2's locked source** adds
**36 Chinese/Japanese Word readings and 44 trilingual authored sentences**. It covers
journey updates, the other entrance/exit, meeting positions and changed routes, not GPS or
live navigation. That batch's totals were **6,143 English forms / 803 contexts /
1,570 sentences (44 layers)**, **3,268 Chinese readings / 797 contexts / 1,592 sentences**,
and **568 Japanese readings / 285 contexts / 569 continuations**. All 23 frozen new checks
pass unchanged, including 25 complete old candidate-vector controls and a private nine-pack
Japanese fixture. Broader local acceptance is recorded separately in [development notes](DEVELOPMENT.md).
This batch does not change the locked 0.9.2 CI commit or its packages; an existing installation
still needs a later rebuild/upgrade. Earlier batch totals below retain their historical scope.

The released 0.9.2 source `4108bc0` includes the messaging, appointment/walk-in and aftercare/packaging
follow-ups below: **100 Chinese/Japanese Word readings and 122 authored sentences/continuations**
over 0.9.1, with the same 6,143 English indexed forms. It also includes the measured English/Japanese
candidate-computation optimizations. Historical “unreleased” batch records below describe their
development-time status, not separate current releases. Exact commit
`4108bc0deb8d08d9c421cfbd9c47cd8ed45e3883` passed all eight required gates in
[CI 39084362121](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/39084362121).
Its accepted `.deb`, `.tar.gz` and both checksum files passed package acceptance and
post-upload download verification. They are published as the
[0.9.2 Linux Alpha prerelease](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.9.2),
not latest stable. The [release notes](docs/releases/0.9.2.md) retain their source-preparation record;
the later publication result is recorded here. Subsequent route, call, storage and restocking data is not included.
Historical local preparation of that exact 0.9.2 source passed **2,313 Rust checks / zero failures / 32 explicit ignores**,
the separate **19/19 private UI gates**, strict Clippy and formatting. Those local results
did not replace remote CI, clean-container installation or accepted-package checks, and do not
accept the current uncommitted vocabulary follow-ups.

Six configurable input shortcuts are available in **Settings → Shortcuts**:
previous/next candidate, previous/next page, adoption and English/Chinese switching.
Bindings support Ctrl or Alt, optional Shift, and a letter or Space (except reserved
Alt+Space combinations), with conflict checks,
recording cancellation and per-action/all-binding reset. They are input-context shortcuts,
not global hotkeys; desktop/application interception remains possible. See [shortcut rules](docs/shortcuts.md).

Released 0.9.1 extends everyday fallback for sharing, finding belongings, borrowing and returning items,
and adds independent Chinese character readings for known-word composition. Compared with the
0.9.0 source baseline, built-ins add **132 sentences/continuations**, **88 Chinese readings**
and **36 Japanese readings**; English indexed forms are unchanged. Its released totals are
**6,143 English indexed forms / 773 contexts / 1,510 sentences**,
**3,178 Chinese readings / 767 contexts / 1,532 sentences**, and
**522 Japanese readings / 262 contexts / 523 continuation records**.
Chinese candidate-source fixes retain authored continuations across selected partial-word and
homophone conflicts without increasing list limits. Missing readings and full-pool branch
competition remain; builtin 里/那 are not added. The model and offline-pack responsibilities
remain separated behind the existing public APIs.

The **local-input performance work included in 0.9.2 source** reduces warm candidate-computation mean
time by about **34% for English, 54% for Japanese, and 58% with nine Japanese packs** in five
alternating release-build replay pairs on this machine. All 419 sampled candidate vectors retain
exact text, labels, types, sources, score bits and order. English stops rendering after six unique
sentence choices; Japanese indexes immutable vocabulary membership, not users' drafts.
A Chinese memoization trial was withdrawn for insufficient measured benefit; no Chinese,
GUI/native-input, cold-start or model-latency speedup is claimed. See the
[replay tool](examples/input_hotpath_bench.rs), [equivalence test](tests/input_hotpath_equivalence.rs)
and [measurement limitations](DEVELOPMENT.md). Related local regression passes **1,417 checks**
with zero failures and three existing explicit ignores; strict all-target/all-feature Clippy passes.
This is scoped engine acceptance, not a complete native-input or release run, and does not change
vocabulary or the installed app.

The historical **aftercare and packaging development batch** adds six English contexts / twelve
sentences, eighteen Chinese Word readings / six contexts / twelve sentences, and ten Japanese
Word readings / five contexts / ten continuations: **28 CJK readings and 34 authored sentences**.
That batch's totals were **6,143 English indexed forms / 795 contexts / 1,554 sentences**
(43 ordered layers), **3,244 Chinese readings / 789 contexts / 1,576 sentences**, and
**556 Japanese readings / 279 contexts / 557 continuation records**. Examples include
`i still have the receipt`, `bao'zhuang` → `包装`, `gou mai ping zheng zen me ti gon` →
`购买凭证怎么提供`, and `hoshousho` → `保証書`. Existing return, refund, receipt and repair
vocabulary was already available; explicit Words and authored requests do not determine store
policies, refund eligibility or real after-sales progress. 包装 deliberately gains an exact Word.
The [English](tests/daily_aftercare_english.rs), [Chinese](tests/daily_aftercare_chinese.rs),
[Japanese](tests/daily_aftercare_japanese.rs),
[baseline compatibility](tests/daily_aftercare_compatibility.rs) and
[isolated nine-pack](tests/daily_aftercare_packs.rs) fixtures were fixed before the append.
Before-data checks confirm the new gaps; the first after-data focused run passes all **23 new
checks** without changing the five fixtures, including all 18 sampled complete old vectors.
Final scoped regression has **1,408 passes / zero failures / three existing explicit ignores**:
112 integration targets plus the complete library, with focused repetitions counted only once.
It adds parcel, Chinese/English shopping-pack and fallback coverage to the prior scope, not a
full-repository, native-input or installation run. Strict all-target/all-feature Clippy passes
(42.67 seconds), as do formatting, static guards and byte-preservation checks; the
[execution record](DEVELOPMENT.md) gives their exact scope.
At that batch's acceptance, all three follow-ups were uncommitted and
unreleased at **0.9.1 / build 41**, with unchanged algorithms, model contracts, optional packs and
actual installation. They are now included in the published **0.9.2 / build 42**; only the
separately described English/Japanese hot-path work changes production algorithms.
The two earlier batch descriptions below retain their own historical totals
and scoped acceptance records; they are not the latest totals or this batch's acceptance.

The messaging follow-up was still **unreleased at its 0.9.1 / build 41 checkpoint**, adding missed-message,
reply and arrangement-confirmation examples: eight English contexts / sixteen sentences,
24 Chinese Word readings / eight contexts / sixteen sentences, and twelve Japanese Word readings /
six contexts / twelve continuations. That batch reached **6,143 English indexed forms /
781 contexts / 1,526 sentences**, **3,202 Chinese readings / 775 contexts / 1,548 sentences**, and
**534 Japanese readings / 268 contexts / 535 continuation records**. Examples include
`i only just saw your messag` → `i only just saw your message`,
`gang kan dao ni de xiao x` → `刚看到你的消息`, and `messe-ji` → `メッセージ`.
The [English](tests/daily_messaging_english.rs), [Chinese](tests/daily_messaging_chinese.rs),
[Japanese](tests/daily_messaging_japanese.rs) and
[reviewed compatibility](tests/daily_messaging_compatibility.rs) tests cover this finite data append;
they do not certify physical keys, a real model or a new installation. One useful Japanese Word
addition intentionally enriches an old second page and lowers four existing sentence weights by
0.5 through the unchanged mixed-index formula; identical weights or rankings are not promised.
No conversion algorithm, provider contract, new indexed English form or installed package is changed.
This follow-up is **not included in the published 0.9.1 packages**.

The subsequent historical **appointment and walk-in follow-up** adds eight English contexts / sixteen
sentences, 24 Chinese Word readings / eight contexts / sixteen sentences, and twelve Japanese Word
readings / six contexts / twelve continuations: **36 CJK readings and 44 authored sentences**.
That batch's totals were **6,143 English indexed forms / 789 contexts / 1,542 sentences**
(42 ordered layers), **3,226 Chinese readings / 783 contexts / 1,564 sentences**, and
**546 Japanese readings / 274 contexts / 547 continuation records**. Records include
`qu'hao` → `取号`, `do i need to book ahead`, and `yoyakuhenkou` → `予約変更`.
Appointment, opening-hour and queue vocabulary already existed; some longer text was already
composable as a Sentence. This adds explicit Word entries and their own authored contexts,
not the first ability to type those characters, live opening hours or an actual booking service.
The [English](tests/daily_appointments_english.rs), [Chinese](tests/daily_appointments_chinese.rs),
[Japanese](tests/daily_appointments_japanese.rs),
[baseline compatibility](tests/daily_appointments_compatibility.rs) and
[isolated nine-pack](tests/daily_appointments_packs.rs) tests define the intended scope.
Before-data runs confirm the language gaps. All **23 new checks** pass unchanged, including the
two isolated pack workflows. Final scoped regression has **1,348 passes / zero failures / two
existing opt-in diagnostic ignores**; strict Clippy and preservation checks pass. This is not
a complete new native/release acceptance; [the execution record](DEVELOPMENT.md) lists the
exact included targets and explicit terminal-n filter, without counting focused repeats twice.
At this historical acceptance, both follow-ups remained uncommitted at **0.9.1 / build 41**, with unchanged algorithms/model
contracts and no installed-package, service or publication change.

The optional data-only catalog contains **25 packs: 8 English, 8 Chinese and 9 Japanese**,
including shopping, internet and additional Japanese topics. The unchanged installation limit is
**16 packs, including disabled packs**; catalog availability does not automatically install or enable them.
These finite presets do not read personal routines, contacts or typing history, and are not
open-ended model generation or a full Chinese/Japanese dictionary.

Chinese unfinished Pinyin and adopted mixed-language drafts retain intended spacing,
word/sentence choices and exact undo. Shared-engine committed-context undo cancels obsolete model work
without removing the current draft or deliberately selected candidate. Late replies/errors
cannot overwrite a newer draft; local candidates remain immediately usable during model failure.
The prediction protocol remains model-independent; no model is bundled or required.
Native-panel previews retain complete text and absolute selection indexes.
Japanese preset continuations better retain unfinished Romaji and grammatical endings before
punctuation. Standalone number choices can be undone; confirmed configuration changes invalidate
old adoption history even while a native view is showing. Held Back gestures cannot undo newly
adopted text after repaint or an undo/reapply roundtrip.

Optional pack changes still require host/panel restart. Space continues writing; deliberate
adoption and exact submission remain separate. Linux candidate preferences now persist by default,
learning only confirmed choices; the tray exposes save/clear status and can clear them at any time.
The local cache stores keyed fingerprints and counts, not raw drafts, but is still sensitive data.
See [0.9.2 release notes](docs/releases/0.9.2.md),
[lexicon scope](data/lexicons/README.md) and [known limitations](docs/known-limitations.md).
Source publication does not update an existing installation. Native Wayland, real-model quality,
broader Japanese morphology and non-Linux native behavior remain outside current acceptance.

## Install the Linux preview

Get the matching `.deb` and `.sha256` from the
[0.9.2 Linux Alpha prerelease](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.9.2).
For 0.7.x, source tags can precede downloadable packages; use a release with attached packages or
build from source. Starting with **0.8.0**, downloadable Linux packages and checksums are a
[release requirement](docs/linux-packaging-data.md#从-080-开始的发布要求).
Use the verified 0.9.2 assets in the download directory on Ubuntu 24.04 amd64;
the later uncommitted vocabulary follow-ups are not included in these packages:

```bash
sha256sum -c suzaku_0.9.2_amd64.deb.sha256
sudo apt install ./suzaku_0.9.2_amd64.deb
```

Installation alone does not activate an input method or start a user service. Register once
**as your desktop user, without sudo**, then open the panel:

```bash
/usr/bin/suzaku-tool linux-register install
/usr/bin/suzaku-tool linux-register verify
/usr/bin/suzaku-panel
```

Registration adds Suzaku and enables its user host service, preserving the active input method.
Choose **Activate Suzaku** in the tray to type; **Release / restore** returns to the previously
observed input method. Full quit restores it before stopping the managed host; hiding to tray keeps
the host running. GNOME tray visibility depends on the desktop's tray support.

Finish/cancel the draft and fully quit before upgrading. To remove, quit first, run
`/usr/bin/suzaku-tool linux-register uninstall` without sudo, then `sudo apt remove suzaku`.
Settings and backups are retained. The `.tar.gz` is an alternative, not a static universal binary.
See [Linux installation and data](docs/linux-packaging-data.md) for dependencies, tarball use,
service behavior and safe restore. Checksums provide integrity, not a signature.

## Type continuously

In a normal text field, type `hel`, press **2** to adopt `hello`, then **Space** and `world!`.
The entire `hello world!` remains editable in the candidate area/companion panel until **Enter** or
a candidate click. The target application's text is unchanged before submission. With no active
draft, ordinary digits go directly to the application, so `2026` and `12.5` need no candidate keys.

| Key | Action in an active draft |
| --- | --- |
| 1–6 | Adopt that candidate into the editable draft |
| Alt+0–9 / numeric keypad | Enter literal digits |
| Space / punctuation | Continue the draft, without submitting |
| Tab / Shift+Tab, arrows | Navigate candidates |
| Shift+Enter | Adopt the selected candidate without submitting |
| Immediate Backspace after completion | Restore the previous spelling |
| Ctrl+Backspace in English | Delete the last whitespace-delimited draft word |
| Enter / primary candidate click | Submit the candidate |
| Esc | Cancel the draft |

By default, **Ctrl+Shift+Space** switches English ↔ Chinese, even with an empty draft; from Japanese it
switches to English. It preserves unsubmitted text, does not repeat while held and leaves system
input-source shortcuts and lock states unchanged. It is local to Suzaku's ordinary IBus fields,
not a global activation or interface-language shortcut. A custom language binding replaces
this default until cleared. Japanese remains available in the tray.

In the focused panel, **F1** opens Settings → Shortcuts and **Ctrl+,** opens settings.
The optional home-row IBus profile adds **Alt+J/K** navigation, **Alt+H/L** paging and
**Alt+;** adoption in public, nonempty drafts. See the [shortcut reference](docs/shortcuts.md)
for scope, conflicts and repeat behavior; these are not global activation hotkeys.

Password/PIN and explicitly declared numeric/decimal/phone fields bypass these candidate rules.
Applications must report input purpose correctly. See [IBus behavior](docs/ibus-candidates.md)
for literal input, selection, privacy and recovery after an unconfirmed send.

## Choose the panel layout

Settings → Appearance → **Panel Layout** offers **Auto** (the default), **Follow caret** and
**Bottom dock**. Auto shows its detected choice: desktop/unknown environments prefer caret
following; Android and explicitly identified Linux phone/tablet or mobile/gaming sessions
prefer the bottom layout. Detection runs at startup, without input-event monitoring or network
access; touchscreen presence and screen size alone do not select a mobile layout. Explicit saved
choices remain overrides, while saving Auto keeps the preference portable between machines.
Bottom dock is a phone/tablet-style layout saved with the display settings. It
centers a borderless panel along the current input monitor's bottom edge, puts candidates
above larger screen-keyboard keys, and keeps candidate paging available when the keyboard
is folded. Short screens reduce its width before clipping content. Switching back restores
the previous floating width; the orb can still be used in either mode.

After a public native draft wakes the expanded bottom keyboard, submitting a candidate leaves
it available for the next screen-keyboard draft. It does not automatically open for every empty
field. This is a floating overlay, not a system keyboard that reserves desktop space or moves
other applications. Positioning depends on the window system and client cursor information;
native Wayland placement and physical touch comfort are not yet qualified.

## Models and privacy

Model suggestions are opt-in. Default discovery checks fixed loopback endpoints for models already
served locally. Suzaku does not install or start Ollama/llama.cpp for you.

```bash
/usr/bin/suzaku-tool model discover
/usr/bin/suzaku-tool model status
```

Follow the [model guide](docs/model-providers.md) to configure providers. Cloud use needs explicit
consent and may incur charges. Key values come from named environment variables, not settings/backups.
A local gateway can forward data elsewhere; trust and configure the server separately.

Enabled suggestions can send the current draft, local conversion and up to 160 characters of
committed context from the current focus session. Explicit translation sends its source draft.
Suzaku does not collect surrounding application text or write an input history to disk.
0.8.8 saves bounded, decaying IBus candidate preferences locally by default on Linux.
Confirmed explicit choices can adjust existing candidates; private fields do not learn or use them.
The tray can clear preferences and reports pending or failed deletion truthfully. Fingerprints,
counts and their key persist across restarts; this is not encryption or anonymity. Explicit
`SUZAKU_PREFERENCE_MODE=memory` keeps a host in memory-only mode without deleting an existing
disk cache. See [candidate learning](docs/ibus-candidates.md#偏好频率缓存).
See [privacy boundaries](docs/privacy.md); third-party provider retention is outside this guarantee.

## Build and contribute

Rust 1.95.0 is the CI toolchain. Install the dependencies in [CONTRIBUTING.md](CONTRIBUTING.md), then:

```bash
cargo build --locked --release --all-features --bin panel --bin linux_ime_host --bin linux_ime_probe --bin suzaku_tool
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo test --locked --all-features -- --test-threads=1
```

Native tests use private IBus/Xvfb sessions, not your active desktop. macOS/Windows CI checks
compilation, not native input acceptance. Report bugs with synthetic text and exact reproduction
keys, never real typing logs or credentials. For security concerns, read [SECURITY.md](SECURITY.md).

## Documentation

- [Code responsibilities and dependency boundaries / 代码职责与依赖边界](docs/functional-network.md#当前-rust-模块与职责) — start here for architecture work; runtime routes and maturity evidence are separate
- [Current functional network / 当前功能链路网络](docs/functional-network.md) — architecture/function maturity inventory and audited input paths
- [Input and candidates](docs/ibus-candidates.md) · [Model providers](docs/model-providers.md)
- [Keyboard shortcuts / 快捷键](docs/shortcuts.md) — EN/ZH toggle, opt-in IBus home-row layout and in-panel reference
- [Translation](docs/translation.md) · [Interface languages](docs/interface-languages.md)
- [Linux installation/data](docs/linux-packaging-data.md) · [Known limitations](docs/known-limitations.md)
- [0.9.1 release notes](docs/releases/0.9.1.md) · [Development history and architecture](DEVELOPMENT.md)

MIT licensed; see [LICENSE](LICENSE). Packages include dependency license metadata and available
license/notice files. External model weights have their own licenses.
