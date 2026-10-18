# Suzaku Map · 朱雀

A local-first, continuous-writing input method with word and sentence candidates,
optional language models, and a compact floating companion panel.

**0.9.2 · Linux / IBus Alpha preview source candidate.** Current validation targets **Ubuntu 24.04,
amd64, IBus, X11 or GNOME with XWayland**. Keep your usual input method available
as a fallback. Other platforms remain experimental.

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

The 0.9.2 source candidate includes the messaging, appointment/walk-in and aftercare/packaging
follow-ups below: **100 Chinese/Japanese Word readings and 122 authored sentences/continuations**
over 0.9.1, with the same 6,143 English indexed forms. It also includes the measured English/Japanese
candidate-computation optimizations. Historical “unreleased” batch records below describe their
development-time status, not separate current releases. Publication requires this exact new commit's
CI and package acceptance; the currently published packages remain 0.9.1 until that completes.
See [0.9.2 release notes](docs/releases/0.9.2.md).
Final local preparation passes **2,313 Rust checks / zero failures / 32 explicit ignores**,
the separate **19/19 private UI gates**, strict Clippy and formatting. Those local results
do not replace the new commit's remote CI, clean-container installation or accepted-package checks.

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

The latest **aftercare and packaging development batch** adds six English contexts / twelve
sentences, eighteen Chinese Word readings / six contexts / twelve sentences, and ten Japanese
Word readings / five contexts / ten continuations: **28 CJK readings and 34 authored sentences**.
Latest working-tree totals are **6,143 English indexed forms / 795 contexts / 1,554 sentences**
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
actual installation. They are now included in the **0.9.2 / build 42 source candidate**; only the
separately described English/Japanese hot-path work changes production algorithms.
The two earlier batch descriptions below retain their own historical totals
and scoped acceptance records; they are not the latest totals or this batch's acceptance.

An **unreleased messaging follow-up**, still version **0.9.1 / build 41**, adds missed-message,
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

The next **unreleased appointment and walk-in follow-up** adds eight English contexts / sixteen
sentences, 24 Chinese Word readings / eight contexts / sixteen sentences, and twelve Japanese Word
readings / six contexts / twelve continuations: **36 CJK readings and 44 authored sentences**.
Current working-tree totals are **6,143 English indexed forms / 789 contexts / 1,542 sentences**
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

Get the matching `.deb` and `.sha256` from [Releases](https://github.com/chiharu-kiryu/Suzaku-map/releases).
For 0.7.x, source tags can precede downloadable packages; use a release with attached packages or
build from source. Starting with **0.8.0**, downloadable Linux packages and checksums are a
[release requirement](docs/linux-packaging-data.md#从-080-开始的发布要求).
Use the published 0.9.1 Release assets in the download directory on Ubuntu 24.04 amd64;
the 0.9.2 source candidate above is not yet an accepted package upgrade:

```bash
sha256sum -c suzaku_0.9.1_amd64.deb.sha256
sudo apt install ./suzaku_0.9.1_amd64.deb
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
