# Development notes and history

Historical implementation notes, including earlier behavior and experimental platforms.
Start with the [README](README.md) and [known limitations](docs/known-limitations.md).

Current source version: **0.8.8 — Persistent candidate preferences and everyday fallback**.

## Current priority: Chinese and English input on Linux

Polish these two input languages before expanding language coverage or visual features.
English focuses on word endings, context-sensitive next words and short-sentence continuation.
Simplified Chinese focuses on continuous/separated Pinyin, unfinished readings, useful word/sentence
choices and continued typing after conversion. Both must retain literal input, deliberate adoption,
one-step completion undo, Space continuity and exact explicit submission, with or without a model.

Gradually extend basic Japanese Romaji/Kana and everyday fallback while keeping Chinese/English
as the core. Preserve the existing interface/translation languages; do not equate eight interface
languages with eight complete native input systems. Mixed Chinese/English,
broader vocabulary and real-model latency/quality still need work. Treat input-state correctness,
authored offline examples and real-model quality as separate acceptance gates.

The [local lexicon](data/lexicons/README.md) is the fallback when model capabilities are
unavailable, not a substitute for open-ended model generation. Publish local candidates immediately,
including while prediction is pending; disabled, missing, failed, timed-out or empty providers must
not block basic word/sentence adoption, editing or exact commits. Prioritize high-frequency gaps
over corpus size, preserve old ranks, and keep vocabulary data independent of model and host code.

The paired [English](tests/english_completion_quality.rs) and
[Chinese](tests/chinese_completion_quality.rs) regression suites provide a reproducible starting point,
not a general language-quality score. The first follow-up fixes separated Pinyin within existing
dictionary entries; see the [N47 audit](docs/bug-audit-bilingual-core-2026-09-24.md).

## Release requirement starting with 0.8.0

Version releases starting with **0.8.0** must include downloadable Linux packages in the
corresponding GitHub Release, not only source tags or expiring CI artifacts. Initially target
the validated Ubuntu 24.04 / amd64 baseline: attach the `.deb`, binary `.tar.gz` and matching
SHA-256 checksum files, with installation/upgrade instructions and known limitations.
Build from the exact, clean tagged source; require the same commit's CI, package-content
checks and clean-container installation checks to pass before publication. Other platforms
are added only after their own acceptance, not implied by the shared version number.
This is a release requirement, not a claim that automatic publication is implemented or
that existing 0.7.x tags already have assets. See [Linux packaging](docs/linux-packaging-data.md).

### 0.8.8 preference restart readiness repair — 2026 10 12

- Repair commit `660950242a47ee1ad5019a34a81345ddc848f40a` passes the new
  settings-focus guard, but [CI 38229734734](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/38229734734/job/114745985217)
  fails `preference_host` at the first restart: `Factory.CreateEngine` reaches
  its unchanged 2,000-ms D-Bus deadline before any learning, save or clear case.
  This is not the 120-second fixture timeout or evidence of lost preferences.
  Exact job/step and logs remain in `/tmp/suzaku-ci-6609502.OcLSJV`; no CI rerun
  or publication is performed and the automatic follow-up stays paused.
  The completed remote run passes Linux applications, packages/clean install,
  formatting, secrets and both desktop checks; the aggregate fails because the
  core regression failed. Those successful lanes do not authorize publication.
- The unchanged local nine-workflow baseline passes. Source inspection also
  rules out the proposed registration-before-socket ordering: production binds
  the companion socket only after factory/name/component registration succeeds.
  The remote trace does not identify why that particular creation call stalled.
  Do not label this follow-up as a proven fix to the remote root cause.
- A definite fixture gap remains: restart waits only for a socket pathname,
  unlike initial startup's responding-host checks. The new helper spends the
  **same single five-second startup budget** on binding plus an actual `W`
  response with a new, nonempty host identity and an empty, unfocused, public
  state. It then issues exactly one unchanged `CreateEngine` call. Missing/stale
  replies, private/draft state, child exits and real engine timeouts still fail;
  no retry, extra startup wait, relaxed assertion or global deadline increase.
- A separate diagnostic gap discarded per-host logs with the temporary test
  directory. Failed runs now report the phase, owned process state, text-free
  snapshot metadata and bounded tails of the three newest host logs before
  cleanup; startup stdout and stderr share each owned log. Cleanup/diagnostic
  errors cannot replace the primary exception. Logs never read the cache file
  or print draft/candidate text from companion frames.
- Deterministic tests execute the real readiness helper and the original shared
  wait loop without GI or a desktop. The old ordering produces 22 failing
  assertions across 14 tests (including subcases), retained alongside the
  baseline and fixed checks in `/tmp/suzaku-preference-restart.waF57d`.
  All 21 final readiness/diagnostic cases pass, including child death coincident
  with an acknowledgement, log bounds/privacy and preservation of primary
  exceptions. The fixed native target passes both Rust tests and all nine
  original preference workflows. AST comparison confirms their exact original
  case bodies and isolation/outer-runner functions remain unchanged.
  The guard is part of the existing Linux Rust test target; the shared IBus
  fixture, production code, injected fsync library and CI workflow are unchanged.
  Personal installation, services, settings and model/network behavior are not
  modified. Local results cannot replace new exact-commit remote acceptance.
- The final all-feature Rust run passes **1,796 tests**, zero failures and
  **30 explicitly ignored** opt-in tests across 134 result groups. Strict
  all-feature/all-target Clippy passes with warnings denied. The four targeted
  source guards (restart, settings focus, CI graph and tensor) pass 75 checks;
  refreshed tensor fingerprint, 250 audit links, formatting and diff checks
  pass. The native/full/Clippy runs are serial, one-build-job, memory-capped
  (2 GiB high, 4 GiB max, zero scope swap); logs remain beside the baseline.
  This does not establish a new package build, personal-desktop acceptance or
  the cause of the remote timeout. No commit, push or release is performed.

### 0.8.8 settings-focus fixture repair — 2026 10 12

- Remote run [38224763030](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/38224763030)
  for `0d2236b2a637f1295445ecf1fcd2c99c45828caa` fails the application job's
  auto-layout settings-return stability check. The final host frame is unfocused
  and empty while X11 still owns the editor. The old assertion text incorrectly
  suggests that this alone proves a newly created draft; it does not. Preserve
  the remote receipt in `/tmp/suzaku-release-0.8.8.WCaoLg`; publication is paused.
- The unchanged local baseline passes 42 auto-layout workflows, so the exact
  remote failure is not locally reproduced. Deterministic event ordering does
  expose a fixture gap: an old focused snapshot can satisfy the return wait
  before delayed host focus events arrive. The first new guard has ten failures
  across eleven tests against the old code; retain it and subsequent receipts
  in `/tmp/suzaku-auto-focus.9wCnyI`.
- Settings now requires a fresh host/context/revision boundary and a different
  IBus input context before changing layout, then a fresh boundary after the
  final pre-restore snapshot and the exact original input context on return.
  The first repair incorrectly required the outbound host to stay unfocused;
  its native failure is retained. Global-engine IBus may instead focus a fallback
  context. Add a deterministic focused-fallback case and use input-context
  identity plus freshness, not that boolean assumption, for the handoff.
- Review adds two failing cases for an intermediate focused fallback and loss
  of the original input context during stability. A further failing case guards
  host replacement/private frames during settings, before recording a new
  return baseline. Both the initial late rejection and the corrected pre-restore
  rejection are recorded. The final guard passes all 15 cases, executing the
  real fixture functions with controlled event timing.
  Transient nonempty frames and GTK insertions remain failures even if later
  cleared; late focus loss still fails immediately. The original 4-second wait,
  150-ms stability interval and 240-second runner deadline are unchanged. A
  Linux Rust integration target runs this guard in the existing full-test gate;
  no CI command, gate, runner or production code is changed.
- A matrix command mistakenly uses `SUZAKU_GTK3_QA_SYNC_MODE=0`; the unchanged
  runner rejects it before starting a native workflow. Preserve that receipt
  separately and correct only the invocation to the documented `default` value.
- The final source passes both default and synchronous GTK/IBus transports for
  auto-layout (42 each), bottom-layout (39 each) and caret-following candidates
  (30 each): **222 native workflows across six isolated sessions**, including
  real card clicks, pagination, adoption/undo, touch keys and window movement.
  `accepted-*-sync*.log` identifies these final local receipts, not accepted
  release artifacts. All native/build runs are serial in task-owned 2 GiB
  high/4 GiB max, zero-scope-swap scopes with one Cargo build job.
- The new Rust integration target passes and executes the 15-case guard in the
  normal test path. Strict all-feature/all-target Clippy, 18 release-graph,
  13 X11-focus and 21 tensor-validator checks, the refreshed source digest,
  250 audit links, formatting and diff checks pass. All unrelated fixture AST
  bodies, the complete native runner and CI workflow remain byte-for-byte
  unchanged against the candidate. This is targeted validation, not a new full
  Rust-suite or packaging run; the earlier 1,794-test result remains historical.
- Local isolation checks do not qualify a new clean remote commit or establish
  the exact cause of the original remote ordering. No tag, release asset,
  personal installation or input-method service is changed by this repair.

### 0.8.8 release preparation — 2026 10 12

- Synchronize Cargo/package version to 0.8.8 and macOS/Android build number 38.
  Include the preference persistence/clear feedback work and household/mealtime
  fallback batches, with updated release, privacy and data-management guidance.
  `ring` 0.17.14 becomes a direct dependency without changing its locked version.
  No personal installation or input-method configuration changes.
- Retain release receipts in `/tmp/suzaku-release-0.8.8.WCaoLg`. The first versioned
  full all-feature Rust run stops in the library at two obsolete vocabulary-total
  assertions: 560 passed, two failed and two explicitly ignored. The added data
  already passed focused checks; update only the current Chinese reading and
  English context/sentence counts, retaining all uniqueness/reachability/rank
  assertions and historical fingerprints. The original failing log is preserved.
- After that explicit count repair, the complete versioned all-feature suite
  passes **1,794 tests**, zero failures and **30 explicitly ignored** opt-in checks
  across 133 result groups, including the nine-group private native preference
  gate. All 20 CI source/adapter suites pass 275 tests, with the tensor source
  check, six pack-shipping tests and 250 repository audit links also passing.
  Checksum-pinned Gitleaks 8.30.1 finds no leaks in the staged release or the
  existing 131-commit history. Local ShellCheck is unavailable and remains a
  required remote check. A mistaken `--help` invocation of the positional-only
  audit-link checker reports a missing directory; its correctly scoped repository
  check passes, without changing the checker or its assertions.
- Final strict all-feature/all-target Clippy passes with warnings denied, along
  with formatting and diff checks. Local compilation and tests remain serial
  under one-job, 2 GiB high/4 GiB max, zero-scope-swap limits; no native deadline
  or assertion is relaxed to obtain acceptance.
- Complete clean-commit CI, native/application gates, package-content checks and
  clean Ubuntu 24.04 installation/upgrade/removal remain publication prerequisites.
  Do not tag or upload an incomplete candidate as an accepted binary release.

### Post-0.8.7 everyday mealtime vocabulary — 2026 10 12

- Append `daily_mealtime`: eight English contexts/16 sentences, plus 24 Chinese
  readings/eight contexts/16 sentences for passing dishes, tableware, serving,
  meal progress and taking food away. English reuses all existing forms: 6,143
  index entries, now 709 contexts/1,382 sentences. Chinese totals are 2,970
  readings, 703 contexts and 1,404 sentences. Serving 盛 uses `cheng`, while
  remaining 剩 uses `sheng`. No food-storage advice or personal-history corpus.
- Preserve fixed pre-data failures in `/tmp/suzaku-mealtime-vocabulary.3plEqE`:
  Chinese has two passing/five failing tests; English one passing/six failing.
  The fourteen-pack combination separately exposes the absent rice sentence.
  After the data append, all seven new tests per language, 15 resource-contract
  checks and the complete fourteen-pack quality test pass on the first run.
  Cases cover complete/partial spellings, raw text, case/spacing, adopted long
  drafts, line/protected-token boundaries, exact commit/undo and synthetic model
  unavailability. Existing content/order and all English index positions are
  fingerprinted independently; current resource totals are updated explicitly.
- The twelve older household, daily-needs, completion, reading-boundary,
  protected-context, long-draft and offline-pack targets pass all 117 tests on
  their first run. Together with the 30 focused checks this is **147 passing
  tests across 16 targets**, zero failures or ignored tests, not a full-suite
  claim. Strict all-feature/all-target Clippy passes with warnings denied;
  formatting/diff checks, 21 tensor-validator tests and the refreshed tensor
  source digest also pass. Test and Clippy receipts stay beside the pre-data
  failures. Cargo checks run serially with one build job and a 4 GiB memory cap.
- Japanese, all recommended pack bytes, language/model/host implementation,
  capacity limits and previous acceptance thresholds are unchanged. New built-in
  entries still require a rebuild/reinstall. This round does not install, change
  personal input state, commit or push; engine reseeding and typed provider errors
  do not establish physical-key, GTK/Wayland, HTTP or actual model acceptance.

### Post-0.8.7 everyday household vocabulary — 2026 10 12

- Return to Chinese/English fallback vocabulary: append `daily_household` with
  eight English contexts and 16 sentences, plus 24 Chinese readings, eight
  contexts and 16 sentences. Cover dishes, wiping surfaces, rubbish, wet floors,
  putting things away and laundry. English reuses existing forms: 6,143 indexed
  forms, now 701 contexts/1,366 sentences; Chinese is now 2,946 readings,
  695 contexts/1,388 sentences. Japanese and all fourteen optional pack files
  remain unchanged. No model/host coupling, algorithm change or capacity increase.
- Author independent fixed tests before appending data. English RED is one pass
  and six failures, Chinese two passes and five failures; the all-pack quality
  test also reproduces missing `i can wash the dishes` on page one. Preserve
  these logs under `/tmp/suzaku-household-vocabulary.VIvWzo`. After the append,
  all seven new English and seven Chinese tests and the complete existing
  fourteen-pack combination quality test pass on the first run. New checks
  protect all previous index positions/content, explicit and unfinished spellings,
  literal text, case/spacing, adoption/continued drafts, one-time commit/undo,
  line/protected-token boundaries and synthetic missing/failed/empty predictions.
  The existing `yi'jia` first choice remains 衣架; appended 衣夹 is selectable.
- The first older regression run stops on two obsolete full-resource counts in
  the historical function-word tests. Scope their unchanged fingerprints to
  their original 2,922-reading/687-context window, and keep exact current totals
  in the resource-contract test. No old ranking/behavior assertion, fingerprint,
  model deadline or native fixture is relaxed. Re-run those tests together with
  the remaining vocabulary, family-pack and long-draft suites: all **191 tests
  pass across 19 targets**, with one existing process-child helper ignored by
  default. Together with the 15 new/combined checks this is **206 passing tests**,
  not a full-suite claim. Final receipts remain beside the original failures.
- Strict all-feature/all-target Clippy initially flags the new English fixture's
  nested tuple type; split its branch into a named type without changing data or
  assertions, then pass with warnings denied. Formatting/diff, 21 functional
  tensor tests, the refreshed source digest and six pack-shipping checks pass.
- Engine adoption uses explicit reseeding and synthetic provider failures, not
  physical key dispatch or actual HTTP/model quality. No personal history,
  installation, service, keymap or cache changed; no commit/push or release.
  Embedded data takes effect only after rebuilding/reinstalling. Tensor scores
  stay unchanged; this is not a new full-suite or native desktop acceptance.

### Post-0.8.7 preference tray feedback — 2026 10 12

- Fix two source-level UX gaps: a clear that completed after the client's timeout
  could leave a permanent generic operation error, and persistence labels/file
  diagnostics were missing translations. Keep an accepted host epoch/task receipt
  on failure, expose strictly validated clear status, and reconcile only that
  exact receipt on subsequent read-only refresh. Keep the original one-F/700ms
  operation budget and the legacy RAM-only API. Restart, supersession, missing
  acknowledgement and malformed completion never imply successful deletion.
- Separate clear feedback from unrelated activation/settings errors. Show clearing
  separately from saving, retain actionable failure detail, add remembered count
  and an explicit refresh action, and disable repeated clears while the host
  reports pending. New learning dismisses the old cleared-cache notice. Coalesce
  menu-open/explicit refreshes without blocking activation preparation or adding
  background polling. Add 31 resource keys across all eight UI languages,
  including all 18 preference-file diagnostics; translate owned error payloads
  before menu truncation, preserving access-key escaping.
- Focused verification passes 14 protocol, 19 tray and three localization tests;
  the final tray run also asserts diagnostic retention and removal. The unchanged
  nine-group private native persistence gate and strict all-target/all-feature
  Clippy pass. Formatting/diff and all 21 functional-tensor checks pass. Receipts:
  `/tmp/suzaku-preference-feedback.fa3Ewv`. This is a scoped follow-up, not a rerun
  of the preceding 1,758-test full suite. No personal cache/settings, installed
  binaries, services or desktop input state changed; no commit/push, packaging,
  remote CI, rendered-menu or physical-keyboard acceptance is implied. Retain
  the existing tensor scores and historical failures/results.

### Post-0.8.7 preference exit/recovery privacy — 2026 10 12

- Extend the new isolated native gate without changing its original eight
  workflows, input assertions, 120s private-session cap or the host's 700ms
  shutdown bound. First prove that SIGTERM while a real staging `fsync` is held
  flushes the latest coalesced choice if released during shutdown, including
  restored ranking. Then hold a second save past shutdown: the host truthfully
  warns that saving is unconfirmed and preserves the last published cache,
  but the first run reproduces **one abandoned sensitive temporary file after
  restart**. Keep `/tmp/suzaku-preference-lifecycle.Q41Srp/native-orphan-red.log`
  as a failed receipt; this is a real recovery/privacy defect, not a report-count
  failure, and it does not damage the published cache.
- Give preference staging its own `.suzaku-preferences-<pid>-<sequence>.tmp`
  namespace. Generic settings/backup staging names and publication behavior stay
  unchanged. On startup and explicit clear/reset, hold the exclusive preference
  lease, validate the whole bounded inventory, then remove only owned/private,
  single-link regular files with exact canonical names and sync the parent.
  Never parse/replay partial staging or sweep generic temporary files, recurse,
  follow links or remove unrelated files. Limit inventory to 128 entries and
  64 staged files; exceeding a limit or encountering unsafe staging reports an
  error before any deletion. Canceled clear/reset also leaves files intact.
- A timed-out bootstrap must keep its one worker's lease until that worker
  actually exits; the returned error now specifically describes restore timeout,
  rather than falsely reporting another writer. A test-only bootstrap barrier
  checks bounded return, continued exclusive ownership, no late cache handoff,
  eventual lease release and preservation of the published bytes. No production
  barrier, replacement-worker retry, longer deadline or input-side disk I/O is added.
- First focused verification passes **558 library tests**, two explicitly
  ignored, all **19 persistence** and **13 unchanged learning** tests, plus the
  now **nine-group** real native gate. Its timed-out shutdown restarts with zero
  abandoned preference staging, restores only the last published cache, and
  still clears through the existing acknowledged protocol. Four added storage
  tests cover namespace preservation/lease contention, canceled clear/corrupt
  reset, five unsafe staging types and both resource limits; another library
  test rejects invalid staging namespaces before file operations. No personal
  cache/settings, input source, keymap, daily installation or service is modified.
  Native menus, physical typing, actual model quality and non-Linux persistence
  are not implied.
- The first full all-feature run passes **1,758 tests**, zero failures and
  **30 explicitly ignored** checks across 129 result groups. Strict all-feature,
  all-target Clippy passes without warnings. Add an exact near-eight frequency
  assertion to the new pending-shutdown case, then independently pass all nine
  native groups again; this verifies the final coalesced snapshot, not just a
  nonempty file. Both original complete default and inline IBus gates pass their
  whole core and all 106 HTTP failure/timeout fallback cases per mode, keeping
  their original 120s private-session bounds. Nine original source guards,
  the unchanged prediction-test prefix, formatting/diff checks, all 21 tensor
  checks and 18 CI-graph checks pass. Receipts remain in the lifecycle directory
  above; verification is serial and memory-capped. No commit/push, package,
  installation, remote CI or release is part of this follow-up.

### Post-0.8.7 native preference persistence — 2026 10 12

- Follow the user's default-on/anytime-clear policy in the Linux source host,
  leaving other hosts memory-only. Bootstrap the validated cache on a worker
  before entering IBus, capped at 500ms, and refuse late replacement. Use the
  normal XDG state home or an explicit configuration's isolated sibling directory;
  never let a config override fall back to personal state. No installed program,
  personal cache, configuration, input source, keymap or service is changed.
- One lifetime worker owns the private file lease. Input callbacks only capture
  changed confirmed RAM state; coalesce to one latest save plus one priority
  clear, with at most one active operation. No input-session/queue mutex is held
  during parsing, writes or sync. Revalidate a staged save before publication;
  superseding mutations, clear or privacy discard its temporary file. Clear
  fences still reject all pre-clear revisions, including after newer input.
- Explicit global clear works even in a private context: capture only the
  emptied cache, never the private draft. A corrupt/oversized startup file remains
  unchanged on ordinary input; an explicit owned/private reset may delete that
  exact file under the exclusive lease and rebind the empty cache. Another
  writer, symlink or insecure file cannot be erased through that reset. Preserve
  existing external-conflict rejection and communicate failures truthfully.
- Reuse the unchanged C `F` branch without blocking it. Disk clear returns an
  accepted operation, not `ok:true`. Tray/client follows read-only `S` receipts
  bound to both a process-local epoch and operation number, on the original
  single 700ms budget, never repeating `F`. Only completed deletion + parent
  sync qualifies success; empty, mismatched, restarted, failed or incomplete
  replies and timeouts remain unconfirmed. A deletion begun in time may finish
  sync after a disconnected client. Preserve direct legacy RAM-only clear.
- Add truthful tray status/clear labels and a small `preferences status|clear`
  CLI using that same running-host protocol, never direct cache-file access.
  `SUZAKU_PREFERENCE_MODE=memory` is an explicit startup opt-out, not disk deletion.
  The original QA marker keeps its frozen RAM-learning fixture in RAM mode;
  `disk` can override it. A separate new default-policy fixture sets neither
  marker/mode and uses only owned temporary config/data/state/Compose paths.
- GLib main-context SIGTERM/SIGINT callbacks return through the native host's
  off-input-loop, bounded 700ms shutdown. Source-built real SIGTERM/restarts are
  exercised, but forced kills/OOM/hung filesystems cannot guarantee the latest
  unconfirmed save. No changes to C dispatch, shortcut, Compose, privacy or the
  original native fixtures, runners, slow-settings barrier or deadlines.
- Nine first-run worker regressions pass: held/coalesced saves, clear priority
  and no-resurrection, post-clear typing, privacy, expired/newer clear, bounded
  shutdown, corrupt/oversized explicit reset and foreign lease/link protection.
  Four client regressions cover receipt success, seven negative receipt shapes,
  RAM compatibility and one-budget timeout. A policy unit check protects the
  default and explicit overrides. All **556 library tests pass**, zero failures
  and two explicitly ignored checks, followed by the eight-group native test.
- The first native run passes its six actual workflow groups but fails the new
  Rust report-count assertion (expected seven). Preserve that failed receipt;
  fix only the new incorrect count and verify all six again. Add two independent
  ordinary-state/config separation and memory opt-out groups, then require all
  eight. Real stage/parent `fsync` barriers verify responsive input, discarded
  staged saves, unconfirmed 700ms CLI exit, eventual exact clear receipt and
  injected EIO fallback. The new gate runs in normal Linux all-feature Rust CI,
  not an ignored test; missing tools/dependencies or resource exits fail it.
  The old C/fixture assertions are unchanged and their nine source guards pass.
- Receipts are in `/tmp/suzaku-preference-host.iYvSJC`. Full Rust/native and
  strict Clippy validation run serially under task-owned memory caps. The first
  full all-feature run passes **1,752 tests**, zero failures and **30 explicitly
  ignored** checks across 129 result groups, including the new eight-group native
  gate. Strict Clippy first finds only `items_after_test_module` in the new worker;
  move the unchanged worker implementation before its tests, without an allow
  attribute or weaker warnings. Preserve the first failure; strict all-feature,
  all-target Clippy then passes with warnings denied. Full original native
  default/inline gates each then pass their entire original core plus all 106
  HTTP failure/timeout fallback cases, with the unchanged 120-second private
  session limits. After the module-only reorder the nine worker cases and all
  eight new real-file/native workflow groups also pass again. Formatting/diff,
  all 21 tensor checks, its updated source digest and all 18 CI-graph checks pass.
  The inventory now has 69 functions/79 cells, distinguishing the portable
  storage slice from Linux host integration; its original 77 cell scores stay
  unchanged. No remote CI, packages, installation or GUI visual result is inferred.
  These private-bus results do not qualify visual tray labels, installed-app
  upgrade, actual keyboard comfort, Wayland or model quality; no new version,
  commit, push, package or installation is part of this follow-up.

### Post-0.8.7 preference persistence foundation — 2026 10 12

- User policy is **default enabled, clear at any time**. This first stage adds
  a real storage/restore API, not automatic native-host persistence. The current
  host status and tray remain truthfully memory-only; no personal cache file,
  setting, installed program, input source or service is changed. Next wire a
  bounded background writer, startup/shutdown ownership and a durable clear
  acknowledgement into the host and settings/tray before enabling that default.
- Separate the portable versioned codec from the Linux file backend and the
  candidate lifecycle. The existing language/query/candidate namespace now uses
  a 32-byte OS-random HMAC-SHA256 key and a domain-separated, length-framed
  128-bit fingerprint. Keep independent process-local salts for pending-prefix
  and ownership guards. Use the already locked/cached `ring` 0.17.14 directly;
  no dependency version upgrade or download. If entropy is unavailable, local
  learning still works but export refuses to publish a predictable key.
- Format v1 contains the fingerprint key, fingerprints, effective frequency and
  saved wall time, **not typed text, drafts, pending adoptions or undo history**.
  This is not encryption or anonymization: someone who can read the key/file
  can guess common inputs. Treat it as sensitive local data and do not silently
  add it to backup/export. Bound parsing to 512 KiB and 2,048 entries; reject
  unknown versions/algorithms/fields, duplicate keys and invalid frequencies as
  a whole document. Preserve the 24-hour half-life, frequency cap 8 and bonus cap
  24. Account for stopped wall time, resume monotonic decay after restore and
  never increase counts when the wall clock moves backward.
- The Linux synchronous backend requires an explicit private absolute path,
  owned 0700 directory and owned, single-link private regular files; new files
  are 0600. Reject visible symlink ancestors/final symlinks, hardlinks, public
  permissions and FIFOs without blocking on them. Keep a nonblocking exclusive
  cooperating-writer lease. Stage/write/fsync/rename and sync the parent for a
  durable result, rejecting detected external edits rather than overwriting
  them. Corrupt/unknown files remain untouched so a host can fall back to RAM.
  This is not compare-and-swap against hostile same-user directory/file races.
  These blocking APIs must run off the input thread and shared-session mutex.
- RAM-only snapshots exclude unconfirmed choices; private-context export and
  restore are refused. Installing a restored cache requires an unused empty
  input context, so a late load cannot replace live learning, selection or
  adoption undo. Undo/clear advance a monotonic mutation revision; successful
  disk clear requires an empty snapshot, deletes and syncs the file, and rejects
  every pre-clear or same-revision queued save. A later genuine choice may save
  again. Clear success does not imply secure erasure of filesystem snapshots.
- First focused checks pass **27 tests** (14 new integration plus 13 existing
  learning regressions). The first full run passes **1,736**, zero failures and
  30 explicitly ignored checks. Subsequent review adds a RED regression proving
  an old cache's undo receipt could remove a restored entry with a matching key
  and revision. Add independent cache ownership to feedback receipts, preserving
  legitimate new undo chains. Keep the actual failing log rather than claiming
  the earlier full run validated this later guard. The first full post-guard run
  passes **1,737**, zero failures and 30 explicitly ignored checks across 128
  result groups, including all 15 new integration cases and the entropy test.
  Strict all-feature/all-target Clippy passes with warnings denied.
  Formatting/diff checks, all 21 tensor checks, all 18 CI-graph checks and the
  refreshed source fingerprint pass; the tensor adds F69 without raising its
  77 previous cells. The old provider fixture remains a byte-identical prefix.
  Receipts are in `/tmp/suzaku-preference-persistence.WZdNPS`; validation uses
  serial task-owned 2G/4G memory scopes, one build job and no scope swap.
- New cases cover real private files and two separate processes, stopped-time
  decay, malformed/oversized/conflicting files, foreign receipts/snapshots,
  clear/no-resurrection, cold engine ranking, live selection and private-field
  guards. They do not qualify native automatic save/clear UI, shutdown flush,
  slow-I/O cancellation, installation/upgrade, physical input, Wayland or model
  quality. Keep those host integration gates separate; no release, commit or
  push is included in this first stage. Existing native fixtures/assertions,
  deadlines and maturity scores are not weakened.

### Post-0.8.7 repeated engine controls — 2026 10 12

- The released baseline is clean commit
  `98ee5764e8ae494d5d4cc0c2af694e307645d011`. Its exact
  [CI 38161577941](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/38161577941)
  passes all eight jobs, including all original 41 Linux regression/package
  steps and the mandatory final aggregate. Ubuntu 24.04 packages are downloaded,
  independently checked and uploaded as a draft; all four uploaded assets are
  downloaded again and byte/digest compared before publishing
  [v0.8.7](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.8.7).
  The earlier failed/cancelled runs remain failures. Clean-container upgrade
  acceptance uses a synthetic predecessor, not a real historical migration.
- This subsequent, uncommitted source fix is **not part of those release assets**.
  No new version/tag, remote write, package build, installation, personal input
  source, desktop keymap or service change is included in this follow-up.
- First offline regressions reproduce four failures: equal language/alias
  controls reset a deliberate `hello` choice to `hel`, while equal signals
  retire the explicit-choice lock used by continuing Space. A second controlled
  provider run fails all three new tests: equal controls cancel a live request,
  discard Ready model rows/selection and remove accepted error feedback.
  Retain both first RED logs in `/tmp/suzaku-noop-controls.ZO2Fjg`.
- Equal canonical language and already-applied signal values now return the
  existing snapshot without rebuilding, resetting selection, clearing a
  diagnostic or restarting model work. The first signal application still
  calculates its startup confidence. Real language/signal changes retain their
  existing refresh behavior. A small dirty flag preserves lazy registration:
  replacing the active language implementation still refreshes on the next
  control/reseed, while registering an inactive plugin leaves the live choice
  intact. No settings-persistence rule, native dispatch, vocabulary, prediction
  protocol or deadline changes.
- Five offline tests cover 17 fixed short/long EN/ZH/JA word/sentence variants
  for each equal control (no over-budget Japanese conversion claim), plus
  first/changed signals and active/inactive plugin controls. Three appended
  provider tests cover EN/ZH × ordinary/IBus-mixed engine policy × Pending,
  Ready and current-error states (12 fixed cases), with the original request's
  uncancelled token, exact usable reply, full snapshots/pools/selection,
  one exact commit/undo and positive retry after a real edit. The entire old
  eleven-test provider fixture remains a byte-identical prefix. Its existing
  two/three-second limits are unchanged.
- The first repaired focused run passes all **19 tests**, including all eleven
  old prediction/edit regressions. The first full all-feature Rust run then
  passes **1,721 tests**, zero failures, with **30 explicitly ignored** opt-in
  checks across 127 result groups. Strict all-feature/all-target Clippy passes
  with warnings denied. Formatting/diff checks, all 21 tensor checks, its updated
  source fingerprint and all 18 CI-graph checks pass. Compilation/testing stay
  serial under task-owned 2G/4G high/max memory scopes with one build job and
  swap disabled for those scopes. The native runner, fixture, C adapter and
  five original AST guards remain unchanged. These results are not native
  keyboard/display, installed-app, HTTP transport, human comfort, Wayland or
  real-model-quality acceptance; maturity scores are not raised.

### 0.8.7 release preparation — 2026 10 12

- Synchronize Cargo/package version to 0.8.7 and macOS/Android build number 37;
  update current user-facing documentation and add release notes. Preserve
  historical 0.8.6 installation reports, fixture text and optional pack versions.
  No dependency update or personal installation change is part of this release.
- Collect the accumulated custom-shortcut, authored daily fallback, Chinese
  continuation and late-prediction repairs together with their regression
  fixtures. Detailed development entries below retain first failures and exact
  validation boundaries; source-only guards are not native or human acceptance.
- Release validation receipts are retained in
  `/tmp/suzaku-release-0.8.7.rIFkAV`. Compile/native/package checks run serially
  under task-owned resource limits. The first versioned full all-feature Rust
  run passes **1,713 tests**, zero failures and **30 explicitly ignored** native
  or live fixture checks across 126 result groups. Strict all-feature/all-target
  Clippy passes with warnings denied; formatting, diff checks, all nineteen
  source/adapter guard suites, tensor validation and 250 audit-report links pass.
  These source guards are not end-to-end native acceptance. Local ShellCheck is
  unavailable; the remote CI still requires it rather than treating it as passed.
- The checksum-pinned Gitleaks 8.30.1 directory scan initially reports a generic
  API-key match in a fixed regression AST SHA-256 constant. Preserve both first
  and diagnostic logs, verify the independently captured hash, and add a narrow
  `gitleaks:allow` comment to that exact hash line, without changing the value,
  coverage or assertions. An initial misplaced annotation did not resolve it;
  the corrected exact-line scan passes with no leaks. Do not use broad file or
  rule exclusions. The remote gate still scans the full committed history.
- The earlier 0.8.6 remote Linux run failed at its native session deadline and
  did not produce an approved Release. The new partitioned full gate preserves
  every original assertion and deadline. Publication of 0.8.7 still requires
  its own exact clean commit's CI, package-content and Ubuntu 24.04 installation
  checks; neither the earlier local runs nor host-built glibc 2.43 packages
  replace those gates. Publish Linux Alpha assets only after all gates pass.

### 0.8.7 CI format compatibility follow-up — 2026 10 12

- The first pushed release-preparation commit is
  `05515551c5b97b4af9c67400399f719c5634f963`. Its exact
  [CI 38154577312](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/38154577312)
  fails the frozen native-model AST check before full Rust/native/packaging
  stages: two of nineteen checks produce different digests. Formatting, full
  history secret scanning and macOS/Windows compile checks pass; Linux fails,
  zero package artifacts exist and no tag or Release is published from that run.
  Keep the first failure and diagnostic logs in the release receipt directory.
- The same current source under the already available `/usr/bin/python3.12`
  reproduces the two exact CI digests on its first local run. Python 3.14's
  default AST dump omits empty list fields that 3.12 includes. This is a fixture
  fingerprint-format compatibility defect, not permission to refresh historical
  hashes, remove assertions or change any native/model deadline.
- Add a strictly allowlisted source-guard entry point with the independently
  frozen compact AST representation. Only the five guard basenames may run;
  restore `ast.dump` and arguments after execution, preserve target exceptions
  and exit codes, reject arbitrary scripts/options, retain required None and
  literal data, and never mutate the input AST. The five original guard files
  and their historical SHA assertions remain byte-for-byte unchanged, so the
  later guard projection chain also remains intact. No production/native code,
  global Python configuration, model behavior or keyboard semantics change.
- Before this source-only compatibility fix, the first versioned local full
  default and full inline native IBus gates both pass: each completes the entire
  original core plus all 106 HTTP-failure/timeout fallback subcases in separate
  fresh sessions with unchanged 120-second deadlines. These source-build
  private-bus checks do not install, change the daily input source or qualify
  human/physical-device/native Wayland or live-model quality acceptance.
- Validation of the compatibility follow-up and its exact clean-commit CI must
  finish independently before tagging or publishing Linux Alpha packages.
  Version remains 0.8.7 / build 37; dependency and optional pack versions remain
  unchanged. The tensor records source evidence without increasing maturity.
- Python 3.12.3 and 3.14.4 each pass the new thirteen format/scope checks plus
  all five original wrapped guard suites (19 + 13 + 13 + 18 + 22 = 85), for
  **196 total checks**. Five original whole-file SHA checks pass. Receipts are
  in `/tmp/suzaku-ast-guard-compat.vhVGkh`; retain the first thirteen-check 3.12
  result before interruption and the later error-propagation review/result.
  Do not count source checks as native input acceptance.
- The other source/adapter checks also run with the available local 3.12.
  Its GI signal-lifetime check fails at import because this machine's PyGObject
  extension is built for system Python 3.14, not the secondary 3.12 interpreter.
  Preserve that failure as an environment limitation; do not skip or patch the
  check. The unchanged signal-lifetime gate separately passes all six tests
  with its matching system Python 3.14. Remote Ubuntu 24.04 installs matching
  Python/GI dependencies and must pass the same mandatory gate independently.

### 0.8.7 bounded CI lanes — 2026 10 12

- The corrected format commit is
  `f0e2bc184a3f9d9fc65c3e0b3f79578099eea8be`. Its exact
  [CI 38157429301](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/38157429301)
  passes formatting, both desktop compile jobs, full-history secret scanning,
  and every Linux source, Rust, native, UI, GTK, physical-keyboard, lifecycle,
  Qt, settings and data gate (steps 6–42). The Linux job nevertheless ends
  cancelled: GitHub's original annotation explicitly reports the **40-minute
  overall job limit**. Release compilation starts at 17:33:59 UTC and is
  cancelled at 17:35:08; package construction, contents, clean installation and
  final upload are skipped. This is not a successful CI or release result.
  The artifact API returns zero artifacts. Keep the original cancellation log,
  annotations and final job audit in `/tmp/suzaku-release-0.8.7.rIFkAV`.
- Split the growing monolithic workflow into independent Ubuntu 24.04 lanes
  for source/Rust/native/UI, real-application/data workflows, and release
  construction/package-content/clean-installation gates. Each keeps the original
  40-minute job limit; all pre-existing commands, environment conditions,
  assertions, case sets and private-session deadlines remain intact. This is
  parallel orchestration on separate CI machines, not concurrent heavy checks
  on the user's memory-constrained desktop or a retry-until-green policy.
- Package handoff uses a clearly named **unaccepted staging** artifact from the
  same run and SHA, retained for one day. Only a final job requiring all three
  lanes, formatting, full-history secret scanning and both desktop matrix jobs
  may upload the original accepted `suzaku-linux-ARCH-SHA` artifact (14 days).
  A staging artifact, an earlier partially passing run or a source tag alone
  never authorizes publication. Keep the existing aggregate check display name
  for required-status compatibility; isolate cache-write keys by lane.
- Independent review finds an important required-check edge: a default-success
  dependency join would be skipped after upstream failure, which GitHub treats
  as not blocking even a required check ([official conditions reference](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-jobs-with-conditions)).
  The final job must therefore always report a result, with a mandatory first
  step rejecting any missing, failed, cancelled or skipped upstream result.
  All later checkout/download/binding/upload steps retain default-success
  behavior and are skipped after that failure. This is always **reporting**,
  never always **publishing**; freeze the exact ordering and fail-closed body.
- Add source-level graph regressions for preserved gate groups, unchanged
  limits, exact-SHA handoff, mandatory failure propagation and fail-closed final
  dependencies. These checks
  cannot qualify actual parallel CI, native input or package installation.
  The new exact clean commit must pass every lane and the final aggregate
  before tagging/publishing. Version remains 0.8.7 / build 37, with unchanged
  product code, Cargo dependencies, optional packs and personal installation.
- The local private custom-shortcut gate and all nineteen UI/instance checks
  also pass on the format-follow-up source. Their first results are retained
  in `custom-shortcuts-first.log` and `ui-first.log`; they do not replace the
  new exact-commit remote release gates or human daily-desktop acceptance.
- The revised graph guard first passes all **18 tests** on both Python 3.12.3
  and 3.14.4. Its actual frozen result predicate admits one all-success input
  and rejects 43 failed/cancelled/skipped/missing-result or invalid-ID cases
  with nonzero exits. Two independent source comparisons confirm all original
  gate/setup/global hashes, plus the binding body, are unchanged. Preserve
  `/tmp/suzaku-ci-release-graph.B24KYv` and its separate initial sixteen-test
  design logs; those earlier logs do not validate the corrected required-check
  behavior. No local heavy rebuild or installed-user-state change is needed
  for this CI-only change; complete remote execution remains mandatory.

### Unreleased — Late prediction errors and long-draft diagnostics 2026 10 11

- Follow the preceding late-reply audit into the shared error branch. Two
  independent read-only reviews find no new confirmed production defect:
  ordinary errors and caught provider panics use the same stopped/generation
  publication gate. Successful context undo already clears obsolete work and
  diagnostics; no-history undo intentionally preserves them. Selecting a local
  row after a completed failure retains model-health feedback, not a stale
  in-flight error. Do not change these semantics merely to remove a diagnostic.
- Append four tests / 28 fixed workflows to the existing engine module without
  changing its original seven tests / 28 workflows, helpers or deadlines.
  Exact first 508 lines / 21,016 bytes retain SHA
  `cd558a00d4281a00f5095e713614c24a8af4c83064b91bac1e8b2ec1879286b4`
  before and after formatting. The module now has 11 tests / 56 workflows.
  All provider waits remain three seconds and result waits two seconds.
- Sixteen late-error paths cover long edit/restore ABA, multibyte 256→257→256,
  cleared-field same-draft recreation and pending context undo, each in EN/ZH
  and plain/IBus-mixed engine modes. Release obsolete typed HttpStatus(429),
  then require a fresh request on the same single worker before checking exact
  whole frame/pool, Pending and no diagnostic. The fresh provider stays blocked
  during the negative check; its later exact Model/Sentence success cannot mask
  an incorrectly published error. ABA request inputs match completely.
- The other twelve paths positively accept current HttpStatus(404) before
  selecting the exact complete Local literal row. Successful history undo
  removes only committed context, clears the diagnostic and becomes Idle;
  no-history undo keeps the entire Unavailable/error/choice state unchanged.
  A subsequent deliberate reseed clears the old diagnostic and accepts a new
  complete prediction. Provider-none undo stays Disabled and preserves the
  complete selected pool through both undo outcomes, one exact commit,
  duplicate rejection and engine context/history clearing. This last fixture
  does not exercise disabling an already-in-flight provider.
- The first all-feature run passes all 92 tests across eight related suites:
  conflict 11, history 4, core 13, prediction 27, long completion 13, protocol
  16, preservation 5 and English spacing 3. No production repair, retry or
  assertion/deadline weakening is needed. Separately, first default and first
  inline-preedit native fallback gates each pass all 106 real loopback failure
  workflows: 20 English, 20 Chinese and 13 Japanese examples × HTTP 503/timeout.
  Pending/Unavailable keep immediate local words/sentences, numeric adoption,
  exact spelling undo, Space continuity and single exact commits. Update the
  contributing guide's outdated 16-example/32-workflow count; no cases changed.
  Each native gate retains its 120-second private-session deadline.
- Receipts are retained in `/tmp/suzaku-prediction-error-audit.3Eigl7` under
  serial task-owned 2 GiB high / 3 GiB max / zero-swap scopes. Whole-file SHA
  checks confirm engine/worker, native fixture/runner, C bridge/language and
  slow-fsync shim are unchanged. Record evidence without increasing maturity.
  These new 28 engine cases do not qualify provider-panic recovery, transport
  cancellation or native keystrokes; the separate fallback runs do not replace
  the complete native core/controlled-save/GTK/physical-input gates. No no-GPU
  build retry, live-model/Wayland acceptance, installation, desktop input,
  personal-settings write, commit, push or release.
  Strict all-feature/all-target Clippy, formatting and diff checks pass;
  tensor validation passes 21 checks with 68 functions / 77 cells unchanged,
  and all 250 audit-report links resolve. Update the source fingerprint and
  evidence only. Daily service remains active at PID 1549795 with Rime selected.

### Unreleased — Late prediction and committed-context undo 2026 10 11

- Audit delayed model replies during manual editing using a dedicated
  provider-neutral engine suite, not another shared native helper. Independent
  reviews identify a shared-engine bug: successful committed-history `undo()`
  changes the context captured by a pending prediction without invalidating
  its generation. This is not native/panel completion-adoption undo, which
  already replaces the seed and cancels predictions through its own history.
- The first all-feature regression run passes five tests and fails exactly the
  two context-undo tests: an observed in-flight cancellation token remains live,
  and an already-ready prediction retains Ready status after its context is
  removed. Preserve this red receipt; do not change test or provider deadlines.
  Fix only successful history undo by cancelling prediction after the history
  pop succeeds. No-history undo remains a strict no-op. Keep the entire live
  draft, candidate pool, source/kind, selected index and explicit-choice lock;
  do not rebuild/restart, clear displayed model choices or alter adoption undo.
- Add seven fixed tests / 28 EN/ZH × plain/IBus-mixed engine workflows: long
  byte-identical edit/restore ABA, selected-row cancellation then edit/unlock,
  multibyte 256→257→256 scalar-budget ABA, old-field context clear plus same
  draft recreation, pending context undo, no-history Pending undo, and ready
  selected-model context undo with exact once-only commit. The provider ignores
  cancellation deliberately. After releasing the old reply, receipt of the
  next request proves the same worker processed it before whole-snapshot and
  whole-candidate negative assertions. ABA inputs match exactly, so input
  rejection cannot mask generation bugs. Every live fresh response must add
  the exact Model/Sentence row; cancelled-status polling is not a drain oracle.
- The first post-fix all-feature run passes all 88 tests across eight suites:
  new conflict 7, commit history 4, engine core 13, prediction 27, long draft
  completion 13, protocol 16, preservation 5 and English spacing 3. Another 34
  host tests pass, including unchanged draft/history semantics.
  An additional no-GPU build fails before entering the new suite with 17
  existing unguarded GPU/winit import errors. This already-documented feature
  boundary remains unqualified; preserve its first failure separately, do not
  count it as a passing gate or broaden this undo fix into platform refactoring.
  The supported desktop validation uses `--all-features`. Receipts are retained in
  `/tmp/suzaku-prediction-edit-audit.WxFhuO`; builds stay serial in task-owned
  2 GiB high / 3 GiB max / zero-swap scopes. No deadline/assertion weakening,
  lexicon/native-fixture change, installation, desktop input, personal-setting
  write, commit, push or release. New engine cases do not qualify HTTP socket
  cancellation, physical keys, native display modes or real-model quality.
- Separately, the first existing private native core partition passes in both
  default and inline-preedit modes, including old socket-cancellation, length,
  row/page, long local/model completion, adoption/Compose undo, committed
  context, preference and focus/privacy/restart workflows. Do not confuse
  this with the complete core-plus-HTTP-failure gate or a rerun of controlled
  settings-save/GTK/physical-input tests. Whole-file SHA checks confirm the
  native runner, fixture, C bridge/language and slow-fsync shim are unchanged.
  Independent continuation/model-fixture guards pass 16/19 tests; tensor checks
  pass 21 and audit-report links resolve all 250 references. Update evidence
  and source fingerprint without increasing the 68-function/77-cell scores.
  Daily service remains active at PID 1549795, with Rime still selected.
  Strict all-feature/all-target Clippy, final formatting and diff checks pass.
  The unchanged CI all-feature test command discovers the new integration
  module automatically; no CI/runner rewrite or remote run was needed.

### Unreleased — Word-to-sentence editing during custom language saves 2026 10 11

- Continue the long-draft custom-language audit with one appended helper and
  one exact adjacent call in the controlled gate. Read-only production and
  independent fixture reviews find missing combination coverage, not a new
  confirmed defect. Leave all production C/Rust, lexicons, shared helpers,
  runner, isolation, one-second language deadline and old matrices unchanged.
  Exact append-only projection recovers the whole pre-edit native SHA
  `811d7c23545c35c9508477f44d186699de2b2b0806fc1df24dc1d20c09d84670`
  and the four pre-edit historical guards, without replacing old assertions.
- Add 16 fixed workflows: standard/home-row × EN→ZH / ZH→EN × successful/
  failed saves × held sentence/continued literal edit. Authored English uses
  `Earlier note. ` × 30 + `please sen` → `please send` →
  `please send me the details.`; Chinese uses `前文。` × 100 + `hui yi` →
  `会议` → `会议 什么时候开始？`. The explicitly owned long prefix comes
  from a revision-bound companion action, with actual native tail events
  drained individually. Candidates must match full text, Local source and
  Word/Sentence kind; use configured row bindings, not display labels.
- Start the real durable-write barrier before adopting the word. While the
  language save is blocked, adopt via A38, preserve held L46 through modifier
  release/repeats, release A38, type Space, adopt the entire sentence, use
  clamped row/page gestures, undo exactly to full word-plus-Space, then release
  A38 and re-adopt. Every stage requires the original context/language, visible
  pending hint, one worker, unchanged disk bytes and zero commits. Edited cases
  then type x and Alt+7; the entire two-key editing/verification step remains
  under the unchanged 250 ms bound, not just the individual key calls.
- Success keeps the latest entire sentence or edited draft, persists the
  target language, retires the old context/actions and discards old undo.
  Actual fsync EIO keeps original language/disk/context and the latest sentence
  undo: Backspace restores complete word-plus-Space, not the original raw
  reading. This is deliberately different from successful `sentence[:-1]`;
  never use the coincident `please sen`/`please send` single-delete result as
  the success/failure oracle. Edited cases in both branches delete only 7,
  then x, then the sentence's final scalar, without reviving old undo.
- Held adoption/language codes, changed keysyms and wrong releases remain
  inert across acknowledgement and clamped row/page boundaries. Wait for zero
  workers/no temporary files, release matching held keys, keep complete text
  through Space, explicitly select its exact local literal row, then Enter
  commits once and retires context; release/empty Enter cannot duplicate it.
  Cleanup preserves primary errors and fully restores private settings plus
  persisted JSON before reporting PASS.
- First default and first inline-preedit controlled gates each pass all old
  75 workflows plus the new 16; no native retry or production repair was needed.
  The new independent source-only guard passes 16 checks; historical boundary/
  custom-language/held/model-fixture/language/keypad/custom-long/keyboard-scope
  checks pass 15/13/18/19/13/13/11/5. All first full guard runs pass. Projection
  also freezes exact added support blocks, so unrelated new assertions cannot
  be silently stripped while preserving only an old hash.
  Strict all-feature/all-target Clippy, formatting and diff checks pass. Wire
  the independent source-only guard into the existing CI block and update
  evidence/source fingerprint without raising maturity scores. Receipts live
  in `/tmp/suzaku-custom-continuation-audit.QWrXFY`; heavy gates remain serial
  in task-owned 2 GiB high / 3 GiB max / zero-swap scopes. Daily service stays
  PID 1549795 with Rime selected. No installation, desktop input, personal
  settings change, commit, push or release.
  This turn does not rerun the full Rust/native/HTTP/physical GTK suites or
  qualify live-model/physical-keyboard/Wayland quality. Acknowledgement while
  stopped at the undone word-plus-Space, other binding choices and late model
  candidates during manual editing remain distinct follow-ups.

### Unreleased — Custom language ownership and Compose boundaries 2026 10 11

- Extend the preceding private long-draft audit, not the desktop installation.
  Independent lifecycle review finds an untested combination, not a confirmed
  production defect. Append one helper and one exact call after the existing
  controlled long-custom helper. Keep every old workflow, shared helper,
  deadline, runner and production C/Rust/lexicon byte unchanged. An independent
  guard precisely removes the addition to recover whole-native SHA
  `918bc1306587da6294de8f08e3f3b0afe77ca9be464c8659818578c5af6fa78c`
  and projects three historical guards back to their pre-edit hashes.
- Add 28 authored workflows: two shortcut profiles × both EN/ZH directions ×
  explicit FocusOut, direct transfer to another engine without prior FocusOut,
  password, PIN, private hint, pending Compose and completed Compose. Use the
  same complete six custom bindings and distinct long `hel`→`hello` /
  `hui yi`→`会议` undo oracles. A revision-bound owned prefix precedes drained
  actual native tail, adoption and Ctrl+Shift+L events; do not infer physical
  typing of the prefix or real-model quality from these fixtures.
- The actual fsync barrier retains one settings worker and P0 reports busy
  after cancellation. Original disk bytes and language stay unchanged.
  Focus/privacy boundaries retire the old context, clear its long draft and
  allow new public native letters with old held codes 38 and 46. After a
  direct transfer, late old-engine Reset, FocusOut, password ContentType and
  keys leave the new owner's complete fresh frame and both commit lists intact.
  Password/PIN keys return unhandled; private-hint local typing is handled but
  every snapshot stays redacted, then returning public retires that private
  context too. These are deliberately different assertions.
- Beginning Compose cancels the pending save without retiring the context.
  Pending Compose first releases both held codes, then checks physical
  modifiers and registered custom chords fall through with an unchanged whole
  frame. Two Backspaces first cancel the unfinished sequence, then restore the
  entire raw spelling. Completed `é` keeps the exact full draft; deleting the
  accent then the adopted word's last scalar cannot revive old undo. Do not
  assert the replaced default Ctrl+Shift+Space is inert during Compose: it is
  an ordinary system shortcut and follows the existing cancellation path.
- Both first default and first inline-preedit controlled gates pass their
  unchanged 47 workflows plus all new 28. After worker exit require zero
  workers, no temporary files, identical settled frame and unchanged disk;
  Space preserves full text and deliberate exact selection/Enter commits once
  to only the owning engine. PASS follows successful private settings/watcher
  restoration; primary errors remain primary. Receipts are in
  `/tmp/suzaku-custom-boundary-audit.HWFmex`.
- The independent source-only boundary guard passes 15 checks; historical
  custom-language/held/model-fixture/language/keypad/custom-long/keyboard-scope
  checks pass 13/18/19/13/13/11/5. All first complete runs pass, without native
  retries or production fixes. Strict all-feature/all-target Clippy, formatting
  and diff checks pass. Wire
  the independent source-only guard alongside existing checks and update
  evidence/fingerprint without raising maturity scores. Heavy native gates
  remain serial in task-owned 2 GiB high / 3 GiB max / zero-swap scopes. The
  daily service remains PID 1549795 with Rime selected; no installation,
  desktop keys, personal configuration changes, commit, push or release.
  This turn does not rerun the complete native/HTTP fallback, physical GTK or
  full Rust suites. Same-engine repeated FocusIn, pending successful-switch
  word-to-sentence editing, physical keyboard/Wayland ergonomics and real-model
  quality remain distinct follow-ups.

### Unreleased — Custom language saves on long drafts 2026 10 11

- Continue the preceding shortcut audit with two appended helpers and three
  exact adjacent calls in the native fixture. Independent review finds a
  missing regression matrix, not a confirmed production defect. No C/Rust,
  lexicon, worker, runner, isolation or deadline changes. The new source-only
  guard removes exactly these additions to recover the complete pre-edit
  native SHA `b101512a7dadccfa924ee7a7835d21be684f3904d2b8f9237dffebc8bf0f3dc8`.
  It also precisely projects the two adapted historical guards back to their
  pre-edit hashes; their old matrices and assertions remain frozen.
- Four normal workflows cover standard/home-row × EN→ZH / ZH→EN with all six
  custom bindings. Owned revision-bound long prefixes precede actual native
  tail keys and custom adoption/language events. Fixed `hel`→`hello` and
  `hui yi`→`会议` distinguish successful ordinary deletion from complete raw
  undo. Success preserves the entire draft, retires old undo/context actions,
  keeps held physical identities through inert page/row boundaries and allows
  immediate reuse after the matching release. Space continues the full draft;
  an explicit Enter submits it once, without an empty-Enter duplicate.
- Sixteen controlled workflows cover both profiles/directions × success,
  real fsync EIO, Reset cancellation and explicit full-draft commit cancellation.
  A real durable-write barrier checks bounded native response, worker count one,
  busy save-channel rejection and unchanged disk bytes. Failure retains the
  original language and full raw undo; success persists the language and clears
  old undo. Cancellation retires the context without a late language apply;
  new native typing works while the canceled worker still holds its permit.
  After release require worker count zero and no temporary settings file.
  Commit-cancellation checks two deliberate submissions: the original full
  draft at cancellation, then the independently edited new-context draft.
- The first default normal and controlled gates both pass. Tighten only the
  new commit-cancellation fixture to select the exact full draft before Enter,
  avoiding an assumption about the first candidate; do not alter ranking.
  Final default and inline-preedit normal gates each pass old 26+12+4 and new
  four cases. Final default and inline controlled gates each pass old 27+4
  and new sixteen cases. The full default non-fallback native core also passes,
  including recovery and both normal additions. This is not a rerun of the
  separate 106-case HTTP fallback gate or the complete native matrix.
- Root source-only checks pass custom-language/held/model-fixture/language/
  keypad/custom-long/keyboard-scope 13/18/19/13/13/11/5 respectively. Strict
  all-feature/all-target Clippy, formatting and diff checks pass. Add the new
  independent guard to the existing CI check block; this is local validation,
  not a claim that remote CI ran. Update linked evidence and the source
  fingerprint without raising any maturity score.
- Receipts remain in `/tmp/suzaku-custom-language-audit.fo2SnG`. Heavy gates
  run serially in task-owned 2 GiB high / 3 GiB max / zero-swap scopes; all
  native sessions use private buses and settings. No installation, commit,
  push, release, desktop input or personal configuration changes. Custom long
  drafts with Compose/privacy/focus cancellation, adoption/word-to-sentence
  editing during a successful pending switch, physical keyboard/Wayland
  ergonomics and real-model quality remain separate follow-ups. This turn
  does not rerun the full Rust or physical GTK suites.

### Unreleased — Long drafts with custom shortcuts 2026 10 11

- Read-only audits find a coverage gap, not a confirmed production defect:
  the private custom shortcut gate previously used short `hel`/`nihao` drafts.
  Append `run_long_checks` and one call immediately after the unchanged old
  `run_checks`. Keep the whole old fixture, runner, preflight, deadlines and
  six phase groups unchanged; no C/Rust implementation or lexicon edits.
- Add ten fixed workflows: standard/home-row × English/Chinese × word/sentence
  (eight), plus a genuine long-English multi-page case per profile (two).
  English uses `Earlier note. ` × 30 + `please sen`; Chinese uses `前文。` × 100
  + `hui yi`. Expected complete word/sentence payloads are authored constants,
  selected by exact text, Local source and kind, never by display labels.
  The revision-bound companion prepares an explicitly owned prefix; actual
  IBus events type each tail character with bounded frame drainage. These are
  not claims of physical typing of long prefixes or free-form model quality.
- Exercise configured Ctrl+N/P rows, Ctrl+Shift+F / Alt+Shift+B pages and
  Alt+Shift+A adoption. Check held code 38 with changed keysyms/modifiers,
  wrong and matching releases; clamped row/page operations compare complete
  fresh snapshots and retain adoption undo. Backspace restores the entire raw
  draft. Space retains the full text; word cases continue to a fixed sentence,
  undo exactly to the adopted word plus Space, then re-adopt. Alt+7 stays
  literal. Enter emits exactly one full CommitText with a retired context;
  release and a second empty Enter cannot duplicate it. Long `note ` × 60 +
  `hel` requires more than six rows, traverses all pages and both inert boundaries,
  then adopts/undoes the complete `prefix + hello`.
- Preserve the first failure (`native-custom-first.log`): all ten input actions
  finish, but this new helper incorrectly tries restoring `language` with the
  deliberately narrow U protocol. The host correctly rejects that field.
  Repair only the new cleanup to restore with separate L then narrow U; require
  both current settings and complete persisted JSON to equal the saved settings.
  Primary failures remain primary; a cleanup failure after successful assertions
  fails the gate. Publish the new PASS only after successful restoration.
- Final default private IBus gate passes all old phase groups and ten new cases.
  The separate source-built GTK/XTest custom gate passes its unchanged seven
  groups plus full-U, atomic rejection/clear and actual settings-window
  recording/acknowledgement/clear/close. The compiled C adapter passes seven
  checks; focused Rust tests pass 27 (custom settings 8, long completion 13,
  English window 6). Strict all-feature/all-target Clippy, formatting and diff
  checks pass. Existing language/keypad/held/model-fixture/keyboard-scope guards
  pass 13/13/18/19/5 respectively. The new independent source-only guard passes
  eleven checks and projects
  only the helper and exact adjacent call out to recover the frozen old whole-file
  SHA `ea2c9e0ed59a791d479dc46723142fffa4468fb7d4128ac5ae6ad29a15db4dc1`;
  it also checks the fixed matrix, actual chord paths and strict cleanup.
- Evidence and the implementation/test source fingerprint are updated without
  changing any maturity score. Logs live in `/tmp/suzaku-custom-long-audit.4ijcsB`;
  synthetic GTK files remain in `/tmp/suzaku-app-qa.zQ7iv6`. Builds/native gates
  run serially in task-owned 2 GiB high / 3 GiB max / zero-swap scopes. Daily
  service remains PID 1549795 and the selected desktop engine remains Rime.
  No installation, commit, push, release or personal configuration change.
  This turn does not rerun the complete native/model-fallback matrix, legacy
  49+4 GTK gate or all Rust tests; long-draft custom language switching,
  controlled write failures/cancellation, real Wayland delivery and physical
  keyboard comfort remain separate follow-ups.

### Unreleased — Held adoption across language and paging 2026 10 11

- The first new private-engine probe passes all old 26 language cases and
  twelve adoption workflows, then fails at a clamped PageUp after a successful
  keyboard language switch (`native-language-first.log`). A diagnostic run
  preserves the exact defect: only companion revision changes, 597→598
  (`native-language-red-diagnostic.log`). The completed-feedback clear calls
  full render before the otherwise inert page branch. Change only that call
  to presentation render: dismiss the hint without republishing input state
  or emitting another inline-preedit update. No draft, key semantics, worker,
  deadline, held-key set, candidate ranking or language algorithm change.
- Append four fixed normal workflows: standard/home-row × EN numeric `hello`
  adoption across ZH acknowledgement and clamped PageUp, or ZH literal `hel`
  Shift+Enter across EN acknowledgement and genuine PageDown→6/PageUp→0.
  Same physical-code repeats, changed keysyms/modifiers and other releases
  cannot edit, adopt or commit. Success retires old-language undo (`hello`→`hell`);
  a released key can immediately edit/adopt again, with exact new undo, Space
  continuity and one explicit final commit. The first repaired run exposes a
  new fixture's wrong assumption that the Chinese literal row is first
  (`native-language-fixed-first.log`); select the exact local Literal `hel`
  through the unchanged relative helper instead of changing ranks or text.
- Append four controlled slow-save workflows: both profiles × success/failure,
  with numeric `hello` adoption held before the language chord and through its
  durable-write barrier. Assert progress, bounded input response, exact disk,
  context and draft, pending/settled boundary snapshots, held repeats, success
  `hell` versus failed-save `hel` undo, matching-code release and exact commits.
  The first default slow gate passes its old 27 and new four cases. The first
  inline slow gate records a new observation race, revision 250→251 at the
  settled boundary (`native-pending-inline.log`): the success callback publishes
  once when rebuilding the draft and again for the acknowledgement hint.
  A client-only cached frame may lag that second publication. Read fresh host
  snapshots before and after each gesture, synchronizing the original watcher
  before the key, while keeping complete-frame/revision equality and deadlines.
  Independent read-only audit confirms this cause; no extra production change.
- The final default core gate passes its entire original non-fallback partition
  plus all four normal additions (`native-core-default.log`). This is not a new
  run of the separate 106-case HTTP fallback gate. Portal startup warnings stay
  in every receipt. The final inline language gate passes old 26+12 and new four
  (`native-language-inline-final.log`); the final default/inline slow gates
  each pass all old 27 plus new four (`native-pending-default-final.log`,
  `native-pending-inline-final.log`). Neither new helper replaces an old matrix
  or modifies shared typing, selection or peer code.
- Eighteen independent [held-language guards](scripts/test-held-language-coverage.py)
  pass (`held-static-root-final.log`). Remove only two new helpers and three
  exact adjacent calls to recover the complete prior native-file SHA; freeze
  eleven old functions, the worker and runner. C projection permits only the
  one presentation-render substitution and requires it in current source.
  Thirteen language and thirteen keypad guards retain their old hashes/matrices,
  updating only the exact status-render expectation/projection. Nineteen HTTP
  fixture guards retain their old core prefix and tail digests after deleting
  only the three new argument/order-checked calls; all HTTP 53-pair assertions
  and deadlines remain fixed. Actual static shape failures are retained in
  `language-static-red.log` and `partition-static-red.log`, not hidden by new hashes.
  Add the new held guard alongside the existing two CI commands; YAML parsing
  verifies all three. No claim that remote CI has run.
- Extend only the extracted adapter's render stubs/counters and existing
  feedback assertions: dispatch makes no feedback-dismissal render call while
  saving, and completed/failed feedback dismissal calls presentation once,
  full render zero times. Input-edit helpers are stubs here; this compiled check
  does not measure their normal real draft/preedit rendering. All seven checks
  pass (`adapter-final.log`), retaining five structure tests and the old seven
  adapter subcases. The first actual owned XTest→GTK gate passes all unchanged
  49 workflow groups plus the unchanged four keypad flows (`keyboard-default.log`);
  this does not add physical coverage of the eight new held-language combinations.
  Five physical-custom scope guards, 21 tensor checks and all 250 repository audit
  links pass. The latter runs against the working tree, not a newly built package.
  Strict all-feature/all-target Clippy passes in 27.84 seconds with warnings
  denied; formatting and whitespace checks pass. No kernel OOM entry is recorded
  during this window. The daily service remains running as PID 1549795 and the
  desktop input method remains Rime; no personal input state was altered.
  Source SHA is `ce5f27b47b9341e19bc5119f28d413724438c27d3c354689152d7233c6c758c8`;
  all 77 maturity cells stay unchanged. Receipt directory:
  `/tmp/suzaku-held-transition-audit.BJKfKB`. A misspelled scope property and a
  missing link-checker argument were rejected before their requested validation;
  those invocation logs are retained separately, not counted as successful checks.
  Heavy gates are serial with one build job, 2 GiB high / 3 GiB max / zero-swap
  task-owned scopes. No install, release, commit, push, personal input-method
  switch, physical-hardware or real-model acceptance.

### Unreleased — NumLock-off keypad navigation 2026 10 11

- A new fixed private-engine probe reproduces `KP_Up` falling through despite
  a live draft (`native-red.log`); all old row-boundary checks pass first.
  NumLock-on keypad digits are already explicitly literal input, not numbered
  candidate keys, so do not change that contract. This is a bounded ergonomic
  compatibility addition, not a claim that old keypad-navigation coverage failed.
- Add only `KP_Up/Down/Left/Right/Prior/Next` to the existing row/page branches.
  Keep raw keysyms through Compose first, then use the unchanged move/page
  helpers. Clamped rows and missing pages retain the entire frame and adoption
  undo; genuine navigation remains deliberate. No physical-keycode remapping,
  new input hook, NumLock/LED change, keypad Home/End/Insert/Delete support,
  settings worker, dictionary or Rust input algorithm change.
- Append 24 fixed native cases: two profiles × EN/ZH × six navigation keysyms.
  Check real relative movement, page starts, complete fresh snapshots at the
  boundaries, unselected Space spelling, immediate adoption undo, no commits,
  and empty/password/numeric/modifier ownership. Default and inline row gates
  both pass the old matrix and all 24 additions (`native-default.log`,
  `native-inline.log`). The first default native core gate passes its complete
  non-fallback partition (`native-core-default.log`); the separate 106-case HTTP
  fallback gate is not rerun this turn. Preserve portal warnings and all limits.
- The first actual private XTest→GTK keyboard run passes all old 49 workflow
  groups plus four additions (`keyboard-default.log`). Use previously cached
  physical KP2/4/6/8/3/9 positions with NumLock off, not injected IBus keyvals.
  EN/ZH row/page movement, boundary snapshots, adoption/undo, Space and exact
  single application commits pass; the observed editor stays empty until Enter.
  XKB state/keymap are unchanged. A new fixture's initially authored language
  wait was corrected in review before execution: repeated equal-value `L` can
  preserve revision, so confirm persisted/frame language and the empty draft,
  not a forced revision increase. No legacy assertion or deadline was changed.
- A separate adjacent check exposes a previous-turn fixture omission:
  the extracted real C adapter now reads `language_job`, but the adapter test's
  stub struct did not declare it. Preserve the actual compiler failure in
  `custom-native-static.log`. Add the matching presence-only pointer, reset it
  between fixtures, and a new adapter test for pending feedback surviving input
  and release, followed by normal completed/failed feedback clearing. All five
  structure checks and both compiled-adapter tests pass (`custom-native-final.log`);
  the old seven adapter subcases and two-second bounds stay unchanged.
- Thirteen new [keypad guards](scripts/test-keypad-navigation-coverage.py) pass
  (`static-root-final.log`). Exact source projection restores the pre-change
  whole C and GTK files; old 49/custom-7 globals/helpers, digit path, Compose
  handling and held/page/multilingual matrices remain fixed. Guard the six-key
  after-Compose placement and fixed 24/4 matrices. This static order check is
  not execution of a custom Compose sequence containing keypad navigation.
  Existing nineteen HTTP-fixture, thirteen language-workflow and five physical
  custom-scope guards pass. Add both language/keypad guard commands to CI without
  replacing its old stages; parsed YAML confirms the new stage's exact commands.
- Strict all-feature/all-target Clippy passes in 29.00 seconds; workspace
  formatting and whitespace checks pass. The tensor freshness check, all 21
  tensor guard tests and all 250 packaged audit links pass. No kernel OOM entry
  appears during this validation window. The daily `suzaku-ibus.service` remains
  running and the selected desktop input method remains Rime.
- Source SHA is `450ff3f74e47ce0a0f3c370d62e9afba118351a2e2589e430b8b7064fecdbaa3`;
  all 77 maturity cells stay unchanged. Receipt directory:
  `/tmp/suzaku-keypad-flow-audit.0nFQOH`. Heavy validation remains serial with
  2 GiB high / 3 GiB max / zero-swap scopes. No new physical-hardware,
  installed-version, native Wayland, arbitrary keypad/Compose, real-model or
  remote-CI acceptance. Long-held adoption across keyboard-language switching
  and real paging was still a separate combined probe in this keypad-only turn;
  the subsequent held-language chapter above records that coverage. It is not
  inferred from these independent chains. No install, release, commit or push.

### Unreleased — Language-switch adoption workflows 2026 10 11

- A controlled private IBus probe reproduces an auxiliary-feedback defect:
  while a keyboard language switch is still saving, numeric word adoption
  prematurely clears `正在切换输入语言…`. All nine existing slow-settings
  and ten delayed-language cases pass before the new `en/success-adopt` case
  fails (`native-pending-red.log`). Preserve this actual RED separately from
  the independent static guard's earlier shape failures (`static-red.log`).
- Change only the ordinary-key feedback-clear condition to require no live
  `language_job`. Adoption, undo and further typing retain the progress hint
  while saving. Success/failure/timeout still retire the job before publishing
  feedback; focus, Reset, privacy, commit and Compose independently cancel and
  clear it. No input-loop disk work, language protocol, worker-permit, key
  semantics, one-second language deadline or native-session deadline changes.
- Append two private-engine helpers without replacing old coverage. Twelve
  fixed success workflows cover both shortcut profiles, EN→ZH / ZH→EN and
  an ASCII-first mixed `API。nihao` prefix. Each deliberately adopts Word and
  Sentence rows via page-local numbers, restores exact spelling with Backspace,
  adopts again, continues with Space and Alt+7, deletes three Unicode scalars
  without reviving old undo, and commits the exact local payload once. Empty
  Enter remains application pass-through. Same-payload English rows can be
  Word rather than Literal; their exact text/source is the commit contract.
- Eight controlled pending workflows cover adoption, adoption→undo,
  adoption→ordinary edit, and failed save in both directions. Assert visible
  progress before adoption, immediately afterwards, and after undo/edit,
  all before releasing the `fsync` barrier. Success keeps the latest draft and
  retires old-language undo; failure keeps old language, exact on-disk settings
  and unchanged adoption undo. Cleanup releases the private worker and retains
  the primary failure instead of masking it with teardown errors.
- Default and inline focused language gates pass the unchanged 26 cases plus
  twelve new workflows (`native-language-default.log`, `native-language-inline.log`).
  Both slow-I/O modes pass the unchanged nineteen plus eight new workflows
  (`native-pending-default-final.log`, `native-pending-inline.log`). The earlier
  repaired default run remains separately retained (`native-pending-default.log`).
  The first complete default IBus gate passes core plus all 106 controlled HTTP
  fallback cases (`native-complete-default.log`). Inline coverage is focused,
  not a claim that its complete HTTP gate ran. Preserve portal startup warnings.
- The independent [workflow guard](scripts/test-language-workflow-coverage.py)
  passes thirteen checks: pre-edit hashes freeze both old language matrices and
  three shared typing/selection/private-peer nodes; projection permits only the
  exact two appended calls. Fixed new matrices, ordered adoption/commit checks,
  pending feedback stages and branch-specific undo are protected. Nineteen
  existing native HTTP-fixture guards and 21 tensor checks pass separately;
  these static checks are not native execution. All 77 maturity scores stay
  unchanged. Current implementation/test source SHA is
  `b520bbdf862a72644e3c698060fa58cd9959404766e5fc40da4604b537d6157d`.
- Receipt directory: `/tmp/suzaku-language-flow-audit.e5z2vR`. Heavy checks
  remain serial in task-owned 2 GiB high / 3 GiB max / zero-swap scopes.
  Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy-final.log`, 28.32 seconds); formatting and patch whitespace pass.
  This turn does not modify Rust input algorithms, dictionaries, settings
  workers, version/lock files, the CI runner or the slow-write fixture.
  Private Unicode protocol typing is not physical GTK/keyboard acceptance,
  real-model quality, a new installation or remote CI. The documented external
  settings-write publication window is not claimed fixed. No install, release,
  commit or push; the personal service and engine remain untouched.

### Unreleased — Chinese mixed-literal draft boundaries 2026 10 11

- An actual two-case probe confirms `已确认 API。nihao` has only its Literal
  choice in a short draft, while the same suffix after a long frozen Han prefix
  already converts to `你好`: one pass / one failure (`probe-red.log`). The
  long case is a positive control, not a second newly repaired defect.
- Add failure-only short routing: when a draft has at most 256 Unicode scalars
  and the whole decoder has no non-Literal conversion, reuse the existing
  written Han/CJK boundary and decode its tail once. Freeze the exact raw prefix,
  including acronyms, case, digits and Unicode. Share the old long-draft boundary
  and protected-token check; keep authored-progress routing ahead of that fallback.
  No recursive hint, new language detector, ASCII boundary splitting or unknown
  word inference. Successful whole conversions, local kinds, old homophone ranks,
  beam/completion caps, twelve displayed choices and model budgets are retained.
- First focused regression passes 59 checks (`focused-first.log`). The independent
  expanded matrix initially records thirteen passes / two failures
  (`matrix-first.log`). One new fixture incorrectly expected the exact context
  `我想学习中文` to match across an inserted internal Han-space; retain its
  no-gap sentence expectation, and correctly require the existing shorter `中文`
  sentence for the spaced case. Word/Literal/raw-prefix assertions stay unchanged.
  This is a fixture correction, not a claimed production fix for gap matching.
- The other failure exposes an older closing-quote hole: `zaishiyi'` consumes
  its terminal apostrophe as the boundary before an untyped `ci`, manufacturing
  a quote-free `再试一次` Word and authored sentence. Reject only Prefix matches
  ending in an apostrophe. Exact readings still leave closing quotes to the
  decoder; real internal syllable separators, horizontal padding and tones remain.
  A separate unit guard checks Prefix/Exact boundary semantics. No quote assertion
  is relaxed; the first repaired quote check passes (`quote-fixed-first.log`).
- Seven focused Rust targets pass 72 checks (`focused-final.log`). Strengthened
  final [mixed guards](tests/chinese_mixed_boundary.rs) pass all fifteen
  (`matrix-final.log`): short/long and 255/256/257-scalar raw prefixes, partial
  words and existing sentences, exact submission/undo, adoption and Space,
  tone/numeric distinctions, protected tokens, invalid splits and seven vertical
  boundaries. Quote preservation includes bare/adopted readings, short/long mixed
  prefixes and trailing horizontal padding, without discarding a closing quote.
- Existing private native bilingual checks pass all 102 workflows in default
  and inline preedit sessions separately (`native-bilingual-default.log`,
  `native-bilingual-inline.log`): numeric adoption, exact spelling undo, continued
  typing and single commits. These unchanged IBus-protocol cases do not qualify
  the new Unicode mixed fixtures through GTK typing or a physical keyboard,
  nor replace complete native HTTP, installed-version or real-model acceptance.
  Preserved portal startup warnings are not hidden. Receipt directory:
  `/tmp/suzaku-mixed-boundary-audit.QSky0U`.
- The first full all-feature Rust run passes 1,702 tests / zero failures /
  thirty ignored across 125 result entries (`full-rust-first.log`,
  `full-rust-summary.log`). Ignored opt-in cases are not executed acceptance.
  Heavy build/native/test checks remain serialized in task-owned 2 GiB high /
  3 GiB max / zero-swap scopes; no failure log is overwritten or deadline relaxed.
  The ten fallback-vocabulary checks finish in 75.45 seconds, rather than
  hanging. Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy-first.log`, 27.77 seconds).
- Lightweight gates pass: 22 vocabulary partition guards, 21 tensor checks,
  nineteen native HTTP-fixture shape tests and six pack-shipping checks. These
  shape tests are not execution of the 106 native HTTP subcases. Workspace
  formatting and patch whitespace pass; all 250 repository audit links resolve,
  not a new package/install qualification. Current tensor source SHA is
  `f1cd5393bbceff4578b5c3edeb6df5d7cb1255a93a0e78dc85fee9022027f16d`;
  all 77 maturity scores remain unchanged.
- The resource audit confirms EN/JA/ZH JSON, Cargo version/lock and English
  source are byte-identical to this turn's start, and all fourteen packs match
  the prior frozen audit (`resource-guard-readonly-audit.log`). `draft.rs` is an
  intentional production refactor, not counted as an unchanged resource.
  Chinese changes are boundary recovery and the quote Prefix guard, not new
  vocabulary, personal history, C/host/core engine or model-protocol changes.
  Read-only desktop checks retain the active daily host at PID 1549795 and Rime;
  no kernel OOM entries are observed since this turn's 20:40 baseline. No version
  bump, install, service restart, commit or push.

### Unreleased — basic Chinese connectors and adopted input flow 2026 10 11

- The direct adopted-word path lacks two basic standalone readings. The actual
  pre-data RED fails all three independent tests (`data-red.log`): missing
  `hai -> 还` metadata/Word, and `优惠券hai zai` autocompletes to longer entries
  instead of offering Sentence `优惠券还在`. The loops stop at those first
  failures; they do not independently establish a second `hou` failure. Existing
  longer-word adoption and normal editing could already construct the Han context;
  these basic readings improve direct input rather than making physical testing possible.
- Append only Word `hai -> 还` and `hou -> 后` to the JSON resource. All 2,920
  previous reading records, flags/order and other fields are unchanged
  (`append-audit.log`); totals are now 2,922 readings / 687 contexts / 1,372
  sentences. Bare `hai` and `hou` intentionally prefer the new exact words;
  old whole-word primary choices remain, not necessarily every combined array.
  No `huan` alias, new `候` / `厚` entry, authored sentence or context.
  EN/JA JSON and Cargo version/lock remain byte-identical to this turn's baseline
  (`unchanged-core.sha256`), and fourteen packs are unchanged. Chinese production
  source is unchanged; its later unit-test count update is separately audited below.
  Vocabulary remains model/host/decoder-independent; no personal history or corpus.
- First focused Rust run passes 59 checks (`focused-first.log`), including the
  original three new expectations and all six adjacent targets. Five additional
  [guards](tests/chinese_function_words.rs) pass with the original three unchanged
  (`function-guards-first.log`, eight total): full old-data fingerprints, old
  whole-word/prefix priority, selectable longer completions, page limits,
  protected tokens, invalid splits, vertical boundaries and immutable Han prefixes.
- Add separate `adopted-transition` QA: two physical Pinyin/numeric bootstrap
  chains, each with and without Space before the partial tail (four variants).
  Start `优惠券` / `登录失败` is Word; composed `优惠券还在` /
  `登录失败后可以` is Sentence, followed by verified Word completion and an
  existing authored Sentence. The fixture checks exact stage-specific undo,
  empty editor before commit and one physical Enter's exact insertion through
  unchanged shared helpers. Tail candidates are required within twelve, not
  falsely promised on page one or necessarily forced onto page two.
  Static RED records twenty passes / two missing-scope/helper errors
  (`partition-red.log`); all 22 guards pass after implementation (`partition-root.log`).
  The old raw workflow and four shared helpers are fingerprint-locked, as are
  ALL167/PACK74 tuples, old loops/paging/pack preparation and the 240-second limit.
  This focused scope cannot report a complete builtin/pack/native-HTTP gate.
- Formatting initially reports only the new test's edition-2024 layout
  (`fmt.log`); formatting alone fixes it (`fmt-repair.log`, `fmt-final.log`).
  Assertions and semantic expectations remain unchanged. Receipt directory:
  `/tmp/suzaku-adopted-flow-audit.sMuTqz`.
- The first full Rust run records 1,684 passes / two failures / 30 ignored over
  124 result entries (`full-rust.log`, exit 101). Both failures are exact reading
  counts, actual 2,922 versus the previous 2,920, in the language unit test and
  migrated-resource test; no behavior assertion fails. Update only those two
  exact metadata baselines to 2,922, retaining all old fingerprints, ranks,
  context/sentence counts and behavioral assertions. Replacing that one unit-test
  count back to 2,920 recreates the starting Chinese source SHA exactly
  (`production-unchanged.log`), proving no production-code delta in this turn.
  The first targeted command accidentally filters out all 23 integration tests
  (`metadata-targeted.log`), so its exit zero is not acceptance. The correctly
  selected integrations execute and pass all 23 (`metadata-integrations.log`),
  followed by the previously failed continuation unit test, one pass
  (`metadata-continuations.log`). No assertion is weakened or deadline extended.
- Current two-reading panel/host/tool binaries were rebuilt before the native
  check (`build.log`). All four `adopted-transition` variants pass actual
  XTest -> GTK -> IBus input (`gtk-adopted-first.log`): both entirely physical
  Pinyin/numeric Han chains, with and without literal Space, Word/Sentence
  bridge types, each adoption's exact undo, selectable tail Word and authored
  Sentence, actual page-local adoption and once-only Enter's exact buffer/file.
  The live panel agrees with host revision/context/cursor and page rows while
  the editor stays focused at each observation and empty before commit.
  These cases select whichever page contains the target; they do not themselves
  promise a second-page target or qualify broader matrix spellings. Private files:
  `/tmp/suzaku-app-qa.x1Yo8h`. Portal fallback/inhibit warnings remain visible
  in the preserved log, not hidden or counted as candidate failures.
- Both original raw `transition` groups also pass in a separate fresh private
  session (`gtk-raw-transition.log`, `/tmp/suzaku-app-qa.L2Dv6G`), including
  the second 收拾 sentence's mandatory page-two numeric selection, Word/both
  Sentence adoption and exact undo, Space continuation and once-only saved commits.
  Neither focused session runs or claims complete builtin, all-fourteen-pack,
  native HTTP/model, installed-version or personal-desktop acceptance.
- Final all-feature Rust executes and passes 1,686 tests / zero failures /
  30 ignored across 124 result entries (`full-rust-final.log`,
  `full-rust-summary.log`). Opt-in ignored tests are not executed acceptance.
  The fallback-vocabulary target passes all ten cases in 76.16 seconds; it is
  not a hung native session. Heavy checks are serialized in task-owned 2 GiB
  high / 3 GiB max / zero-swap scopes; preserved failures are not overwritten.
  Lightweight gates pass: 22 partition guards, 21 tensor checks, nineteen
  native HTTP-fixture shape checks and six pack-shipping checks. The HTTP shape
  result is not execution of the 106 native HTTP subcases. Python/shell syntax,
  formatting and patch whitespace pass; 250 repository audit links resolve,
  not a new package/install qualification. Tensor source SHA is
  `17ea2814b467f54b4da09fd13bd9fa61527c61246bf249c3135a6384a3110637`,
  all 77 maturity scores unchanged. No version bump, install, commit or push.
- Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy-first.log`, 30.72 seconds). Final read-only desktop check still
  shows the original daily service active at PID 1549795 and selected engine
  Rime; the installed input service was not replaced or restarted. No kernel
  OOM entry is observed since this turn's 20:00 baseline. This does not diagnose
  older OOM incidents or turn private automated events into physical-user acceptance.

### Unreleased — adopted Chinese partial sentence progress 2026 10 11

- Fix an existing-data continuation gap, without another vocabulary batch:
  `登录失败后可以zaishiyi` already offers Word `登录失败后可以再试一次`,
  but its authored sentence `登录失败后可以再试一次。` was absent. The
  correct verified Word is the sixth decoder choice; both sentence emission
  and long-draft routing previously inspected only the first five. The actual
  independent [probe](tests/chinese_partial_sentence_probe.rs) RED is one pass /
  one failure (`probe-red.log`). `优惠券还在youxi` already passes because
  `有效` is enough sentence progress; keep it as a positive control, not a
  second claimed defect. The preliminary read-only decoder analysis was not
  evidence that both inputs failed.
- Reuse existing typed decoder choices: the original first five retain their
  order, followed by at most four already verified Word completion paths.
  Actual emission still uses the precise-primary-Word guard and exact authored
  suffix validation. The long-draft hint only retains the raw Han context;
  it cannot emit or inject completion text. No new last-Pinyin-span parser,
  reading, beam width, completion cap, score, page quota or model budget.
- The first focused run passes all 53 unchanged/new checks (`focused-first.log`),
  including the two probes and old care, sentence-progress, horizontal-reading,
  long-draft and vocabulary-quality suites. Nine further independent
  [regression tests](tests/chinese_partial_continuation.rs) pass
  (`matrix-first.log`): five spellings, six horizontal paddings, frozen
  café/emoji prefixes, 255/256/257/512-scalar boundaries, Word/Sentence/Literal
  once-only commits/undo, adopted-word progress, wrong Han, invalid splits,
  unknown tails, seven vertical boundaries, protected tokens and the unchanged
  `shijian` / `世间` first-page rule. These are engine checks, not physical typing.
- Add a separate focused `transition` GTK diagnostic for the previous raw
  inputs `zaishiyi` and `chumenqianshoushiyixi`. It does not append to ALL167
  or PACK74, replace the complete gates or run their unrelated progress controls.
  All old tuples, complete tuple hashes, three physical loop hashes, old paging
  and pack-prepare hashes, and the 240-second deadline remain unchanged.
  Static RED retains the old fifteen passes and four missing-scope/helper
  errors (`partition-red.log`); final nineteen guards pass, including independent
  page-local number mapping (`partition-root.log`, `partition-green.log`).
- Rebuilt current panel/host/tool binaries (`build.log`) before the private
  native check. Both raw input groups pass actual XTest -> GTK -> IBus
  acceptance (`gtk-transition-first.log`): whole Word and both fixed Sentences,
  physical page-local numeric adoption, second-page selection of the second
  收拾 sentence, exact raw undo, Space continuity and one physical Enter's exact
  saved insertion. The actual panel frame agrees with host revision/context/cursor,
  page rows and local numbers; editor focus is retained at each observation and
  the owned buffer stays empty before commit. The one-Enter check observes a
  short stable exact buffer; it does not send another empty-draft Enter or claim
  arbitrary delayed-event coverage. No host-injected draft or keyboard-map change.
- The two adopted-Han contexts above are core-engine coverage, not new physical
  GTK coverage in that run: their direct bootstrap lacked independent `还` / `后`
  readings. Existing longer-word adoption and editing could also construct the
  Han context, but that route was not exercised here. No new complete builtin, all-pack, native HTTP/model,
  personal-desktop or installed-version acceptance is claimed by this diagnostic.
  Existing private portal warnings remain in the log, not hidden or counted as
  candidate failures. Synthetic session files: `/tmp/suzaku-app-qa.4rpCz0`.
- Final all-feature Rust passes 1,678 tests / zero failures / 30 ignored across
  123 result entries (`full-rust.log`); opt-in ignored tests are not counted as
  executed. Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy.log`, 46.00 seconds). Heavy checks run sequentially under task-owned
  2 GiB high / 3 GiB max / zero-swap limits; no timeout or assertion is relaxed.
  Tensor tests pass 21, partition guards 19, unchanged native-HTTP fixture
  shape tests 19 and pack-shipping checks six. The HTTP shape result is not
  execution of the 106 native HTTP subcases. Formatting, Python/shell syntax
  and patch whitespace pass. The first audit-link command omitted its root
  argument and exits with usage (`audit-links.log`), not a product defect;
  the correctly scoped repository check passes all 250 links
  (`audit-links-final.log`), not a new package build/install qualification.
  The functional tensor records the new evidence/source fingerprint
  `9f47e91365c8f1e4104277eb3a2a20291ec2ebc495119d2630aa987837731679`
  without changing any of its 77 maturity scores.
- EN/ZH/JA JSON and Cargo version/lock files are byte-identical to this turn's
  starting snapshot (`frozen-resources.sha256`); totals and the strict English
  6,144 index ceiling are unchanged. No personal history or external corpus.
  Detailed receipts: `/tmp/suzaku-transition-audit.6kwZWi`. The daily input
  service is not replaced or restarted: final read-only check remains active at
  PID 1549795, with the desktop still using Rime. No commit, push or release requested.

### Unreleased — washing tidying and before leaving 2026 10 11

- Append EN `daily_care`: three missing basic forms (`wash`, `hair`, `hands`),
  eight contexts and sixteen original sentences. Append ZH 24 readings /
  eight contexts / sixteen sentences and JA twelve readings / six contexts /
  twelve sentences. Current totals: EN 6,143 indexed forms / 693 contexts /
  1,350 sentences; ZH 2,920 readings / 687 contexts / 1,372 sentences;
  JA 366 readings / 184 contexts / 367 continuation records. The existing
  English index guard stays strictly below 6,144; further word growth needs
  separate capacity work rather than silently raising this guard.
- Independent [English](tests/daily_care_english.rs),
  [Chinese](tests/daily_care_chinese.rs) and
  [Japanese](tests/daily_care_japanese.rs) expectations precede data.
  Actual unchanged-data RED passes six tests and fails 22 missing-candidate
  expectations (`red.log`). No third-party corpus or personal history is used.
  Read-only append audit confirms every old top-level field, nested value,
  ordered entry and separator flag unchanged, including all 6,140 old English
  first-index positions. All fourteen recommended packs remain byte-identical
  to HEAD, with no new context collision (`append-audit.txt`, `en-review.txt`).
- The first complete-data run passes 79 tests and fails three new expectations
  (`first-append.log`), not a regression of an old fixture. Two broad contexts
  already have both sentences but their second new sentences are on page two:
  EN `i'll check the ` preserves old `latest` / `latest version.`, and complete
  ZH `我先洗把脸zaichumen` preserves old 在 and 出门-prefix words. Add specific
  page-two exact selection/one-time commit/undo and independent old-choice
  checks; specific English `li` and Chinese unfinished `zaichume` remain strict
  first-page expectations. No page size, score or old data change.
- The third failure is a genuine continuation-generation gap, not pagination:
  `chumenqianshoushiyixi` offers the correct whole Word 出门前收拾一下, but its
  two fixed sentences are absent. The correct completion lies beyond the
  decoder's first five converted paths, which are finished alternate syllable
  splits. Generate additional continuations only from the already verified
  at-most-five whole-reading Word candidates, after the original decoder
  outputs, and only from each word's exact own context. Keep beam width,
  completion-table size, scoring, ranking implementation and candidate limit
  unchanged; never infer a shorter embedded context or rewrite adopted text.
- Preserve the first fix run (`focused-fix.log`): 83 passes / one new
  first-page assumption fails. Both previously missing sentences now exist;
  the second occupies page two. Keep the original first-page word choices and
  scores; explicitly verify page two rather than change quotas. Final focused
  run passes all 85 tests (`focused-final.log`), including 31 new tests, all
  prior carry fixtures, resource contracts and Japanese boundary/coexistence
  checks. Five full/partial Pinyin spellings, old primary conversion, literal
  input, exact commits and undo are checked without relaxing old assertions.
- The first full Rust run exposes a genuine old-page regression: new 时间安排
  sentences move 世间 to page two for exact `shijian`. Keep that old first-page
  assertion unchanged. The failed run is retained (`full-rust.log`, 1,609 passes /
  one failure / 29 ignored across 112 result entries, then fail-fast stops).
  Restrict the supplemental whole-word continuation path to inputs whose primary
  conversion is not already a dictionary Word. Accurate word/homophone inputs
  keep their previous continuation behavior; this deliberately does not promise
  extra sentences for a broad exact prefix such as `youxian` -> 优先.
  All 50 focused care/old-quality checks pass after this guard
  (`exact-word-guard.log`). An additional independent regression using only the
  old 再试一次 reading/context passes actual Rust validation in all five partial
  spellings (`old-word-generalization.log`): its correct completion beyond the
  first five split paths now produces both existing sentences, with exact
  once-only commit/undo. Exact 时间/世间 remains first-page, and invalid syllable
  splits, line boundaries and protected/unknown prefixes cannot manufacture
  these sentences. Empty conversion lists are also ineligible for supplementation.
- Other coverage includes case/apostrophes, horizontal padding, long Unicode
  frozen-prefix progress, line/protected-token controls, Romaji/Kana and five
  typed synthetic model failure/empty paths. Japanese 片付け remains canonical
  `かたづけ` / `kataduke`; this is not a new `katazuke` alias or a change to its
  existing >256-character limit. Host-like reseeding is not physical typing.
- Append six independent EN/ZH/JA GTK tuples while preserving every old tuple,
  physical assertion, loop fingerprint and original 240-second deadline.
  Complete builtin is 167 fixed cases + six controls = 173 workflows in
  parts 62 / 56 / 55. Complete all-fourteen-pack is 74 fixed cases + five
  controls = 79 workflows in parts 42 / 37. Focused `care` is six new cases
  + three Chinese controls = nine, not a complete builtin-gate pass.
  All fifteen partition guards pass (`partition-final.log`), with actual
  unchanged-runner RED retained in `partition-red.log`. Old ALL161/PACK68
  exact-tuple fingerprints are now frozen alongside all earlier fingerprints.
- Final complete all-feature Rust passes 1,667 tests / zero failures /
  30 ignored across 121 result entries (`full-rust-final.log`). This includes
  all 32 new tests and every unchanged old first-page assertion; ignored
  native tests are not claimed as executed. Twenty-one tensor tests, fifteen
  partition guards, nineteen unchanged HTTP-fixture shape tests, six
  pack-shipping checks and 250 audit links pass, with formatting, shell/Python
  syntax and patch whitespace. The full run uses no-fail-fast only to retain
  all target results; no assertion or time limit is changed.
- Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy.log`, 31.08 seconds). Current panel/host/tool binaries build
  successfully (`build.log`), before any private native acceptance run.
- Focused physical GTK acceptance passes all nine workflows (`gtk-care.log`):
  six new independent EN/ZH/JA word/sentence samples plus three old Chinese
  progress controls. Actual editor focus, numeric adoption, exact spelling undo,
  Space continuity and saved text equality pass within the original private
  session deadline. Portal idle-inhibition warnings are retained, not assertion
  failures. This is not the full 173-workflow builtin gate or physical-keyboard
  comfort/personal-desktop acceptance. New page-two core tests are exact index
  selection/commit/undo, not a claim of physical paging for these specific seeds.
- Complete all-fourteen-pack physical GTK coexistence passes all 79 workflows
  in two fresh private sessions (42 / 37, `gtk-packs.log`). Each session
  exports/installs every pack with the current adjacent tool before host/panel
  startup, then checks the live companion/system-font draft, editor focus,
  labels, numeric word/sentence adoption, exact spelling undo, Space continuity
  and saved exact commits. Both original 240-second deadlines are unchanged;
  no old case, tuple or physical assertion is skipped. Portal warnings remain
  in the retained log. The harness cleans only its own synthetic temporary
  files, not personal files. This is complete pack coexistence, not the full
  builtin, HTTP/live-model, Wayland/personal-desktop or physical-keyboard gate.
- Final source digest is
  `3e2657b78d7f78e0fad93abdabe4b1d9b7a77791ebd15c64649d83f4db26c079`;
  add evidence only for F16/F17/F18/F19/F67, with all 77 maturity rows unchanged.
  Validation remains serialized in task-owned MemoryHigh 2 GiB / MemoryMax
  3 GiB / MemorySwapMax 0 scopes with one build job. Final read-only desktop
  checks still show daily host PID 1549795 active and Rime selected.
- Receipt directory: `/tmp/suzaku-care-audit.4NPe3r`. Source remains dirty
  0.8.6; no installation, version, commit, push or release requested. Embedded
  vocabulary requires a later rebuild/reinstall for daily use. Typed provider
  errors and nineteen unchanged HTTP-fixture shape checks are not a new actual
  HTTP/live-model gate; the old 53-pair HTTP matrix stays unchanged. No personal
  desktop keys, input-source change or remote-CI acceptance is implied.

### Unreleased — clothing and carry before leaving 2026 10 11

- Append EN `daily_carry`: one explicit form (`glasses`), eight contexts and
  sixteen original sentences. Append ZH 24 readings / eight contexts / sixteen
  sentences, JA twelve readings / six contexts / twelve sentences. Totals:
  EN 6,140 indexed forms / 685 contexts / 1,334 sentences; ZH 2,896 readings /
  679 contexts / 1,356 sentences; JA 354 readings / 178 contexts / 355
  continuation records. No decoder, model protocol, candidate capacity or
  index-budget change; 6,140 remains strictly below 6,144.
- Independent [English](tests/daily_carry_english.rs),
  [Chinese](tests/daily_carry_chinese.rs) and
  [Japanese](tests/daily_carry_japanese.rs) fixed expectations run before data:
  actual unchanged-data RED has six passes and 21 missing-data failures
  (`red.log`). English and Chinese/Japanese data are authored independently,
  with no third-party corpus or personal history.
- Preserve the dirty baseline: all 28 English layers / 677 contexts / 1,318
  sentences and all 6,139 first-index positions, 2,872 Chinese readings /
  671 contexts and 342 Japanese readings / 172 contexts. Read-only semantic
  audit confirms all old top-level fields, ordered entries, flags and nested
  values unchanged; only the fixed append is present. All fourteen recommended
  pack files are byte-identical to HEAD and no new context collides with them
  (`append-audit.txt`). Old `glass` stays a Word; `glasse` reaches `glasses`.
- Preserve the first append run (`first-append.log`): it compiled the English
  append before the parallel Chinese/Japanese append had finished, so those
  missing-data failures are not a test of the final resources. English passes
  all nine new tests; two historical English zero-growth assertions fail
  because they compare their own old baseline to every future layer. Fix each
  equality to its immutable own append window and additionally assert every
  old word's rank against the current full index; retain all old hashes.
- The first complete-data focused run passes 109 tests and fails two new
  Chinese first-page assumptions (`focused.log`). `koudaijianchayixi` has
  old 以 + 谢谢/现在 branches; complete `我先换双鞋zaichumen` has old 在/再
  and 出门-prefix choices. Their second new sentences occupy page two.
  Keep old first-page words/capacity, add explicit second-page exact selection,
  one-time commit/undo and independent ambiguity checks. Preserve a subsequent
  nine-pass / one-fail result (`chinese-pages.log`): unfinished `zaichume`
  has its second sentence on page one, unlike complete `zaichumen`. Only the
  verified complete spelling takes the page-two branch; partial/Hanji and all
  other first-page assertions stay strict. Final focused acceptance passes
  all 112 tests (`focused-final.log`), including all 28 new tests.
- Coverage includes full/partial spelling, case/apostrophes, Romaji/Kana,
  horizontal spacing, long Unicode frozen-prefix continuation, literal
  preservation, explicit selection/commit/undo, line/protected-token controls
  and five typed synthetic model error/empty paths. Core reseeding is not
  physical typing; Japanese's existing >256-character bound remains a limit.
- Append six complete EN/ZH/JA GTK tuples to both builtin and pack gates;
  every old tuple, physical assertion and original 240-second deadline remains.
  Builtin is 161 fixed cases + six progress/paging controls = 167 workflows
  (60 / 54 / 53). All-fourteen-pack is 68 fixed cases + five controls = 73
  (39 / 34), each part a fresh private session with all packs and the real panel.
  Focused `carry` is six new cases + three Chinese controls = nine, not a
  complete-gate pass. Fourteen partition guards pass after retained actual
  unchanged-runner RED (`partition-red.log`, `partition-final.log`); historical
  ALL149/PACK56 and ALL155/PACK62 hashes and old loop fingerprints stay fixed.
- Complete all-feature Rust passes 1,635 tests / zero failures / 30 ignored
  across 118 result entries (`full-rust.log`). Ignored native tests are not
  claimed as executed. Twenty-one tensor tests, nineteen unchanged controlled
  HTTP-fixture shape tests and six pack-shipping checks pass, with formatting,
  shell/Python syntax, patch whitespace and 250 audit links. The audit-link
  check's first invocation omitted its required directory (`audit-links.log`);
  corrected invocation succeeds (`audit-links-final.log`), not a code fix or
  assertion change. The initial read-only maturity comparison used the wrong
  JSON key `tensor`; corrected `cells` comparison verifies all 77 rows unchanged.
- Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy.log`, 26.77 seconds); current panel/host/tool binaries build
  successfully (`build.log`, 0.23 seconds).
- Focused physical GTK acceptance passes all nine workflows (`gtk-carry.log`):
  six independent new EN/ZH/JA word/sentence fixtures plus three existing
  Chinese progress controls. Real editor focus, numeric adoption, exact
  spelling undo, Space continuity and saved-text equality are checked under
  the original private-session deadline. Portal idle-inhibition warnings are
  preserved, not assertion failures. This does not claim the full 167-workflow
  builtin gate, personal desktop or physical-keyboard acceptance. The harness
  cleans only its own synthetic temporary files; the outer receipt logs stay.
- Complete all-fourteen-pack physical GTK coexistence passes all 73 workflows
  in both fresh private sessions (39 / 34, `gtk-packs.log`). Each session
  exports/installs all fourteen packs through the current adjacent tool before
  host/panel startup and verifies the live companion's system-font draft and
  editor focus, labels, numeric adoption/undo, Space continuity and saved exact
  word/sentence commits, including all six new samples. Both original
  240-second limits are unchanged; no case, old tuple or assertion is skipped.
  Portal idle-inhibition warnings remain in the retained log. Synthetic private
  test files are cleaned by the harness, not personal files. This is not the
  full builtin gate, a new HTTP/live-model gate or personal-desktop acceptance.
- Current source digest is
  `e9402497ebf3189740c89a47bc920f0a00152409aba998481e1d4a67d15808f1`;
  evidence is added only for F16/F17/F18/F19/F67 without raising any score.
  Validation is serialized in task-owned MemoryHigh 2 GiB / MemoryMax 3 GiB /
  MemorySwapMax 0 scopes with one build job. Read-only desktop checks still
  show daily host PID 1549795 active and Rime selected.
- Receipt directory: `/tmp/suzaku-carry-audit.kKXYJB`. The source remains
  dirty 0.8.6, with no installation, version, commit, push or release requested.
  Embedded vocabulary needs a later rebuild/reinstall for daily use. Typed
  failures do not count as new HTTP or real-model acceptance; the frozen old
  53-pair HTTP fixture remains unchanged. No personal-history/packing access,
  physical-keyboard comfort, personal-desktop or remote-CI acceptance is implied.

### Unreleased — borrowing finding and small requests 2026 10 11

- Append EN 8 contexts / 16 original sentences in `daily_requests`, ZH
  24 readings / 8 contexts / 16 sentences and JA 12 readings / 6 contexts /
  12 sentences. Current totals: EN 6,139 indexed forms / 677 contexts /
  1,318 sentences; ZH 2,872 readings / 671 contexts / 1,340 sentences;
  JA 342 readings / 172 contexts / 343 continuation records. English has
  zero index growth; no decoder, model protocol, capacity or budget change.
- Independent [English](tests/daily_requests_english.rs),
  [Chinese](tests/daily_requests_chinese.rs) and
  [Japanese](tests/daily_requests_japanese.rs) expectations are fixed before
  data. Actual unchanged-data RED passes eight tests and fails 19 missing-data
  expectations (`red.log`); after appending, all 27 new tests pass.
  The complete focused run passes 84 tests, including prior routine fixtures,
  resource contracts, optional Japanese family coexistence and Japanese
  completion boundaries (`focused.log`). No old assertion is weakened.
- Freeze the current dirty baseline, not only HEAD: 27 English layers /
  669 contexts / 1,302 sentences and all 6,139 first-index ranks, 2,848 Chinese
  readings / 663 contexts, 330 Japanese readings / 166 contexts. New complete
  content/order/type/separator fingerprints coexist with all earlier ones.
  Readings and contexts only append; all fourteen optional pack files remain
  unchanged. No third-party corpus or personal history is used.
- Coverage includes full and specific partial spellings, case/apostrophes,
  Romaji/Kana, horizontal spaces, literal preservation, deliberate selection,
  adopted-word continuation, exact one-time commit/undo, line/protected-token
  boundaries and typed synthetic missing/unavailable/timeout/503/empty errors.
  Host-like reseeding is not physical typing; the actual GTK harness is separate.
  Japanese >256-character negative controls preserve the existing bounded
  prediction limitation, not a claim of newly generalized long Japanese input.
- Append six independent GTK requests tuples to both the complete and pack
  lists without removing/reordering old tuples. Full builtin list is 155 fixed
  cases plus five progress controls and one paging control = 161 workflows
  (58 / 52 / 51). Packs are 62 fixed cases plus five progress controls = 67.
  Since the previous 61-workflow pack session was near its 240-second limit,
  partition the complete pack gate into two fresh mixed-language sessions
  (36 / 31); each still installs all fourteen packs and starts the real panel.
  The original deadline and every old physical assertion remain unchanged.
  Exact-tuple guards, old-loop fingerprints, fail-closed scopes and fail-fast
  dispatch live in [partition regressions](scripts/test-vocabulary-partitions.py).
- Complete all-feature Rust passes 1,607 tests / zero failures / 30 ignored
  across 115 result entries (`full-rust.log`). Strict all-feature/all-target
  Clippy passes with warnings denied (`clippy.log`, 48.15 seconds); formatting
  and patch whitespace pass. The three current application binaries build
  successfully (`build.log`). Ignored native tests are not claimed as executed.
- New builtin physical GTK diagnostic passes all nine workflows: six new
  independent EN/ZH/JA samples plus three Chinese sentence-progress controls
  (`gtk-requests.log`). Numeric word/sentence adoption, exact spelling undo,
  Space continuity and actual editor saved-text checks run under the original
  240-second private-session limit. Synthetic files are
  `/tmp/suzaku-app-qa.FTOXDc`. This is focused new-request acceptance, not the
  full 161-workflow builtin gate or personal-desktop/physical-keyboard comfort.
- Complete all-fourteen-pack GTK coexistence passes 67 workflows in both
  fresh sessions (36 / 31, `gtk-packs.log`), each under its unchanged
  240-second deadline. All packs are CLI exported/installed into private QA
  data before host/panel startup; real companion system-font draft preview,
  editor focus, labels, numeric adoption/undo, Space continuity and saved
  exact commits are checked, including all six new request samples. Synthetic
  files are `/tmp/suzaku-app-qa.rq7KrR` and `/tmp/suzaku-app-qa.ccchXw`.
  Portal idle-inhibition warnings are retained; no assertion is relaxed.
- Thirteen lightweight partition tests pass (`partition-green-final.log`),
  after preserved unchanged-runner RED (`partition-red.log`) and one new
  fixture AST-selection error (`partition-first.log`); fix only the new AST
  lookup's `Expr` wrapper, without changing a physical assertion. CI now runs
  this guard. Also pass 21 tensor tests, 19 frozen native HTTP-fixture shape
  tests, six pack-shipping tests, 250 audit links and Python/shell syntax.
  These static checks do not execute the old 106 HTTP subcases in this round.
  The first format check identifies only new Chinese-test formatting; project
  formatting fixes it, with no semantic edit. Original outputs remain retained.
- Final tensor digest is
  `4862a76fcbc87ffcea20140ee192666755434806d0524c38db5361ee593aa7cc`;
  all 77 maturity scores match HEAD, with new evidence only for F16/F17/F18/F19/F67.
  Read-only desktop checks still show daily host PID 1549795 active and Rime
  selected. Validation is serialized in task-owned MemoryHigh 2 GiB /
  MemoryMax 3 GiB / MemorySwapMax 0 scopes, with one build job.
- Validation receipts and current JSON baselines are kept in
  `/tmp/suzaku-requests-audit.ResqCG`. The source version remains dirty 0.8.6;
  no new version, release, commit, push, installation, personal registry or
  input-source change is requested. Embedded data needs a later rebuild/reinstall
  for daily use. New typed errors do not count as new HTTP or real-model acceptance;
  the old frozen 53-pair HTTP function remains unchanged.

### Unreleased — daily routine/status fallback 2026 10 11

- Append finite authored waking, bedtime, work/rest and everyday-status expressions:
  EN 12 contexts / 24 sentences, ZH 32 readings / 12 contexts / 24 sentences,
  JA 16 readings / 8 contexts / 16 sentences. Current totals are EN 6,139
  indexed forms / 669 contexts / 1,302 sentences; ZH 2,848 readings /
  663 contexts / 1,324 sentences; JA 330 readings / 166 contexts /
  331 continuation records. English reuses existing forms: zero index growth.
  No model protocol, decoder, candidate capacity or 6,144-form limit changes.
- Independent [English](tests/daily_routine_english.rs),
  [Chinese](tests/daily_routine_chinese.rs) and
  [Japanese](tests/daily_routine_japanese.rs) fixtures precede the data append.
  The actual unchanged-data RED has six passes and 23 failures (`red.log`):
  22 missing-data expectations and one incorrect new legacy-sentence fixture.
  That fixture is corrected to the existing `i'll be home late tonight.`;
  no old sentence is changed or created to make it pass.
- First focused append validation passes 43 tests and fails one new Japanese
  first-page assumption (`green.log`). `kib` also matches existing 希望;
  its old word/sentences retain page one and both new 気分 sentences are
  independently selected/committed/undone on page two. Specific `kibu` remains
  a first-page probe. First full Rust validation (`full-rust.log`) finds the
  same kind of new-fixture error at Kana `しごとがえ`: preserve the exact
  `仕事がえ` progress, existing 選ぶ/映画 words and first new sentence on
  page one; select the second new sentence on page two. This now has its own
  test rather than being claimed as an unambiguous two-sentence first-page case.
- That first full run also finds three stale current-total guards: Chinese and
  English library tests and the Japanese optional-family test. Update only
  the explicit current counts. All historical content/rank assertions and
  the family package's exact metadata, absent/enabled and startup checks stay
  in place. The new routine suites now contain 31 independent tests.
- Read-only append audit confirms every old JSON metadata/array prefix against
  HEAD, old EN 26 layers / 657 contexts / 1,278 sentences, ZH 2,816 readings /
  651 contexts and JA 314 readings / 158 contexts, including types, flags,
  contents and order. All 6,139 old English first-index ranks and 49 historical
  resource hash constants remain unchanged. All fourteen pack filenames/bytes
  are unchanged, with no merged context, type or separator-flag conflicts
  (`append-audit.txt`). No personal input/history is used to author this data.
- New fixtures cover full/specific partial spellings, uppercase and Kana,
  apostrophes and horizontal spacing, long Unicode drafts, adopted-word
  continuation, literal text, single explicit commits/undo, seven line/control
  boundaries and five typed synthetic missing/unavailable/timeout/503/empty
  model paths. Host-like cumulative `seed(&raw)` tests are not physical-key
  acceptance. The old complete 53-pair HTTP function and its frozen AST hash
  remain unchanged; its actual 106 HTTP subcases do not become new-routine
  HTTP coverage. No live model or personal-desktop qualification is implied.
- Eight fixed routine samples are appended to the physical GTK complete and
  all-fourteen-pack gates without removing or reordering old tuples. The
  complete fixed list is 149 samples, split 50 / 50 / 49; five progress
  controls and one page control make 56 / 50 / 49 = 155 workflows.
  Packs have 56 fixed samples plus five progress controls = 61 workflows.
  A focused `routine` diagnostic is eight samples plus three Chinese controls,
  not a complete gate. Original 240-second sessions, focus/saved-text checks,
  numeric adoption/undo and the exact partition-Counter guard remain in place.
- Final all-feature Rust passes 1,580 tests / zero failures / 30 ignored
  across 112 result entries (`full-rust-final.log`), including all 31 new
  routine tests, corrected current-total guards and unchanged old regressions.
  The ignored private/native tests are not claimed as executed by this command.
- Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy.log`, 26.78 seconds); project formatting and patch whitespace pass.
- The complete private GTK editor gate passes all 155 workflows, with all
  three mixed-language parts passing (56 / 50 / 49) under their unchanged
  240-second per-session limits (`gtk-all.log`). Physical XTest input covers
  labels, numeric adoption, exact spelling undo, Space continuity and saved
  word/sentence commits, including all eight new routine samples. Retained
  synthetic files are `/tmp/suzaku-app-qa.9XudpZ`,
  `/tmp/suzaku-app-qa.xor7p5` and `/tmp/suzaku-app-qa.4tlRix`.
  Portal fallback/idle-inhibition warnings are preserved; no focus or
  saved-text assertion is relaxed. This is private GTK/IBus acceptance,
  not personal-desktop or human physical-keyboard qualification.
- With all fourteen recommended packages exported/installed only into private
  QA data before host/panel startup, the GTK coexistence gate passes all
  61 workflows (`gtk-packs.log`, original 240-second deadline), including all
  eight new routine samples, live system-font draft preview, editor focus,
  numeric adoption/undo, Space continuity and exact saved commits. Synthetic
  files are `/tmp/suzaku-app-qa.QhGbQm`; the personal pack registry is untouched.
- The unchanged 53-pair focused private IBus HTTP fixture passes all 106
  503/actual-timeout subcases on the final built-in data: EN 40 / ZH 40 /
  JA 26 (`native-fallback.log`). Pending/Unavailable retain local choices,
  numeric adoption, exact undo, Space continuity and single commits. Original
  500 ms request timeout, wait limits and 120-second outer session are kept.
  This is the old complete fallback diagnostic, not new-routine HTTP coverage,
  the complete native/core matrix, inline-preedit or live-model acceptance.
  Private portal bus-startup warnings are retained alongside the passing
  request-received/finished, fallback and commit checks.
- Final static validation passes 21 tensor tests, 19 native HTTP-fixture
  shape tests, six pack-shipping tests, all 250 audit-document links,
  project formatting, patch whitespace and Python/shell syntax. Tensor
  digest is `26a2ba97f13716f3876ff796a93212385ae51f4eb65cbdf784e8425db443e5ec`;
  all 77 scores match HEAD and only F16/F17/F18/F19/F67 gain routine evidence.
  Final read-only desktop checks still show active daily host PID 1549795
  and Rime selected. No private fixtures modify that installation or registry.
- Validation is serialized with one build job in task-owned MemoryHigh 2 GiB /
  MemoryMax 3 GiB / MemorySwapMax 0 scopes. An initial binary-build command
  named the nonexistent `linux_ibus_probe`; its rejected command is retained
  (`build.log`) and the actual `linux_ime_probe` build succeeds
  (`build-correct.log`). An initial format check identifies only the new
  Chinese test's default-edition formatting; project-edition formatting fixes
  it without a semantic change. Failures are preserved, not treated as passes.
- Source remains 0.8.6 and dirty; no release, commit, push, install or personal
  configuration/input-source change is requested in this round. Embedded
  additions require a later rebuild/reinstall to reach the daily installation.
  All 77 maturity scores stay unchanged. Original remote Linux CI timeout is
  not resolved or requalified by these local checks. Receipts are retained in
  `/tmp/suzaku-routine-audit.HL3K2V`.

### Unreleased — bounded custom shortcuts 2026 10 11

- Settings → Shortcuts records six context-local actions: previous/next choice,
  previous/next page, editable adoption and EN/ZH switching. The shared logical
  protocol accepts Ctrl **or** Alt, optional Shift and A–Z/Space only; rejects
  AltGr, Super, Alt+Space, duplicates and another action's effective preset.
  Candidate bindings add aliases; a custom language binding replaces the default
  Ctrl+Shift+Space until cleared. Reset keeps the selected shortcut profile.
- The fixed-size bindings object is model/platform independent and included in
  configuration backups. Profile/bindings are validated together before any
  mutation. Narrow asynchronous saves retain the existing owner, baseline and
  deadlines; late acknowledgements cannot partially roll back a newer pair.
  The Linux client must not reduce enabled+bindings to legacy P0/P1. Only U
  controls gain a 1024-byte payload cap; other controls keep the old <128 bound.
- Recording is local to a focused settings window, before panel/editor commands;
  Esc, focus loss, closing and IME preedit cancel it. A known physical command key
  remains consumed until release even if modifiers are released first. Recording
  and errors do not mirror into the main window or persist as input text. All
  eight interfaces have translated controls, wrapped instructions and errors.
- First UI validation passed 245 panel tests but failed one of eight new tests:
  narrow/Large recording chips clipped translated text. Only new record controls
  now share multiline measurement/render/hit geometry; the unchanged assertion
  passes in all interface locales. Fixed focused results: 245 panel tests,
  eight custom-settings tests and four localization tests, zero failures.
- Two new native fixtures initially failed before exercising input: dynamic
  RegisterComponent is not listed by list_engines, and the settings test window
  started hidden. Registration is now proved by NameHasOwner and an actual
  Factory.CreateEngine; the recorder uses the production window-show completion
  and X11 VIEWABLE before focusing. No sleeps/retries or deadline increases.
  Fresh isolated IBus six-group acceptance and the real X11/winit recorder pass;
  the extended native settings acknowledgement/reload probe also passes.
- Actual private IBus coverage includes six-field U persistence, case/lock
  normalization, aliases, paging/clamps, adoption hold/release/undo, Space and
  exact explicit commit, language replacement/reset, Compose/privacy/focus,
  invalid-patch atomicity, persisted reload and actual control size boundaries.
  Existing shortcut flows and all 26 EN/ZH language-key flows pass. The extracted
  C/fixture suite passes six tests and existing controlled-HTTP/runner checks pass
  19; neither is a substitute for the actual IBus acceptance.
- Initial feature-disabled lib compilation encountered the existing ungated
  GPU/winit boundary. The supported all-feature lib run then exposed an incorrect
  new test assumption: selecting a candidate normally cancels prediction. The
  fixture re-arms its held synthetic provider before testing narrow settings;
  no production timing or assertions are relaxed. Fixed lib: 540 passes, two
  ignored. Receipts (including initial failures):
  `/tmp/suzaku-shortcut-audit.bHMlP4`. ShellCheck is unavailable locally; shell
  syntax, formatting, 21 tensor tests and 242 source audit links pass.
- New `ibus-custom-shortcuts` and existing `ui` CI gates contain these regressions;
  no remote run is implied. At that source-validation checkpoint no personal
  settings, daily 0.8.6 installation, version, release, commit or push changed.
  The approved installation follow-up below records the subsequent daily update.
  Global chord interception, hardware
  comfort and real-model quality remain separate checks; all maturity scores
  remain unchanged. See [shortcut usage](docs/shortcuts.md).
- Final serial all-feature Rust regression passes **1,548 tests / zero failures /
  30 ignored** across 109 result entries (`rust-all-first.log`). The original
  ignored gates remain opt-in, not implicitly qualified by this result. The
  isolated application gate passes all 49 strict physical-keyboard groups
  (`gtk-physical-keyboard-first.log`), including held legacy adoption, layouts,
  Caps/Num Lock, Compose and focus return; it does not certify arbitrary new
  custom chords against GNOME interceptors or personal hardware. The updated
  tensor source digest is
  `cc3a5c87b6f5c9f7c1ae63c19769c510964bc127af2ff7011d276c9c5c98e7e4`.
- Strict all-feature/all-target Clippy passes with warnings denied (25.64s,
  `clippy-first.log`). The wired `ibus-custom-shortcuts` runner also passes in
  its own fresh session (`ibus-custom-runner.log`), independently of the direct
  fixture run. All heavy checks are serial, one build job, under task-local
  MemoryHigh 2 GiB / MemoryMax 3 GiB / MemorySwapMax 0; no desktop-wide limits,
  keyboard mappings or other processes are changed.

### 0.8.6 custom-shortcut development installation — 2026 10 11

- Built the four optimized binaries serially, then packaged and tested them from
  an independent extraction before updating the user-local installation. This
  remains version **0.8.6**, based on `81bbce8` with `source_dirty=true`, not a
  new tagged release. Build libc is glibc 2.43, not Ubuntu 24.04 qualification.
- Package checking exposed a missing linked 0.8.6 installation report. Ship it
  and extend the audit-link checker to `install-audit-*` as well as `bug-audit-*`.
  The original artifact's corrected checker fails on the four missing links;
  the fixed package passes. Five package-CLI tests, five physical-fixture shape
  tests, six extracted native-adapter checks, formatting and strict all-target
  Clippy pass. The lightweight checks are not application acceptance.
- The tested package passes the six-group private IBus custom-control gate,
  all **32 GTK3 workflows** with `SUZAKU_GTK3_QA_INPUT_TASKS=1`, and the new
  **seven physical custom-key groups** plus full U/rejection/clear gates and
  actual installed-panel recording, host persistence ACK and clear. CI wires
  the physical custom opt-in separately from the unchanged legacy 49 groups.
  No remote CI is rerun or inferred green.
- First upgrade preparation fails before any program replacement because its
  helper compared GNOME array strings as unquoted Bash patterns. Exact byte
  comparisons and a synthetic bracketed-value reproduction prove unchanged
  preferences; quote both right sides and add non-sensitive stage/line errors.
  The corrected helper passes without bypassing a check or changing deadlines.
  It keeps the old host connected until the official registrar captures Rime,
  then restarts the user service. All four installed files and actual new host
  PID **1549795** match the tested package; original settings, registry, launchers,
  service preference, Rime and source/MRU bytes are preserved.
- Directly using `/home/chiharukiryu/.local/libexec/suzaku`, fresh private sessions
  again pass the seven custom physical groups/actual recorder, all **49 legacy
  physical-keyboard groups**, and all **32 GTK3 workflows**. These use synthetic
  documents and isolated Xvfb/D-Bus/IBus/configuration, never personal desktop
  key injection. They do not prove a post-ACK rendered binding label, personal
  GNOME/Wayland chord delivery, hardware LEDs, real-model quality or full CI.
- Retain first failures, both recovery backups and receipts under
  `/tmp/suzaku-custom-install.5LTNSX`; move the unused first staging directory
  there instead of leaving it beside the installed binaries. The successful
  recovery backup is `install-custom-shortcuts-20261011.puxbWq` in the user backup
  directory. Heavy checks use one build job and the same task-local 2/3 GiB
  memory limits with no scope swap. No personal shortcut choice, version, commit,
  push or release changed. See the [installation report](docs/install-audit-0.8.6-2026-10-11.md).
  The final implementation/test fingerprint is
  `a58d25bf174635449a6f784ba11542387e6adc67941f1da7ce48c36bf4af12f4`;
  all 77 maturity scores remain unchanged.

### 0.8.6 release preparation 2026 10 11

- Synchronize Cargo/package metadata to 0.8.6 and macOS/Android build 36;
  locked dependency records and fourteen optional packages remain unchanged.
  The six data batches since 0.8.5 add 432 authored sentences, five indexed
  English forms, 224 Chinese readings and 112 Japanese readings. Version
  metadata does not qualify a package, personal installation or non-Linux host.
- A release audit catches an additional Japanese alias regression: the new
  standalone noun `した → 下` can split an already supported adopted Word
  spelling, e.g. `ニホンゴを勉強shitai → ニホンゴを勉強下い`.
  Eight independent fixed [regressions](tests/release_japanese_tail.rs) first
  yield three passes / five failures (`japanese-alias-red-all-features.log`).
  The first invocation omitted GPU features and failed to compile against the
  previously documented feature boundary (`japanese-alias-red.log`); that is
  retained separately and is not the behavioral RED or an acceptance result.
- `is_authored_tail` keeps its exact-spelling fast path, then compares known
  Katakana Word aliases against each authored sentence. It uses the existing
  longest written segment, retains homophone alternatives and original output,
  and requires the consumed canonical prefix to include the continuation's
  context. No arbitrary reverse lookup, decoder budget, data, rank, model
  protocol or new whole-sentence candidate is introduced. Unknown text and
  all seven vertical boundaries cannot be discarded by the comparison.
- The unchanged eight tests and seven existing Japanese integration suites
  pass 64 tests / zero failures in the first focused GREEN
  (`japanese-alias-green.log`). They cover
  leading/middle aliases, adopted-word continued typing, unfinished consonant
  probes, horizontal separators, original raw spelling, exact one-time commit
  and undo; existing canonical sentences and standalone noun choices remain.
- After the version update and alias fix, the serial all-feature Rust run
  passes 1,521 tests / zero failures / 29 ignored across 108 result entries
  (`rust-all.log`). The ignored fixture/live gates retain their original opt-in
  scope; this is not a post-fix GTK/native-device or package-install gate.
  Static checks pass 21 tensor tests, ten controlled HTTP-fixture tests and
  six pack-shipping tests, all 242 repository audit-report links, formatting,
  shell/Python syntax and patch whitespace. An initial link-check invocation
  omitted its required root (`audit-links.log`); only the corrected source-root
  result (`audit-links-source.log`) qualifies these repository links, not
  an extracted-package check. ShellCheck remains unavailable.
  All 77 maturity cells keep their original scores; the updated source digest
  is `e19550b324458e8acbc9b9b02c60e283b09d48fad6ce4af804a48c104c290504`.
- Post-update strict all-feature/all-target Clippy passes with warnings denied
  (`clippy.log`, 27.88s). Both compiler gates run sequentially with one job
  under MemoryHigh 2 GiB / MemoryMax 3 GiB / MemorySwapMax 0.
- Full-history secret scanning before commit passes 127 commits with fully
  redacted output. Retained release-preparation receipts are in
  `/tmp/suzaku-release-0.8.6.1VPDay`. Exact clean commit CI, packaging and clean
  installation gates are still required before publishing Release assets;
  this preparation does not update the personal input method or settings.

### 0.8.6 installed input tasks 2026 10 11

The clean `81bbce8` tar build and package checks pass on this machine (glibc 2.43,
not a clean Ubuntu 24.04 release acceptance). Its four binaries pass all 147
vocabulary/progress workflows, both 32-workflow GTK3 transports, 49 keyboard groups,
and 115 browser/Qt checks on owned displays, buses, settings and synthetic documents.
Eight opt-in GTK3 tasks add long physical English typing, literal numbers/punctuation,
adoption undo, real language chords, Japanese adopted-alias tails and consecutive
three-language commits without bypassing the original 24 baseline workflows.

The exact 0.8.6 remote Linux CI fails at the native session's 120-second boundary,
with 77 of 106 HTTP fallback subcases completed. The uncommitted runner follow-up
builds once and separates the full core and fallback into two original-limit
sessions. Frozen AST checks retain the original prefix, all 53 pairs and complete
tail controls. Nineteen lightweight checks and the first complete local native run
pass, including all 106 fallback cases; this does not turn the remote failure green.

After these gates, the authorized daily user installation upgrades from 0.8.1 to
0.8.6, with backups and exact binary/process verification. An initial manual-stop
attempt fails registration preflight and rolls back; the corrected atomic-rename
sequence lets the official registrar capture Rime before restarting the service.
Rime, settings, eight installed packs, input-source/MRU state, autostart and launchers
remain intact. Installed-file GTK3 default transport passes all 32 workflows;
installed native candidate default and synchronous transports each pass 30, with all ten English cards
clicked. Full receipts and scope are in the
[installation audit](docs/install-audit-0.8.6-2026-10-11.md). No production fix,
model inference, personal desktop key injection, CI rerun or Release publication
is implied; all 77 maturity scores remain unchanged.

### Unreleased contact vocabulary 2026 10 11

- Add authored phone clarity, volume, redial, connection-status and message-contact
  expressions: EN 12 contexts / 24 sentences, ZH 32 readings / 12 contexts /
  24 sentences, JA 16 readings / 8 contexts / 16 sentences. Current totals:
  EN 6,139 indexed forms / 657 contexts / 1,278 sentences;
  ZH 2,816 readings / 651 contexts / 1,300 sentences;
  JA 314 readings / 158 contexts / 315 continuation records.
  Every English next word and sentence token already exists in the index;
  zero growth preserves the strict 6,144-form budget and all 6,139 old ranks.
  These are finite presets, not contact/message-history access, calls,
  message delivery, connectivity probes or network configuration.
- Independent [English](tests/daily_contact_english.rs),
  [Chinese](tests/daily_contact_chinese.rs) and
  [Japanese](tests/daily_contact_japanese.rs) expectations run against the exact
  saved pre-contact data first: five passes / 22 expected missing-data failures
  (`contact-red.log`). All three JSON files are byte-identical to those saved
  copies before and after RED. Fingerprints lock every old entry/flag/order:
  25 English layers / 645 contexts / 1,254 sentences,
  2,784 Chinese readings / 639 contexts / 1,276 sentences, and
  298 Japanese readings / 150 contexts / 299 continuation records.
  New authoring is independently deduplicated against that full corpus and
  all fourteen recommended packages, not checked only against nearby examples.
- After only the frozen append and explicit current-total guards, the first
  focused run passes 71 tests / zero failures (`contact-focused-first.log`).
  This includes all 27 new contact tests, old Chinese choices, Japanese
  location/transit, historical resources, Japanese family optionality and
  all-fourteen-pack coexistence. No new-test correction, old assertion
  weakening, decoder, ranking quota or model protocol change is needed.
- Full all-feature Rust passes 1,513 tests / zero failures / 29 ignored
  across 107 result entries (`rust-all.log`). Strict all-feature/all-target
  Clippy passes with warnings denied (`clippy.log`, 33.18s).
- New tests cover complete and specific unfinished spellings, Word/Sentence
  branches, exact raw restoration, one-time commit and undo, six horizontal
  spacings, case/curly apostrophes, adopted-word continued typing and long
  Unicode drafts, seven control boundaries and protected/unknown negatives
  with positive controls. English cumulative raw drafts use the same
  `seed(&raw)` flow as the host, not the whole-fragment `append_seed` API.
  Five typed synthetic missing/failed/timeout/HTTP/empty-result paths retain
  immediate local candidates under the original two-second deadline.
- Preserve old Chinese `这个` / `这边` / `在哪里` / `neng` first-page choices.
  Japanese `bango` / uppercase / Kana keeps old `晩ご飯` ahead of new `番号`;
  both number sentences fit page one without removing the old word or raw
  spelling. Old `kanji`, `kou` and standalone `shit` keep their previous
  defaults, relative order, page-two choices and exact commit/undo paths.
  This is not a promise that every broad abbreviation can show every match
  on the first page, nor a full Japanese morphological analyzer.
- The fixed six contact samples are added to the complete, all-pack and
  controlled HTTP fixtures while preserving all old case tuples, assertions,
  deadlines and the part-Counter completeness guard. A focused `contact`
  scope is available without replacing the complete CI scope. Validation
  continues serially under a task-owned user scope with one build job and
  MemoryHigh 2 GiB / MemoryMax 3 GiB / MemorySwapMax 0; no desktop-wide
  limits or unrelated processes are changed.
- The complete private GTK editor gate passes 147 workflows: 141 fixed
  EN/ZH/JA samples and six sentence-progress/page controls, split into
  53 / 47 / 47 mixed-language sessions (`gtk-all.log`). All three parts
  pass under their unchanged 240-second deadlines; this is not a focused
  scope presented as complete coverage. Physical XTest events exercise
  Word/Sentence labels, numeric adoption, raw undo, Space continuation and
  exact saved commits. Synthetic editor files and panel logs remain in
  `/tmp/suzaku-app-qa.Y67jWQ`, `/tmp/suzaku-app-qa.9ZujNV` and
  `/tmp/suzaku-app-qa.nPITDI`. Portal idle-inhibition warnings are retained;
  all assertions and focus checks still pass in this run.
- All fourteen recommended packages are exported and installed only into
  private QA data before host/panel startup. Their GTK coexistence gate passes
  53 workflows: 48 fixed vocabulary samples and three Chinese/two Japanese
  sentence-progress controls (`gtk-packs.log`, original 240-second deadline).
  System-font draft rendering, editor focus, numeric adoption, raw undo,
  continued typing and exact saved commits all pass. The retained package
  QA files are in `/tmp/suzaku-app-qa.BsyXhm`; the personal pack registry is
  untouched. CONTRIBUTING now states this gate's actual count instead of
  its stale eleven-plus-three count; the independent native pack-only
  fourteen-workflow scope is unchanged.
- The focused private IBus HTTP-fallback diagnostic passes 106 workflows:
  the same 53 fixed samples under HTTP 503 and actual request timeout,
  EN 40 / ZH 40 / JA 26 (`native-fallback.log`). Local Word/Sentence
  candidates remain available during Pending and Unavailable; numeric
  adoption, exact raw undo, Space continuation and single commits all pass.
  The original 500 ms request timeout, two-second IPC/five-second waits and
  120-second outer gate remain unchanged. This focused diagnostic does not
  qualify the complete native matrix, live-model latency/quality, a personal
  installation or a human physical-keyboard session.
- Final static checks pass 21 tensor tests, ten HTTP-fixture tests, six
  pack-shipping tests, all 243 packaged audit links, formatting, patch
  whitespace, Python syntax and shell syntax (`static-final.log`). The
  tensor digest remains
  `2a43142263915460da17b3513689b27f5d731aa1f95696aff8ee115b23749c2c`.
  ShellCheck is unavailable, so no ShellCheck result is claimed.
- The completed read-only append audit protects all old JSON records,
  flags/order/metadata, exactly fourteen byte-identical package files and
  unchanged implementations except explicit total guards (`append-audit.log`).
  Two additional final-helper schema mistakes—optional English arrays and
  the long-form `evidence` field—are retained in
  `append-audit-final-first-error.log` / `append-audit-final-corrected.log`;
  those failed helper runs are not acceptance results. The final tensor
  comparison uses the actual schema and preserves all 77 old cells,
  scores, limits and evidence, adding only the five contact links
  (`tensor-audit-final.log`). No production data/assertion changes follow
  those helper corrections. The personal host PID 7135 is unchanged and
  no private Xvfb remains after the gates.
- Vocabulary documentation and tensor evidence distinguish this authored
  model-independent fallback from real-model prediction and personal-device
  acceptance. Only F16 / F17 / F18 / F19 / F67 gain contact evidence; existing
  cells, limits and maturity scores remain unchanged. A rebuild/reinstall is
  required to use these additions in the desktop IME.
- Receipts and exact pre-contact snapshots are retained in
  `/tmp/suzaku-vocabulary-contact.JrEhML`. Source stays 0.8.5; no release,
  commit, push or personal settings/installation/input-source change is made.

### Unreleased transit vocabulary 2026 10 11

- Add original boarding, transfers, platforms, arrival and ticket expressions:
  EN 12 contexts / 24 sentences, ZH 32 readings / 12 contexts / 24 sentences,
  JA 16 readings / 8 contexts / 16 sentences. Totals are now
  EN 6,139 indexed forms / 645 contexts / 1,254 sentences;
  ZH 2,784 readings / 639 contexts / 1,276 sentences;
  JA 298 readings / 150 contexts / 299 continuation records.
  All sentence tokens and next-word forms already exist in the English index;
  zero index growth still preserves the strict 6,144-form budget.
  This is authored fallback, not location access, live routes, timetables,
  fares, arrival alarms or ticket purchasing.
- Independent [English](tests/daily_transit_english.rs),
  [Chinese](tests/daily_transit_chinese.rs) and
  [Japanese](tests/daily_transit_japanese.rs) expectations run before appending
  data: six passes / 21 expected missing-data failures (`transit-red.log`).
  All three production JSON files match their saved pre-transit copies
  both before and after this run. Full historic fingerprints protect
  24 English layers / 633 contexts / 1,230 sentences and every old index rank,
  2,752 Chinese readings / 627 contexts / 1,252 sentences, and
  282 Japanese readings / 142 contexts / 283 continuation records.
  Old reading kinds, separator flags, content and order are not weakened.
- The first focused run passes 70 tests and fails two new fixture assumptions
  (`transit-focused-first.log`). The English cumulative-draft fixture used
  `append_seed` as if it accepted physical spaces; that interface deliberately
  trims and joins whole fragments. Only the new fixture is changed to
  `seed(&raw)`, matching the host's complete raw-draft flow. Every incremental
  character, Space, uncommitted state, final Word/Sentence selection and
  one-time commit/undo assertion remains. It is core draft coverage, not
  independent physical keyboard acceptance.
- Japanese partial `bas` also matches old `場所`. Its old Word and both
  authored sentences retain page one; new `バス` stays a first-page Word,
  with both bus sentences reachable on page two. Fixed `bas` / `BAS`
  expectations now check the unchanged default and old order, later-page
  selection, exact single commit, duplicate rejection, undo and raw restoration.
  Complete `basu` / uppercase / Kana still require both new sentences on page one.
- The next focused run has 26 passes / one new partial-fixture failure
  (`transit-fixture-corrected.log`). A bounded diagnostic prints all 25
  fixed Japanese partial variants before the unchanged failure
  (`transit-partial-diagnostic.log`). Only `bas` and `joushake` variants need
  later-page sentence expectations. For `joushake` / uppercase / `じょうしゃけ`,
  preserve the `乗車け` default and existing `け` suffix alternatives in order;
  `乗車券` and its first sentence remain on page one, while its second sentence
  is exactly selectable/committable/undoable on page two. Full `joushaken`
  retains both sentences on page one. Temporary diagnostic printing is removed.
  No production data, decoder, candidate capacity or old assertions change
  for these new-test corrections.
- All 27 new transit tests pass (`transit-focused-final.log`), including
  full/specific partial readings, adopted word continuation, long Unicode
  draft prefixes, literal spelling, horizontal spacing, seven control
  boundaries, malformed/unknown-input negatives, and five typed synthetic
  provider failure/empty paths under the original two-second deadline.
  Chinese short `这个` / `这边` / `在哪里` and Japanese `kanji` / `kou`
  page/order controls remain explicit. The shared provider fallback,
  all-fourteen-pack coexistence, historical resource and Japanese family
  tests pass in the first focused run. Append-only audit verifies every
  old record and unchanged metadata plus exactly fourteen byte-identical
  pack files (`append-audit-final.log`); the earlier audit's nonexistent
  `data/packs` target checks zero packs and is not pack acceptance.
- Full all-feature Rust passes 1,486 tests / zero failures / 29 ignored
  across 104 result entries (`rust-all.log`). Compilation and later native
  gates run serially with one build job under a user scope capped at
  MemoryHigh 2 GiB / MemoryMax 3 GiB / MemorySwapMax 0; no desktop-wide
  resource limits or unrelated processes are changed.
- Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy.log`, 27.45s). Formatting, patch whitespace, Python syntax,
  shell syntax, 21 tensor checks, ten HTTP-fixture checks, six offline-pack
  shipping checks and 243 packaged audit links pass before native testing
  (`static-before-native.log`). Tensor evidence is added only to
  F16 / F17 / F18 / F19 / F67; all old cells, limits and maturity scores
  remain unchanged. Source fingerprint is refreshed, not a remote CI claim.
- The complete private-display GTK editor gate passes all three sequential
  mixed-language sessions: 51 / 45 / 45 workflows, 141 total (`gtk-all.log`).
  This covers all 135 fixed vocabulary samples plus three Chinese and two
  Japanese sentence-progress controls and the existing later-page selection.
  Every new transit sample is physically typed, with Word/Sentence labels,
  numeric adoption, undo, Space continuity and exact saved Word/Sentence commits.
  The original 240-second deadline per session, all existing case tuples,
  assertions and part-Counter completeness guard remain unchanged.
  Private artifacts: `/tmp/suzaku-app-qa.zcFNaC`, `.uMWaMz` and `.tp3wNF`.
  Portal idle-inhibition warnings remain in the receipts; they do not fail
  the gate or replace its independent key/focus/document observations.
- With all fourteen recommended packages enabled only in private QA data,
  the GTK pack gate passes 47 workflows: 42 fixed samples plus three
  Chinese and two Japanese sentence-progress controls (`gtk-packs.log`).
  Every new transit sample runs alongside the existing pack and built-in
  samples, with exact Word/Sentence adoption, undo, Space and saved commits.
  No catalog package or personal pack activation changes.
  Private artifacts: `/tmp/suzaku-app-qa.2BCp6o`.
- The focused private IBus HTTP fallback diagnostic passes 47 fixed samples
  under both HTTP-503 and socket-cancelling timeout: 94 scenarios
  (36 EN / 36 ZH / 22 JA; `native-fallback.log`). This observes immediate
  local Word/Sentence choices through Pending/Unavailable, numeric adoption,
  exact undo, Space continuation and one explicit commit. The unchanged
  500ms model timeout, two-second IPC, five-second wait and outer 120-second
  diagnostic deadline remain. It is explicitly not the complete native gate,
  real-model quality/latency or personal desktop/physical-device acceptance.
- Documentation keeps actual data counts, isolated test scope and unresolved
  language/model limits separate. Rebuilding/reinstalling is still necessary
  to use these additions in the desktop IME; no existing personal settings,
  input sources, package activation or installed host is changed.
- The final post-native static checks pass again (`static-final.log`),
  including the refreshed tensor digest and formatting/whitespace/syntax
  checks. ShellCheck is not installed: shell syntax is checked, but no
  ShellCheck result is claimed. Failed and diagnostic receipts remain intact.
- Receipts and exact pre-transit copies are retained in
  `/tmp/suzaku-vocabulary-transit.pPOba6`. Source remains 0.8.5;
  no release, commit, push or personal installation is performed.

### Unreleased shopping vocabulary 2026 10 11

- Add original shopping, price, size, availability, payment and return expressions:
  EN 12 contexts / 24 sentences, ZH 32 readings / 12 contexts / 24 sentences,
  JA 16 readings / 8 contexts / 16 sentences. Current totals are
  EN 6,139 indexed forms / 633 contexts / 1,230 sentences;
  ZH 2,752 readings / 627 contexts / 1,252 sentences;
  JA 282 readings / 142 contexts / 283 continuation records.
  English adds no indexed words; all prior records, flags, ordering and
  first-index ranks stay intact. The fourteen recommended pack files are
  byte-identical to HEAD. No decoder rule, model protocol, ranking quota
  or index capacity changes in this batch.
- Independent [English](tests/daily_shopping_english.rs),
  [Chinese](tests/daily_shopping_chinese.rs) and
  [Japanese](tests/daily_shopping_japanese.rs) tests run before the append:
  five historical/boundary checks pass and 19 missing-data checks fail
  (`shopping-red.log`). All three JSON files match their exact saved
  pre-shopping copies before and after that RED run. New tests cover
  Word/Sentence choices, full/specific partial readings, case and spacing,
  adoption and continued editing, literal preservation, one commit/undo,
  protected/control boundaries, long drafts and synthetic model failures.
  Japanese includes an empty reply mapped to NoCandidates as well as
  missing, unavailable, timeout and HTTP-503 errors.
- The first expanded focused run passes 51 checks and fails one new
  fixture assumption (`shopping-focused-first.log`): partial
  `wo'zai'bi'jiao'yi'xi` retains the old primary Sentence `我在比较以西`.
  The new Word `我再比较一下` and price continuation are on page one;
  the color continuation remains selectable on page two. A dedicated
  fixed regression proves all four spellings, the old default and three
  old first-page Word choices, exact later-page selection, single commit,
  undo and raw Literal. Complete readings still require both new sentences
  on page one; no prior assertion or candidate quota is relaxed.
- After correcting that new partial-reading assumption, all 25 new tests
  pass (`shopping-fixture-corrected.log`). Shared provider fallback,
  full historical resources, Japanese family-pack coexistence and all
  fourteen recommended packs also pass in the expanded focused run.
- The first complete all-feature Rust run passes 1,458 executions, fails
  one unchanged old Chinese choices test and retains 29 dedicated skips
  (`rust-all.log`, 101 result entries). Newly authored `这个打折吗` occupies
  the fourth Word slot for `zheg`, pushing the old standalone `这个` onto
  page two. `这个还有货吗` would take that same slot if only the discount
  reading were changed. Only the two new readings, texts and availability
  context/sentence heads are rephrased with the natural clothing-context
  equivalents `这件打折吗` and `这件还有货吗`. All previous corpus data,
  old first-page assertions, candidate quotas and decoding stay unchanged.
  Clippy does not run after this failed full gate.
- Before changing those new data entries, the revised fixed shopping
  expectations reproduce four expected absence failures, while the unchanged
  old choices regression still fails (`shopping-compat-revision-red.log`:
  ten passes / five failures). The shopping JSON remains exactly unchanged
  during that revision RED. This separates the compatibility correction
  from deleting or weakening an old expectation.
- With only those new authoring records corrected, all 25 shopping tests
  and all six unchanged old Chinese choices tests pass
  (`shopping-compat-green.log`, 31 passes / zero failures).
- The final complete all-feature Rust gate passes 1,459 executions with
  zero failures and the unchanged 29 dedicated skips across 101 result
  entries (`rust-all-final.log`). Strict all-feature/all-target Clippy
  passes without warnings in 23.47s (`clippy-final.log`). Compilation
  and native gates run serially with one build job in task-owned,
  memory-capped user scopes; no desktop-wide limits or unrelated processes
  are changed.
- The complete private GTK vocabulary gate passes all three sequential
  mixed-language sessions: 49 / 43 / 43 workflows, 135 total
  (`gtk-all.log`). This covers all 129 fixed samples, three Chinese and
  two Japanese progress controls and the unchanged physical page-two
  selection. Every sample checks labels, numeric adoption, exact undo,
  Space continuity and saved Word/Sentence commits. All six new shopping
  samples pass actual fixture key events. Original session and IPC
  deadlines remain unchanged; retained fixtures are
  `/tmp/suzaku-app-qa.6icHGx`, `/tmp/suzaku-app-qa.81xbeq` and
  `/tmp/suzaku-app-qa.E8OdAx`. This is not personal-desktop acceptance.
- With all fourteen packs exported, installed and enabled in private QA
  data, the GTK pack gate passes 41 workflows: 36 fixed samples plus
  three Chinese and two Japanese progress controls (`gtk-packs.log`).
  The companion displays the fixed new CJK Word previews; the old
  `junbichu` partial retains its separate Sentence preview contract.
  The fixture is retained at `/tmp/suzaku-app-qa.rrj4m2`; personal
  configuration and published-package installation are outside this gate.
- The controlled native fallback diagnostic passes all 41 fixed samples
  under both HTTP-503 and socket-cancelling timeout, 82 scenarios
  (`native-fallback.log`: 32 English / 32 Chinese / 18 Japanese).
  Local Word/Sentence choices remain usable through Pending/Unavailable;
  numeric adoption, exact undo, Space and explicit commits all survive.
  Japanese raw padded input is deliberately keyboard-selected before
  Return. This focused diagnostic does not certify the complete native
  IBus matrix, personal desktop or real-model quality. Deadlines and
  assertions are unchanged, with all failing source-test receipts retained.
- Formatting, whitespace and shell syntax pass alongside 21 tensor tests,
  ten controlled-model-fixture tests, six pack-shipping checks and 243
  packaged audit links (`static-final.log`). The tensor gains shopping
  test/evidence paths and the matching source digest, with maturity scores
  unchanged. ShellCheck remains unavailable; no ShellCheck pass is claimed.
- These finite presets do not inspect purchase history, query live prices
  or execute payments. They remain independent of model and host code.
  Current receipts and exact pre-shopping resources are retained in
  `/tmp/suzaku-vocabulary-shopping.baesr4`. The source stays at 0.8.5;
  personal installation, published packages, commits and remotes are unchanged.

### Unreleased time vocabulary 2026 10 11

- Append independent authored time, waiting and rescheduling fallback data:
  EN 12 contexts / 24 sentences with zero indexed-word growth;
  ZH 32 readings / 12 contexts / 24 sentences;
  JA 16 readings / 8 contexts / 16 sentences. Current totals are
  EN 6,139 indexed forms / 621 contexts / 1,206 sentences;
  ZH 2,720 readings / 615 contexts / 1,228 sentences;
  JA 266 readings / 134 contexts / 267 continuation records.
  All fourteen recommended pack files are byte-identical to HEAD.
  No decoder rules, model protocol, ranking quota or index capacity change.
- Fixed independent [English](tests/daily_time_english.rs),
  [Chinese](tests/daily_time_chinese.rs) and
  [Japanese](tests/daily_time_japanese.rs) expectations first run against
  unchanged data: each has two passing historical/boundary checks and six
  expected missing-data failures (six passes / 18 failures, `time-red.log`).
  Preserve every old layer, reading flag, context, sentence and item order,
  and every old English first-index rank. An independent duplicate audit
  finds that the draft `i'll be there in a few minutes.` already exists.
  Strengthen its new expectation to `i'll be there in a few minutes, please wait.`,
  reproduce its absence (`time-duplicate-revision-red.log`), then change
  only the new authored record; the old sentence remains intact.
- The first expanded focused run (`time-focused-green.log`) has 48 passes
  and four failures in new fixtures, not four production regressions.
  Two require unchanged English drafts ending in known `to` to be Literal;
  the existing classifier correctly labels complete known tokens as Word.
  Keep both Word/Sentence completions and exact raw submission/undo, with
  fixed existing `to`/`be` labels. Strengthen protected URL/file-wrapper
  negative examples with relevant authored tails and an ordinary-wrapper
  positive control instead of unrelated manufactured strings.
- The other new assumptions place both sentences on page one for ambiguous
  `ti'xing'wo'yi'xi` and `jikanta` / `じかんた`. Existing `以西` and `時間＋た`
  conversion paths occupy the primary Sentence slot; both new Word and first
  authored Sentence are on page one, while the second is still selectable
  on page two. Preserve the six-row page and old split/homophone choices.
  Dedicated fixed tests now prove both page positions, exact later-page
  selection, one commit/undo and raw preservation in all spelling variants.
  Complete readings still require both authored sentences on page one;
  no old first-page or later-page assertion is weakened.
- The first helper refactor fails compilation because its expected kind
  was not passed into the shared commit helper (`time-fixture-corrected.log`).
  Pass the expected kind explicitly; retain that failed receipt alongside
  the semantic failures. There is no resource-limit or timeout diagnosis.
- After these fixture corrections, all 26 new fixed tests pass
  (`time-fixture-helper-fixed.log`). Both complete/new partial choices and
  the explicit ambiguous later-page controls remain strict. The shared
  missing/failed/timeout/empty-provider and fourteen-pack checks already
  pass in the expanded focused run.
- The complete all-feature Rust gate passes 1,434 testcase executions with
  zero failures and the unchanged 29 dedicated skips across 98 result
  entries (`rust-all.log`). Strict all-feature/all-target Clippy passes
  without warnings in 27.00s (`clippy.log`). All compilation and native
  gates run serially with one build job in task-owned memory-capped scopes;
  no personal input service, keyboard setting or unrelated process is changed.
- The complete private GTK vocabulary gate passes all three sequential
  mixed-language parts: 47 / 41 / 41 workflows, 129 total (`gtk-all.log`).
  This includes all 123 fixed samples, three Chinese and two Japanese
  progress controls, and the existing physical later-page choice. Every
  sample checks labels, numeric adoption, exact undo, Space continuity
  and exact saved Word/Sentence commits. New `kon'ya` also passes actual
  fixture key events, not just pure Romaji conversion. Original session
  and IPC deadlines are unchanged; fixtures are retained at
  `/tmp/suzaku-app-qa.XmA6xs`, `/tmp/suzaku-app-qa.j6nZvi` and
  `/tmp/suzaku-app-qa.jOvS7v`.
- With all fourteen packs exported, installed and enabled only in private
  QA data, the GTK pack gate passes 35 workflows: 30 fixed samples plus
  three Chinese and two Japanese progress controls (`gtk-packs.log`).
  The independent companion displays the fixed new CJK Word previews;
  the old `junbichu` partial retains its separate Sentence preview contract.
  Its retained fixture is `/tmp/suzaku-app-qa.bybngr`; this is not a
  personal installation or a published-package acceptance result.
- The controlled native fallback diagnostic passes all 35 fixed samples
  under both HTTP-503 and socket-cancelling timeout, 70 scenarios
  (`native-fallback.log`). Local Word/Sentence choices remain usable and
  unchanged through Pending/Unavailable; numeric adoption, exact undo,
  Space and explicit commits all survive. Japanese raw padded text is
  explicitly selected with keyboard navigation before Return. This focused
  diagnostic does not certify the complete native IBus matrix, a personal
  desktop or real-model prediction quality.
- Final checks pass formatting, whitespace, shell syntax, 21 tensor tests,
  ten controlled-model-fixture tests, six unchanged pack-shipping checks and
  243 packaged audit links. The tensor test/evidence paths and source digest
  are synchronized without changing maturity scores. ShellCheck remains
  unavailable on this machine; no ShellCheck pass is claimed.
- Current receipts and exact pre-expansion JSON copies are in
  `/tmp/suzaku-vocabulary-time.6DjZvg`. Prior October 8 temporary receipts
  are no longer present as of this run; their development entries remain
  historical records, not currently inspectable log files.
  These presets do not access calendars or provide scheduled reminders.
  Source version, published 0.8.5 packages and the personal installation
  remain unchanged; embedded data still needs a rebuild/reinstall.

### Unreleased location vocabulary 2026 10 08

- Append location, directions and meeting expressions to independent JSON data.
  English adds five forms (`across`, `corner`, `corners`, `street`, `streets`),
  16 contexts and 32 sentences; Chinese adds 48 readings, 16 contexts and
  32 sentences; Japanese adds 24 readings, 12 contexts and 24 sentences.
  Totals: EN 6,139 indexed forms / 609 contexts / 1,182 sentences;
  ZH 2,688 readings / 603 contexts / 1,204 sentences;
  JA 250 readings / 126 contexts / 251 continuation records. All fourteen
  optional packs remain unchanged. This is authored input fallback, not
  location collection, navigation or general model-generated language quality.
- Fix independent [English](tests/daily_location_english.rs),
  [Chinese](tests/daily_location_chinese.rs) and
  [Japanese](tests/daily_location_japanese.rs) expectations before adding data.
  The baseline has six passing old-content/boundary checks and 23 expected
  missing-data failures (`location-red.log`). Appended data passes all 29
  new tests and the 29 previous courtesy tests (`location-and-courtesy-green.log`).
  Preserve all old item wording, reading flags, contexts, sentences and order;
  protect every previous English first-index rank and the original 6,144 cap.
  Prior courtesy shape checks use their fixed batch window, not a permanently
  frozen whole-catalog size; original content fingerprints and page requirements
  stay intact, with strict current totals checked separately.
- The original complete Rust run (`rust-all.log`) records 1,396 passes,
  three failures and 29 dedicated skips. New `下` exposes greedy splitting
  of `したい`/`したら` after recognized words, losing the old authored sentence;
  new `中央` exposes deeper splitting of the partial `準備中` reading, pushing
  its old second sentence off page one. Keep both new words and share a whole
  authored-tail check between conversion and completion. A first broad
  completion-depth return restores these failures but changes the old `kyouyu`
  later-page requirement (`japanese-fix-focused.log`); reject that approach.
  Instead filter only deeper completions that strictly extend an already
  completable Word, keeping other ambiguities and same-depth homophones.
  No score, page size, quota, timeout or old test assertion changes.
- The narrowed fix passes 82 focused testcase executions: the original
  conversion case, nine Japanese language units, seven coordination tests,
  all 58 location/courtesy tests and seven independent
  [Japanese completion-boundary tests](tests/japanese_completion_boundaries.rs)
  (`japanese-fix-narrow-focused.log`). Fixed variants cover Romaji, uppercase,
  Kana, adopted/written prefixes, positive `下`/`中央`, unknown/control boundaries
  and exact-once Word/Sentence/Literal submission with single undo. This only
  protects finite authored progress, not arbitrary Japanese conjugation.
- Its first full follow-up (`rust-all-japanese-fixed.log`) passes 1,404 tests
  but fails two old `準備gadeki` Word checks: stopping completion at the whole
  authored tail also blocks `が` progression and the genuine `できれば` completion.
  Stop only when an actual dictionary split would replace the authored tail;
  still advance particles and collect genuine longer partial readings. All
  64 affected fallback/language/new-boundary checks then pass
  (`japanese-fix-partial-completions.log`). An additional fixed consonant-probe
  test reproduces `完了shit` offering `完了下`
  (`japanese-consonant-probe-red.log`); apply the exact-tail protection to each
  matched vowel query too. Keep independent `shit` -> `下` completion valid.
  The boundary suite now has nine tests, including four `gadeki` variants.
  Authored-tail matching remains literal to the author's script; this does not
  promise arbitrary Katakana aliases or full-sentence phonetic/morphological inference.
- The full query-protection run (`rust-all-japanese-tail-final.log`) passes
  all old regressions and 1,407 tests, with one failure in the newly added
  standalone `shit` probe's page-one assumption: existing `失礼`/`質問` words
  and sentences retain priority, and `下` is correctly reachable at index 7.
  Correct only that new positive fixture to require a later-page Word and
  prove its exact selection, single commit and single undo. Keep all original
  first-page and unknown-tail assertions, and do not change production ranking.
- The corrected complete all-feature Rust gate passes 1,408 testcase
  executions with zero failures and the unchanged 29 dedicated skips across
  95 result entries (`rust-all-final.log`). All original failed receipts above
  are retained; no timeout, old expected word/page or source budget was relaxed.
- Strict all-feature/all-target Clippy passes without warnings in 22.39s.
  The first private GTK all run reaches 57 completed samples but exits 124 at
  the unchanged 240-second first-part deadline (`gtk-all.log`), before its
  remaining samples/progress/paging and before any pack or HTTP gate starts.
  No assertion traceback is recorded; the task scope reports a 608.7 MiB
  peak, not a confirmed OOM diagnosis. Retain its private fixture at
  `/tmp/suzaku-app-qa.05g3Ee`. Split exactly the same 117 fixed tuples into
  three 39-sample mixed-language parts instead of increasing deadlines:
  the first also runs three Chinese progress checks, two Japanese progress
  checks and one physical later-page check (45 / 39 / 39 = 123 workflows).
  A Counter guard checks whole-tuple coverage and each part must cover EN/ZH/JA;
  all three must pass before the launcher reports complete success.
- The first three-part run (`gtk-all-three-parts.log`) completes parts one
  and two (45 and 39 workflows), then rejects an overbroad new preview assertion
  on old partial `wo que ren yi xi`: default `我确认以西` is separate from the
  intended `我确认一下` Word at index 1 and its authored sentence at index 5.
  The historical bare-host contract requires these choices on page one, not
  that every partial reading's default preview is the completed Word. Move
  only the new preview check back inside the original panel/pack branch.
  Keep its old complete-word preview assertions, the new independently fixed
  `junbichu` preview, and every existing word/sentence label, adoption, undo,
  Space and exact saved-commit check. All three default parts run again after
  this fixture correction; original failure/fixtures stay retained.
- The corrected default private GTK gate completes all three parts and reports
  full success: 45 / 39 / 39 workflows, 123 total
  (`gtk-all-preview-scoped.log`). This includes the two physical adopted-word
  Japanese tails, the broad `junbichu` Word/second-sentence choice, old Chinese
  partial-reading choices, and original later-page selection/undo/commit.
  Its private fixtures are `/tmp/suzaku-app-qa.mrudiX`,
  `/tmp/suzaku-app-qa.mg3YeR` and `/tmp/suzaku-app-qa.SbUXAI`.
- With all fourteen packs exported, installed and enabled only in private
  QA data, the focused GTK pack gate passes 29 workflows: 24 fixed word/sentence
  samples plus three Chinese and two Japanese progress controls
  (`gtk-packs.log`, fixture `/tmp/suzaku-app-qa.8IwN06`). Complete CJK samples
  retain fixed Word-first companion previews; the broad `junbichu` sample
  independently expects the selected `準備ちゅ` Sentence preview, while its
  `準備中` Word and second authored sentence stay selectable on page one.
- The controlled native HTTP fallback diagnostic passes 29 samples under both
  HTTP-503 and socket-cancelling timeout, 58 scenarios (`native-fallback.log`).
  Immediate local choices remain identical through Pending/Unavailable;
  numeric adoption, undo, Space and exact commits survive both failures.
  Japanese padded Literal is still explicitly selected with real Tab/Up
  navigation before Return. This focused diagnostic does not certify the
  complete native IBus matrix, real-model quality or a personal installation.
- Final static checks pass formatting, whitespace, shell syntax, 21 tensor
  tests, ten controlled-model-fixture tests, six unchanged pack-shipping checks
  and 243 source-tree audit links. ShellCheck is unavailable on this machine;
  no ShellCheck pass is claimed. The tensor source digest/test/evidence links
  are synchronized without increasing maturity scores. Packaging, personal
  desktop input and live-model performance are not acceptance claims here.
- Original and follow-up logs are retained in
  `/tmp/suzaku-vocabulary-location.3CJEkT`. The model protocol, pack catalog,
  candidate capacity and English index budget are unchanged. Version 0.8.5,
  published packages and the personal installation remain unchanged.

### Unreleased courtesy vocabulary 2026 10 08

- Append project-authored thanks, apologies, polite requests and considerate
  responses to the independent JSON resources. English adds 16 contexts and
  32 sentences with zero indexed-word growth; Chinese adds 48 readings,
  16 contexts and 32 sentences; Japanese adds 24 readings, 12 contexts and
  24 sentences. Current totals: EN 6,134 indexed words / 593 contexts / 1,150
  sentences; ZH 2,640 readings / 587 contexts / 1,172 sentences; JA 226 readings /
  114 contexts / 227 continuation records. All fourteen optional packs stay unchanged.
- Fixed [English](tests/daily_courtesy_english.rs),
  [Chinese](tests/daily_courtesy_chinese.rs) and
  [Japanese](tests/daily_courtesy_japanese.rs) expectations first reproduce the
  missing vocabulary against unchanged data: 1/7, 2/9 and 2/10 tests pass,
  respectively; 21 expected semantic failures remain in `courtesy-red.log`.
  Before appending data, add two English shape/old-priority checks and one
  Japanese broad-`ryou` paging check. All 29 new tests then pass. Full hashes
  retain the old 20 English layers, all earlier word ranks and every previous
  Chinese/Japanese reading, separator flag, context and sentence in order.
  Old need-help sentence priority and Japanese homophones remain protected;
  broad Japanese requests can expose the new choices on a later page.
- The complete all-feature Rust suite passes 1,370 testcase executions with
  zero failures and the original 29 dedicated skips across 91 result entries.
  The generic fallback matrix now includes six courtesy samples under missing,
  failed, timed-out, HTTP-503 and empty synthetic providers. These checks do
  not measure real-model language quality or latency.
- The complete private GTK vocabulary gate passes 114 workflows: 55 word/sentence
  samples plus three progress checks and one physical page-two check in part one,
  then 55 samples in part two. The six new EN/ZH/JA samples test actual editor
  contents, numeric adoption, exact undo, Space continuity and saved word/sentence
  submission. Both original mixed-language partitions run under the unchanged
  240-second per-session deadline; neither partition alone is the full gate.
- With all fourteen packs exported, installed and enabled only in private QA
  data, the focused GTK pack gate passes 20 workflows, including the same six
  new courtesy samples. The controlled native HTTP fallback diagnostic passes
  all 22 samples under both HTTP-503 and socket-cancelling timeout: 44 scenarios.
  It retains Pending/Unavailable local choices, adoption, undo and exact commits;
  the focused mode does not replace or certify the complete native IBus gate.
  Its first attempt passes 40 EN/ZH scenarios then fails a new Japanese
  literal-padding assumption (`native-fallback.log`). Japanese consumes reading
  separators for conversion but retains the padded raw choice. Fix only that
  new fixture path to explicitly navigate to the local Literal before Return,
  preserving the exact bytes, original EN/ZH assertions and all deadlines;
  the corrected run is `native-fallback-literal-selection.log`.
- Strict all-feature/all-target Clippy passes without warnings (22.36s), as do
  formatting, whitespace checks, 21 tensor tests, 10 controlled-model-fixture
  tests and six unchanged pack-shipping checks. Source-tree validation resolves
  243 audit links; this is not built-package or installation acceptance.
  Update the dirty tensor snapshot's base HEAD/date, source digest and courtesy
  test/evidence links while leaving every maturity score unchanged.
- Validation logs, including the original failed baseline, are retained in
  `/tmp/suzaku-vocabulary-courtesy.sCLhqP`; GTK fixtures are
  `/tmp/suzaku-app-qa.PjTyC1`, `/tmp/suzaku-app-qa.ogxZ0A` and
  `/tmp/suzaku-app-qa.aOTot5`.
  The model protocol, decoders, rankings, candidate capacity and 6,144 English
  index budget are unchanged. This is unreleased source data: version 0.8.5,
  published packages and the personal installation remain unchanged.

### 0.8.5 release preparation (2026-10-08)

- Collect the native long-candidate display fixes into a patch release. Only
  English/Chinese full-text labels with a verified shared prefix use bounded
  previews; the renderer measures the actual numbered card before shortening
  that prefix further. Full candidate text, frame state, absolute paging indices,
  adoption and exact-once submission remain unchanged. Short/custom labels,
  Japanese and uncertain Unicode boundaries retain their previous display.
- Keep the changing suffix visible and protect the final printable ASCII token,
  including one directly following Han text. This is conservative display logic,
  not general Unicode word segmentation or a replacement for the model protocol.
  Vocabulary data, dependency versions and candidate ranking are unchanged.
- Development checks pass 105 Rust testcase executions: 21 candidate-mix units,
  27 GPU units, 54 long-draft/language regressions and three private native UI
  cases. The candidate action case contains a 252-scenario pure-layout matrix
  across widths, font sizes, layouts and pages; these are not 252 additional
  Rust tests or acceptance on every physical desktop/font. Strict all-target
  Clippy and formatting pass. Logs and both original failed attempts remain in
  `/tmp/suzaku-native-responsive-qa.fI6gsp`; no personal input session is used.
- Synchronize the Rust root package/lock entry, two macOS plist versions,
  Android version name and current guides to 0.8.5; platform build number is 35.
  Keep historical 0.8.4 notes and dependency lock entries unchanged. Require
  fresh all-feature regression checks and the exact committed source's full CI,
  Ubuntu 24.04 package-content and clean-container installation gates before
  attaching the four Linux Release assets. The deferred upstream IBus daemon
  abort and unqualified platforms are not declared fixed by this release.
- After the version update, the full all-feature suite passes 1,341 testcase
  executions with zero failures and 29 dedicated skips across 88 result entries;
  strict all-target Clippy passes (22.82s). Formatting, version/build consistency,
  all unchanged dependency entries, 21 tensor checks and 81 Python fixture checks
  pass. The 242 audit links here are source-tree checks, not package acceptance.
  Pinned Gitleaks 8.30.1 finds no leaks in staged changes or existing 126-commit
  history. Release logs are in `/tmp/suzaku-release-0.8.5.GOJlIn`; local native
  development receipts above do not substitute for the committed source's CI.

### 0.8.4 release preparation (2026-10-07)

- Collect the preceding 2026-10-07 input-reliability, local-candidate and
  everyday/family fallback work into the next patch version. Historical
  `Unreleased` section titles below describe the development stage when those
  checks were run; old results and failed attempts are not rewritten.
- Synchronize the Rust root package/lock entry, two macOS plist versions,
  Android version name and current guides to 0.8.4; platform build number is 34.
  No other locked dependency version changes. The declared winit 0.30.13 path
  patch, complete upstream source, license and Suzaku provenance notice must be
  included in the source commit and package notices.
- Latest development validation before this metadata update: 1,335 Rust tests
  pass, 29 explicit/fixture entries stay outside the default gate, strict
  Clippy passes; complete private GTK vocabulary 108 and all-fourteen-pack
  focused GTK 14 workflows pass. These are not a post-version exact-commit CI
  result, full native matrix, real-model or personal-desktop acceptance.
- New release checks/logs use `/tmp/suzaku-release-0.8.4.BfFnx1`.
  After synchronizing metadata, all-feature Rust again passes 1,335 tests with
  zero failures/29 dedicated skips (`rust-all.log`, 272.47s); strict all-target
  Clippy passes (`clippy.log`, 21.65s). Formatting, version/build/dependency
  consistency, unique-key CI YAML, changed-script syntax, 21 tensor checks,
  nine GTK event-observer, 28 private-trace, one probe-teardown, ten controlled
  model-fixture and six shipping-manifest checks pass. The 242 audit links
  checked here are source-tree references, not packaged-artifact acceptance.
  Pinned Gitleaks 8.30.1 reports no leaks in staged source or existing 125-commit
  Git history. Local ShellCheck is unavailable; its required CI step remains.
  Main-branch CI and clean Ubuntu 24.04 package-content/container acceptance
  must pass on the exact source commit before Release assets are published.
  A pushed source tag alone does not complete the 0.8.0+ binary-release gate;
  there is no automatic tag-triggered Release workflow. The old 0.8.3 tag/CI
  failure and deferred upstream daemon abort remain unchanged.

### Unreleased — Everyday clarification and confirmation fallback (2026-10-07)

- Append authored independent data only: English `daily_clarification` adds
  16 contexts/32 sentences using existing indexed forms (`words=[]`), keeping
  6,134 words within the unchanged 6,144 builtin budget. It now has 20 layers/
  577 contexts/1,118 sentences. Chinese adds 48 readings/16 contexts/32 sentences,
  now 2,592/571/1,140; Japanese adds 24 readings/12 contexts/24 sentences, now
  202/102/203. Language algorithms, model protocol, candidate budgets and the
  fourteen-pack catalog are unchanged. These are finite authored fallback
  examples, not general prediction or Japanese morphological analysis.
- Before adding data, freeze new independent expectations and reproduce gaps:
  English 1/7 pass, cargo exit 101 in 1.95s; Chinese 2/8, exit 101 in 0.71s;
  Japanese 2/9, exit 101 in 1.40s. Passing cases preserve prior complete content
  fingerprints and literal/unknown input boundaries. `english-red.log`,
  `chinese-red.log` and `japanese-red.log` are retained under
  `/tmp/suzaku-vocabulary-clarification.i1ML24`.
- New guards freeze all old 19 English word-layer priorities and their 6,134
  first indexing positions, 561 contexts/1,086 sentences; all 2,544 Chinese
  readings/555 contexts and all 178 Japanese readings/90 contexts including
  kinds, separator flags, complete wording and order. Earlier snapshots remain.
  Current resource totals and the optional Japanese pack's builtin count
  expectations are updated; the latter still proves an absent registry doesn't
  install or activate family data.
- First data-green attempt stops at Chinese 7/8 (`targeted-green.log`,
  exit 101, 18.49s): adopted `我理解的是` + unfinished `zheg` emits the intended
  sentence at index 6, after valid older `这个词` choices. The adjacent run stops
  at English 6/7 (`adjacent-green.log`, exit 101, 2.25s): byte-exact original
  `…let me say it an` is classified Word because `an` is an existing whole word,
  not Literal. A further adjacent run passes Japanese 9/9 and all family EN/ZH
  cases, then reports the Japanese optional-pack test's stale builtin count
  178 versus 202 (`japanese-and-adjacent.log`, exit 101, 9.40s). All originals
  are preserved, not erased or presented as product fixes.
- Source/preview diagnostics distinguish test premises from defects. Chinese
  longest-context lookup applies separately to each bounded decoder result:
  the primary `这个词` branch does not match the new sentence, while another
  `这个意思` branch does. In a private 144-preview diagnostic, the 96 empty/long
  prefix + six-spacing + four-partial-spelling cases actually used by the test
  retain targets at index 6 (`我理解的是…`, pool 10) or 5 (`我确认一下…`, pool 7).
  The additional short mixed prefix with unknown `café` only retains literal
  input; it is a separate existing mixed-input limitation, not qualified here.
- Keep every original input/candidate text and old first-page expectation.
  For the ambiguous new `zheg` continuation explicitly freeze all six older
  first-page entries, the pool size 10 and exact second-page target at index 6,
  then still require Local, original spacing, exact-once submission and one undo.
  All full tails and other first-page cases stay strict. For English retain all
  sixteen original long `an` inputs with strict Word classification and add
  sixteen `anoth` inputs with strict Literal classification/byte-exact commit.
  No ranking, budget, timeout or production behavior is changed to force green.
- The resulting targeted run passes 73 tests across ten outer suites with one
  subprocess-only English family helper ignored externally but invoked by its
  parent tests (`targeted-final.log`, 31.29s). It includes all three new suites,
  all three family suites, resource snapshots, input/pack boundaries, all-pack
  quality and the nineteen CLI/SDK/registry integration tests. Typed provider
  errors are synthetic, not real network or live-model acceptance.
- The first full Rust run stops at two stale language-unit count assertions:
  Chinese 2,544 versus actual 2,592, English 561 versus actual 577 contexts;
  522 library tests pass, two fail, two are dedicated ignored entries
  (`rust-full.log`, cargo exit 101, 92.83s including compilation). Update only
  current totals (also English sentences 1,086 to 1,118); retain the exhaustive
  unique/reachable/padding loops and every historical hash. These are test-count
  maintenance, not relaxed limits or claimed language-algorithm fixes.
- The next full Rust run exposes an actual old first-page regression
  (`rust-full-final.log`, exit 101, 116.25s): new `这个怎么理解` is the fourth
  prefix completion for unfinished `zheg`, moving old Word `这个` to index 6.
  Keep the old `daily_choices_chinese` test unchanged. Replace only this
  unreleased new reading with independent `如何理解这句话`
  (`ru'he'li'jie'zhe'ju'hua`) and its fixed new-test expectation; all 2,544 old
  readings, 48-entry addition count, ranking and decoder limits are preserved.
  The new clarification-adoption page-two contract is separate from the old
  standalone demonstrative's first-page requirement; neither is relaxed.
  All twelve focused Chinese suites pass 75/75 after that data-only correction
  (`chinese-prefix-fixed.log`, 24.63s), including old daily, choices, social,
  needs, objects, coordination, follow-up, leisure and reading/progress guards.
- The all-feature Rust suite on the corrected data passes 1,335 tests across
  88 outer entries, with 29 dedicated/explicit gates ignored by the default
  invocation (`rust-full-fixed.log`, 255.88s). They are not claimed as executed
  native/live/device gates; the English family child-only helper is invoked
  explicitly by its parent integration tests. Earlier failed logs stay intact.
  Strict all-feature/all-target Clippy passes with warnings denied
  (`clippy.log`, 21.18s). Formatting, resource/fixture syntax, the six shipping
  manifest checks and 21 tensor-checker tests pass; the current implementation
  digest is refreshed without changing maturity scores or historical evidence.
- Correct the unreleased Chinese family wording from clothes `留一份` to
  `留一套`: fixed wording test first fails (`family-wording-red.log`, exit 101,
  1.48s), then both Chinese/Japanese family suites pass 9/9
  (`family-refinement-green.log`, 16.53s). Japanese family homophone priority is
  now checked for `sentaku`, `SENTAKU` and `せんたく`, preserving old `選択`
  choices and explicit `洗濯` selection. Original eleven shipped packs stay
  byte-identical; package sizes/units and manifest versions stay unchanged.
- Add six EN/ZH/JA word/sentence examples plus one physical page-two Chinese
  selection to the complete private GTK vocabulary gate: 104 examples, three
  older sentence-progress controls and one paging workflow (56 + 52 = 108).
  The focused `clarification` scope is six + three + one = ten, not the full
  gate. Paging must keep the raw draft and pool unchanged, numeric selection
  adopts without committing, Backspace restores exact unfinished spelling and
  Enter saves the exact intended sentence. The six examples also enter the
  existing synthetic missing/failed/timed-out/HTTP503/empty-provider matrix.
- The corrected builtin data passes the complete isolated Xvfb/GTK/IBus
  vocabulary gate: part-1 56 plus part-2 52 = 108 workflows, including the new
  page-two raw-draft/selection/undo/exact-save check (`gtk-full.log`, 404.70s).
  The native test keys are XTest-generated inside the fixture, not a human's physical
  keyboard or the personal desktop. Both partitions retain the existing
  240-second session deadline and use independent private configuration.
- The original separate pack-focused GTK scope passes eleven
  word/sentence examples plus three original Chinese progress controls = 14
  workflows (`gtk-packs.log`, 59.13s). Final log/source inspection shows its old
  setup only installed eleven selected packs, omitting EN/ZH outdoors and JA
  rail; do not confuse fourteen workflows with fourteen enabled packages.
  Extend only the fixture setup to explicitly export/install/assert all fourteen
  recommended packs in catalog order, retaining every word/sentence check and
  deadline. The expanded all-fourteen setup then passes all fourteen workflows
  (`gtk-all-packs.log`, 61.42s); its log independently confirms fourteen loaded
  recommended packages. The original eleven-selected-pack run is retained.
  This is not another complete builtin gate or a Release-package/
  personal-installation test. All expensive jobs were
  serial/offline under the task-owned 2G-high/4G-max/no-swap scope.
- Version remains 0.8.3; no personal installation, commit, push, Release package,
  live-model or physical-keyboard qualification is performed. Builtin additions
  require rebuilding/reinstalling before the personal desktop sees them;
  optional collections still require explicit install and host/panel restart.
- Follow-up coverage, not confirmed defects: independently enumerate very
  broad Chinese `ne`, `mingba`, `quere`, `bir`, `wozhida`, `nideyisish`,
  `zhejuhu` and `jieshi`, and Japanese `kan`/`かん`, `saika`/`さいか` in
  short/long drafts with all recommended packs active. Current complete Rust
  checks do not imply exhaustive reachability for every prefix; `jun`/`じゅん`
  already has fixed old-order/new-reachability coverage. Unknown short mixed
  prefixes and full Japanese morphology remain outside this batch's claims.

### Unreleased — Optional EN/ZH/JA family and home collections (2026-10-07)

- Append three model-independent v1 data packs with `family` / `home` topics:
  English 60 explicit forms/16 contexts/32 sentences, Chinese 36 readings/
  16 contexts/32 sentences, Japanese 24 readings/12 contexts/24 sentences.
  These are 88 authored short sentences, not personal family data or downloaded
  material. English contributes 26 new indexed forms against builtins, including
  sentence projections; explicit package entries are not unique net additions.
- Extend the optional catalog from eleven to fourteen. Preserve the original
  eleven raw files and catalog prefix with a fixed regression fingerprint.
  Builtin EN 6,134/561/1,086, ZH 2,544/555/1,108 and JA 178/90/179 are unchanged;
  English's 6,144 builtin budget, decoder/rank rules and model protocol stay
  unchanged. Japanese `sentaku` retains old `選択` ahead of appended `洗濯`.
  The new packs occupy 393/84/60 extension units and 12,692 bytes total,
  within existing limits; all fourteen packages can coexist.
- Before adding JSON/catalog entries, fixed independent tests reproduce missing
  optional collections: English 0/3 pass with one child helper ignored (1.37s),
  Chinese 0/7 (0.93s), Japanese 0/2 (0.98s), each cargo exit 101.
  Preserve `family_pack_english-red.log`, `family_pack_chinese-red.log` and
  `family_pack_japanese-red.log` under `/tmp/suzaku-family-packs.vFIsQu`.
  Expectations remain fixed; formatting changes no candidate expectations.
- First targeted green passes 49 tests across seven outer suites with
  one explicit child entry ignored by the outer runner (`targeted-green.log`,
  36.75s). Two English parent tests invoke that helper in fresh isolated
  processes. Family suites cover all authored words/sentences, spelling variants,
  partial readings, adoption/undo, editable Space and exact-once commits,
  synthetic typed model errors and absent/disabled/frozen startup snapshots.
  Earlier all-pack quality scenarios pass with all fourteen packs active.
- Fresh CLI tests filter each family language without installing on browse,
  export/install all fourteen into private data and independently disable each
  family package without affecting the other two. The package and package-smoke
  lists include the three new files. Six static shipping-list tests reject
  missing, duplicate or unknown entries; this is not artifact construction,
  Ubuntu ABI validation, clean-container installation or a new Release.
- Extend the private native pack gate from eleven to fourteen representative
  EN/ZH/JA workflows. The private GTK `packs` scope explicitly exports/installs
  eleven selected study/cooking/travel/work/family packs before host and panel
  startup, then exercises eleven examples plus three older sentence-progress
  controls. Japanese companion expectations use the existing selected-conversion
  snapshot contract, while the host must retain exact Romaji; previous EN/ZH
  assertions and all timeouts stay unchanged. This focused scope does not
  replace the unchanged complete builtin 101-workflow gate recorded below.
- Full all-feature Rust passes 1,311 tests across 85 result suites, zero
  failures (`rust-full.log`, 247.92s including compilation). The 29 outer-runner
  ignored entries are the usual 28 dedicated gates plus the English snapshot
  child explicitly exercised by its parent tests, not a skipped family workflow.
- All-feature/all-target Clippy with `-D warnings` passes (`clippy.log`,
  25.99s). Default and inline-preedit private IBus pack scopes each pass all
  fourteen workflows (`native-packs-default.log`, 3.86s;
  `native-packs-inline.log`, 3.09s). Both retain the 120-second bound, deliberately
  skip an independently damaged registry record, load all fourteen valid packs
  and keep the startup snapshot after later registry corruption.
- Real GTK editor and live companion pack scope passes eleven authored
  EN/ZH/JA pack examples plus three older sentence-progress controls, all
  fourteen workflows (`gtk-packs.log`, 61.55s). It exports/installs into its
  own private data, keeps editor focus, checks system-font draft rendering,
  numeric word/sentence adoption and exact spelling undo, editable Space,
  saved exact word/sentence commits and Japanese `掃除が…` continuation.
  No assertion or deadline is relaxed and no failing native run is retried.
  This is focused pack qualification, not a rerun of the complete builtin
  101-workflow, full C/IBus/UI, live-model or personal-desktop gates.
- Format/diff, Python/shell syntax, six shipping-list checks, source digest
  and all 21 tensor tests pass. Expensive jobs run serially with one build job
  in a task-owned 2GiB-high/4GiB-max/no-swap user scope. Parallel review confirms
  no data/metadata conflicts in the combined catalog; scores are unchanged.
- Version remains 0.8.3; these are opt-in collections, not automatically loaded
  into the personal desktop. No personal installation, commit, push, release
  package, live-provider or physical-keyboard acceptance is performed.

### Unreleased — Leisure, hobbies and weekend fallback (2026-10-07)

- Append authored model-independent data only: English `daily_leisure` adds
  16 contexts/32 sentences and five indexed forms (`listening`, `movie`,
  `podcasts`, `walks`, `watched`), reaching 6,134/561/1,086 within the unchanged
  6,144 index budget. Chinese adds 48 readings/16 contexts/32 sentences, now
  2,544/555/1,108; Japanese adds 24 readings/12 contexts/24 sentences, now
  178/90/179. Language algorithms, model protocols and candidate budgets stay
  unchanged. No downloaded corpus or personal-input data is used.
- Fixed independent tests reproduce the gaps before editing data: English
  0/5 pass (12.15s including compilation), Chinese 0/5 (0.81s), Japanese 1/7
  (0.67s). Each exits 101; `english-red.log`, `chinese-red.log` and
  `japanese-red.log` are retained under `/tmp/suzaku-vocabulary-leisure.nsejEZ`.
  The Japanese passing negative case preserves unknown text and line boundaries.
- Freeze all 18 previous English layers/ranks, 545 collocations/1,054 sentences,
  all 2,496 Chinese readings/539 contexts and all 154 Japanese readings/78 contexts.
  Retain every older snapshot. New tests keep literal spelling, word/sentence
  kinds, case/separators, long EN/ZH prefixes, explicit adoption, continued
  editable Space and exact-once submission/undo. Ambiguous Japanese prefixes
  explicitly retain old choices and later-page selection instead of requiring
  all matches on page one or changing their order.
- Add six mixed-language GTK/IBus workflows to the complete vocabulary gate:
  98 word/sentence workflows plus three older sentence-progress controls in two
  original bounded sessions (52 + 49 = 101). The `leisure` scope is only the six
  new workflows plus three controls. Synthetic NoLocalModel/Unavailable/Timeout/
  HTTP 503/empty-provider coverage includes all six new expressions.
- The first targeted green run passes all 60 assertions across seven suites:
  Chinese 5, English 5, Japanese 7, typed fallback 10, resource snapshots 15,
  all-pack quality 1 and pack/SDK integration 17 (`targeted-green.log`, 75.03s
  including compilation). Test expectations did not change after the red runs.
  The synthetic failure matrix preserves immediate Local candidates and exact
  adoption/commit after errors; all eleven recommended packs still coexist.
- Full all-feature Rust passes 1,297 assertions across 82 result suites with
  zero failures and the usual 28 ignored dedicated gates (`rust-full.log`,
  238.33s including compilation). All-feature/all-target Clippy with
  `-D warnings` passes (`clippy.log`, 23.32s).
- Complete private GTK/IBus vocabulary qualification passes all 101 workflows:
  part-1 passes 49 vocabulary examples plus three sentence-progress controls,
  part-2 passes 49 vocabulary examples (`gtk-full.log`, 389.75s including fresh
  binary compilation). Both mixed-language sessions retain their original
  240-second deadlines. All six new representative cases pass word/sentence
  labels, numeric adoption, exact spelling restoration, editable Space and
  saved word/sentence commits; Japanese `料理が…` also retains its sentence
  through adoption and particle continuation. This is the complete vocabulary
  gate, not every new authored phrase typed natively, a full C/IBus/UI gate,
  live-provider acceptance or human-keyboard testing.
- Format, diff/Python syntax, source digest and all 21 tensor tests pass.
  Expensive jobs are serial with one build job and a task-owned
  2GiB-high/4GiB-max/no-swap scope.
  Version remains 0.8.3; no personal installation, commit, push, release package,
  live-model/physical-keyboard qualification or maturity increase is made.

### Unreleased — Daily followup and Japanese coordination fallback (2026-10-07)

- Append only authored independent vocabulary data: English `daily_followup`
  has 24 contexts/48 sentences and only two new indexed forms (`having`, `stuck`),
  taking the built-in index to 6,129, still below the unchanged 6,144 budget.
  Chinese adds 64 readings/24 contexts/48 sentences, now 2,496/539/1,076;
  Japanese adds 32 readings/16 contexts/32 sentences, now 154/78/155.
  Decoder behavior, model protocol, candidate budgets and optional packs are
  unchanged. These are finite fallback examples, not a general language corpus.
- Before touching production data, fixed new tests confirm missing candidates:
  English 0/5 pass (11.97s including compilation), Chinese 0/5 (0.78s), Japanese
  1/6 (0.63s). Logs are `daily_followup_english-red.log`,
  `daily_followup_chinese-red.log`, and `daily_coordination_japanese-red.log`
  under `/tmp/suzaku-vocabulary-followup.9VPlRn`; each exits 101. Literal and
  unknown/boundary behavior is retained, not replaced by optimistic suggestions.
- Freeze all 17 old English word layers and their exact rank fingerprint,
  all 521 collocations/1,006 sentences, all 2,432 Chinese readings/515 contexts,
  and all 122 Japanese readings/62 contexts before appending. Earlier snapshots
  remain unchanged. Current count assertions are updated alongside actual
  authored input/commit regressions, not used as a substitute for them.
- The initial green attempt stops at two new Japanese tests, 4/6 pass,
  exit 101 in 14.95s including compilation (`targeted-green.log`). A separate
  diagnostic retains exact seeds and full candidates (`japanese-ranking-diagnostic.log`,
  exit 101, 0.96s): `kyouyu` has the second `共有` sentence at index 7 because
  `今日ゆ` and old-word tails compete; Kana `てん` keeps old `天気`, then `転送`,
  with `添付` at index 6. Both targets exist in a nine-entry candidate pool.
  This is an overstrong new-test first-page premise, not missing dictionary
  data or a corrected decoder defect. The tests retain every input and add a
  seventh strict ambiguous-prefix case: older priorities remain, later-page
  choices can be adopted/restored and committed/undone exactly once. All full
  readings and other specific partials still require first-page candidates.
  No production ordering, capacity, deadline or older regression is weakened.
- Independent bilingual/model-fallback/package checks pass all six result
  suites (52 assertions, `bilingual-packs-green.log`, 57.24s). The typed
  NoLocalModel/Unavailable/Timeout/503/empty-provider matrix includes both new
  English and Chinese expressions and Japanese `返信`/`準備中`; immediate local
  candidates remain identical through failure. These errors are synthetic,
  not live provider latency or model-quality acceptance. All eleven recommended
  packs coexist with the new built-ins; no pack content or registry was changed.
- The real GTK vocabulary harness now includes six fixed new mixed-language
  word/sentence cases in the default full scope (95 workflows in two original
  bounded sessions). Its explicit `followup` diagnostic runs those six plus
  three existing sentence-progress controls; passing that nine-workflow scope
  alone does not qualify the complete 95-workflow gate. Version remains 0.8.3; no personal
  install, restart, commit, push, release artifact or maturity increase is made.
- Final Japanese contract/resource qualification passes all 21 assertions
  (`japanese-contract-green.log`, 1.51s). All-feature Rust then passes 1,278
  assertions with zero failures across 79 result suites, retaining the usual
  28 ignored dedicated gates (`rust-full.log`, 249.71s including compilation).
  All-feature/all-target Clippy with `-D warnings` passes (`clippy.log`, 26.38s).
  The actual GTK/IBus `followup` scope passes all nine workflows with exact
  saved word/sentence commits, adoption, Backspace undo and Space continuity
  (`gtk-followup.log`, 34.41s including fresh binary compilation). These logs
  share the audit directory above. Full 95-workflow GTK and full native C/IBus
  gates were not rerun, and private XTest is not human-keyboard acceptance.
  Builds and native jobs were serial with one build job under a task-owned
  2GiB-high/4GiB-max/no-swap user scope; no OOM is inferred from earlier failures.
  Format, diff/Python syntax, source digest and all 21 tensor tests pass;
  architectural maturity scores remain unchanged.

### Unreleased — Event-time X11 clicks and presented-layout safety (2026-10-07)

- The independent owned-Xvfb regression reproduces the general stationary
  pointer bug without restoring the earlier semantic waiting-layout trigger:
  root point `[243,269]` initially clicks `z` at local `[167,186]`. Moving the
  owned GTK input/caret moves the real panel; both the rendered frame and the
  X server then identify `w` at local `[163,126]`, but the old binary appends
  another `z` (`qzxvzz`, not `qzxvzw`). The first button-only post-move tap fails
  exact-text validation, exit 1 in 10.99s. Original log:
  `/tmp/suzaku-pointer-audit.cK6oKR/pointer-red.log`; retained synthetic files:
  `/tmp/suzaku-app-qa.OBHCVX`. No desktop pointer, keymap or service was changed.
- Upstream winit 0.30.13's XI2 button conversion discards event coordinates,
  whereas its motion conversion de-duplicates cached local coordinates. Real
  ConfigureNotify on the no-WM fixture also does not emit the public Moved
  callback. Moved-only rebasing cannot cover this case. A later QueryPointer
  snapshot can belong to another queued click, so the fix does not poll the
  pointer or create an extra connection/worker. The fixed local dependency
  passes coordinates from the same XI2 button event before each MouseInput,
  with the original touch-emulation filter and wheel-only mapping retained.
  The full 3.2MiB upstream crate and its Apache-2.0 license are retained;
  `vendor/winit/NOTICE-SUZAKU.md` describes provenance, changes and removal.
- Two exploratory dependency-test invocations stop before compiling/running
  any assertions: without enabled GPU the dependency is not selected, and
  dependency-only feature selection is disallowed outside a workspace (both
  exit 101, `vendor-unit.log` / `vendor-unit-features.log`). The exact pure
  dispatcher helper is therefore shared with a root integration-test module,
  rather than adding a new workspace, test-only dependency graph or download.
- The new actual-window checks also exercise a real resize and button-only
  second tap, and queue clicks at two different positions before dispatch.
  Planning reads real old/new rendered geometry; it does not retry failed
  input, inject fake Configure events or extend the fixture deadline.
- The first shared-helper run passes four exact-module tests (18.42s including
  compilation). Focused actual-window default/synchronous runs both pass all
  three checks (9.25/7.20s), including `qzxvzw` → `qzxvzww` after the original
  move and two exact `d` inputs after resize, with ordered queued `q` → `qp`.
  These runs used the first CursorMoved-based patch, not the final separated
  button-position event. Review then identified a queued manual-drag risk:
  treating button coordinates as motion could advance the drag on release.
  The actual queued-motion/release regression confirms this: from `[76,83]`
  one motion of `[12,18]` ends at `[100,119]` (twice the displacement), not
  `[88,101]`. `drag-red.log` exits 1 in 11.09s; synthetic files are retained
  at `/tmp/suzaku-app-qa.YZ261x`. The complete native draft frame is unchanged,
  so the assertion measures movement rather than input-context replacement.
  The final adapter instead reports button positions without pretending they
  are a physical motion sample; that changed snapshot requires its own gates.
- The separated button-position adapter's focused gates pass all four actual
  stationary/queued-click/drag checks in both default and synchronous transport
  (`pointer-default-final.log`, 9.21s; `pointer-sync-final.log`, 7.39s). The drag
  ends at `[88,101]` exactly once, with focus, GTK insertions and the complete
  native draft frame unchanged. These are owned-Xvfb source-build results,
  not personal-desktop, multi-monitor, native Wayland or package acceptance.
- Initial UI runs also preserve failures rather than qualify a partial pass:
  `ui-first.log` (exit 101, 13.77s) expects `hela` but correctly inserts `ahel`
  because the new fixture left its caret at zero. The test now explicitly moves
  its caret to the end, retaining exact `a`/`aa`/`aab` assertions.
  `ui-complete.log` (exit 101, 19.18s) passes the window/drag gate, then rejects
  the old hidden-panel fixture's unpresented measured layout. Pointer fixtures
  now read back and actually present their owned viewport before testing a
  gesture; they do not bypass the guard, replay a press or alter ACK assertions.
  None of the later full Rust/Clippy/application pipeline stages ran after
  that stop, and neither UI run is counted as a complete pass.
- The next UI stop (`ui-final.log`, exit 101, 22.73s) reveals a genuine
  completion-release regression, not just a fabricated-frame fixture: after
  an identical candidate refresh, a held `hello` completion is lost (`hel`
  remains). With the click fixture changed to actual rendering, the independent
  confirmation still fails identically (`ui-identical-red.log`, exit 101,
  26.20s). The guard now keeps a narrow permission for that already-acquired
  completion release only, comparing its full edit, seed, language, original
  presented rectangle, geometry and truncation. It does not keep a whole stale
  scene or authorize a new unpresented press; candidate replacement, canceled
  gestures and changed geometry still fail closed. Later pipeline stages did
  not run after either failing UI stop.
- A subsequent qualification attempt compiles the production binaries in 4.58s,
  then stops before UI execution because the newly added resize fixture omitted
  the `PhysicalSize` import (`ui-qualified.log`, exit 101, 2.05s). It now uses
  the fully qualified type; this compile failure is retained and is not counted
  as a UI result. The candidate fixture also checks duplicate release and twelve
  mouse/touch boundaries without rendering between refresh and release.
- After those fixes the complete UI/launch gate passes all 18 required opt-in
  entries (`ui-release-guards.log`, 104.62s). Identical-refresh completion and
  duplicate release pass, as do the twelve negative boundaries and actual
  rendered-window fold/scale/orb/drag checks. This is the original gate with
  its deadlines and semantic assertions retained, not a selected subset.
- Full all-feature Rust qualification then passes 1,259 assertions, zero
  failures, across 76 result suites (28 dedicated tests remain normally ignored;
  `rust-qualified.log`, 228.80s including compilation). All five tests sharing
  the exact final production button-position helper execute and pass; this does
  not substitute for upstream's complete crate suite or another platform gate.
- The same snapshot's all-target Clippy gate is warning-free (21.94s), and
  actual GTK3 input/presentation passes all 24 workflows in both default and
  synchronous modes (21.64/21.79s). The subsequent default native-candidate gate
  stops at page-arrow readiness, exit 1 in 9.02s; none of the later layout gates
  ran in that pipeline. This failure is not attributed to an OOM or weakened.
- Read-only event diagnostics isolate a fixture transition, not a bad click
  coordinate: revision 6/page 0 first presents at 900×231, then requests its
  fitted 900×278. Resize is processed before the queued click and before redraw,
  so both presented-scene caches and the acquired press are absent; the product
  correctly rejects the unpresented-layout click. Retained diagnostic runs:
  `candidates-page-diagnostic.log` (exit 1, 15.39s; the first trace flag did not
  match application QA), `/tmp/suzaku-app-qa.7mqWxE`; and
  `candidates-page-events.log` (exit 1, 14.87s),
  `/tmp/suzaku-app-qa.etTyjh/process-2.log:18–22`. The temporary event traces
  have been removed. No claim is made that a delayed same-revision ACK caused it.
- Synthetic frame logs now report `viewport_settled`: actual native/render/config
  sizes agree, the existing resize acknowledgement is not pending, and the
  follow-caret Main frame equals the existing fitted-size calculation. The
  no-WM candidate fixture requires this before its single click; it neither
  retries a failed click nor adds sleep/deadline budget. This QA-only predicate
  is not a new production click policy or a universal compositor guarantee.
- With settled-fit readiness, the default full candidate gate passes 30 cases
  (21.07s). The synchronous gate passes its original 26 cases, then its new
  move planner detects a frame/server-position mismatch before sending the
  exercised click (`candidates-sync-settled.log`, exit 1, 12.51s;
  `/tmp/suzaku-app-qa.aEzLYy`). Caret geometry can change without a new candidate
  revision, so revision equality alone cannot prove the companion processed the
  newest cursor. Synthetic frame logs now include the observed native cursor,
  and `page_ready` requires it to equal the owned host subscription's cursor.
  This observation does not change the public companion protocol or input path.
- The first cursor-observation build stops before application execution because
  `NativeCursorRect` deliberately does not implement `Serialize`
  (`candidates-default-cursor.log`, exit 101, 1.31s). The private diagnostic now
  explicitly maps the four geometry fields instead of changing that public type.
  No application result or unchanged-snapshot rerun is inferred from this stop.
- With both readiness observations, full candidates pass 30 cases in each
  transport mode. The next default bottom-layout gate stops before layout
  activation (`bottom-default-settled-cursor.log`, exit 1, 9.94s;
  `/tmp/suzaku-app-qa.swlCSp`): its settings click used the initial 620×460
  presentation while settings fitting requested 620×485. `click_control` now
  requires settled presentation too. Settings readiness uses the same existing
  fitted-size calculation as the production request, extracted unchanged into
  `fitted_settings_size` rather than duplicating constants in the fixture.
- The corrected settings click passes, then the old immediate bottom stability
  fixture treats an unobserved new cursor as movement (`bottom-default-controls.log`,
  exit 1, 24.86s). Its failure snapshot has the same revision/context and position:
  host cursor `[220,116]`, companion cursor `[76,56]`, dock position `[12,464]`,
  size 1000×292 and settled viewport. This is not evidence that the dock moved.
  Both negative caret checks now await current-cursor observation while asserting
  unchanged composition identity and observed position on every poll, then run
  the original 0.15s stability predicates unchanged. The existing four-second
  wait budget is retained; no click is replayed and movement during the observation
  wait is also a failure.
- The next full bottom gate passes its original workflows, including restored
  follow-caret behavior, but the first new keyboard prime clears `qzxv` instead
  of appending `z` (`bottom-default-final-validation.log`, exit 1, 28.63s).
  Context 24→26 alone cannot distinguish two GTK resets from focus boundaries.
  A separate read-only root-pointer diagnostic stops before injecting that
  click: the actual recipient is GTK window 4194308, not panel 6291459
  (`bottom-stack-diagnostic.log`, exit 1, 22.97s; retained fixture
  `/tmp/suzaku-app-qa.NemNRB`). The window-tree stacking order confirms GTK
  is above the panel. The fixture's settings return reused an app-launch
  helper that explicitly raises GTK; it now restores only keyboard focus,
  without raising either window. Button-only regressions additionally require
  the root-pointer recipient to be their owned panel, rather than assuming
  translated panel coordinates prove ownership. No production click guard,
  stack policy, input retry or deadline is changed. This excludes an artificial
  no-WM fixture obstruction, not real-compositor stacking compatibility.
- The final settled-size/cursor/recipient snapshot passes every complete native
  candidate application gate in both transport modes: follow-caret 30/30
  (`candidates-default-qualified-stack.log`, 14.49s;
  `candidates-sync-qualified-stack.log`, 14.73s), bottom layout 39/39
  (`bottom-default-qualified-stack.log`, 25.47s;
  `bottom-sync-qualified-stack.log`, 23.94s), and production-detected Auto
  42/42 (`auto-default-qualified-stack.log`, 27.98s;
  `auto-sync-qualified-stack.log`, 28.56s). All six include real stationary
  move/resize double taps, ordered queued clicks and exactly-once manual drag.
  Their 222 workflows use private Xvfb/IBus/GTK only; no personal installation,
  live model quality, physical touch or compositor compatibility is inferred.
- Final source qualification after the settings-size extraction also passes
  the complete 18-entry UI/launch gate (89.39s), all-feature Rust with 1,259
  passes/zero failures across 76 suites (28 normally ignored, 180.64s), and
  all-feature/all-target Clippy with `-D warnings` (4.34s). Logs are
  `ui-final-validation.log`, `rust-final-validation.log`, and
  `clippy-final-validation.log` in the audit directory above. Both final GTK3
  input/presentation gates pass all 24 workflows (`gtk3-default-final-validation.log`,
  21.29s; `gtk3-sync-final-validation.log`, 21.76s). Jobs ran serially with
  one build job in a task-owned 2GiB-high/4GiB-max/no-swap user scope. Format,
  diff, shell/Python syntax, tensor digest/check and its 21 tests also pass.
  The full native C/IBus gate and release archives were not rerun in this
  pointer-only round. Version remains 0.8.3; no installation, commit, tag,
  push, remote CI qualification or desktop input-service mutation was made.
- Linux packages must collect licenses/notices for path dependencies as well
  as registry crates; `source:null` is not a reason to omit the modified winit
  license. Both tar/deb content gates now require its license and patch notice.
  Packaging-script changes and metadata checks alone do not qualify a newly
  built release artifact or a personal installation.

### Unreleased — Host-owned screen keys and linear commit history (2026-10-07)

- Native screen Backspace used to send a locally guessed full replacement:
  adopted `hello` became `hell` instead of restoring `hel`; Chinese `你好`
  similarly became `你` instead of `nihao`. The opt-in panel red regression
  exercises the actual `backspace_seed` path and captures `A... 11 Thell`
  (`/tmp/suzaku-screen-followup.SLVe7E/panel-screen-red.log`, exit 101).
  Screen Space also used the raw draft rather than the explicitly selected
  candidate. These edits now use host-owned Backspace/continuation semantics,
  including cancellation of unfinished Compose before any draft deletion.
- The distinct `E{host} {revision} B/S` requests require the same public,
  non-bypass input context and exact current revision. Success returns one
  acknowledgement byte plus the complete newline-delimited current frame on
  the same connection, within the original 350ms/64KiB result budget. The
  result supplies the actual spelling after undo, not an optimistic guess.
  Unknown/old hosts, stale targets, malformed/incomplete/oversized replies and
  privacy/focus changes fail closed; there is no replacement fallback or
  automatic resend. Existing `A` requests and frame schema v1 remain compatible.
  Empty screen Backspace is a fresh-revision no-op; empty screen Space starts
  an exact editable space rather than touching the application's document.
- The old compiled host rejects the new screen request with `0` in the protocol
  red gate (`native-screen-red.log`, exit 1). This is missing-capability evidence,
  not a second reproduction of the panel's wrong-string edit. That run also
  records a secondary fixture cleanup failure: it closed the engine before
  requiring a same-value settings publication. Restoration now acknowledges
  public focus and settings while the owned engine is alive; the original error
  remains primary and every normal revision/context assertion is retained.
- The first updated-host screen gate passes exact physical/semantic EN/ZH
  replay comparisons, adoption undo, default/explicit Space, Compose cancel
  and Space results, geometry-preserved undo, Unicode scalar deletion,
  consecutive returned-ACK requests, empty drafts and stale/focus/privacy/
  malformed rejection in 0.591 seconds. No CommitText is emitted in these
  editing checks. It does not alone qualify the panel's asynchronous queue.
- The shared engine's commit history previously cloned every accumulated
  committed prefix. With 1,000 commits of 1KiB each, those retained history
  copies alone total about 488MiB; 2,000 total about 1.9GiB. History now stores
  one UTF-8 byte checkpoint per commit and truncates to it on undo, preserving
  unlimited undo depth, existing editor separators and native verbatim commits.
  Current committed text still grows linearly; this is not a total-memory cap
  or proof of the cause of any earlier OOM. Four integration regressions cover
  Unicode/whitespace, failed commits, clearing, branch-after-undo and 130-step
  undo. One internal regression asserts the byte-only storage and exact native
  and editor joins. Future in-place changes to committed text must maintain
  those checkpoints. The first library run passes 522 tests with zero failures
  and two existing opt-in skips, including that storage test and four keyboard
  transport contracts (`rust-lib-first.log`).
- The first full Rust pass was deliberately stopped during registration tests
  after review found that disconnecting before a semantic reply and immediately
  recovering locally discarded the original draft prefix. The partial
  `rust-all-final.log` (exit 143) is preserved and is not a full validation pass;
  its later Clippy/UI/native/application stages did not run. Recovery now needs
  the last same-target authoritative public draft, including confirmed adoption
  and later semantic flights, plus unsent literal text. Unconfirmed Backspace
  and Space must never be guessed or automatically replayed.
- The panel keeps at most 64 ordered semantic/literal edits and 8192 unsent
  literal bytes. It waits for both parts of preceding adoption/tool actions,
  then for the complete semantic reply before sending following text. Opening
  settings, entering local focus, changing targets, failures and uncertain
  replies stop the queue rather than revive it. A selection barrier also checks
  the selected row: if the latest-only reader skips the chosen row and sees a
  later physical selection with the same raw spelling, queued Space must stop
  in either ACK/frame order instead of accepting that other candidate.
- New host fixtures cover partially written semantic requests across focus and
  privacy boundaries, and an edit that crosses the mirror byte limit. In the
  latter case the host may have applied Space but can only return a redacted
  snapshot; lack of a public confirmed result is not proof that nothing happened.
  The panel stops without retrying. One physical Backspace restores an in-budget
  public draft; the original request stays stale. These checks keep the original
  deadlines and do not inject keys into the desktop session.
- The complete-source serial pass (`*-complete.log`) records 1254 passed Rust
  tests, zero failures and 28 opt-in skips; strict all-target/all-feature Clippy
  and all 18 UI/launch opt-ins pass. Both full native gates pass (103.78/104.96s)
  and GTK3 passes 24 workflows in each transport (21.47/21.61s). It then fails
  the new real bottom-key sequence: the last
  intended Backspace at `[926,666]` leaves `hel p`, not `hel`
  (`bottom-default-complete.log`, exit 1). The synchronous bottom gate did not
  run. This is not a complete green application result; the original failure is
  preserved while click/frame/window geometry is investigated independently.
- The read-only diagnostic repeats the same failure (22.07s, exit 1) and keeps
  synthetic artifacts in `/tmp/suzaku-app-qa.oseQmu`. Immediately before each
  click, the rendered frame and X server agree on `[12,264]`, `1000x492`, and
  Backspace at `[926,666]`. Semantic waiting had temporarily cleared all
  candidates, shrinking the dock by 106px before restoring it. A stale local
  pointer coordinate could then land exactly on `p`. Local winit 0.30.13 source
  confirms that real ConfigureNotify without a WM need not emit Moved, and
  MouseInput omits the native button event's coordinates; a Moved-only rebase
  or mixing event-local coordinates with a later server origin is insufficient.
- The scoped fix retains the current host page as a read-only placeholder
  during pure semantic waiting. Busy cards are muted and absent from both hit
  paths; paging and native actions remain blocked. Ordinary unconfirmed literal
  typing still hides its old candidates. The new panel cases assert unchanged
  dock height and every global keyboard rectangle while B/S waits, then exact
  authoritative updates and restored interactions. The renderer regression
  checks identical busy/ready geometry, no busy hits, and restored ready hits.
  The unchanged real bottom sequence now passes all 35 workflows (34.33s
  including 11.46s rebuilding, `bottom-steady-first.log`); its synthetic images
  and logs are kept in `/tmp/suzaku-app-qa.qVNE95`.
  This removes the new semantic-wait resize/restore trigger, not every possible
  stale-pointer case after an unrelated window move. A general X11 pointer
  solution remains separate and must not introduce an unbounded UI-thread query
  or guess uncertain coordinates.
- Final steady-layout validation completes successfully in a fresh serial scope:
  all-feature Rust again passes 1254 tests, zero failures and 28 opt-in skips
  (241.65s); strict all-target/all-feature Clippy passes (21.86s); all 18
  UI/launch opt-ins pass (81.27s), including immediate disconnect recovery,
  expected-selection conflicts and pending dock geometry. GTK3 default/sync
  passes 24 workflows each (21.58/19.96s); bottom layout passes 35 each
  (19.88/20.47s); Auto layout passes 38 each (24.44/24.67s), including real
  settings, fresh-process persistence and both new screen-key sequences.
  See the distinct `*-steady-final.log` files under
  `/tmp/suzaku-screen-followup.SLVe7E`; the interrupted and red logs remain intact.
  The earlier full native default/inline gates qualify the unchanged C/transport
  protocol; they were not rerun after the scoped read-only-candidate UI change.
  Compilation and application checks used one build job, one Rust test thread,
  offline cached dependencies and task-owned 2GiB-high/4GiB-max/no-swap scopes;
  the last observed final-scope peak was 1.69GiB. No assertion, suite deadline or
  negative observation was relaxed. Functional tensor validation and its 21
  fixtures pass with the reviewed source digest; formatting, shell syntax and
  diff checks pass. Shellcheck is unavailable locally, not counted as a pass.
  No personal installation, desktop keyboard state, release version, commit,
  push or remote CI result changed. Real user/Wayland/touchscreen acceptance and
  the general stale-pointer boundary remain separate work.

### Unreleased — Adopted Compose undo and physical navigation coverage (2026-10-07)

- Starting a dead-key/Compose sequence used to discard the last adopted-word
  undo before the sequence changed any text. A private native red regression
  reproduces `hel -> hello -> dead_acute -> Escape -> Backspace` failing to
  restore `hel`, while the daemon and bus remain connected
  (`/tmp/suzaku-app-followup.WhcwwM/compose-undo-red.log`, exit 1).
  Pending Compose now keeps undo; actual printable edits still retire it
  through the existing input-sync path. No keyboard layout or lock state changes.
- Ten EN/ZH native workflows cover Escape/Backspace cancellation, a completed
  accent, an unmatched printable character, and the existing private rejected-control
  Compose result. Cancellation and rejected non-printable results must preserve the
  adopted draft/candidates and exact spelling undo, without committing or
  treating the final Compose digit as a candidate number. Real edits must keep
  UTF-8 scalar deletion and must not revive old undo. The initial eight-case
  focused post-fix run passes in 0.231 seconds (`compose-undo-green.log`);
  the two additional rejected-control cases are also included in the full gate.
  The fixture's Return keysym yields one raw U+000D byte on the local xkbcommon;
  printable validation rejects it. The test's `empty-result` label means no
  accepted draft text, not coverage of the distinct raw-length-zero branch.
- English `hello”` discarded a pending preference even though `hello"` retained
  it: the incremental red test fails with zero learned entries instead of one
  (`preferences-quotes-red.log`, exit 101). The closing smart double quote now
  shares the ASCII double-quote boundary rule. Eight incremental quote variants
  require exact literal commits, no learning before confirmation, and reversal
  on undo. Four straight/smart apostrophe possessive/contraction controls remain
  non-boundaries; URL, identifier and punctuation-run controls remain intact.
  All 13 preference tests pass after the fix (`preferences-quotes-green.log`).
  The full native gate also checks closing quotes versus smart apostrophes.
- The preceding source snapshot passes the original 17 strict GTK3 workflows
  in default/synchronous transport (19.69 / 20.01 seconds). Its native drawn
  candidate gate passes 26 workflows per transport (14.06 / 11.69 seconds),
  including all ten English card clicks, Chinese word/sentence commits,
  long-candidate packs, paging and caret placement. Those runs precede the
  Compose/quote fixes and do not qualify the final snapshot by themselves.
- GTK3 now adds seven physical-navigation workflows without replacing the
  original 17: EN/ZH first-row Up/Shift+Tab preserve exact state and literal
  Space; adopted-word boundaries preserve spelling undo; genuine adjacent-row
  roundtrips still select explicitly; one-row Down/Tab must not insert Tab or
  transfer focus. It checks the complete frame, focus and buffers for 150ms,
  with unchanged four-second key and 240-second session deadlines.
- The first expanded GTK3 run fails because its widget observer only sees
  key releases after IBus consumes the presses, not because the draft changed
  (`gtk3-default-expanded.log`, 8.72 seconds). The fixture now observes GDK
  before GTK's input-method filter, on its owned window only, and forwards
  each original event to the known default GTK handler exactly once. The
  old widget log remains, callback errors retain/raise the first failure, and
  teardown restores default dispatch. All nine no-display observer contracts
  pass; Linux CI runs them. No retyping, event reinjection or deadline relaxation.
  Model Pending preservation is still qualified by the separate private native
  HTTP gate, not this model-disabled GTK3 fixture. All logs are retained under
  `/tmp/suzaku-app-followup.WhcwwM/`.
- Final functional snapshot: all-feature Rust regression passes 1,239 tests
  with zero failures and 27 existing opt-in skips; formatting and strict
  all-target/all-feature Clippy pass. Complete default/inline private IBus
  gates pass in 105.92 / 104.87 seconds, including all ten adopted Compose
  workflows and all 14 preference-boundary cases per mode. No critical
  diagnostics occur in these two logs. The expanded GTK3 gate passes all
  24 workflows in each default/synchronous transport (22.39 / 21.96 seconds).
  These are owned Xvfb/IBus application tests, not human keyboard or Wayland
  qualification. No install, commit, push, version change or remote CI claim.

### Unreleased — Indexed local candidates and attributed native latency (2026-10-07)

- English sentence matching now scans a stable first-ASCII-letter bucket rather
  than every authored sentence at each eligible word boundary. The index retains
  layer/authored order and duplicates; the existing case, apostrophe, whitespace,
  protected-token and longest-context matching rules remain unchanged.
- Chinese dictionary-word classification now uses a startup-frozen membership
  set rather than rescanning all readings on every beam classification. This
  stores vocabulary only, not typed text, and does not change decoder or
  homophone priority. There are no new words, candidate budgets or model rules.
- Three deterministic equivalence tests cover the complete preset vocabulary,
  every English authored ASCII word start with case/apostrophe/Unicode-space
  variants, and Chinese reading/text/continuation/whitespace probes. All 28,452
  English probes preserve exact ordered results: 2,446,052 indexed comparisons
  versus 28,622,712 in the linear reference. This counts work, not UI latency.
- `SUZAKU_NATIVE_TIMING=1` reports outer native workflow times; nested checks
  retain one phase, and failures retain their phase for teardown diagnostics.
  Timing-output failures cannot replace the original workflow failure. Five
  isolated trace-helper tests cover these contracts (28 helper tests total).
- The pre-index full native baseline takes 115.42 seconds, with bilingual
  completion at 10.210 seconds, exact model context at 4.956 seconds and fallback
  provider failure/timeout coverage at 15.527 seconds. N32's 20 probes now hold
  explicit Pending response gates until their exact request context is checked,
  then release and require response completion/settlement. This removes only
  their second artificial delay; ten model-fixture checks pass. The real 500ms
  timeout cases, stable-negative windows, all workflow assertions and the
  120-second native session limit remain unchanged.
- All-feature Rust regression passes 1,238 tests with zero failures and 27
  existing opt-in skips; strict all-target/all-feature Clippy passes. Evidence
  remains in `/tmp/suzaku-native-followup.tPjRh7/`.
- Complete default/inline private IBus gates pass in 103.09 / 103.56 seconds
  including build/teardown, without critical diagnostics in these two logs.
  Bilingual completion takes 4.223 / 4.272 seconds versus 10.210 before indexing;
  N32 takes 3.776 / 3.733 seconds versus 4.956 before its explicit gates.
  These are local observations, not remote CI or real-keyboard qualification.
- A separate bounded GDB diagnostic, with `G_DEBUG=fatal-criticals` only on its
  owned private daemon, stops after presentation and during activation attempt
  1 at `_g_dbus_method_invocation_new`'s invalid-sender critical (7.278 seconds).
  It is an unsuccessful diagnostic, not a passing activation/release gate or
  a reproduction of the distinct `g_dbus_message_set_sender` value check.
  Preserve `capture-critical.hz_0f529/`: the worker is in method dispatch while
  the main thread processes a closed connection/unregisters an object. A debugger
  freezes other threads; this stack alone proves neither a deadlock nor the
  proposed connection-name use-after-free. Do not merge this failure into the
  successful uninstrumented gates or call the daemon risk resolved.
- On 10-07, official IBus source still has the borrowed name getter and name
  release in [connection.c](https://github.com/ibus/ibus/blob/d464316d9b9613005c5573a010fec7f9b40b93b6/bus/connection.c).
  The nearby [engineproxy cancellation fix](https://github.com/ibus/ibus/pull/2958)
  touches another lifetime; it is not evidence of a fix for this sender warning.
  No system library, personal installation, version/tag or remote publication
  is changed by these candidate optimizations or the isolated investigation.
- The second bounded diagnostic also fails (18.252 seconds, activation attempt
  6), now with identifier-only stack metadata: a `RemoveMatch` request has an
  invalid sender (`60c4` bytes), while worker dispatch and closed/unregister
  processing identify the same connection. Preserve `capture-critical.45g_vf1f/`.
  This strengthens the close-lifecycle evidence but still does not identify the
  exact allocation owner or reproduce the distinct sender-setter check.
- A concrete probe lifecycle gap is fixed: GDBusProxy finalization queues its
  unsubscribe only at the last context unref, which used to follow every engine
  restoration check and immediately precede bus unref/close. The probe now
  requires a same-connection `GetNameOwner` reply after context finalization and
  before closing the bus (250ms bound). Failure is stage 23 unless an earlier
  probe error already exists; the earlier error wins. No duplicate RemoveMatch,
  new sleep, substituted CLI or system-library patch is introduced. This repairs
  the owned probe's sequencing; it is not proof that every upstream sender
  lifetime fault or the original warning is resolved.
- The real cleanup block and helper are compiled against deterministic Gio-linked
  stubs by `scripts/test-native-probe-teardown.py`. All 12 subcases fail on the
  read-only HEAD baseline and pass after the fix: successful acknowledgement,
  missing/closed connection and failed RPC, each with prior stages 0/7/10.
  They require final context unref before the same-connection call, bus close
  last, bounded failure reporting, original-error precedence and reply/error
  release. Logs remain in `/tmp/suzaku-probe-teardown-logs.ApJJtO/`; this is
  not an IBus or upstream daemon qualification. The fixture is added to Linux CI.
- The final cleanup-fix snapshot again passes 1,238 all-feature Rust tests with
  zero failures and 27 existing opt-in skips, and strict all-target/all-feature
  Clippy (`rust-teardown-final.log`, `clippy-teardown-final.log`). The complete
  private UI gate passes 17 tests on the immediately preceding indexed snapshot
  (`ui.log`); the subsequent change touches probe cleanup, not UI input paths.
- Complete default/inline private IBus gates on the final cleanup-fix snapshot
  pass in 103.54 / 102.14 seconds including build/teardown
  (`native-teardown-default.log`, `native-teardown-inline.log`). Neither log
  contains a critical diagnostic. Bilingual completion takes 4.230 / 4.240
  seconds; the exact model-context workflow takes 3.718 / 3.653 seconds.
  The unchanged 120-second session bound and all complete-gate assertions
  remain in force; this does not replace the failed remote CI evidence.
- The final snapshot's independent offline-pack gate passes all 11 EN/ZH/JA
  workflows in 2.61 seconds (`native-teardown-packs.log`), including sentence
  continuation after word adoption and the immutable startup snapshot. The
  final cleanup contract also passes (`probe-teardown-final.log`).
- One post-fix diagnostic uses the same ten-activation/45-second bound and
  daemon-only fatal-critical instrumentation. All ten activation/release checks
  finish, driver exit is 0 in 27.772 seconds, and neither sender-setter nor
  invocation-sender critical is reproduced (`capture-critical.zphzyu09/`).
  Its owned debugger is terminated during fixture cleanup (exit 255), not
  qualified as a naturally exiting daemon. This is a bounded non-reproduction,
  not proof that every upstream sender fault is fixed; both pre-fix diagnostic
  failures remain preserved. No installation, commit, push or tag update occurs.

### Unreleased — Inert input boundaries preserve the writing flow (2026-10-07)

- First/last-row relative navigation now leaves a clamped selection unchanged
  in the shared engine, IBus host and native panel. It no longer locks a default
  choice, cancels Pending prediction, discards adoption undo or publishes/sends
  redundant revisions/actions. Actual movement and explicit same-row selection
  still lock the choice. Empty/single-row lists and extreme signed deltas are
  bounded without arithmetic overflow; no navigation keys or global bindings change.
- Local panel Backspace at the caret's start or on an empty draft no longer
  rebuilds candidates, clears word-completion undo or cancels prediction/translation.
  The native typing queue retains priority, and real character deletion still
  invalidates derived results. This does not add cursor movement inside IBus drafts.
- Red regressions reproduce both core row-boundary failures, an IBus Up causing
  an unnecessary revision, a panel boundary sending a Select request and local
  Home/Backspace losing completion history. Logs are retained under
  `/tmp/suzaku-input-ux.jZLlsL/`. After the fixes, all 27 prediction tests and the
  focused private IBus row gate pass. The latter includes EN/ZH keyboard/home-row/
  IBus cursor gestures, Space spelling, exact adoption undo, one-row fallback and
  four controlled HTTP preserve/cancel cases; real movement and explicit selection
  remain positive controls. The complete private UI gate passes all 17 tests,
  including local Backspace preservation and revision-bound panel navigation.
- The first complete native run stops at 17.254 seconds in the Japanese
  fixture (`native-default.log`): its row-zero Up expected an explicit selection
  and new revision. Test preparation now makes an actual adjacent move and
  return, checking each revision, index and unchanged full draft/candidate list;
  the single-row case requires a literal payload and uses explicit numeric
  selection. Editable-completion, destruction and unchanged-control fixtures use
  the same helper; shortcut boundaries require a fresh identical snapshot.
  No assertion or deadline is removed. The original failure log is retained;
  complete post-fixture results follow.
- The corrected complete default/inline IBus gates pass in 118.09 / 111.47
  seconds including build/teardown, with the 120-second session limit unchanged
  (`native-default-final.log`, `native-inline-final.log`). Default-mode local
  timing headroom is small and is not remote CI qualification. One
  `g_dbus_message_set_sender` GLib-GIO critical diagnostic
  appears during activation; the run completes without a daemon abort. Preserve
  it in the log rather than treating this UX fix as a resolution of that warning
  or the separately deferred daemon investigation.
- All-feature Rust regression passes 1,235 tests with zero failures and 27
  existing opt-in skips; strict all-target/all-feature Clippy also passes.
  Formatting, source digest, 21 tensor checks and nine model-fixture checks pass.
  Native/UI execution uses private sessions and owned synthetic drafts under
  serial, memory-capped scopes; the existing Xvfb DRI3 and private portal warnings
  are not suppressed or presented as personal desktop qualification.
- No model, vocabulary, timeout, personal installation, version, tag or remote
  publication changes are part of this work. The earlier v0.8.3 CI failure and
  deferred daemon-abort investigation retain their separate status.

### Unreleased — Discarded decoder work and native-gate latency (2026-10-07)

- The first 0.8.3 CI (`37462265391`, source `1bf93e1`) failed with exit 124
  at the native session's 120-second deadline, not a reported assertion or
  daemon abort. The prior successful native gate used about 112.7 seconds;
  its sentence-only marker was at 96.6 seconds versus 115.4 seconds in the
  failed run. This is cumulative timing evidence, not proof of a hung N52 case.
  Keep the failed CI log at `/tmp/suzaku-release-0.8.3.v3zXu7/ci-failed.log`.
- Fix actual input-thread work: after the existing English/Chinese bounded-tail
  route and empty/whitespace handling, IBus drafts beyond 256 scalars now take
  the existing literal-only result before invoking a whole-draft decoder whose
  candidates would be discarded anyway. Non-IBus plugin behavior, payloads,
  output budgets, long English/Chinese completions and model limits are unchanged.
- Six deterministic plugin-call regressions first produce two failures and
  four passes, then all pass: cover direct/generic decoders, Unicode scalar
  boundaries, adoption/deletion back to the budget, exact commit/undo, nonmixed
  semantics, supported tails and empty/whitespace inputs. The original red log
  and 35 passing focused Rust checks remain in `/tmp/suzaku-native-timeout.9hjcgy/`.
- The unchanged 72-case private native draft test now has a focused selector and
  per-language timings. Its Japanese 24-case segment falls from 7.907 to 0.275
  seconds on this host, while English/Chinese stay near 0.4/0.3 seconds. These
  local observations quantify the discarded work; they are not CI or all-device
  latency guarantees, and do not expand Japanese long-draft conversion support.
- Explicitly gated synthetic model replies no longer add a redundant 80ms sleep
  after the caller has released their Pending state. Ungated replies retain
  80ms; five-second gate limits, real HTTP timeout tests, cancellation guards,
  stable-negative windows and every original workflow/assertion remain intact.
  Nine isolated Python fixture tests pass and are included in CI.
- No timeout extension, retry-until-green, vocabulary/model/configuration change
  or personal installation is part of this fix. The deferred IBus daemon abort
  remains unresolved. The original failed CI and blocked package release are
  not relabeled as passes.
- Both complete private native gates pass on this host under the unchanged
  120-second session deadline: default preedit takes 107.50 seconds and inline
  preedit takes 108.45 seconds including build/teardown. The runs include the
  long-model, privacy, recovery and restart cases beyond the failed CI's last
  marker. Logs are `native-default.log` and `native-inline.log` in the same
  `/tmp/suzaku-native-timeout.9hjcgy/` directory. These are local results, not a
  replacement for fresh CI or package acceptance on a subsequent release commit.
- Full all-feature Rust regression passes: 1,232 passed, zero failed and 27
  existing skips (`rust-all.log`). Strict all-target/all-feature Clippy initially
  flags the new test's tuple type; factoring it into a named alias resolves the
  warning without changing test behavior. Clippy then passes (`clippy-fixed.log`)
  and all six budget tests pass again (`budget-final.log`). Formatting, source
  digest and diff checks, 21 tensor checks, 23 private-diagnostic checks, nine
  model-fixture checks and six borrowed-signal-lifetime checks also pass. The
  signal checks emit existing system PyGObject/Python deprecation warnings.

### 0.8.3 release preparation (2026-10-06)

The six accumulated development sections immediately below are included in
0.8.3; their "Unreleased" headings and validation statements retain their
historical scope. See [0.8.3 release notes](docs/releases/0.8.3.md) for the
consolidated changes and limits. Rust/lockfile, both macOS bundles and Android
share version 0.8.3, with platform build number 33. Only the root package
version changes in Cargo.lock; third-party dependency versions are unchanged.
Publication requires the exact clean tagged commit's full CI and package /
clean-container installation acceptance, followed by permanent Linux assets.
The deferred local IBus daemon abort is not declared fixed by this release.
Release review also aligns the legacy adapter's English language aliases with
the shared protocol policy: `en`, `en-US` and `en_GB` retain the same raw
indentation when an alternative completion differs from the local conversion.
The existing focused unit regression now checks all three IDs. Local release
logs are retained at `/tmp/suzaku-release-0.8.3.v3zXu7/`.
After the version and alias changes, all 1,226 Rust tests pass again with zero
failures and 27 opt-in skips. Strict all-target/all-feature Clippy, formatting,
source digest, 21 tensor checks, six borrowed-signal-lifetime checks, 23 private
diagnostic checks, shell/Python syntax and release metadata checks pass. The
full-history and staged secret scans pass. Local ShellCheck is unavailable;
the unchanged required CI step and full native/package gates remain pending.

### Unreleased — Prediction preservation and English boundary follow-up (2026-10-06)

- Apply provider-independent draft semantics in the worker, after structural
  protocol validation and before either display limit. New and legacy custom
  providers cannot drop an English raw prefix/indentation, echo the unchanged
  draft or expose generated request metadata. Filter individual invalid rows;
  an entirely rejected batch leaves the local pool intact with `NoCandidates`.
- Keep raw English text independent of the best local completion. Only positive
  dictionary-prefix evidence triggers the incomplete-word guard; mere absence
  from the fallback dictionary is not a universal vocabulary ceiling. CJK
  phonetic/homophone conversion remains allowed. Exact preserved Unicode and
  typed JSON prefixes are excluded from generated-metadata checks, even when
  an alternative conversion differs from the primary local conversion.
- Restore separate English word projections when a model sentence inserts extra
  horizontal spacing after an already typed separator. Preserve ASCII, NBSP and
  Unicode horizontal spacing in the payload, including a model separator after
  a known complete word. Do not cross vertical separators or split technical
  tokens. A protocol-to-word-to-explicit-commit test checks exact whitespace.
- Opening brackets/quotes now only continue an existing English word boundary;
  a wrapper inside a protected URL/path/identifier cannot expose hidden authored
  phrase context. Normal/nested wrappers and whitespace after protected tokens
  still work across short drafts, long local windows and own-session commits.
- Preserve original red logs in `/tmp/suzaku-prediction-followup.ol75Et/`:
  two spacing failures and five protocol/protected-context failures. The first
  shared guard over-restricted unlisted words, caught by three existing fixture
  failures in `focused.log`; refining the code (not weakening assertions) passes
  all 52 focused integration tests in `focused-refined.log`. Extra policy units
  cover CJK typed metadata and English alternative completions.
- Full all-feature Rust validation passes 1,226 tests with zero failures and
  27 existing opt-in/fixture tests ignored (`rust-all.log`). Strict all-target,
  all-feature Clippy passes (`clippy.log`), as do the 21 tensor-validator tests,
  source-digest, formatting and diff checks. Maturity scores remain unchanged.
- No vocabulary data, model/service installation, model sampling/network format,
  personal input source, native event routing, release version, commit or push
  changes. No real-model quality or full native/GTK/physical-keyboard acceptance
  is claimed; the previously deferred daemon investigation remains deferred.

### Unreleased — Model-independent prediction protocol v1 (2026-10-04)

- Introduce `prediction::PredictionProvider` with serializable versioned requests
  and responses, caller-owned request IDs, complete-draft replacement semantics,
  language/session input, typed word/sentence candidates and bounded output limits.
  No model name, endpoint, credential or UI annotation is part of the contract.
  Existing context/input/output budgets remain unchanged; callers may reduce the
  candidate count and generated-character budget, not expand global limits.
- Connect the engine worker, Linux host and standalone panel to the new contract.
  Validate request/response envelopes independently of providers while keeping
  local revision rejection, one worker, cancellation and immediate local fallback.
  Preserve the legacy completion provider and struct-literal APIs through an
  explicit adapter. Candidate ranking accepts neutral types with a legacy wrapper.
- Move chat prediction instructions, language/prefix rules, candidate content
  parsing and schema into `prediction/prompt.rs`. HTTP adapters retain discovery,
  authorization, transport, service envelopes and finish-state checks. Keep the
  small-model prompt and network format unchanged; translation remains separate.
- Add 15 protocol/engine integration tests, four controlled HTTP contract tests
  and one legacy English-spacing regression. The compatibility adapter preserves
  raw English indentation even when the local conversion is a different word;
  malformed legacy rows cannot erase other valid candidates. Dual HTTP envelopes
  and arbitrary model names produce equivalent task data and typed responses.
- Full all-feature Rust validation passes 1,209 tests, with zero failures and
  27 existing opt-in/fixture tests ignored. The 21 tensor-validator tests,
  strict all-target/all-feature Clippy, source-digest, formatting and diff checks
  pass; maturity scores are unchanged.
  Logs are retained at `/tmp/suzaku-prediction-protocol.TbEH8f/`. All model I/O
  validation uses owned synthetic providers/loopback fixtures, not real model
  services. No complete native gate, GTK or physical keyboard test is claimed.
- This creates a backend-independent extension point, not improved model accuracy,
  streaming generation, a new server, broader text collection or real-device
  acceptance. No vocabulary, model installation, personal input source, version,
  commit or push is changed by this work. See [the protocol contract](docs/model-providers.md).

### Unreleased — Chinese partial-reading spacing (2026-10-04)

- Fix disappearing Chinese word/sentence candidates when an unfinished Pinyin
  draft gains trailing horizontal whitespace: `zhongw `, `shu ru f  ` and
  adopted Han text such as `发音 ke y ` now retain their existing completions.
  Match through a trailing-spacing-trimmed view, without changing decoder byte
  offsets, literal drafts, internal syllable boundaries, quotes or line breaks.
  Keep adopted Han spacing and long frozen prefixes exact. Required separators
  still distinguish the primary `xian → 先` and `xi an → 西安` readings.
- Seven independent integration tests first produced five failures and two
  passing negative-boundary checks; the original log remains at
  `/tmp/suzaku-pinyin-spacing.A2r656/red.log`. After the fix all 44 focused
  tests passed, including provider failures, all eleven recommended packs and
  the previous Japanese reading-boundary regressions. Add one direct unit test
  for mandatory separators and exact consumed-byte offsets.
- The private bilingual fixture adds three seeds/six word-or-sentence routes
  for actual Space events, numeric adoption, immediate BackSpace undo, continued
  input and a single explicit Return commit. `SUZAKU_NATIVE_BILINGUAL_ONLY=1`
  selects this existing bilingual gate without executing unrelated lifecycle
  diagnostics; the default full gate still runs all cases as before.
- Both focused native modes pass all 102 bilingual workflows: default and
  `SUZAKU_IBUS_INLINE_PREEDIT=1`. The private desktop-portal helper's startup
  `Not connected to the ibus bus` warning remains visible in both logs; the
  owned input daemon/host complete the assertions and both commands exit zero.
  This is private IBus validation, not GTK/physical-keyboard or personal-install
  acceptance, and not a rerun of the previously failed full native gate.
- Full Rust validation passes 1,189 tests (27 ignored). Strict all-target,
  all-features Clippy, formatting, diff checks and the 21 tensor-validator
  tests also pass. Logs remain under `/tmp/suzaku-pinyin-spacing.A2r656/`;
  maturity scores are unchanged.
- Lexicon contents, candidate budgets, model protocols and native key routing
  are unchanged. The intermittent upstream IBus abort investigation is deferred
  at the user's request, not fixed or reclassified as a passing complete gate.
  No version, commit, push, personal installation or desktop input source changes.

### Unreleased — Japanese reading boundaries (2026-10-04)

- Fix disappearing partial-word/sentence choices after trailing horizontal
  separators (`nihong `, `JYUNB\t`, `jouk` plus NBSP). Probe incomplete Romaji
  vowels before those separators, using the same seven vertical boundaries as
  composition conversion. Literal drafts remain exact; do not join a syllable
  across internal whitespace or a line boundary.
- Fix conversion splitting an already recognized mixed-script word: adopting
  `同じ` then typing `kanji` incorrectly produced `同時間じ`. The primary
  conversion and tail completion now share a longest-prefix decision that
  preserves known written words/Katakana when longer than a phonetic match.
  Longer readings and equal-length original dictionary/homophone priorities
  still win. `同じ漢字` / `同じ感じ`, `後で` and `別の` have fixed regressions;
  no phrase-specific decoder branch or new model dependency is introduced.
- Five new independent tests first produced four failures and one passing
  negative-boundary test. After the fixes all 23 Japanese focused tests pass,
  preserving seven horizontal variants, seven vertical boundaries, unknown
  text, adoption, exact commits and undo. The provider-error and all-eleven-pack
  fixtures add fixed partial-space and mixed-script examples. Lexicon files,
  candidate budgets, native key routing and model deadlines are unchanged.
- Private IBus coverage adds six partial-Romaji Space word/sentence workflows,
  adopted Katakana plus partial-tail Space, and `同じ` adoption/continued
  Romaji/homophone adoption/undo/one explicit commit. Validation logs, including
  the original failures, are under `/tmp/suzaku-japanese-boundaries.mvgN0w/`.
  The focused Japanese fixture passes in both default and inline-preedit modes.
- Full Rust validation passes 1,181 tests (27 ignored), including typed provider
  failures and all eleven recommended packs. The 21 tensor-validator tests,
  strict all-target/all-features Clippy, formatting and diff checks pass;
  maturity scores are unchanged.
- The complete default IBus gate **failed**, not passed: after presentation
  checks, tray activation encountered a daemon SIGABRT at 8.738 seconds
  (`ibus-daemon` PID 3441209, return code -6, bus disconnected). The log shows
  the invalid-sender assertion followed by `call_in_idle_cb`'s missing-vtable
  assertion, matching the previously recorded abort signature below. This
  happened before the complete gate reached the Japanese fixture; no recent
  kernel OOM was logged. The specific sender-lifetime cause remains unproven,
  and the two focused Japanese passes do not turn this into a full native pass.
  Preserve `native-default.log`; do not retry until green, raise deadlines or
  alter installed dependencies. No version, commit, push, personal installation,
  input source or GTK fixture is changed.
  Installed IBus `1.5.34~rc2-1` / GLib `2.88.0-1ubuntu0.1` match the earlier
  investigation. Preserve the newly timestamped Apport report privately as
  `ibus-daemon.crash` alongside the logs (mode 0600); it is not uploaded or
  added to Git, and has not yet been unpacked or analyzed as a new core.
  Report SHA-256: `aa9a38cde12f54c5d0334d9f687e4945d94638fe337b28d671a5f83d19652f67`.

### Unreleased — Japanese choices fallback (2026-10-04)

- Extend daily Japanese comparisons, preferences, conditions and alternatives
  through independent authored data: 48 readings, 24 contexts and 48 sentences,
  for totals of 122 / 62 / 123. Preserve all 74 released readings and
  38 continuation contexts with new full historical fingerprints, in addition
  to the original 26-reading / 14-context snapshots. The preceding uncommitted
  English/Chinese choices work stays intact; no model-specific dispatch is added.
- Six new independent tests distinguish a verbatim Kana literal from a converted Word,
  specific Romaji prefixes from broad prefixes, and authored continuation from
  arbitrary sentence-level reading conversion. Exercise uppercase/Hiragana and
  horizontal separators, word/sentence choices, adopted words plus particles,
  exact commit/undo and negative unknown/line-boundary cases.
- Tests-first reproduced the missing choices: five new tests failed and the
  negative-boundary test already passed; the provider-fallback red run also
  failed on missing `KONOMI → 好み`. Both logs are preserved before appending
  data. Expectations are not generated by reading the production JSON.
- Provider-failure and all-eleven-pack tests add Japanese choices independently
  of the resource contents: four mixed-spelling word/sentence examples plus two
  adopted-particle progress examples in the pack gate. The focused run passed
  41 tests across new/old Japanese vocabulary, sentence progress, resources,
  provider failures and all packs. Missing/error/timeout/503/empty responses
  are synthetic typed errors, not live-model evidence.
  Private IBus exercises numeric word/sentence adoption,
  exact undo, literal Space, adopted-word particles and conflicting tails. No
  personal installation, input source, GTK fixture, model or timeout is changed.
- The first private IBus run exposed a genuine entry gap: `ひかく` works in
  the engine, but the native empty-draft path rejected its initial `ひ`.
  Accept only alphabetic Hiragana in Japanese mode; leave other languages,
  non-reading idle keys, application chords, hard-bypass purposes and unfocused
  events unchanged. Dedicated synchronous native checks assert these boundaries
  and no implicit commits. This is not a JIS Kana-layout implementation.
- After that fix, the native run found an old workflow displaced by new word
  competition: `準備gadeki` still offered its authored continuation, but only
  on page two. An independent engine red test reproduced the lost first-page
  choice. Rank non-primary Japanese base Sentence conversions below authored
  continuations, using the existing Chinese weighting rule; preserve the
  primary conversion, Word/Unspecified weights, literal, quotas and list limit.
  This also applies to non-primary sentence conversions from Japanese packs,
  not only mixed Katakana variants. No candidate text, kind or source changes.
  The first engine rerun then caught an incorrect new-test label: a pure
  phonetic Katakana variant is Unspecified, not Sentence. Correct the fixed
  expectations per variant, without changing classification; retain that log
  as `focused-final.log`. All seven progress tests then pass in
  `progress-fixed.log`, including the original first-page failure.
  Add the same fixed word/sentence expectation to provider-failure coverage
  and both adopted/Romaji variants to the all-eleven-pack progress gate.
- Both focused Japanese native modes and both complete private IBus modes
  (default and `SUZAKU_IBUS_INLINE_PREEDIT=1`) pass after these fixes. The
  Japanese fixture covers first-key/bypass boundaries, 16 daily word/sentence
  workflows, five positive/five conflicting-tail continuations, homophones,
  `nn`, adopted Katakana and later-page literal submission. These are private
  IBus events, not a physical-keyboard or personal-desktop acceptance.
- Validation logs and red/green results are kept under
  `/tmp/suzaku-japanese-choices.bo3VDg/`. The final all-features Rust run passes
  1,176 tests (27 ignored), including the provider/pack ranking additions;
  strict all-target/all-features Clippy, formatting and diff checks pass.
  The 21 tensor-validator tests pass without raising maturity scores.
  This remains bounded fallback, not full Japanese
  morphology, arbitrary Katakana-to-Kanji conversion or a longer Japanese draft
  window. No version, commit or push is changed.

### Unreleased — Daily choices bilingual fallback (2026-10-04)

- Append-only `daily_choices` covers comparisons, preferences, conditions and
  alternatives without model-specific or language-algorithm changes. English
  adds 24 contexts / 48 authored sentences and only three indexed forms
  (`depends/hurry/simpler`); Chinese adds 65 readings / 24 contexts / 48 sentences.
  Totals: EN 6,127 indexed forms / 521 contexts / 1,006 sentences; ZH 2,432
  readings / 515 contexts / 1,028 continuations. Japanese and all recommended
  pack contents are unchanged; the 6,144 English index budget stays fixed.
- Eleven new independent engine tests cover both branches, spelling variants,
  partial words/readings, first-page word/sentence choices, preserved literal
  text/long prefixes, editable adopted drafts, continuation and exact commit/undo.
  New snapshots freeze all v0.8.2 English ranks, contexts and sentences and all
  Chinese readings/contexts, in addition to every existing historical snapshot.
- Both tests-first runs failed all five new tests before their language's data
  was appended; logs remain in `/tmp/suzaku-vocab-choices.XZixA7/`. Provider-error
  and all-eleven-pack gates add five fixed bilingual variants. These synthetic
  missing/error/timeout/503/empty results are not live-model quality evidence.
  The first full run then found one stale Chinese unit-test total (2,367 versus
  the new 2,431); only that current-total assertion was updated. Every historical
  fingerprint and per-entry reachability/uniqueness assertion remains intact,
  and the failing full-run log is retained as `rust.log`.
  The next full run (`rust-final.log`, retained despite its original filename)
  exposed a real data gap: `这个词/这个意思` existed, but standalone `这个` did not.
  After adopting `我更喜欢`, `zhe ge` expanded to longer words and could not
  continue the authored sentence. Append the basic `zhe'ge → 这个` reading;
  keep the original sentence expectations and separately test it alone and
  after different adopted prefixes. No phrase-specific decoder logic is added.
  The focused rerun passed all eleven new engine tests, then caught a new
  fixture mislabeling the complete multiword `我更喜欢这个` as Word rather than
  Sentence. The bilingual word/sentence helper now explicitly checks the old
  longer Word `我更喜欢这个词` alongside the intended authored sentence; the
  dedicated demonstrative test retains exact full-conversion Sentence checks.
- The private IBus vocabulary fixture adds four seeds / eight word-or-sentence
  adoption, undo, continuation and single-commit routes (96 bilingual routes
  total). Its reset now requires a newer context plus an empty draft before
  subsequent language controls or typing, closing an asynchronous fixture race.
  Native typing tests internal double spaces; engine tests additionally accept
  leading spaces supplied as a draft, since Space on an empty native draft
  correctly passes through. No GTK scenarios or timeouts are changed.
- Final serial validation passed 1,168 non-ignored all-features Rust tests,
  strict all-target Clippy, formatting, diff checks and 21 tensor tests. The
  27 opt-in Rust tests remain separate. The successful complete Rust run is
  `rust-verified.log`; earlier failures are not reclassified as passes.
  All eleven recommended packs passed together, including the new bilingual
  variants and the adopted `这个` sentence route. Default and inline-preedit
  full private IBus gates passed (`native-default.log` / `native-inline.log`),
  each including all 96 bilingual vocabulary workflows. These two runs do not
  establish a fix for the previously investigated upstream IBus crash.
- Tensor source evidence and the current dirty-snapshot base (v0.8.2 commit
  `a019b7c`) are refreshed without changing maturity scores or historical runs.
  No GTK gate, physical-keyboard/personal-desktop test or live-model quality
  evaluation was run this round. No personal installation, input source,
  version, commit or push is changed.

### 0.8.2 release preparation (2026-10-04)

The accumulated development sections below are included in 0.8.2; their
"Unreleased" headings and validation statements retain their historical scope.
See [0.8.2 release notes](docs/releases/0.8.2.md) for the consolidated changes,
platform limits and installation instructions. Rust/lockfile, both macOS bundles
and Android now share version 0.8.2, with platform build number 32. Only the
root package version changes in Cargo.lock; dependency versions are unchanged.
CI also explicitly validates the functional tensor and its 21 fixture tests.
Publication still requires the exact commit's complete CI and clean-container
package acceptance, followed by permanent Release attachments. Local validation
logs for this preparation are under `/tmp/suzaku-release-0.8.2.H7rcvj/`.
After the metadata bump, 1,155 ordinary all-features Rust tests and strict
all-target Clippy passed again (27 opt-in tests stay separate), together with
formatting, shell syntax, 21 tensor tests and staged/full-history secret scans.
Local ShellCheck is not installed; the mandatory CI gate remains enabled.

The first [0.8.2 CI run](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/37182510232)
passed Rust, both complete IBus modes, UI, native layout/candidate gates, the
complete GTK vocabulary gate and physical keyboard checks. Stock-popup navigation
then failed: its old fixture expected row 9 on a second Down click with ten
candidates, although the new page-start contract correctly retained row 6.
The failed log remains at `ci-failed.log` in the preparation directory above.
The fixture now explicitly requires a partial second page and expects Down
`6,6,6` / Up `0,0,0`, while preserving all real XTest, accessibility, focus,
literal draft and exact-save assertions. It additionally checks every observed
navigation snapshot for page-start selection and unchanged draft/candidate state.
All 47 stock-popup workflows, including 18 ordinal-glyph checks, passed locally
after this fixture-only correction (`popup-fixed.log`, owned files at
`/tmp/suzaku-app-qa.eBXOyy`). Product code and timeouts were not changed; no failed
CI is reclassified as success, and final-commit CI/packaging must run again.

### Unreleased — Japanese sentence progress (2026-10-04)

- Fix authored sentences disappearing once Japanese input advances beyond an
  exact trigger: `予定ga`, `nihongo wo` and `JYUNBI GADEKI` now retain matching
  sentences. Match the whole converted draft, prefer longer applicable contexts
  stably, deduplicate and bound added sentence choices. Existing exact-trigger
  order remains covered independently; unknown prefixes/conflicting tails are
  never removed to recover an unrelated sentence.
- Continue consuming horizontal Romaji reading separators only in converted
  choices, while the literal draft remains exactly selectable. Preserve LF,
  CR, CRLF, vertical tab, form feed, NEL and Unicode line/paragraph separators;
  conversion and sentence matching must not silently join separate lines.
- The first full regression run exposed another real boundary in a new rail-pack
  case: `shuudennha` became `終電んは`, although `shuudenn` already ended in one
  `ん`. Keep a completed `nn` as one `ん` before the next consonant; preserve
  `nna`, `nni`, `nnya`, `nnna`, repeated n and both explicit apostrophe rules.
  Fixed unit expectations cover the intermediate `shuudennh` state too, with
  a private IBus `denn` → `dennwa` → `電話` adoption/undo/commit workflow.
- No new vocabulary or model-specific path: the implementation consumes the
  active lexicon. Six new independent engine tests cover 21 particle/progress
  spellings, every literal prefix of three fixed sentences, adoption, negative
  boundaries and exact commits/undo. Ten rail-pack progress spellings exercise
  the same behavior with all eleven recommended collections enabled.
- Red runs reproduced both issues: two unit failures and five of six new engine
  test failures. The subsequent full-run rail failure and focused `nn` red test
  are also retained; the original test was not removed or relaxed. Logs remain
  in `/tmp/suzaku-ja-progress.LS2hah/`. The first
  purported inline invocation used an unrecognized environment variable and
  was another default-mode run (`native-extra-default.log`); the corrected
  `SUZAKU_IBUS_INLINE_PREEDIT=1` run is recorded separately as `native-inline.log`.
- Final validation passed 1,155 non-ignored all-features Rust tests, strict
  all-target Clippy, formatting, diff checks and 21 tensor checks. The 27 opt-in
  Rust tests remain separate. This includes the ten rail progress cases in the
  all-eleven-pack fixture; the first failing full run is not reported as a pass.
- Focused private IBus passed again after the `nn` fix in default and inline modes, retaining the old
  Japanese cases and adding three sentence-progress and three conflicting-tail
  workflows plus the `nn` consonant transition, with numeric adoption, exact
  spelling undo and single Enter commits.
  Four focused GTK/XTest workflows passed, including new adopted-word + Space
  + `ga` sentence selection/undo and observed exact editor saves for `予定` and
  `準備`. Final owned editor artifacts: `/tmp/suzaku-app-qa.G0s3jy` (the earlier
  pre-`nn` run remains at `/tmp/suzaku-app-qa.klFSAZ`).
- This round does not rerun full IBus, full 89-workflow GTK, personal installation,
  physical keyboard, native Wayland or live-model quality acceptance. It does
  not infer readings of unknown Kanji in sentences (`予定がkima`), perform full
  morphological analysis, scan arbitrary history or extend the Japanese
  256-scalar local window. Version, dependencies, personal input source and
  service installation remain unchanged; no commit or push is performed.

### Unreleased — Gradual Japanese everyday input (2026-10-04)

- Japanese now begins a bounded expansion alongside the English/Chinese core.
  Append 48 authored readings and 24 pairs of continuations: 74 readings,
  38 contexts and 75 continuation records in total. The original 26-reading
  fingerprint remains, and a new fingerprint freezes all 14 original contexts.
  All English/Chinese resources and recommended packs remain unchanged this round.
- Fix terminal `nn` before punctuation/non-Romaji text duplicating `ん`, recognize
  curly `n’` alongside `n'`, and accept `jya/jyu/jyo` aliases. ASCII-only case
  normalization protects unrelated Unicode such as `Ω`, `Σ` and `İ`; uppercase
  Romaji still works. Preserve the old `nn'` interpretation rather than leaking
  an apostrophe through the new terminal-n folding branch.
- Exact final readings after known prefixes now retain homophones:
  `watashihakanji` offers `私は漢字` and `私は感じ`. Known adopted spellings,
  including `ニホンゴ`, can precede partial Romaji (`wobenky` → `を勉強`)
  without replacing that spelling. Prefix matching is allocation-free and bounded;
  arbitrary unknown prefixes, digits and emoji are not skipped to find a suffix.
- Independent daily tests cover complete/partial Romaji and Hiragana, uppercase,
  first-page words/sentences, original text, exact single commits and undo.
  Typed missing/error/timeout/503/empty providers cover the new vocabulary without
  pretending to be live-model evidence. Full-pack tests check new Japanese words
  alongside the existing rail collection. New private IBus and GTK vocabulary
  scenarios cover actual key delivery and editable adoption; their execution
  results are recorded below, separately from test-source coverage.
- Initial red tests reproduced lost suffix homophones, failed adopted Katakana
  completion and repeated `ん`. A data test additionally assumed literal input
  must be on page one; `as` legitimately retains that choice on the next page.
  The test now searches the full list only for Literal, while words and sentences
  still require page one for complete/specific spellings. The next run exposed
  the same overbroad assumption for a single Kana (`あ`); those broad prefixes
  instead require reachability within the bounded list, retaining old word ranks.
  A native keyboard case explicitly selects the later literal row. Logs and
  retained failures: `/tmp/suzaku-japanese.3vStYH/`.
- Final validation passed 1,147 non-ignored all-features Rust tests, strict
  all-target Clippy, formatting and 21 tensor checks; 27 opt-in Rust tests remain
  separate. Default-mode full private IBus passed, including 88 EN/ZH workflows
  and the new Japanese suite (eight daily word/sentence routes, two homophones,
  adopted Katakana and later-page literal). The focused Japanese suite also
  passed with inline preedit; this is not an inline full-suite claim.
  Four focused Japanese GTK/XTest workflows passed with actual key delivery,
  observed editor buffers and exact saved commits, including uppercase `JYUNBI`.
  All new cases are also included in the default full vocabulary gate, now
  86 vocabulary cases plus three Chinese sentence-progress controls; that
  complete 89-workflow GTK gate was not rerun this round. No personal-desktop,
  native Wayland, physical-keyboard or live-model quality acceptance is claimed.
- This is not a complete morphological analyzer or arbitrary Kana/Kanji converter.
  The 256-scalar local/model budget, Japanese long-draft limitation, model interfaces,
  candidate capacities and Space-as-reading-separator contract remain unchanged.
  No personal installation, input source, dependency, version, commit or push is changed.

### Unreleased — Everyday objects bilingual fallback (2026-10-04)

- Append-only `daily_objects` data covers finding/placing belongings, borrowing
  and returning items, small requests, quantities and sizes. English adds 24
  contexts and 48 authored sentences, reusing existing vocabulary except seven
  explicit forms: `chairs/cups/mine/plates/scissors/tape/yours`. Chinese adds 64
  readings, 24 contexts and 48 continuations. Current totals: 6,124 English
  indexed forms / 497 contexts / 958 sentences; 2,367 Chinese readings / 491
  contexts / 980 continuations. Japanese and recommended pack contents are unchanged.
- Vocabulary remains independent JSON and a model-unavailable fallback, not a
  replacement for open-ended generation. No decoder/model branches, candidate
  capacities or index budgets change. New snapshots freeze the previous 15
  English layers' ranks, all 473 contexts/910 sentences and the previous 2,303
  Chinese readings/467 contexts; every older snapshot remains in place.
- Independent fixed expectations cover all new expressions, both sentence
  branches, partial/separated Pinyin, case/apostrophes/spacing, long mixed
  prefixes, literal choices, exact commits and undo. Six new bilingual scenarios
  join typed provider-error fallback, native IBus and GTK/XTest vocabulary gates.
  Full-pack regression also checks new built-ins and the old cooking pack together.
- The first package regression caught a real new-data collision:
  `please pass me the` already belongs to the cooking pack. Only the new built-in
  context was changed to `could you pass me the`; the shipped pack and its
  `colander/spatula` choices remain intact. Logs, including the original failed
  run, are retained in `/tmp/suzaku-vocab-objects.7nYSBX/`.
- The first GTK full-vocabulary run stopped before delivering `There's a spare ch`:
  its typing fixture explicitly rejected uppercase ASCII. The helper now sends
  uppercase letters through the existing physical Shift/key press/release path;
  lowercase, digits and supported punctuation retain their old mapping, and
  unsupported characters are still rejected. The uppercase scenario and exact
  text assertions remain unchanged. A display-free mapping check verifies this
  fixture correction; the original failure and owned QA artifacts are retained.
- Final source validation: 1,138 non-ignored all-features Rust tests, strict
  all-target Clippy, formatting and 21 functional-tensor tests passed. The 27
  opt-in tests remain distinct from that Rust count. Default and inline-preedit
  full private IBus gates passed, each including 88 bilingual vocabulary
  word/sentence adoption/undo/continuation/commit workflows. The default gate
  still emitted the previously investigated IBus invalid-sender GIO warning but
  completed successfully; this data change does not claim to fix that dependency
  issue. Source fingerprints and evidence are updated without raising scores.
- After the fixture correction, the full isolated GTK editor vocabulary gate
  passed both mixed-language partitions: 41 vocabulary cases plus three Chinese
  sentence-progress controls in part 1, and 41 vocabulary cases in part 2 (85
  workflows total). This includes the uppercase English example and all six
  added bilingual cases, with real XTest delivery, observed editor buffers and
  exact saved word/sentence commits. The original per-session deadlines and all
  assertions remain unchanged. Fourteen editor-observer unit checks also passed.
  Xvfb/XTest is not a physical-keyboard, native Wayland or personal-desktop test;
  no live-model quality, package installation or GPU visual acceptance is claimed.
- This source expansion does not change the personal installation, version or
  desktop input source. Rebuilding/reinstalling is needed to use the compiled-in
  additions on the desktop; no release, commit or push is part of this round.

### Unreleased — Prose boundaries and consistent candidate paging (2026-10-04)

- English mode now completes words and authored sentences after explicit prose
  boundaries `—，。！？；：、`, without requiring an extra ASCII space. Examples:
  `前文，hel` → `前文，hello` / `前文，hello, how are you?`, and
  `说明—please sen` → `说明—please send` / `说明—please send me the details.`.
  Exact prefixes, case and spacing remain intact. Word, sentence, committed-context
  and long-draft paths share a linear boundary scan; ASCII URL/path/identifier
  syntax prevents a punctuation cut from exposing a fabricated natural-language
  suffix. The 160-scalar context and 256-scalar local/model budgets are unchanged.
  Final review caught repeated scanning of already-finished session history;
  the scan now starts at the current window's containing whitespace-delimited
  token. Structural tests exclude old history from the scanned slice, and 350
  boundary combinations compare it with the initial full-scan semantics. An
  unbroken token still retains its complete syntax for conservative URL protection.
- English adoption followed by an em dash retains pending preference feedback
  until explicit commit; undo still reverses it. ASCII hyphens, en dashes, URLs
  and identifiers do not become word boundaries for preference learning. This
  changes neither vocabulary resources nor model/provider coupling. Generated
  model suffixes with CJK punctuation remain whole sentence candidates, not
  automatically projected word candidates.
- IBus PageUp/Down, home-row Alt+H/L and IBus page events now select the adjacent
  page's first row, matching native panel arrows. Unavailable pages are inert:
  selection, revision, adoption undo and in-flight predictions remain unchanged.
  Draft page keys remain consumed; empty/private/unfocused targets retain the
  existing pass-through behavior. Row navigation and explicit adoption are unchanged.
- Red tests established missing mixed-prose completions, lost `hello—world`
  preference feedback, and first-page PageUp changing native revision. The new
  native fixture initially assumed a settings file already existed; it now saves
  the loaded default settings when starting fresh. A new test also incorrectly
  demanded typed sentence candidates from the short legacy nonmixed API; it now
  respects that existing untyped-word contract, while IBus and both long-draft
  modes still require first-page words and sentences. Original failures and
  subsequent validation logs are retained in `/tmp/suzaku-input-ux.6wkCJf/`.
- Final source validation: 1,126 non-ignored all-features Rust tests passed,
  with 27 native/opt-in tests kept separate; strict all-target Clippy, formatting,
  23 native-fixture diagnostic tests and 21 functional-tensor tests passed.
  Default and inline-preedit complete private IBus gates both passed on the final
  source, including the new EN/ZH three-entry page checks and four mixed-prose
  word/sentence adoption/undo/Space/exact-commit workflows. This is synthetic
  native protocol coverage, not physical keyboard, GPU redraw or real-model
  quality acceptance; those independent UI/installation gates were not rerun.
  The tensor source fingerprint is updated without increasing maturity scores.
- The earlier dependency crash investigation is parked, not declared fixed.
  No personal input source, installation, system dependency or version is changed.

### Investigation — IBus daemon abort recovered from the original crash (2026-10-04)

- Recovered the original Apport report at `/var/crash/_usr_bin_ibus-daemon.1000.crash`.
  Its 2026-10-03 21:57:46 +08:00 timestamp, daemon PID 2497370, parent fixture
  PID 2497361 and private `suzaku-sync-qa.NUQA9k/ibus.sock` address match the failed
  inline gate. Report SHA-256:
  `4f6da2c2944ba735473c638d2a9344f845cbaaa814e3aa5972ca58a3cd7bbd4b`.
  This establishes a daemon SIGABRT, not merely a client disconnect, OOM kill or
  outer test timeout. It does not imply that the user's desktop daemon crashed.
- The exact GIO Build-ID `a9791477ead64658f8936bdcc9e63a5d3f02a4f0` debug symbols
  confirm `call_in_idle_cb(user_data=0x0)`, `invocation=0x0`, at
  `gio/gdbusconnection.c:5474`. The assertion is
  `vtable != NULL && vtable->method_call != NULL`. Core-resident diagnostic text
  also retains the earlier `_g_dbus_method_invocation_new` sender-name validation
  failure. Thus an invalid non-null sender caused construction to return NULL;
  scheduling that NULL invocation led to the later assertion and daemon abort.
  The worker had already returned to its event loop: the actual sender bytes,
  method and corruption origin cannot be reliably recovered from this core.
- Scope: Ubuntu 26.04, IBus `1.5.34~rc2-1`, GLib `2.88.0-1ubuntu0.1`.
  This is not the Ubuntu 24.04 CI baseline. Official GLib
  [dispatch source](https://github.com/GNOME/glib/blob/2.88.0/gio/gdbusconnection.c)
  and [invocation construction](https://github.com/GNOME/glib/blob/2.88.0/gio/gdbusmethodinvocation.c)
  explain the observed failure chain; matching symbols were downloaded only into
  a private temporary directory, not installed as system packages.
- Still a hypothesis: the IBus worker's
  [sender assignment](https://github.com/ibus/ibus/blob/1.5.34-rc2/bus/dbusimpl.c#L1554)
  borrows the connection name while
  [connection destruction](https://github.com/ibus/ibus/blob/1.5.34-rc2/bus/connection.c#L80)
  can release it after removing the filter. GLib documents that filter removal
  does not wait for an already-running callback. However, the sender setter also
  validates before copying; proving this particular race requires observing the
  narrow lifetime window, not merely finding the static code pattern. No evidence
  establishes that the separately published engine-request cancellation fix is
  the same bug, or that a system upgrade alone resolves this failure.
- Diagnostic results: a no-Suzaku private IBus control survived 600 connection
  open/close cycles and 38,400 valid no-reply ListNames requests. The unchanged
  complete inline gate passed again, including N45/N44. These are negative
  reproduction results, not proof of a fix. Two temporary attempts to repeat N45
  on one bus stopped at iteration 2 with missing panel content-type events while
  the daemon remained alive; adding an inter-case drain did not resolve that
  harness-reuse limitation. Neither reproduced the historical daemon abort.
- Logs, temporary diagnostic drivers, matching symbols and the privately unpacked
  crash remain under `/tmp/suzaku-disconnect-investigation.TDhKk6/`. Raw core and
  environment data are not added to the repository or uploaded. The initial
  diagnostic-run approval timed out; the permitted retry executed normally.
  Next useful experiment is instrumented/ASan validation of sender ownership on
  a disposable upstream daemon, not changing product sleeps or suppressing errors.
  No production code, dependencies, personal input source, installation or version
  was changed. Ordinary Rust/Clippy and default-mode full gates were not rerun.

### Unreleased — Native recovery fixture diagnostics and cleanup

- Investigated the previous inline N45 connection loss. The original run discarded
  `ibus-daemon` stderr, and a failed panel-name release replaced the first engine
  query assertion. The surviving evidence cannot distinguish daemon termination
  from a single client disconnect; no production root cause is established.
- Keep daemon stderr and unbuffered Python output in the gate log. Before teardown
  changes process status, record the phase, local connection flags, daemon PID/exit
  status and the last eight owned children. No diagnostic D-Bus RPC is sent and
  full child arguments are not logged. Broken observation/stderr cannot hide the
  original error; this is a test-fixture diagnostic, not new product telemetry.
- N45 now owns and retires its delayed GLib source IDs, disconnects the panel
  handler and cancels remaining callbacks before the next recovery workflow.
  This replaces the old fixed 120 ms cleanup pump, not the 350 ms restoration
  stability check. Every cleanup stage is attempted, preserving the first failure
  and attaching later errors. PASS is printed only after cleanup succeeds.
- PyGObject can swallow asynchronous callback exceptions. Save the first callback
  exception and explicitly check it in the probe wait, stability loop and cleanup;
  bound additional error notes. Recheck every switch ACK after the stability loop
  so a late false result cannot pass merely because the engine was already right.
- Validation: 23 isolation/cleanup/diagnostic Python tests and six real GLib/C
  signal-lifetime tests passed, including callback failure and cancellation.
  Both complete default/inline private IBus suites passed, including N45/N44;
  neither reproduced the old disconnect. Shell syntax, ShellCheck, Python syntax,
  diff checks, 21 inventory tests and the refreshed source digest passed. Logs are retained in
  `/tmp/suzaku-recovery-audit.DwfJcp/`; the previous connection-loss log remains in
  `/tmp/suzaku-long-chain.WpURFn/ibus-inline.log`.
  The early scoped inline recovery run also passed. The initial bare `shellcheck`
  invocation was unavailable on PATH; validation used the already-installed
  `target/linux-lint.hijeqZ/root/usr/bin/shellcheck`, with no package download.
  No production Rust/C, personal installation/input source, model, vocabulary or
  version changed. This round does not rerun the ordinary Rust/Clippy suite or
  claim physical-keyboard, GNOME/Wayland, remote CI or release acceptance.

### Unreleased — Exact local-window boundary

- Follow-up on F64/F17/F18: the 256-scalar local-tail search did not inspect the
  separator immediately before a full-sized tail. Chinese could lose all local
  conversion for an otherwise decodable 256-character reading; English could
  discard the first complete word and lose its authored sentence continuation.
- Inspect one extra boundary scalar, then leave that scalar in the frozen prefix.
  The actual decoder window remains at most 256 Unicode scalars. Do not change
  the last-Han/CJK-prefix freezing policy, split protected tokens, normalize the
  preserved prefix or enlarge the whole-draft model/transport limits.
- Recorded red tests independently reproduced Chinese missing conversion and
  English missing sentence context at exactly 256; their short and over-budget
  controls passed. Added tests cover 254/255/256/257 boundaries, both candidate
  policies, Unicode/whitespace prefixes, exact commits and undo, protected tokens,
  and a valid 8,192-byte draft whose expanded conversion must be filtered intact.
  Controlled-provider tests retain the long-draft no-request check and a positive
  exact-256 short-draft request after shortening; no real model is called.
- The private IBus fixture adds 14 compact boundary workflows: owned full-text
  replacement or a final real host key event, word/sentence adoption, exact
  Backspace restoration and one full-payload Enter commit. A 257-scalar reading
  remains literal; English may complete its final word but not reuse frozen
  phrase context, including on later candidate pages. This is not physical-keyboard
  or personal-desktop acceptance, and does not require a personal installation.
- The 38 targeted long-draft/English-window/prediction tests passed after allowing
  the prediction suite's private local endpoint. The first sandboxed attempt hit
  `PermissionDenied` in an existing endpoint fixture; no assertions were relaxed.
  The subsequent complete all-feature Rust run passed 1,116 ordinary tests;
  27 opt-in gates remain separately enabled, not implicitly passed.
  Default private IBus passed the complete suite, including all 14 new boundary
  workflows. The first inline run passed those workflows and later input/host
  restart checks, then its existing IBus connection closed during the final N45
  password probe. The engine query failed, followed by the panel-name release;
  this was not the outer 120-second timeout. The unchanged isolated N45/N44
  recovery gate and an unchanged complete inline rerun both passed. Both final
  preedit modes include all 14 new boundary workflows. No cause of that connection
  loss is established; the first failure remains in `ibus-inline.log`, not erased
  by the passing `ibus-inline-recovery.log` and `ibus-inline-rerun.log`.
  Strict all-feature/all-target Clippy, formatting, diff checks, the 21 inventory
  tests and the refreshed source-digest check passed. No hardware/GUI acceptance,
  reinstall, packaging, version change, commit or push was performed this round.
  Logs, including both independent red tests, are in `/tmp/suzaku-long-chain.WpURFn/`.
  No vocabulary data, model provider, version, personal input source or install changed.

### Unreleased — Pointer deltas and English preference boundaries

- Follow-up from the sparse inventory, along F60/F36/F40 and F66/F16: signed-zero
  pixel-wheel and pinch events previously became ±1 through `signum`, causing
  horizontal/no-motion gestures to page candidates or resize the panel. Nonfinite
  deltas could also trigger actions or contaminate the settings scroll position.
  Check the original vertical value before taking its sign/narrowing precision;
  keep finite line-wheel magnitudes and existing nonzero gesture-phase behavior.
- The production event dispatcher now has 70 inert-event and 28 valid-event cases
  in the existing private native-action gate, covering candidate pages, Ctrl-wheel
  zoom, pinch and settings. These dispatch synthetic winit events inside owned
  Xvfb windows; they are not physical touchpad/touchscreen acceptance.
- English `hel -> hello` adoption no longer teaches that word when subsequent
  typing makes `hello.com`, `hello:world` or another attached period/colon token.
  Sentence endings, whitespace, closing punctuation and CJK separators retain
  their existing learning behavior. Canceled pending choices do not resurrect
  after deleting back to the old spelling; confirmed feedback still has undo.
  This is a choice-lifecycle guard, not a dictionary/parser dependency or new
  persistent cache. The private IBus fixture also checks ten token/punctuation
  continuations with exact commits and no learning while typing.
- Before the fixes, two ordinary gesture tests and the private native-action
  dispatcher gate failed; the new English preference regression recorded one
  erroneous learned entry for `hello.com` while its punctuation controls passed.
  Reproduction and validation logs are under `/tmp/suzaku-chain-audit.Vwd7sH/`.
  No personal installation, input source, vocabulary, model or version is changed.
- Validation: 1,110 ordinary all-feature Rust tests passed; 27 opt-in gates remain
  separately enabled, not implicitly passed. The updated private native-action
  gate passed all 70 inert and 28 finite-event checks, including real separate
  settings-window dispatch. Both complete private IBus preedit modes passed the
  ten English continuation cases and existing input/lifecycle/model-fixture checks.
  Real GTK/IBus/XTest candidate workflows passed 26 cases per default/synchronous
  client transport (52 total), including paging, word adoption/undo and exact long
  English/Chinese candidate clicks. Strict all-feature/all-target Clippy, formatting,
  diff checks, all 21 inventory tests and the refreshed source digest passed.
  Application fixture artifacts remain in `/tmp/suzaku-app-qa.KFvSY9/` and
  `/tmp/suzaku-app-qa.efxBx0/`; these owned X11 fixtures are not personal desktop tests.
- Retain intermediate diagnostics: the first ordinary run could not bind its
  fixture endpoints under the sandbox; the permitted local-fixture rerun passed.
  The settings positive control initially failed because the probe rendered the
  main window, which deliberately excludes settings. The fixture now opens a real
  settings window with a scrollable viewport and retains its nonzero-range and
  bidirectional assertions. A missing test-only enum import briefly blocked the
  first inline-mode attempt before it ran; the corrected compile and gate passed.
  Private Xvfb software-renderer DRI3 and IBus portal startup warnings are retained
  in logs; these are not proof of physical GPU/desktop acceptance.

### Unreleased — Architecture/function/implementation maturity inventory

- Preserve F01–F59 and index nine existing cross-cutting capabilities as F60–F68:
  native paging, caret placement, bottom layout, automatic platform layout, long
  drafts, payload budgets, preference frequency, resource decoupling and shortcuts.
- `docs/functional-tensor.json` records 10 architecture domains, 68 functions,
  47 scoped implementations and 77 sparse relationships (308 maturity coordinates).
  Implementation, regression, real-environment acceptance and delivery readiness
  remain separate ordinal scores; unknown is null, never an implied zero/pass.
- The read-only `scripts/functional-tensor.py` validates references, evidence,
  paths, score constraints and a digest of the declared implementation/test files;
  it can slice by architecture/function/implementation or export COO records.
  The network document explains the rubric and retains historical evidence.
- This is a documentation/data inventory, not a product behavior change. Historical
  source-build/Xvfb results, personal installations and releases are not promoted
  to current dirty-snapshot acceptance. No new Cargo/GUI run, installation,
  remote CI inspection, version change, commit or publication is implied.
- Inventory validation passed 21 isolated Python tests, all 68 unique function IDs,
  both identical Mermaid projections, direct documentation links and source-digest
  checks. Interactive slices and evidence disclosure were checked at 736/320px.
  Linux package documentation now includes the JSON inventory; shell syntax and
  static link checks passed, but packages were not rebuilt or installed this round.

### Unreleased — Input and offline-pack boundary fixes

- Pinyin tone digits now terminate before paired ASCII/CJK aside punctuation and
  Chinese em dashes. Numeric literals, decimals, versions, times and ranges retain
  their digits; conversion, original-text selection, exact commit and undo are covered.
- English contexts and phrase endings use startup-cached canonical keys for case,
  spacing and straight/curly apostrophes. Equivalent appended contexts keep the first
  authored entry. Sentence-only contractions are indexed and completed as whole words,
  preserving the typed prefix and existing vocabulary ranks.
- Offline registry records retain raw JSON until strict typed decoding, rejecting
  duplicate fields instead of silently using the last value. Startup isolates only
  damaged records; listing/mutations report corruption without overwriting the file.
  This enables the existing `serde_json/raw_value` feature, without a version change.
- Short-draft pack candidates now share the native byte/frame budgets already used
  for long drafts. Oversized alternatives are filtered from the actual candidate list,
  never truncated only in the mirror; literal input and rendered commit indices stay
  aligned. Model merges apply the final byte budget after preference ranking.
- Native numbered candidate cards commit the full text with one click even when
  their label is truncated. The independent editor keeps its existing first-click
  scrolling preview; stale-revision and duplicate-release guards remain in place.
- Independent `input_edge_audit_{chinese,english,packs,budget}` regressions cover these
  boundaries, including legal Unicode/JSON-escaped long entries and single-row limits.
  The initial failing reproductions are retained separately from validation results.
  No dictionary data, protocol limits, personal installation or version is changed.
- Local validation on 2026-10-03 passed 1,104 ordinary all-features tests (27 opt-in
  tests remain separate). After the final native-click change, all 228 ordinary panel
  tests, the expanded private native-action test and strict Clippy passed again.
  Both IBus transports of follow-caret/bottom/auto passed 26/33/36 workflows each:
  190 real private GTK/IBus/XTest workflows, including all three long-pack cases.
- First runs are retained in `/tmp/suzaku-input-audit.jMX1kg/`: regression testing
  caught an empty-list fallback and a test's incorrect assumption about the IBus
  12-candidate convenience switch. GUI failure persisted after strengthening the
  undo geometry-readiness check, exposing the native first-click-preview bug; its
  independent mouse/touch red test then passed after the guarded production fix.
  No failed action is retried, no deadline is extended, and these source-build gates
  do not establish personal GNOME/Wayland or physical-touch acceptance.

### Unreleased — Everyday social fallback

- Adds a separate `daily_social` English data layer and append-only Chinese readings for
  social check-ins, listening/support, invitations and polite declines. Both languages add
  24 contexts and 48 authored sentences; Chinese adds 64 readings.
- English adds only four indexed forms (`tiring`, `quiet`, `cafe`, `shall`), reaching 6,117
  indexed forms, 473 contexts and 910 sentences. Chinese reaches 2,303 readings,
  467 contexts and 932 sentences. Earlier ranks, homophone order, kinds and separator flags
  remain unchanged; additional fingerprints lock all pre-existing contexts and wording.
- Fixed-expectation tests cover offline first-page word/sentence choices, partial spellings,
  exact commits/undo, literal spacing/case/apostrophes and continued long drafts. Six added
  GTK scenarios exercise physical spelling/adoption/Space and saved commits; the full gate
  contains 79 workflows split into two mixed-language sessions with unchanged deadlines.
  Synthetic unavailable/timeout/HTTP-error/empty-model outcomes cover the new phrases too.
  These are finite authored expressions, not live message access or a model-quality claim;
  personal installation, Japanese resources and optional packs are unchanged.
- Local validation on 2026-10-03 passed 1,091 ordinary all-features tests (27 opt-in tests
  remain separate), strict Clippy, formatting checks and the complete 79-workflow private
  GTK gate (41 + 38). New phrases were actually selected, undone, continued with Space
  and saved exactly. The first language-only run caught an old aggregate count assertion;
  only its measured totals were updated, retaining every historical rank/content fingerprint.
  Private portal inhibition warnings remain visible; this is source-build validation, not
  a personal installation, native Wayland acceptance or real model-quality evaluation.

### Unreleased — Everyday communication and coordination

- Appends a model-independent `daily_coordination` English layer and Chinese daily phrases
  for clarification, confirmation, polite requests, sharing messages and coordinating help.
  Both languages add 24 contexts and 48 authored sentences; Chinese adds 64 readings.
- English reuses the existing vocabulary except for `hand` and `middle`, reaching 6,113
  indexed forms, 449 contexts and 862 sentences. Chinese reaches 2,239 readings,
  443 contexts and 884 sentences. The released 13 English layers and 2,175 Chinese readings
  are protected by additional rank/order fingerprints; earlier snapshots remain unchanged.
- Extends fixed-expectation word/sentence, literal-prefix, long-draft and unavailable-model
  regression coverage, plus six real GTK input scenarios and a physical consecutive-word
  continuation through `帮我看看` / `有没有`. At that stage the complete vocabulary gate
  contained 73 workflows in two mixed-language sessions with unchanged per-session deadlines.
  Native HTTP 503/deadline checks also cover the new English/Chinese phrases. An explicit
  `SUZAKU_NATIVE_QA_BIN_DIR` override selects installed host/probe/tool binaries; source-built
  panel tests remain distinct from the installed panel's application checks.
  These are finite fallback expressions, not live message access or model-generated suggestions.

### 0.8.1 — Native candidates and everyday bilingual fallback

- Adds optional English/Chinese study-writing, cooking-preparation, travel-lodging and team-work
  packs, bringing the bundled catalog to eleven collections. The eight new packages are version
  1.0.0 and contain 240 explicit English forms, 144 Chinese readings and 256 authored sentences,
  before built-in deduplication.
- Optional packs leave existing embedded vocabulary, prior ranks, Japanese data, decoder/model
  contracts and budgets unchanged. The new JSON can also be installed by the released 0.8.0 CLI;
  its existing catalog does not gain the new exports until the tool is rebuilt/upgraded.
  Nothing is auto-installed.
- Adds candidate-quality and catalog/source/export parity regressions; extends the native IBus
  pack fixture and Linux distribution checks to the new collections. See
  [vocabulary data](data/lexicons/README.md) and [pack usage](docs/linux-packaging-data.md#离线词库包与制包-sdk).
- The travel/work follow-up retains the old meeting-minutes and acceptance-standard continuations,
  adding more specific contexts instead. Independent enable/disable checks keep themes opt-in;
  generated phrases neither read bookings/project records nor perform external actions.
- Adds a private GTK3 per-key gate for the earlier desktop draft-check failure: stable real
  input contexts, exact first/repeated keys, bilingual adoption/undo/commits, language chords,
  field changes and companion focus. CI runs default and forced-sync IBus transport. This
  improves diagnostics and coverage; it does not establish a fix for the GNOME/Wayland failure.
- Completes that gate's tray fixture: an owned StatusNotifierWatcher lets the real panel
  register its menu, and the tenth workflow activates through that menu from a real private
  XKB engine before switching Chinese to English. Menu readiness is observed after one
  AboutToShow, without repeating activation or input. Personal desktop acceptance remains open.
- Fixes a separate tray-activation visibility race: automatic preparation is published when
  the tray accepts activation, before queued refresh/switch work, and never marks the active
  draft as manually hidden. A first native draft arriving before preparation remains visible;
  delayed worker preparation/completion cannot override later explicit Show/Hide. Busy or
  disconnected requests do not prepare visibility.
- Opening settings or editing the seed transfers an automatic popup to explicit visibility;
  empty/unfocused/disconnected native snapshots no longer close that interaction or its window
  after editing finishes. Late native drafts cannot overwrite local text while the window-focus
  event is pending. Native tool tabs still update/auto-hide, and invisible snapshots still clear
  mirrored content. These fixes do not establish the cause of the earlier personal-desktop
  synthetic-key failure.
- Defaults to the custom Suzaku candidate surface once it has actually presented a public
  native frame. A short host/context-bound lease suppresses only IBus lookup/auxiliary UI;
  hidden, folded, occluded or locally edited panels relinquish it. UI-thread heartbeats and
  a 1.2-second host expiry after the last accepted renewal restore system candidates after
  stalls/crashes without another key (up to about 1.8 seconds from a UI stall).
  Passive snapshot subscribers do not suppress candidates, and display acknowledgements never
  change draft revisions, selection, model requests or commits.
- Fills a basic Chinese fallback gap with data-only readings for 强、厉害、很强、很厉害 and four
  authored continuations. Original 2,043 reading ranks are retained; full/separated Pinyin and
  adoption/spacing regressions stay independent of model availability.
- Adds everyday reactions, feelings, replies, meeting updates and rest/meal expressions to the
  default offline fallback. English adds 24 collocations and 48 sentences with just six new
  indexed forms; Chinese adds 64 readings and 24 contexts / 48 continuations. All eleven earlier
  English layers and 2,047 Chinese readings keep their original order and priority. Data-only
  additions remain independent of models and optional packs; index/candidate budgets are unchanged.
- Appends a `daily_needs` batch for food preferences, weather/outings and last-minute plans:
  64 Chinese readings and 24 contexts / 48 sentences per language. English reuses the existing
  index except for three explicit forms (tastes, dessert, road), totaling 6,111 indexed words.
  All twelve earlier English layers and 2,111 Chinese readings retain their ranks; this adds
  authored input expressions, not weather/menu/location services or model-specific behavior.

See the [0.8.1 release notes](docs/releases/0.8.1.md). Historical “unreleased” records below and
in linked audit guides retain their original development context; publication requires same-commit CI.

### 0.8.0 — Offline vocabulary packs and authoring SDK

- Introduces versioned, data-only offline packages independent of model providers. Language/topic
  collections can be browsed/exported, installed, replaced, enabled, disabled and removed through
  `suzaku_tool pack`. Built-ins stay available; conflicting contexts fail safely and damaged startup
  records are isolated. Limits bound file sizes, package counts and per-language indexing work.
- Freezes optional EN/ZH/JA vocabulary once at host/panel startup, keeping file I/O off the input
  path. Changes require restarting both processes; no live reload or settings-panel manager yet.
- Ships English/Chinese outdoors and Japanese rail collections plus a Python standard-library
  authoring SDK. SDK outputs are validated by the Rust tool and published without overwriting.
- Adds `digital`, `home` and `errands` built-in layers: 98 new indexed English forms, 237 Chinese
  readings and 128 / 128 authored English / Chinese sentences. Totals are 6,102 / 2,043 forms/readings
  and 718 / 736 sentences. Existing rank/order fingerprints, candidate and model budgets remain.
- Improves isolated validation on newer GLib/PyGObject: retains borrowed signal payloads until C
  dispatch returns, reuses accessibility observations only within one snapshot, and keeps Compose
  fixture rewrites inside private copies. These are test-harness fixes, not desktop keymap changes.
- Begins the required Linux Release asset policy: publish the exact clean source commit's `.deb`,
  `.tar.gz` and checksums only after that commit's CI and clean-container install checks pass.

See the [0.8.0 release notes](docs/releases/0.8.0.md) and
[pack lifecycle and SDK](docs/linux-packaging-data.md#离线词库包与制包-sdk).

### 0.7.5 — Bilingual sentence continuity and safer English context

- Adds an independent `clarity` vocabulary layer: 58 explicit English forms (70 indexed forms
  including sentence projections), 175 Chinese readings including the additional 再 homophone,
  and 80 authored sentences per language. Totals are 6,004 English forms and 1,806 Chinese readings;
  prior ranks, homophone order, Japanese data and budgets remain protected by existing fingerprints.
- Fixes N58: Chinese authored sentence suggestions retain the full matching progress across
  word adoption and verified Pinyin tails within the existing 256-character local window.
  Same-sound alternatives can supply a sentence without replacing the primary conversion or
  already adopted Han text. Literal spacing and exact adoption/undo/commit behavior remain.
- Fixes N59: English local collocations and sentences no longer cross real line/paragraph
  boundaries or normalize identifiers, domains, digits and clipped context fragments into words.
  Horizontal spacing, case, contractions and valid new phrases after protected text remain usable.
- Expands fallback checks to 44 authored scenarios per language and real GTK vocabulary checks
  to 42 workflows, including Chinese multi-word sentence progress. Native modes each include
  16 actual loopback HTTP 503/deadline cases, three N58 workflows and eight N59 workflows.
  The basic GTK gate adds two physical English boundary/edit-recovery workflows.

See the [0.7.5 release notes](docs/releases/0.7.5.md),
[Chinese sentence-progress audit](docs/bug-audit-sentence-progress-2026-09-27.md) and
[English context audit](docs/bug-audit-english-context-2026-09-28.md).

### 0.7.4 — Reliable model fallback and broader bilingual vocabulary

- Adds three independent fallback vocabulary layers: 658 explicit English forms, 488 Chinese
  readings and 272 short sentences in each language. The English index grows to 5,934 forms;
  Chinese reaches 1,631 readings. Old base ranks, homophone order and Japanese data are preserved.
- Covers everyday clarification, work/travel, time arrangements, shopping/after-sales and common
  verb forms. Local candidates appear immediately with or without a model; this is bounded authored
  fallback, not open-ended generation, a downloaded corpus or runtime dictionary import.
- Fixes N57: a full model response could discard a learned local sentence before applying its
  preference bonus. Ranking now precedes the final cutoff within the existing bounded merge pool;
  final capacity, word/sentence quotas, typing anchors and explicit selection locks stay unchanged.
- Extends offline regression cases to 36 per language, real GTK vocabulary workflows to 36, and
  actual HTTP 503/deadline cases to 12 per native mode. Tests retain literal spacing, adoption undo,
  continued drafts and exact commits without changing the original suite deadlines.

See the [0.7.4 release notes](docs/releases/0.7.4.md),
[fallback vocabulary record](docs/ibus-candidates.md#模型不可用时的本地兜底词库) and
[N57 preference/model audit](docs/bug-audit-preference-model-2026-09-27.md).

### 0.7.3 — Vocabulary resources and in-memory candidate preferences

- Adds two rounds of authored everyday English/Chinese vocabulary: 5,251 indexed English
  forms, 1,143 Chinese readings, 238 English and 256 Chinese sentence examples in total.
  Existing spelling, spacing, literal choices and word/sentence quotas remain protected.
- Moves English, Chinese and Japanese vocabulary into versioned embedded JSON resources.
  The data interface is independent of decoders, native hosts and model providers; parsing
  and indexing happen once per language. This is not runtime import or hot reload.
- Adds a bounded frequency cache independent of dictionary/model implementation. Confirmed
  choices adjust existing IBus candidates; editable adoptions wait for final submission.
  Canceled, undone, private or edited-away choices do not learn. English identifier/word
  boundaries are checked, and asynchronous model refreshes do not stack bonuses.
- Preferences contain salted fingerprints and decaying counts in memory only; the tray can
  clear them, and host restart forgets them. No history file or input-thread persistence is added.
- Adds a real GTK vocabulary workflow suite and connects it to CI. Native preference checks
  cover numeric, Shift+Enter, Space and companion adoption, undo, privacy, focus and clear.

See the [0.7.3 release notes](docs/releases/0.7.3.md), [vocabulary contract](data/lexicons/README.md)
and [candidate preference scope](docs/ibus-candidates.md#偏好频率缓存).

### 0.7.2 — Draft-preserving language shortcuts and responsive settings

- Ctrl+Shift+Space toggles English/Chinese in a normal, focused IBus field, retaining the
  latest draft after persistence succeeds. Empty drafts and both shortcut profiles are supported;
  Japanese switches to English. Held keys remain one-shot, including early modifier release.
- N56 moves settings reads, writes and file sync out of the input loop and shared session lock.
  A single worker stages the change; publication validates the request lifetime, deadline and
  settings baseline. Canceled jobs never publish late, and a busy worker cannot multiply.
- Pending language gestures are canceled by focus, commit, clear, privacy and Compose boundaries.
  Save errors keep the old language and draft. Physical modifier presses preserve pending accents.
  F1 exposes the localized shortcut reference; no global bindings or arbitrary rebinding are added.
- The isolated keyboard gate grows to 49 groups; the controlled slow-I/O gate checks 9 settings
  cases plus 10 keyboard-language cases. Historical release failures are not attributed to this fix.

See the [0.7.2 release notes](docs/releases/0.7.2.md), [shortcut reference](docs/shortcuts.md)
and [N56 settings I/O audit](docs/bug-audit-settings-io-2026-09-27.md).

### 0.7.1 — Activation diagnostics and installation audit

- Private IBus tests preserve real CLI stderr and exit codes on activation/release failures,
  without changing production restoration logic or retrying failures until success. Dedicated
  checks reject desktop access, invalid scope, self-recursion and unrelated commands.
- A guarded repeat-test mode complements the complete native suites. The 0.7.0 CI failure
  did not reproduce in 130 repeated cycles; rerunning the failed job at the same commit passed.
  Its root cause remains unconfirmed, and that older run does not validate the new diagnostics.
- The 0.7.0 installation audit records 124 isolated application checks and 13 real GNOME
  input workflows, plus tray/service quit and reopen. Two desktop cases stopped at the focus
  guard remain incomplete; package metadata and documentation do not update the installed app.

See the [0.7.1 release notes](docs/releases/0.7.1.md),
[activation follow-up](docs/ci-activation-followup-2026-09-27.md) and
[installation audit](docs/install-audit-0.7.0-2026-09-27.md).

### 0.7.0 — Continuous word-to-sentence input

- N54 keeps an adopted Chinese phrase and its horizontal spacing in the bounded local window,
  preserving authored sentence choices before the next Pinyin input. Longest matches retain
  the complete phrase rather than falling back to a shorter suffix. See the
  [round-46 audit](docs/bug-audit-adopted-continuation-2026-09-27.md).
- N55 shares that match with the ordinary IBus mix: adopting Pinyin can shrink a draft from
  above 256 code points back within the short path without losing its sentence. Prefixes,
  whitespace, protected tokens and full-draft model limits remain unchanged. See the
  [round-47 audit](docs/bug-audit-continuation-threshold-2026-09-27.md).
- Bilingual native and real-editor checks cover threshold changes, Space, punctuation deletion,
  exact word/sentence undo and explicit submission. Controlled providers verify full-draft
  requests and stale-result rejection at the model limit; no model quality claim is added.
- The stock-popup fixture no longer assumes exactly nine English candidates. It checks every
  actual slot while still requiring a full first page and a partial last page. This fixes the
  0.6.9 CI fixture failure without reducing click or saved-text coverage.

See the [0.7.0 release notes](docs/releases/0.7.0.md) for verification and upgrade boundaries.
Historical audits retain their original test counts and publication/installation state.

### 0.6.9 — Vocabulary, long drafts and panel ergonomics

- The curated English index grows from 1,242 to 3,906 explicit forms; Chinese Pinyin grows
  from 98 to 602 entries. More authored collocations offer words and sentences together.
  A stable first-letter Pinyin index bounds matching work; ambiguous alternate conversions
  no longer crowd authored/model sentences off the first page. No downloaded corpus,
  personal learning, forced correction or model dependency is introduced.
- English and adopted-Chinese drafts beyond 256 code points retain bounded local tail
  completion, preserving the exact earlier prefix. The existing 8192-byte mirror text and
  65536-byte frame limits remain; fewer alternatives fit very long drafts. Model requests
  still use the unchanged 256-code-point limit. See the [round-45 audit](docs/bug-audit-long-draft-window-2026-09-25.md).
- Settings support visible Tab/arrow navigation, Enter/Space activation, Ctrl+F search and
  scroll-to-focus. Zoom targets are larger and separated, reset uses live scale, and an orb
  press begins WM dragging only after movement. Window titles omit user drafts and debug text.
  See the [installed ergonomics checks](docs/ergonomics-audit-2026-09-25.md); these are not full
  screen-reader or human-comfort acceptance.
- Regression fixtures now compare constant dictionary contents rather than addresses and
  compare runtime-report dispatch against fixed ready/unready states. Development instructions
  distinguish toolchain visibility, required feature flags and short Unix-socket fixture paths.

See the [0.6.9 release notes](docs/releases/0.6.9.md) for verification and upgrade boundaries.
Historical reports retain their original test counts and installation state; source publication
does not update the installed application or establish remote CI/package success.

### 0.6.8 — Bilingual candidate continuity and Linux QA

Includes audit rounds 40–44 and N50–N53. See the [0.6.8 release notes](docs/releases/0.6.8.md)
for validation and upgrade scope; a source tag does not update the installed application.

#### Strict real-editor save synchronization

- The 0.6.7 Linux CI stopped at a popup saved-document assertion. Local repetition also exposed
  a cleared editor buffer with old file contents. File bytes can change before the asynchronous
  editor save has finished; a following Ctrl+S may encounter a disabled Save action.
- Real GTK checks now observe only their owned editor PID/document and require exact buffer text,
  an idle save indicator and exact file contents. Only disposable editor preferences postpone its
  periodic draft autosave beyond the unchanged 240-second gate, avoiding a disabled Save action.
  Editor-only GTK animations are disabled so readiness checks do not wait for decorative fades.
  Each save still uses one real key chord, with no repeated input or relaxed text assertions.
  Fourteen deterministic barrier/scope checks and a real two-editor observation gate are in CI.
- This changes test synchronization, not Suzaku input routing, save behavior in personal applications
  or keyboard mappings. See the [round-40 audit](docs/bug-audit-editor-save-2026-09-24.md).

#### One-word English continuations before Space

- N50 classifies a single next word after a known complete English word consistently with the
  same continuation after a typed Space. `hello → hello world` is a word continuation, while
  longer phrases and punctuated sentences remain sentence candidates. Literal anchors stay put.
- Sentence-only model replies can now also supply that next-word choice before Space, using an
  exact prefix of the returned text and retaining the full sentence. Unknown/incomplete readings,
  identifiers, URLs and hyphenated tails are not split into invented word choices.
- The change is provider-independent and does not add vocabulary, auto-correction, automatic
  submission or new keyboard bindings. Chinese/Japanese algorithms remain unchanged. See the
  [round-41 audit](docs/bug-audit-english-next-word-2026-09-24.md).

#### Pinyin tone numbers at punctuation boundaries

- N51 consumes an optional Pinyin tone number before sentence punctuation or a closing quote/bracket,
  so `ni3hao3，shi4jie4！` converts to `你好，世界！` without leaking tone digits into the text.
  Punctuation and the exact raw-spelling candidate remain available; no automatic commit is added.
- Decimal/time separators followed by alphanumerics and numeric grouping commas remain literal,
  as do multi-digit numbers, fractions, operators and digits outside the existing 1–5 tone range.
  The current vocabulary and English/Japanese algorithms are unchanged. See the
  [round-42 audit](docs/bug-audit-pinyin-tone-punctuation-2026-09-24.md).

#### Exact local prefixes in model continuations

- N52 preserves the whole known CJK local-conversion prefix in provider validation and in both
  standalone/native candidate merging. Indentation, repeated spaces and Unicode horizontal spaces
  no longer disappear from a correctly prefixed Chinese model reply or break its bounded length check.
- Built-in HTTP providers reject prefix-losing replies instead of silently accepting trimmed text.
  English behavior, unconverted phonetic input, candidate budgets, model permissions and cancellation
  stay unchanged. Both protocols and synthetic native adoption/undo/Space/Enter flows are covered.
  See the [round-43 audit](docs/bug-audit-model-prefix-spacing-2026-09-25.md).

#### Pinyin continuation after long adopted drafts

- N53 fixes the offline decoder's 128-round cutoff within its existing 256-code-point input
  limit. Long literal prefixes and repeated syllables no longer hide the exact conversion or
  promote a guessed extension ahead of it. The bounded input determines the search rounds;
  beam width, vocabulary, ranking and maximum input length stay unchanged.
- Long Chinese/English adoption, exact undo, Space continuation and commit checks now cover
  both native entry routes and real GTK editor output. Shortening a 257-character draft restores
  conversion at 256; see the [round-44 audit](docs/bug-audit-long-pinyin-continuation-2026-09-25.md).

### 0.6.7 — Context-local shortcuts and literal boundaries in Chinese drafts

- A searchable Shortcuts settings page preserves the standard layout and offers opt-in QWERTY
  home-row IBus aliases: Alt+J/K navigate, Alt+H/L page, Alt+semicolon adopts without submitting.
  Focus-local F1 opens the reference; Ctrl+comma opens panel settings. No global key grabs.
- Shortcut preferences use acknowledged narrow native patches and configuration backups, with
  failure/conflict protection. Changing only the layout preserves input, selection, undo, Compose
  and prediction. Modifier/privacy/idle guards preserve application keys. See [shortcuts](docs/shortcuts.md).
- N49 makes physical candidate-adoption holds one-shot until release, retaining exact undo and
  immediate separate taps. Releasing a modifier first cannot turn held adoption into submission;
  navigation/text editing still repeat. See the [repeat audit](docs/bug-audit-shortcut-repeat-2026-09-24.md).
- Physical keyboard coverage now includes releasing selection keys in another real editor window,
  reusing the same key, exact undo and returning focus without replaying the old draft. The gate
  has 44 workflows; see the [focus follow-up](docs/bug-audit-shortcut-focus-2026-09-24.md).

- N48 keeps literal whitespace, quotes and non-Pinyin Unicode case around converted/adopted text.
  Only actual phonetic separators are consumed; line/paragraph boundaries are never syllable joins.
  Existing `ü`/`u:` and tone forms, `xi'an` disambiguation and literal fallback remain available.
- Word/sentence continuations retain literal horizontal padding. New paired native checks cover
  owned-prefix keyboard continuation and companion replacement, adoption/undo and exact submission.
  English input is a regression control; its algorithm and dictionary are unchanged.
  See the [literal-boundary audit](docs/bug-audit-chinese-literal-boundaries-2026-09-24.md).
  See the [0.6.7 release notes](docs/releases/0.6.7.md) for validation and upgrade boundaries.

### 0.6.6 — Bilingual input and model responsiveness

- Existing Chinese entries now match joined or correctly separated Pinyin, including unfinished
  readings, while preserving syllable boundaries and literal fallback. Paired Chinese/English
  quality suites and native word/sentence adoption, undo and continuation checks guard the core flow.
- Newer drafts cooperatively cancel obsolete connected local HTTP candidate/discovery requests;
  a single worker and the original total deadline remain. Blocking cloud/legacy calls still
  finish or time out before discarding results. Translation cancellation is unchanged.
- Compact, model-independent candidate prompts request two complete strings and reject request-field
  echoes. Translation restates its task and rejects lost question marks without changing the draft.
  Real CPU LLaMA still has default-budget timeouts and semantic limitations.
- Added isolated physical-keyboard/layout checks to CI and Firefox, VS Code and opt-in real-model
  workflows. These do not qualify hardware LEDs, native Wayland or every browser/editor.
  See the [0.6.6 release notes](docs/releases/0.6.6.md) for validation and release boundaries.

### 0.6.5 — Service retries and verified input-source recovery

- Newly registered host units retry every two seconds without exhausting systemd's start quota.
  Explicit stop still cancels retries; existing units need explicit re-registration to update.
- A panel owning the registered desktop host observes same-bus host replacement and can restore
  only a confirmed missing global engine. Newer valid choices win; IBus disconnection discards
  observation history. No application text is monitored and no old draft is replayed.
- Password diagnostics reset their own input purpose and destroy their synthetic context before
  bounded, read-back-verified restoration. Missing fallback input sources fail before switching.
- Added private daemon-loss, real isolated systemd and input-source recovery regressions. Explicitly
  authorized GNOME 46/X11 installation checks also pass with the same fixes before the version bump.
  See the [0.6.5 release notes](docs/releases/0.6.5.md) for validation and upgrade boundaries.

### 0.6.4 — Smaller build caches and test fixture cleanup

- Development/test profiles omit debug symbols and disable incremental compilation, matching CI.
  Ordinary dependency caching, assertions and overflow checks remain; release settings are unchanged.
- Theme-setting tests remove their freshly owned temporary directories when the fixture drops,
  including during unwinding. A regression verifies cleanup; personal settings are never targeted.
- Kotlin's local cache is ignored, and the contribution guide documents scoped cleanup and debugger
  overrides. No automatic user-cache deletion or input-rule change is introduced.
  See the [0.6.4 release notes](docs/releases/0.6.4.md).

### 0.6.3 — Bounded popup previews and clickable ordinals

- Candidate-area draft previews keep a suffix within 48 display cells and 160 Unicode code points,
  preventing tested unwrapped stock popups from extending beyond the screen. Drafts remain lossless.
- Superscript shortcuts now share the candidate text's click target, avoiding IBus 1.5.29's invalid
  separate-label indices without changing system files, candidate payloads or number-key adoption.
- Strict popup coverage includes 46 workflows, with 17 actual ordinal-glyph checks; another 21
  real GTK lifecycle workflows cover switching, privacy and process reconnection.
  See the [0.6.3 release notes](docs/releases/0.6.3.md).

### 0.6.2 — Candidate-area drafts and literal numeric input

- Default drafts stay in the candidate area/companion, not the application's preedit cache.
  Tested Chrome/Qt fields no longer commit them on focus changes, and cancelling a replacement
  preserves the original selection. Enter/candidate clicks remain explicit submission.
- With no draft, ordinary/keypad digits go directly to the application. Within a draft, number
  choices and Alt+digits retain their existing behavior; Space still continues writing.
- Opt-in inline preedit remains available, with complete empty updates for synchronous IBus
  cleanup. It retains the client's implicit-confirmation risk on focus changes.
- Real GTK/Chrome/Qt workflows and Unicode/long-draft boundaries have isolated regression coverage.
  See the [0.6.2 release notes](docs/releases/0.6.2.md).

### 0.6.1 — Input-target boundaries and safe continuous typing

- Independent panel sends cannot inject into PRIVATE fields. Privacy/bypass transitions invalidate
  old input targets, including brief round trips hidden by coalesced snapshots.
- Actual language changes and whole-draft cancellation reject delayed old sends and stale presses;
  same-language settings, Compose-only cancellation and ordinary editing remain continuous.
- Physical Enter, native candidate clicks and independent text sends end old input work after a
  successful commit. Revision-bound companion commits keep their acknowledged follow-up typing.
- Candidate-commit history remains exact within the same field; late model replies cannot restore
  canceled or committed drafts. See the [0.6.1 release notes](docs/releases/0.6.1.md).

### 0.6.0 — Native input state and exact model context

- Reselecting the current language preserves drafts, completion undo and pending Compose sequences.
- Repeating the current prediction switch or Tone preserves candidate selection and model results;
  configuration conflict checks, failed-save protection and explicit reloads remain enforced.
- Destroying the focused IBus engine without FocusOut ends its session and clears companion state.
  Late destruction or model results cannot affect a newer field.
- Native model context concatenates the exact candidate payloads, without the independent editor's
  automatic separators. Application output, editor spacing and continuous-drafting keys are unchanged.
  See the [0.6.0 release notes](docs/releases/0.6.0.md).

### 0.5.9 — Translation gestures and continuous candidate input

- Translation requests require a fresh gesture after the source, field or configuration changes;
  cancelling also invalidates a press that has not started a request yet.
- Screen-keyboard input after acknowledged tool/candidate operations starts from the resulting draft
  and waits for its authoritative revision. Newer local work and target/privacy boundaries remain protected.
- Intermediate empty snapshots no longer hide a companion with queued input or forget manual hiding.
- Selecting unchanged literal text publishes the new selection to IBus and the companion without
  rebuilding candidates or committing. See the [0.5.9 release notes](docs/releases/0.5.9.md).

### 0.5.8 — Lifecycle recovery and input-target boundaries

- Quit retries retain unfinished input-method restoration and queued host-start cleanup.
- Failed single-instance handoffs no longer open an unguarded extra panel. Coalesced native
  updates can wake the companion for a new field without overriding manual hiding in the old field.
- Voice/handwriting adoption gestures are cancelled when their native input target changes;
  source material remains available for a fresh click. Continuous drafting and explicit submission
  rules are unchanged. See the [0.5.8 release notes](docs/releases/0.5.8.md).

### 0.5.7 — Continuous input and Linux reliability

Completion/undo, tool insertion, settings recovery, confirmed model configuration, data restore
locks and Linux registration/packaging were hardened. See the [0.5.7 release notes](docs/releases/0.5.7.md).

### 0.5.6 — English completion and input reliability

Improved English completion, Compose/dead-key handling and native editing recovery, with functional
network audits and strict Clippy coverage. See the [0.5.6 release notes](docs/releases/0.5.6.md).

### 0.5.5 — Draft translation, clearer text and compact settings

- Declared numeric/decimal/phone fields bypass candidate shortcuts and panel injection.
  Screen-keyboard bursts retain unsent edits behind one acknowledged write; stable letter
  presses survive candidate refreshes. Uncertain writes never automatically replay across fields.
- English word/sentence continuation shares authored collocations through spaces and partial
  words. Long previews show the differing suffix without changing payloads; Ctrl+Backspace
  deletes a whitespace-delimited English draft word.
- Linux packaging declares keyboard runtime and Latin/CJK font dependencies. Clean Ubuntu
  container checks cover install/upgrade/remove; registration checks cover special paths and safety.

- **Settings → Appearance → Interface** selects the application's display language independently
  of input and translation languages. English, Simplified Chinese, Japanese, Korean, Spanish,
  French, German and Portuguese are bundled offline; **System** follows the locale environment
  with English fallback. Changes update panel/settings/tray immediately and survive restarts and
  settings backups. Search accepts translated and English labels; narrow layouts reflow longer
  labels. Drafts, candidates, model results and provider consent are unchanged. Raw system/service
  diagnostics retain their original wording. See [interface languages](docs/interface-languages.md).

- The panel's **文 / Translation** tool translates the current draft between English, Simplified
  Chinese, Japanese, Korean, Spanish, French, German and Portuguese. Choose **From / To**, then
  **Translate**; **Auto** detects the source. Read longer output with the page arrows and use
  **Use draft** to replace the editable composition, never to send it automatically.
  Translation uses the configured local/cloud provider with a separate 30-second deadline;
  cloud consent remains required. Source text, translations and navigation are session-only.
  Editing the source, changing language/context or reloading model settings discards stale results.
  See [translation behavior and limits](docs/translation.md).

- Settings are grouped into **Appearance**, **Input** and **Model** tabs. Search spans every tab;
  selecting a tab clears search and resets scrolling. Labels/options share aligned columns, the
  search field uses the available width, and the window fits the current page's content height
  while retaining scrolling on smaller screens. Category changes never modify saved IME options.

- **Settings → Appearance → Title Bar → Hide** removes the desktop window title bar from the panel and
  settings immediately; **Show** restores it. Suzaku's own drag regions and close buttons stay
  available. The choice survives restarts, settings backups and orb expansion. Existing profiles
  keep their title bars until changed; no GNOME-wide settings are modified.
- Borderless panels and settings now have transparent exteriors instead of a square pale
  backplate. Rounded cards retain their subtle shadow and unchanged controls. Both windows request
  alpha-capable native surfaces, including Linux/XWayland's inherited-alpha path; opaque-only
  drivers fall back to the theme background. Showing the system title bar restores its backdrop.
  On hybrid-GPU Linux laptops, an alpha-capable hardware adapter is preferred over an opaque-only
  adapter, with fallback if device initialization fails; software rendering is not forced for this.

- Panel and settings text now rasterizes at the glyph's physical display height, rather than
  shrinking a shared large bitmap. Pixel-aligned ink retains its native width and antialiasing;
  fractional layout advances and clipping stay intact, including at 90% zoom and higher DPI.
- A bounded, padded atlas caches multiple sizes of each character without stretching narrow
  letters or mixing neighboring glyphs. Zoom and raster eviction retain resolved fonts and stable
  layout measurements. Saved font, smoothing and zoom choices are preserved; system IBus popup
  rendering and global font settings are not changed.

### 0.5.4 Guardian themes, continuous input and service lifecycle

- Linux panel startup now starts the registered `suzaku-ibus.service` and waits for its IPC
  readiness without activating Suzaku. Activation retries a stopped host automatically. A full
  quit restores the observed previous input method before stopping the host; hide-to-tray keeps
  it running. Startup/shutdown run on the background controller, including desktops without a tray
  extension. Restore/stop failures remain visible and retryable; a disconnected host is no longer
  mislabeled as an outdated version. Custom/private endpoints remain owned by their own launcher.
- Native IBus now uses one continuous writing stream: plain **1–6** adopts the current page's
  candidate without committing, **Alt+digits** enters numbers, and Space/punctuation stay in the
  draft. Enter or a primary candidate click submits it. Shift+number symbols retain the active
  keyboard layout. English, Pinyin and Romaji share the same rules; both local and model
  predictions see the complete uncommitted draft. Panel candidate shortcuts also keep text editable.
- New default **Suzaku** theme: warm ivory surfaces, vermilion accents, soft gold details and
  quieter candidate highlights. Choose **Settings → Theme → Suzaku** for an existing profile;
  explicitly saved themes are retained. The native companion and backup format recognize `suzaku`.
- Three additional styles under **Settings → Theme**: **白虎 · 米白** (warm ivory / graphite),
  **青龙 · 青紫** (misty jade / violet), and **玄武 · 靛蓝** (deep indigo / silver blue).
  They cover the panel, candidate highlights, keyboard, handwriting, dictation, settings and
  tooltips. Saved IDs are `baihu`, `qinglong`, and `xuanwu`; settings reloads and backups retain them.
  The orb and Linux tray use matching curved tiger, dragon, and tortoise/serpent emblems. Tray
  icons update only when the theme changes, on the background worker, not on input/render frames.
- Rounded cards and the redesigned toolbar icons use resolution-independent, antialiased curves
  and strokes. The default floating orb and Linux tray share a swept-feather phoenix emblem; tray sizes
  are supersampled individually. The orb has transparent surroundings when the compositor and
  graphics surface support premultiplied alpha, with a themed background fallback otherwise.
- New profiles use the system Auto font and Smooth text. Loading settings no longer silently
  replaces Auto/Smooth with Monaco/Sharp. Text layout, centering and the caret use cached system
  glyph widths, so narrow letters are no longer stretched across a fixed character cell.
- Existing candidate actions, non-functional drag regions, close-to-tray behavior and content-fit
  resizing are retained. Desktop panel styling does not override GNOME's system IBus popup theme.
- Bug fixes: the input row preserves repeated spaces and scrolls long drafts with the caret,
  instead of wrapping over controls or eliding editable text. Its height follows text size.
  Fractional zoom keeps glyph geometry intact, Smooth/Sharp changes the active sampling filter,
  and new tooltip/feedback glyphs are measured before the first frame is displayed. Single-symbol
  controls fit their real glyph width so the zoom `+` cannot turn into an ellipsis at Large text size.
- Model reloads preserve the current IBus draft while a provider change forgets previous committed
  context. English model candidates retain typed indentation, spacing and literal list prefixes
  through parsing, ranking and commit. A missing/disconnected local model invalidates discovery
  for the next request instead of keeping a stale success entry for 60 seconds; no failed generation
  is automatically retried.
- Output fixes: consecutive native commits preserve each candidate's exact leading/trailing
  spaces. Standalone Linux panel sends use the displayed candidate, not another bridge's candidate or
  cumulative history. Only acknowledged delivery clears the editable draft and records a local
  commit; rejected/uncertain sends retain it with an explicit warning and are never retried automatically.
- IBus focus boundaries now reject late key, navigation and candidate-click events from unfocused
  engine objects. Focus-in clears the previous field's preedit, undo and prediction context even
  when focus-out is delayed. Native candidates commit only on primary clicks without application
  modifiers; physical Mod4 and virtual Super/Meta/Hyper shortcuts pass through without changing input.
- Linux configuration loading opens nonblocking and validates the opened file type before reading.
  FIFOs (including symlink targets), sockets and directories fail without freezing native input;
  failed reloads preserve the current settings and draft. Symlinks to regular JSON files still load.
  The real activation/input/release regression now checks six-row English/Chinese/Japanese
  candidates and runs in CI only under a guarded private IBus session.

### 0.5.3 Model services and IBus candidate experience

Model identity, local/cloud scope and API protocol are now independent. Local discovery prefers
an installed LLaMA through fixed loopback metadata endpoints; explicitly configured HTTPS cloud
services require opt-in consent. Credentials are read from a named environment variable, never
saved in settings/backups; restoring a backup revokes cloud consent. Legacy Llama configuration
and command aliases remain compatible. See [model provider configuration](docs/model-providers.md).

IBus now mixes alphabet/Pinyin/Romaji word completions with short phrase/sentence candidates.
There are six rows per page and at most twelve candidates. Superscripts distinguish words (`ᵂ`),
sentences/phrases (`ˢ`), literal input (`ᴿ`) and model suggestions (`ᴬᴵ`); subscripts show bounded
ranking weights, not confidence percentages. The first page reserves room for both types when
available, while preserving the literal/best-offline typing anchors. Offline collocations work
without a model; enabled local or cloud providers can add typed word and sentence suggestions.
See [IBus candidate UX and keyboard controls](docs/ibus-candidates.md).

Continuous CJK input now completes an unfinished final word without losing the converted prefix
(`woxihuanbei` → `我喜欢北京`, `watashihanihong` → `私は日本語`). Pinyin spaces preserve
syllable boundaries (`xi an` / `xi'an` → `西安`, distinct from `xian`).

Shift+Enter adopts a full candidate into editable preedit; Space continues from an explicitly
selected candidate. Immediate Backspace restores the previous spelling. This one-step undo stays
in memory and is cleared at editing, reset, language, privacy and focus boundaries. No candidate
annotation or truncated preview enters the committed text.

### 0.5.2 Linux packaging and data management

- Build binary `.tar.gz` and Debian/Ubuntu `.deb` packages with
  `bash scripts/package-linux.sh`. Packages include the four Linux programs, desktop launcher
  (for `.deb`), manifest, checksums and dependency license inventory. Installation never starts
  a service, registers an input source or downloads a model automatically.
- Tray **数据管理** provides configuration backup and folder shortcuts. `suzaku-tool data`
  supports status, backup, validation and preview-first restore. Linux restore requires stopped
  hosts/panel, saves a before-restore backup and preserves unrelated user files.
- Existing configuration locations are retained; new configuration writes are private and atomic.
  No input history, drafts, caches or external model weights are collected.

See [Linux packaging and data management](docs/linux-packaging-data.md) for installation,
upgrade/uninstall, path conventions, restore commands and compatibility limits.

### 0.5.1 Patch fixes

- Preserve literal English digits and preedit across modifier/function keys. Keep CJK numeric
  candidate selection, and use absolute candidate navigation for later-page AI probe results.
- Prevent stale candidate presses and duplicate mouse/touch completions. Synchronize Tone through
  acknowledged native settings, preserve custom temperatures, and retain decimal/version prefixes
  in Llama responses. Use a process-lifetime lock for concurrent panel startup and crash recovery.
- Keep Linux/IBus IPC reads and replies off the blocking path with main-loop socket sources.
  Incomplete requests have a one-second absolute deadline, a 65,535-byte limit, and a cap of
  16 pending clients. Oversized, invalid UTF-8 and incomplete requests cannot commit a prefix;
  legacy commits are rejected if focus changes before the complete request arrives.
- Refresh Linux panel capability/status checks in one on-demand background worker. Startup uses
  a conservative snapshot; an expired three-second cache returns its last result immediately.
  External status commands have a 350 ms deadline and 64 KiB output limit; timed-out probe
  process groups are stopped and the direct child is reaped. Diagnostic APIs remain synchronous
  with these bounds; UI input handlers never wait for a status subprocess.
- Apply one total deadline to Linux client connections, partial writes and responses: 200 ms
  for subscription setup/each read turn, 350 ms for native actions, availability checks and legacy
  text output, and 700 ms for settings. A saturated accept queue cannot block indefinitely;
  trickled responses cannot reset the budget. Timed-out operations are never replayed.
- Reuse the bounded command runner for tray IBus operations, including output collection, with
  a two-second deadline and 64 KiB output limit. Inherited stdout cannot extend the deadline;
  the unreaped direct child reserves its process-group identity until cleanup. Failed/uncertain
  switches retain their restore target. Native sync workers support cancellation on panel exit,
  and partial subscription frames yield without recursion or discarding their buffered bytes.
- Keep voice transcripts and handwriting drafts until a revision-bound native replacement is
  acknowledged. Busy, rejected, disconnected or timed-out requests retain the source for explicit
  retry, without falling back to another target. Late acknowledgements cannot clear newer drafts.
- Wait for 900 ms without a transcript change before automatic voice insertion, including backends
  that emit each update only once. Changed partials restart that interval; failed handoffs stop
  capture and never auto-retry. This is a quiet-time heuristic, not a final-utterance signal.
- Preserve leading/trailing spaces, tabs and newlines in fallback text output. Blank-only input is
  still rejected and embedded NUL bytes are still replaced by spaces.
- Remove macOS speech file logging, including transcripts and framework error descriptions.
  This prevents new logs; pre-existing logs are not removed. macOS runtime verification is pending.

### 0.5.0 Highlights

- Added shared English, Chinese and Japanese input profiles, with English-first offline word and
  phrase completion, language-aware commit spacing, and persisted settings.
- Added opt-in asynchronous local Llama candidates through Ollama or compatible local servers,
  with configuration, status, warmup and probe tools; offline candidates remain available.
- Completed Linux/IBus tray activation and restoration, plus real-time native composition,
  candidate and selection synchronization with the non-focusing panel. Revision-checked panel
  actions prevent stale commits, and private fields do not expose input to the panel.
- Restored direct panel keyboard editing, including IME preedit/commit events and focus return,
  while keeping candidate clicks and background dragging non-focusing.
- Fixed multilingual glyph fallback, candidate visibility, handwriting overlap, control alignment,
  background dragging, and content-fit resizing through tab folding, zoom and compact mode.

Native input synchronization is currently implemented and exercised on Linux/IBus. The shared
language framework is portable; other native host adapters and larger CJK dictionaries remain
follow-up work. See verification and known limitations below.

### 0.5.0 IME foundation

- Shared Chinese, English and Japanese language profiles with language-specific conversion and
  commit spacing; the former English demonstration sentences have been removed.
- Immediate offline candidates plus debounced, asynchronous local-LLM enrichment in the native IBus
  host and desktop companion. Late responses cannot replace a newer composition or a selected list.
- Tray language selection, local-LLM opt-in, model configuration reload, and persisted IME settings.
- A literal fallback stays available, AI candidates are marked, and only explicit selection commits
  generated text. In native IBus input, password/PIN fields bypass the engine and private fields use
  offline conversion only; the companion commit channel also refuses password/PIN targets.

This is the first functional foundation, not full dictionary coverage. Chinese currently has a small
bootstrap Pinyin vocabulary and bounded phrase segmentation; Japanese has Romaji/Kana conversion
and a small Kanji vocabulary. Large dictionaries, learned user vocabulary, reliable long-sentence
conversion and automatic next-word suggestions after commit remain follow-up work. Language
profiles can be extended independently of the LLM provider and platform host.

### Using multilingual model candidates (local or cloud)

After building/installing the native host, right-click the Suzaku tray icon and select **输入语言**:

- **中文（简体拼音）**: e.g. `nihao` → `你好`, `shurufa` → `输入法`.
- **English**: literal input first, then word/phrase completions; Space keeps the writing stream editable.
- **日本語（ローマ字）**: e.g. `nihongo` → `日本語` / `にほんご` / `ニホンゴ`.

English is the default for new settings; saved Chinese/Japanese choices are retained. The panel
starts with an empty composition instead of a Chinese demo phrase. The project-authored offline
English baseline has over 900 common words, contractions and computing terms in a cached prefix
index, with explicit collocations for context-sensitive ranking. It is not a comprehensive spelling
dictionary or automatic correction system: literal input remains first and unknown input is kept.

Examples: `hel` → `hello` / `help`, `please sen` → `please send`, `good m` → `good morning`,
and `thank you ` → `thank you for` / `thank you very`. Typed case and spacing are preserved;
URLs, paths and identifiers are not split into word completions. Same-session committed words can
rank the next prefix (`good` followed by `m` prefers `morning`); this context is cleared on focus
loss and private fields do not retain it. An empty preedit does not yet open next-word candidates.

Panel word chips complete the current word or append only the immediate next word. For example,
clicking `hello` after `hel` produces `hello`, never `hel hello`; clicking `world` then gives
`hello world`. The back action reverses each exact completion. Single-word full candidates remain
visible for immediate commit; no arbitrary `is/can/will` words fill an empty live candidate list.
Mouse and touch completion/rewind use one repeat guard. If a model response replaces the
candidate list during a press, releasing that old press cannot select or commit its replacement;
refreshing an unchanged list still permits the click.
On native IBus, **1–6** adopts the current page's full candidate into editable preedit in all three
languages. **Tab / Shift+Tab** navigates, and **PageUp/PageDown** changes pages. **Space** keeps
composing: it adopts an explicitly selected candidate and appends a space, or simply preserves the
literal spelling and adds a separator. **Shift+Space** remains an alias. For example,
`hel` → `2` → Space → `world!` stays one editable `hello world!` draft; only **Enter** or a candidate
left-click commits it. Punctuation does not implicitly end the writing stream.
Use **Alt+0–9** or keypad digits for literal numbers such as `v123` and `2026`; this preserves
Shift+number punctuation (`! @ #`, etc.) and the active keyboard layout. Plain number keys select
while candidates exist; empty slots (including unassigned `0/7/8/9`) do nothing. Without a draft,
digits can start literal input. Neither this mapping nor continuation modifies system lock state.
**Shift+Enter** adopts the current full candidate into preedit without adding a space or committing.
An immediate **Backspace** undoes the replacement and restores the exact previous spelling;
ordinary editing, another selection, reset, language/privacy changes and focus loss clear this
one-step, memory-only undo. AI sentence previews also expand to their full editable text.
Shift, Caps Lock and other non-text keys do not submit pending input. AltGr and system shortcuts
remain separate from candidate selection; password/PIN input still bypasses Suzaku entirely.

Enable **LLM 联想** to enrich the current composition. Offline candidates remain usable
immediately; the local first candidate never changes under Space. The model gets a structured
language ID, raw composition, local conversion, and up to 160 characters committed in the current
focused session. Context is cleared on focus loss and never written to settings. No surrounding
desktop text is collected. Local discovery is the default; cloud input requires an explicit HTTPS
endpoint, model and separate consent. Private input fields never request model candidates.
The former synchronous `IME_NEXT_TOKEN_MODEL_*` environment-variable path has been removed;
use the shared settings and LLM switch below. Panel refreshes never make their own model requests,
and Chinese/Japanese previews do not insert English next-word fillers.
Outside the native mirrored view, sending a standalone Linux panel candidate clears its draft only after
the target acknowledges delivery. Rejected or uncertain output leaves the draft and prior committed
context intact; check the target before an explicit retry, since a lost reply cannot prove non-delivery.

On Linux the model configuration is `$XDG_CONFIG_HOME/suzaku-ime/settings.json`, defaulting to
`~/.config/suzaku-ime/settings.json`. Tray changes create it automatically. Example:

```json
{
  "language": "en",
  "llm_enabled": false,
  "llm_scope": "local",
  "llm_protocol": "auto",
  "llm_endpoint": "http://127.0.0.1:11434/api/chat",
  "llm_model": "auto",
  "llm_api_key_env": null,
  "llm_cloud_consent": false,
  "llm_timeout_ms": 1200,
  "llm_temperature_tenths": 4
}
```

The provider is model-agnostic: model identity, local/cloud scope and API protocol are independent.
Default `auto` discovery prefers an installed **LLaMA** without pinning a particular release or size;
other local model families can also be selected. The native Ollama `/api/chat` integration
uses a bounded typed JSON candidate schema (up to three words and three short sentences),
a 256-token output budget, a 2,048-token context, and a five-minute idle residency.
Existing `/v1/chat/completions` configurations remain compatible, including local llama.cpp servers.
English completions and already-converted CJK phrases keep their local prefix; unrelated output,
unchanged input and pronunciation-only replacements are filtered out. Unknown Pinyin/Romaji can
still be converted by the model without being forced to keep the raw Latin prefix. These guards
do not judge semantic correctness; small-model language quality still needs broader evaluation.
English candidates preserve the exact typed prefix, including indentation, repeated spaces and
literal list markers; response cleanup only removes padding outside that prefix.
English model requests distinguish incomplete-word completion from next-word continuation, and
reject outputs that merely append words to an unfinished fragment. The literal and best offline
English completion keep their positions when AI results arrive; selecting a candidate freezes
the list until the next edit. `suzaku_tool llama probe en` exercises five fixed English examples.
Leave `llm_model` as `auto` to discover local models, or set an **already installed** model explicitly.
Start its service separately. Existing saved model names are preserved, not silently migrated to auto.
Suzaku never downloads a model during typing or starts a model server implicitly.
Choose **重新加载模型配置** after editing. `SUZAKU_IME_CONFIG` can select an isolated settings file
for development. An absent, slow or invalid model response leaves offline candidates available.
Reloading preserves the current IBus composition unless the language changes. Changing the model,
endpoint, protocol, scope or credential reference clears earlier committed context and pending
prediction results, so the new provider starts with only the current draft.
The settings window's **Tone** updates the native Linux host and its saved configuration:
Focused = 0.2, Balanced = 0.4, Expressive = 0.7. Both windows follow acknowledged values;
failed writes roll back, and late replies do not overwrite a newer selection. Reloaded custom
temperatures remain exact and appear as **Custom**, rather than being rounded to a preset.
Tone changes preserve the current composition and do not change the model or language.
The compatible API's line parser preserves decimal/version prefixes such as `3.14` and `1.2.3`.

The tray provides **发现 / 检查本机模型** and **预热本机 Ollama 模型**. Checking only reads metadata;
preheating sends an empty request, not typed text, and runs independently of input-method switching.
The configured service, missing model, timeout, malformed response, and empty candidates are
distinguishable instead of silently appearing as one generic failure. Status reports describe the
last explicit check, not continuous monitoring.

```sh
# Install/start Ollama separately first; keep it local-only (OLLAMA_NO_CLOUD=1).
cargo build --release --all-features --bin suzaku_tool
target/release/suzaku_tool model configure   # local auto; preserves language and opt-in
target/release/suzaku_tool model discover    # metadata only; no downloads or warmup
target/release/suzaku_tool model status
target/release/suzaku_tool model warmup      # Ollama only; empty request, bounded to 30 seconds
target/release/suzaku_tool model probe all   # fixed zh-Hans/en/ja examples, not desktop input
```

`model configure --model NAME --endpoint URL --timeout-ms N` updates only the supplied fields;
no-argument `configure` restores local auto discovery and clears cloud credentials/consent references.
Reload from the tray afterward. `llama` remains a command alias. `probe` preheats explicitly configured
Ollama models and reports local-conversion time, model-request time and actual
candidate text. It does not change the active input method or enable LLM input by itself.

Discovery checks only `127.0.0.1:11434` (Ollama), `127.0.0.1:8080` (llama.cpp) and
`127.0.0.1:1234` (compatible desktop servers), using `/api/tags` or `/v1/models`. A custom
endpoint is checked alone. It uses an 800 ms total metadata budget, caches success for 60 seconds
and failure for 5 seconds, and runs on the prediction worker when first needed, never the UI thread.
Local generation failures reporting a missing model/interface (404) or unavailable service discard
the matching success entry; the next request discovers again, without retrying the failed request.
Timeouts, rate limits and cloud failures do not trigger discovery.
It does not scan files/ports, load/download weights, or fall back to cloud. Ollama entries marked
remote are excluded even when an explicit local model alias is configured. Keep Ollama in local-only
mode as an additional runtime safeguard. The inventory describes the service at check time.

For HTTPS services exposing compatible chat completions, see [model configuration](docs/model-providers.md).
The panel now says **Configured service**, not a fixed Llama preset; `settings.json` is authoritative.

Meta's Llama 3.2 model card does not list Chinese or Japanese among officially supported languages;
these profiles retain deterministic local candidates, and model-quality evaluation is still needed.
See the [Llama 3.2 model card](https://huggingface.co/meta-llama/Llama-3.2-3B-Instruct),
[Ollama chat API](https://docs.ollama.com/api/chat), and
[local-only/preload settings](https://docs.ollama.com/faq).

Romaji spelling conventions were checked against the upstream
[Mozc conversion table](https://github.com/google/mozc/blob/master/src/data/preedit/romanji-hiragana.tsv).
Suzaku's current converter and bootstrap vocabulary are independent, limited implementations.

### IME foundation verification

The Linux build is verified with `cargo test --all-features` and release builds using
`--all-features`. Native IBus checks cover all three languages, AI-candidate selection, private
fields, password/PIN bypass, panel commits, and activation/restoration. Model transport checks
include synthetic loopback HTTP fixtures as well as the local real-model smoke test below.

On 2026-09-08, Ollama 0.33.3 with `llama3.2:3b` (Q4_K_M, approximately 2.02 GB on disk)
was exercised on a Linux machine with an RTX 4050 Laptop GPU (6 GB VRAM). The model used a
2,048-token context and approximately 2.32 GB of VRAM. Initial empty-request loading took about
26 seconds; early generation requests hit the 1.2-second timeout and safely kept local candidates.
After warmup, one fixed `probe all` run measured 523 ms (Chinese), 665 ms (English), and 789 ms
(Japanese), with local conversion taking 14–35 microseconds. These are individual observations,
not latency percentiles, end-to-end keypress timings, or a comprehensive quality benchmark.

An isolated real IBus session also successfully selected and committed AI candidates for `nihao`,
`hello`, and `nihongo`, retained local-only candidates in private fields, bypassed password fields,
and restored its previous engine after each probe. The desktop's active `rime` engine and LLM
opt-in remained unchanged. The current feature-enabled regression suite passes 634 tests, with eight
opt-in tests ignored by default. All seven isolated UI/GPU checks pass separately; the live-desktop IME
activation test is intentionally not run. The local service binds loopback only with cloud use disabled;
model residency expires after five idle minutes.

The Linux blocking-path follow-up uses private IBus/Xvfb sessions and a fixed, non-recursive slow
status-command fixture. In one local run, an idle/partial IPC client increased native key handling
to about 953 ms before the fix and about 1–2 ms afterward. With a 1.2-second status-command fixture,
panel focus/edit/selection handlers returned in about 0.4–0.8 ms, including cache expiry. These are
synthetic regression observations, not latency percentiles or whole-desktop performance claims.
Transport checks also cover fragmented Unicode, exact/oversized frames, trickling clients, focus
changes before a legacy commit, abandoned replies, and bounded pending-client/descriptor cleanup.

The client/tray follow-up also reproduces a full Unix accept queue and inherited stdout without
using the desktop host. The corrected subscription/action/settings/availability calls returned at
about 200/350/700/350 ms, a trickled settings reply stopped at 700 ms, and the tray command that
previously waited three seconds stopped at its two-second limit. Isolated native-sync shutdown
checks cancelled and joined both workers in about 0.6–24 ms, including connected peers withholding
frames/acknowledgements. These timings describe the fixed fixtures, not whole-application shutdown
or general performance guarantees; IME restoration on quit remains a separate bounded operation.

The English-completion follow-up verified native Tab + Space commits for `hel`, `HEL`, `sched`,
`compati` and `don'`, alongside lossless literal commits and Chinese/Japanese privacy checks.
Five fixed English Llama examples returned useful-prefix candidates in 225–461 ms in one warmed
run. This is a transport/interaction smoke test, not a grammatical-accuracy benchmark. GPU readback
also covers English word chips and equal-width collapsed word cards without short-word truncation.

Known repository-wide follow-ups remain: strict `cargo clippy --all-features --all-targets -- -D warnings`
is not clean (including existing GPU/FFI diagnostics), and no-GPU/default-feature builds still have
UI-type/feature coupling. Use the feature-enabled commands above for this native/desktop build.

### Continuous integration

[GitHub Actions CI](https://github.com/chiharu-kiryu/Suzaku-map/actions/workflows/ci.yml)
runs on pushes to `main`, pull requests targeting `main`, and manual dispatches. It uses Rust
1.95.0 and the checked-in lockfile, with read-only repository permissions and pinned action commits.
New commits cancel superseded runs on the same branch or pull request.

- **Rust formatting** checks the entire workspace with `cargo fmt --all --check`.
- **Linux** runs the complete feature-enabled test suite serially, then exercises real IBus
  composition/candidate synchronization in a private D-Bus session. Separate Xvfb processes test
  keyboard focus, mouse/touch candidate completion and async-refresh races, content-fit
  resizing/dragging, Tone acknowledgement/reload, voice/handwriting handoff safety, slow status
  probes, native-worker cancellation, and multilingual GPU readback using Mesa software rendering
  and installed CJK fonts.
  The job also verifies data-folder actions against a private FileManager1 fixture, builds the four
  Linux release binaries, packages `.tar.gz`/`.deb`, checks extracted contents and uploads packages
  plus checksums as 14-day CI artifacts. None of these checks installs a package on the host.
- **macOS and Windows** compile-check GPU-enabled desktop code and test targets on native runners;
  these checks do not claim native input-method or graphical runtime coverage.

The native checks use temporary settings and a deterministic local model fixture. They do not
download Llama, contact a real model, capture the desktop, or change the desktop's input method.
They check actual model-request temperatures, rejected/failed configuration writes, draft/context
boundaries on model reload, and persistence across host restart. Single-instance tests cover
concurrent launches, crash recovery and legacy running panels without opening desktop windows.
Candidate tests also cover exact consecutive commits, standalone delivery acknowledgements,
confirmation gates, and retained drafts after rejection or a lost acknowledgement.
Reproduce them on Linux after installing the packages listed in `.github/workflows/ci.yml`:

```bash
bash scripts/test-linux-ci.sh ibus
LIBGL_ALWAYS_SOFTWARE=1 bash scripts/test-linux-ci.sh ui
```

CI does not yet gate on the known strict-Clippy/default-feature issues above, package Android,
publish releases, or claim real-model quality/latency coverage. A committed workflow is not proof
of a passing run: GitHub Actions must be enabled, and reading private-repository results requires
a GitHub connection authorized for this repository.

### 0.5.0 UI refinements

- Allocate handwriting title, status, guidance, canvas and footer using full text line heights.
  Narrow guidance can wrap onto two rows without moving the canvas when status changes; Undo,
  Clear and candidate chips share a non-overlapping row. Ink is clipped to the canvas after resize.
- Drag from the header, margins and other non-interactive space in the main panel, settings or
  floating icon. Functional hit targets (including their touch padding) take priority; the seed
  text row remains editable, and dragging across a button cannot click it on release. Grab/grabbing
  cursors distinguish window movement from text editing, handwriting and scale/scroll controls.
- Fit the native panel height to the visible input mode and candidates on startup, tab fold/unfold,
  zoom and content changes. Width-driven control scaling avoids height/scale feedback; the panel
  stays top-anchored during resize and no longer has a fixed 1360px content-width cap.
- Deduplicate native resize requests and keep delayed programmatic size acknowledgements out of
  the zoom base, including a quick shrink-and-restore before the first acknowledgement arrives.
  Content fitting respects the monitor height limit; settings keep their independent window.
- Switch minimum/maximum size constraints when entering the 92×92 floating icon, then restore the
  main panel constraints and its previous decoration style when expanding again.
- Render visible Unicode glyphs on demand, with per-character system-font fallback instead of
  replacing Chinese/Japanese candidates with question marks. Repeated frames reuse glyphs and
  GPU coordinates; the atlas has a fixed memory budget and evicts stale glyphs when full.
- Use full-width CJK geometry consistently for candidate labels, wrapping, cursor measurements
  and scrolling. Collapsed CJK candidates share available width so alternate readings stay visible.
- Center keyboard, candidate-chip, and toolbar labels within their button surfaces at every text scale.
- Share one settings layout between standalone and embedded views, with measured option widths and font-aware row heights.
- Keep matching settings expanded during search, show an empty-result hint, and clip scrolling text and click targets to the content viewport.
- Draw tooltips and commit feedback above the full panel scene; delay hover hints until a 600 ms dwell, hide them during input, and keep them inside the window without idle redraw polling.

#### Font compatibility and visual verification

The panel keeps the chosen UI font and falls back per missing character. On Linux, Fontconfig
selects a language-appropriate CJK face (including the face index inside `.ttc` collections);
known Noto/WenQuanYi paths cover systems without Fontconfig. macOS and Windows have native CJK
fallback paths as well; Windows font paths respect `WINDIR`. Only Linux has been exercised on
hardware for this change. A CJK font must be installed (this machine uses Noto Sans CJK SC/JP).
After installing fonts, restart the panel. Unsupported glyphs use a distinct missing-glyph marker,
not literal `?` candidate text; the window title also reports missing coverage. Color-emoji and
complex-script shaping are not provided by this single-glyph renderer.

The opt-in visual test renders synthetic Chinese, English and Japanese candidates using the actual
panel shaders, font atlas and GPU readback, including 2.5× scaling, collapsed/expanded layouts,
bounded-cache eviction and repeated-frame reuse. Handwriting scenes additionally cover 420–2500px
widths at medium/large text sizes. It does not capture the desktop or read typed text:

```bash
cargo test --all-features --offline --bin panel \
  multilingual_candidates_render_through_the_gpu_without_question_mark_fallback -- --ignored --nocapture
# Optional: set SUZAKU_GLYPH_QA_DIR to an output directory to retain synthetic BMP images.
cargo test --all-features --offline -- --test-threads=1
# In an isolated test IBus session with English selected:
target/release/linux_ime_probe --complete hel   # Tab + Space (editable), then Enter; verifies exact text
```

Some existing FFI tests share global theme state and can interfere under parallel test execution;
the serial regression command avoids that interference. This does not remove the repository-wide
strict-Clippy and default-feature limitations described above.

The isolated UI suite also checks voice/handwriting handoff acknowledgements, source retention on
failure and late replies, and Linux's consume-once transcript path with an injected clock. It uses
only synthetic text, with live voice capabilities disabled; no microphone is opened. This validates
panel/bridge integration, not production Linux speech recognition. Text-output tests use a private
Unix socket, and a cross-platform source guard prevents macOS speech logging from being reintroduced.

The Linux native-window regression uses an isolated Xvfb display and temporary settings. It tests
keyboard/voice/handwriting folding, zoom and restore through real window-size events. It also checks
mouse/touch drag routing, control priority, cancellation, releases over other controls, compact
background clicks, and actual X11 window movement. It does not capture the desktop, switch its input
method or inject global input events. The fixed English example goes from 900×487 expanded to
900×165 folded and back without zoom-base drift:

```bash
suzaku_fit_qa=$(mktemp -d /tmp/suzaku-fit-qa.XXXXXX)
env -u WAYLAND_DISPLAY XDG_CONFIG_HOME="$suzaku_fit_qa/config" \
  SUZAKU_IME_CONFIG="$suzaku_fit_qa/ime.json" SUZAKU_PANEL_NATIVE_QA=1 \
  xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
  cargo test --all-features --offline --bin panel \
  native_window_fits_content_through_fold_zoom_and_restore -- --ignored --nocapture
```

Geometry tests also cover empty/non-empty English/CJK candidates, three input modes and scaled
viewports; GPU readback uses the fitted height rather than a fixed large canvas. Only Linux native
window behavior has been exercised on this machine.

The keyboard regression sends X11 key events only to windows it creates on an isolated Xvfb
display. It checks actual focus acquisition after clicking the input field, text/digits/spaces on
all three input tabs, IME preedit/commit routing, focus return, and self-target commit protection:

```bash
suzaku_key_qa=$(mktemp -d /tmp/suzaku-keyboard-qa.XXXXXX)
env -u WAYLAND_DISPLAY XDG_CONFIG_HOME="$suzaku_key_qa/config" \
  SUZAKU_IME_CONFIG="$suzaku_key_qa/ime.json" SUZAKU_PANEL_NATIVE_QA=1 \
  SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus \
  xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
  cargo test --all-features --offline --bin panel \
  native_keyboard_editing_and_focus_return -- --ignored --nocapture
```

### 0.4.9 Highlights

- Added a native Linux IBus engine host, user-service installer, runtime diagnostics, and a
  private panel-to-host commit channel.
- Added a Suzaku system tray with show/hide, settings, position reset, and quit actions, plus
  per-session single-instance activation.
- Refined the Ubuntu / GNOME Wayland panel with non-focusing input, monitor-bound dragging,
  close-to-tray behavior, compact outer spacing, and correctly positioned settings windows.
- Reduced desktop input latency through idle-aware redraw scheduling, reusable GPU buffers,
  cached system-font atlases, displayed-scene hit testing, and width-aware candidate wrapping.
- Improved the Android IME with clipboard privacy controls, batched native render snapshots,
  interruption-safe animations, and frame-aligned handwriting updates.

### 0.4.8 Highlights

- Scaled-aware text rendering pipeline for clearer zoomed desktop UI.
- Performance-safe font atlas rebuild behavior on window scale changes.
- Updated font atlas and sampling defaults to improve readability at larger scales.
- Tunable handwritten-stroke sampling and smoothing via environment variables, enabling faster on-device touch calibration for 0.4.8.
- Added theme presets for desktop panel visuals.
  - Supported values in `panel-settings.toml` (`theme_preset`):
    - `daylight`
    - `device_dark`
    - `solarized`
    - `high_contrast`
  - Default config paths:
    - Linux: `~/.config/suzaku-panel/panel-settings.toml` (or `$XDG_CONFIG_HOME/suzaku-panel/panel-settings.toml`)
    - macOS: `~/Library/Application Support/SuzakuPanel/panel-settings.toml`
    - Windows: `%APPDATA%/SuzakuPanel/panel-settings.toml`

The repository currently contains three layers that evolve together:

- a shared Rust IME core
- platform-specific system IME host adapters
- debug and companion UIs for desktop and Android

The codebase is no longer a single-file prototype. The current shape is:

- Shared engine: [src/ime.rs](src/ime.rs)
- Host session contract: [src/ime_host.rs](src/ime_host.rs)
- Platform adapters: [src/platform](src/platform)
- Desktop GPU companion: [src/bin/panel.rs](src/bin/panel.rs)
- Android IME app: [android](android)

## Architecture

### 1. Shared IME Core

The shared IME core owns:

- seed text normalization
- candidate generation
- candidate selection
- commit flow
- language-plugin dispatch

Key files:

- [src/ime/core_engine.rs](src/ime/core_engine.rs)
- [src/languages/mod.rs](src/languages/mod.rs)
- [src/languages/english.rs](src/languages/english.rs)
- [src/languages/llm.rs](src/languages/llm.rs)
- [src/languages/llama.rs](src/languages/llama.rs)

### 2. System IME Host Layer

The host layer is where platform-native IME lifecycles connect to the shared session.

Core contract:

- [src/ime_host.rs](src/ime_host.rs)

Adapter and lifecycle contract:

- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)

Dispatch and capability map:

- [src/platform/mod.rs](src/platform/mod.rs)
- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)
- [src/platform/ime_host_dispatch.rs](src/platform/ime_host_dispatch.rs)
- [src/platform/ime_host_runtime.rs](src/platform/ime_host_runtime.rs)
- [src/platform/panel_companion_dispatch.rs](src/platform/panel_companion_dispatch.rs)

Current platform direction:

- macOS: InputMethodKit host path is the most complete
- Windows: TSF skeleton is present
- Android: InputMethodService app path is active
- Linux: native IBus engine host is available; Fcitx remains a registration scaffold

### 3. Companion UIs

The project uses two kinds of companion UI:

- desktop GPU debug companion
- native or app-hosted candidate/input surfaces per platform

Desktop GPU scene builders live in:

- [src/ime/gpu](src/ime/gpu)

Companion style lives in:

- [src/platform/companion_style.rs](src/platform/companion_style.rs)

## Repository Layout

### Core Rust

- [src/lib.rs](src/lib.rs)
- [src/ime.rs](src/ime.rs)
- [src/ime_host.rs](src/ime_host.rs)
- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)
- [src/platform/ime_host_runtime.rs](src/platform/ime_host_runtime.rs)
- [src/panel_support.rs](src/panel_support.rs)

### Desktop Panel

- [src/bin/panel.rs](src/bin/panel.rs)
- [src/bin/panel](src/bin/panel)

### macOS

- [src/platform/macos_ime.rs](src/platform/macos_ime.rs)
- [src/macos/ime_host_bridge.m](src/macos/ime_host_bridge.m)
- [src/macos/speech_bridge.m](src/macos/speech_bridge.m)
- [src/macos/text_output_bridge.m](src/macos/text_output_bridge.m)

### Android

- [src/platform/android_ime.rs](src/platform/android_ime.rs)
- [src/platform/android_jni_bridge.rs](src/platform/android_jni_bridge.rs)
- [android/app/src/main/java/dev/suzaku/android/ime](android/app/src/main/java/dev/suzaku/android/ime)

### Concept Notes

The original system notes are still part of the repository and now serve as background design documents:

- [src/00-Map.md](src/00-Map.md)
- [src/01-Signal.md](src/01-Signal.md)
- [src/02-Control.md](src/02-Control.md)
- [src/03-Resilience.md](src/03-Resilience.md)
- [src/04-Coordination.md](src/04-Coordination.md)
- [src/05-Expression.md](src/05-Expression.md)
- [src/06-IME-XR-Tablet.md](src/06-IME-XR-Tablet.md)

## Main Commands

### Rust checks

```bash
cargo check --features gpu
./scripts/test-gpu-smoke.sh
cargo test --features gpu
cargo build --features gpu
cargo build --release --features gpu
cargo run --release --features gpu --bin suzaku-map -- --help
cargo ime-host
```

### Desktop GPU companion

```bash
cargo run --features gpu --bin panel
```

Or use one-command launcher:

```bash
./scripts/panel-gpu.sh
./scripts/panel-gpu.sh release
./scripts/panel-gpu.sh build-release
```

#### Current local priority: Ubuntu / Wayland

The desktop GPU companion is the primary development path on the current workstation. Its event
loop sleeps while the UI is idle, wakes at frame cadence only for active voice/feedback/text-scroll
animation, reuses streaming GPU vertex buffers, and uses the last displayed scene for pointer hit
testing. Installed platform fonts are rasterized into a cached GPU atlas, with the built-in bitmap
font retained only as a startup fallback. On GNOME Wayland with XWayland available, the main panel
automatically uses a non-focusing X11 override-redirect window. It opens at the bottom center,
continues to accept pointer/touch input, and leaves IBus focus in the target application while
candidates are clicked. The top-center grip provides manual drag handling for this unmanaged window,
keeps the panel inside the active monitor, and preserves its expanded position. The default 100%
layout uses a compact outer gutter, a dedicated close-to-tray button, and readable two-column
candidates with an unpaired candidate spanning the final row. A native StatusNotifierItem with a
Suzaku red-and-gold icon remains available after the panel is hidden: click it to show or hide the
panel, or use its menu to activate/release the input method, open settings, reset the window
position, and quit the process. Activating Suzaku hides the large companion and remembers the
previous IBus engine; native candidates appear only while composing in the target app. Releasing
or normally quitting restores the previous engine, unless the user has already switched to another
input method. A failed restore keeps the tray open with a retryable error. The
settings window is placed above the main panel and clamped to the active monitor. Tray-state updates
run off the UI thread. Launching the panel again signals the existing process to show its window
rather than creating duplicate windows or tray icons. Set
`SUZAKU_LINUX_PANEL_BACKEND=wayland` to force the native Wayland window path.
Cross-platform adapters remain in the tree, but local desktop behavior is validated first.

### GPU smoke test profile

The GPU smoke suite includes deterministic loopback HTTP fixtures. No downloaded model is required.

```bash
# Local validation (default)
cargo test-gpu-smoke
./scripts/test-gpu-smoke.sh
```

### Integration validation (local-loopback llama bridge required)

```bash
cargo test-gpu-smoke-llm
./scripts/test-gpu-smoke.sh llm-bridge
```

### Common build note (important)

The root crate is feature-gated for GPU components.
Builds without `--features gpu` may fail with errors about missing `winit` or `ime::gpu` items.
For desktop companion/scene/debug work, prefer:

```bash
cargo build --features gpu
cargo run --features gpu --bin panel
cargo run --release --features gpu --bin suzaku-map -- --help
```

Shortcuts from Cargo aliases:

```bash
cargo panel-macos
# GPU-focused tests:
cargo test-gpu                 # raw GPU test run (no --nocapture)
cargo test-gpu-smoke           # local smoke run (bridge test skipped unless enabled)
cargo test-gpu-smoke-llm       # includes a deterministic local HTTP fixture, not a live model
```

### macOS panel bundle

```bash
cargo panel-app-macos
cargo open-panel-app-macos
cargo install-panel-app-macos
cargo install-open-panel-app-macos
```

### macOS IME bundle

```bash
cargo ime-app-macos
cargo open-ime-app-macos
cargo install-ime-app-macos
cargo install-open-ime-app-macos
cargo enable-ime-app-macos
cargo install-enable-ime-app-macos
```

### Android

Environment check:

```bash
cargo android-doctor
```

Linux registration helper:

```bash
cargo linux-register install
cargo linux-register status
cargo linux-register uninstall
cargo linux-register verify
cargo linux-register diag
```

Rust-side host bootstrap:

```bash
cargo ime-host
cargo android-ime-host
```

Native helper actions (Rust-native runner):

```bash
eval "$(cargo android-env)"
cargo android-build-native
cd android && ./gradlew assembleDebug
cargo android-install-debug
cargo android-enable-ime
```

### Handwriting tuning (desktop panel)

The desktop panel supports runtime tuning for touch/mouse handwriting capture through environment variables (set before launch):

- `SUZAKU_HANDWRITING_TOUCH_SAMPLE_MS` (default `8`) – minimum touch sample interval, milliseconds.
- `SUZAKU_HANDWRITING_MOUSE_SAMPLE_MS` (default `4`) – minimum mouse sample interval, milliseconds.
- `SUZAKU_HANDWRITING_EDGE_PADDING` (default `4.0`) – in-canvas inset for clamped stroke coordinates.
- `SUZAKU_HANDWRITING_MIN_POINT_DISTANCE` (default `1.2`) – base minimum point distance.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP` (default `3.2`) – base interpolation segment size.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA` (default `0.22`) – base smoothing amount.
- `SUZAKU_HANDWRITING_SPEED_REFERENCE` (default `16.0`) – speed reference for adaptive profile.
- `SUZAKU_HANDWRITING_SPEED_RATIO_MIN` (default `0.55`) – minimum speed ratio.
- `SUZAKU_HANDWRITING_SPEED_RATIO_MAX` (default `2.0`) – maximum speed ratio.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP_MIN` (default `2.0`) – lower clamp for interpolation step.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP_MAX` (default `6.8`) – upper clamp for interpolation step.
- `SUZAKU_HANDWRITING_MIN_DISTANCE_MIN` (default `0.78`) – lower clamp for min distance.
- `SUZAKU_HANDWRITING_MIN_DISTANCE_MAX` (default `2.0`) – upper clamp for min distance.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA_MIN` (default `0.14`) – lower smoothing clamp.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA_MAX` (default `0.34`) – upper smoothing clamp.
- `SUZAKU_HANDWRITING_TOUCH_SMOOTH_FACTOR` (default `1.05`) – extra touch smoothing multiplier.
- `SUZAKU_HANDWRITING_TOUCH_MIN_DISTANCE_SCALE` (default `1.18`) – touch distance scale.
- `SUZAKU_HANDWRITING_TOUCH_INTERPOLATION_SCALE` (default `0.9`) – touch interpolation scale.
- `SUZAKU_HANDWRITING_TOUCH_SAMPLE_DISTANCE_SCALE` (default `0.85`) – touch-specific distance scale.

Example:

```bash
SUZAKU_HANDWRITING_TOUCH_SAMPLE_MS=6 SUZAKU_HANDWRITING_TOUCH_MIN_DISTANCE_SCALE=1.2 cargo run --features gpu --bin panel
```

Recommended default entry (no script dependency):

```bash
eval "$(cargo android-env)"
cargo android-build-native
cargo android-install-debug
cargo android-enable-ime
cargo linux-register install
```

APK output (generated locally after an Android build, not tracked in the repository):

`android/app/build/outputs/apk/debug/app-debug.apk`

## Current Platform Status

### macOS

Most complete desktop path today.

Available:

- GPU panel companion
- app bundle packaging
- IMK host bootstrap
- marked text / commit roundtrip skeleton
- native candidate companion window path
- voice bridge

### Windows

Scaffolded:

- platform capability map
- IME host skeleton
- voice backend skeleton

### Android

Active app-host path.

Available:

- InputMethodService app
- shared HostImeSession integration
- adaptive letter, number, and symbol keyboard layouts
- one-shot Shift, Caps Lock, editor actions, and Unicode-safe repeating backspace
- context-aware email/URI shortcuts, dedicated numeric/phone/date-time pads, and action labels
- system keyboard switching plus candidate-boundary spaces, punctuation, and Enter actions
- app-provided Android completions and stale-composition cancellation on cursor movement
- transient clipboard paste drawer with sensitive-preview protection and no saved history
- persistent Android controls for auto-capitalization, a dedicated number row, and haptics
- a setup/settings hub for enabling Suzaku, choosing it, sharing preferences, and diagnostics
- keyboard, voice, and handwrite drawers
- secure password-field input with candidates, voice, and handwriting disabled
- compact-bubble-first activation flow
- in-panel candidate strip
- batched native render snapshots, reused keyboard/candidate views, interruption-safe panel
  animation, and frame-aligned handwriting redraws for a smoother input loop

### Linux

Current workstation-first path: the Rust GPU companion and native IBus host on Ubuntu / GNOME
Wayland.

- event-driven desktop redraw with idle suspension and bounded animation wakeups
- reusable GPU vertex buffers and displayed-scene pointer hit testing
- platform-font GPU atlas rendering with a deterministic bitmap fallback
- settings-window synchronization without redraw loops or repeated idle config writes
- bottom-centered, pointer-active, non-focusing XWayland panel mode on GNOME Wayland, with an
  explicit native-Wayland override, visible drag grip, and monitor-bound position clamping
- explicit click-to-type focus for the panel input field, independent of keyboard/voice/handwriting
  tabs; regular text, digits, repeat keys and single spaces take priority over panel commands
- OS IME preedit/commit input for Chinese, Japanese and accented text; preedit stays a display-only
  preview until committed. The event lifecycle follows the [winit IME contract](https://docs.rs/winit/0.30.12/winit/event/enum.Ime.html).
- Enter / Esc finishes panel editing and restores its borrowed X11 focus if the user has not
  already switched apps; show, drag and candidate clicks never request editing focus. On native
  Wayland and other platforms, focus activation/restoration remains subject to the OS/window manager.
  Before sending a candidate, focus the target app; Linux rejects commits into the panel itself.
- StatusNotifierItem system tray with a built-in Suzaku icon, show/hide activation, position reset,
  direct settings access, close-to-tray behavior, and an explicit quit action
- per-session single-instance control that reopens the existing panel on a repeated launch;
  a process-lifetime OS lock serializes startup and releases on crash. The empty `panel.lock`
  file is intentionally retained after clean shutdown so contenders always lock the same file.
- Ubuntu / Arch / SteamOS capability profiles
- Linux voice backend and probe path
- native IBus `Factory`/`Engine` host backed by the shared Rust candidate engine
- engine-owned IBus drafts shown in the candidate area's auxiliary line by default, with six
  visible candidate rows, mixed word/sentence ranking, and bounded annotated previews;
  arrow/Tab navigation, paging, 1–6 editable selection in all three languages, Alt+digits
  literal draft input, candidate clicks, Space continuation, and explicit commit
- tray-controlled IBus activation/restoration with verified switches, bounded off-thread host I/O,
  on-menu-open status refresh, and no global keyboard hook or input polling
- native roundtrip probe coverage for input-triggered preedit/candidates, final commit, and dismissal
- local user-only panel-to-IBus commit channel at `$XDG_RUNTIME_DIR/suzaku-ime/host.sock`
- live native-to-panel composition subscription: preedit, exact candidate labels/text, selection,
  language and asynchronous AI updates come from the native host rather than a second LLM request
- revision-checked panel candidate commits and word/on-screen completion edits; stale clicks, old
  host instances and private contexts are rejected without retrying or falling back to a new target
- truthful runtime registration status:
  - bootstrap reads `SUZAKU_LINUX_IME_FRAMEWORK=fcitx` to switch to Fcitx checks
  - a component marker alone does not imply marked-text or commit readiness
  - IBus static and dynamically registered engines are probed separately from daemon and host state
  - quick local override for staging: `SUZAKU_LINUX_IME_REGISTERED=1`
  - lifecycle simulation overrides for host checks:
    - `SUZAKU_LINUX_IME_DAEMON_READY=1|0`
    - `SUZAKU_LINUX_IME_RUNTIME_VISIBLE=1|0`
    - `SUZAKU_LINUX_IME_ACTIVE=1|0`
    - `SUZAKU_LINUX_IME_HOST_READY=1|0`
    - `SUZAKU_LINUX_IME_MARKED_TEXT=1|0`
    - `SUZAKU_LINUX_IME_COMMIT=1|0`
    - `SUZAKU_LINUX_IME_NATIVE_CANDIDATE_WINDOW=1|0`

Build and install the native host without root:

```bash
# Normally install the build headers once through your distribution:
# sudo apt install libibus-1.0-dev
cargo build --release --features linux-ibus --bin linux_ime_host --bin suzaku_tool
SUZAKU_LINUX_IME_HOST_BIN="$PWD/target/release/linux_ime_host" \
  target/release/suzaku_tool linux-register install
target/release/suzaku_tool linux-register verify
```

The installer copies the host to `~/.local/libexec/suzaku/linux_ime_host`, enables
`~/.config/systemd/user/suzaku-ibus.service`, dynamically registers
`dev.suzaku.linux.ime` with the running IBus daemon, and safely appends Suzaku to GNOME's input
source switcher when that setting is available. During a host upgrade it records and restores
the active IBus engine, so restarting the service does not leave the desktop on an empty engine.
After installation, launching `panel` ensures the registered user service is ready without
changing the selected input method. Right-click its tray icon:

- **激活 Suzaku 输入法** first starts a stopped host, then selects the native engine and hides the large panel. Focus a text field
  and type to summon the cursor-anchored candidate window; on the non-focusing Linux backend the
  companion also reappears as a collapsed candidate view. No held shortcut is required.
- **释放并恢复：…** restores the input method used just before activation. The menu shows that
  actual engine, not a guessed English/default layout. Repeated activation preserves it.
- **退出 Suzaku** (or the full quit shortcut) restores the observed previous input method, stops
  the managed host service, then exits. Restore/stop failures leave the UI available for retry.
  Hiding the panel or closing it to the tray leaves input working. A manually selected different
  engine is never overwritten on release. The release action alone keeps the host ready for reuse.

This control currently targets Linux / IBus. It does not install global shortcuts, change the
configured input-source list, or automatically take over at startup. If Suzaku was already selected
outside this tray session, switch back through the system input-source menu; there is no known
previous engine to restore. Private/custom host endpoints remain owned by their original launcher,
not by the desktop service controller. Forced process termination is not a normal tray quit and cannot run
the session's restore step.

Native companion synchronization uses a persistent `W` subscription on the existing user-only
host socket. Frames carry a host-instance UUID, focus-context ID and monotonic revision. The host
pushes on input, selection, prediction, commit/cancel and focus/privacy changes; the panel queues
only the newest frame and does not run its own predictor while mirroring. Slow readers are
disconnected using nonblocking writes, and a restarted host supplies a fresh snapshot on reconnect.
The settings/status (`S`) channel remains text-free.

Clicking a mirrored candidate submits that exact native candidate through a revision-checked `A`
action; word chips and the on-screen keyboard update native preedit through the same channel.
There is no automatic retry or fallback commit when focus/content/selection changes. A candidate
press is cancelled if its displayed revision changes before release. Directly clicking the seed
input returns to the independent panel draft and the normal click-to-type behavior.

Password/PIN and application-marked private contexts send empty frames, never text or candidates;
surrounding text and committed history are never included. Privacy detection depends on the client
reporting its IBus content purpose/hints. Automatically summoned views hide after completion or
focus loss; an explicitly shown panel stays open but clears native text. Manually hiding a native
view suppresses automatic reopening for that focus context. This transport is implemented for
Linux/IBus; other platform hosts still need their own native subscription adapters.

The real native sync test runs its own D-Bus and IBus daemon, creates synthetic input contexts,
and uses a deterministic loopback model fixture (not the user's model/settings or desktop):

```bash
bash scripts/test-linux-ci.sh ibus
```

This checks full candidate/selection equality, late subscriptions, panel commits and preedit edits,
stale actions across input contexts and host restarts, asynchronous AI updates, English/Chinese/
Japanese, privacy, Escape and focus-out. Direct engine peers also check reordered focus notifications,
late key/navigation/click events, field-context isolation and primary-click selection on both pages.
The runner builds and locates the activation test executable, verifies the test exists, and exercises
the tray controller's real activation/input/release path in all three languages, including cleanup
restoration. Special/invalid configuration reloads must leave both native keys and the host responsive.
The Xvfb keyboard regression additionally exercises the panel's native-view routing, independent draft
preservation, and cancellation of outdated presses.

You can also use `Super+Space` to select **Suzaku**, type a Latin seed, use arrows or Tab to
move through candidates, `Page Up` / `Page Down` to change pages, and `1`–`6` to adopt a candidate
without committing. Space and punctuation continue the draft; Enter or a left click submits it.
Without an active draft, ordinary digits go directly to the application. Alt+digits explicitly
enters literal numbers into the draft. Escape cancels the current draft.

The 0.6.3 IBus presentation places each superscript shortcut inside the candidate's text
hit target and leaves the separate shortcut label empty. Clicking the visible ordinal therefore
uses the same correct row index as clicking its text, including on later pages; commit payloads
and number-key adoption are unchanged. See the [ordinal compatibility fix](docs/bug-audit-ordinal-click-2026-09-23.md).

Unconfirmed text appears in the candidate area/companion panel, not in the application's inline
preedit buffer. This prevents clients such as Qt and Chrome from committing cached draft text
when focus changes. The 0.6.3 candidate-area preview keeps a suffix within 48 display cells
(wide characters count as two, zero-width marks as zero), capped at 160 Unicode code points.
This limits unwrapped native labels, not the draft or committed text; see the
[native popup audit](docs/bug-audit-native-popup-2026-09-23.md). Advanced users may start the host with
`SUZAKU_IBUS_INLINE_PREEDIT=1` to restore inline preedit, but that also restores the risk of
client-initiated commits on focus changes. See the
[cross-application fix audit](docs/bug-audit-cross-app-fixes-2026-09-23.md) for tested boundaries.

`linux-register uninstall`
removes Suzaku from the GNOME switcher, disables the user service, and removes the installed host
and component metadata.

Useful commands:

- `cargo linux-register install`
- `cargo linux-register status`
- `cargo linux-register uninstall`
- `cargo linux-register verify`
- `cargo linux-register diag`
- `cargo run --features linux-ibus --bin linux_ime_probe -- ni`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --ipc "panel commit probe"`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --llm nihao`
  (requires an enabled, running local model; waits for an AI candidate and selects it)
- `cargo run --features linux-ibus --bin linux_ime_probe -- --private nihao`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --password nihao`
- `bash scripts/test-linux-ci.sh ibus` (private activation/input/release and native regression suite)
  (opt-in live-desktop check: activates, types in an isolated input context, then restores)

Framework selection is shared with bootstrap:

- `SUZAKU_LINUX_IME_FRAMEWORK=fcitx` to register and check Fcitx layout
- default remains IBus when not set

Fcitx currently retains marker/status support; a native Fcitx engine service is still pending.

## Optional bottom panel layout (unreleased)

Display settings persist `panel_layout_mode=auto|follow-caret|bottom-dock`; missing or invalid
values default to Auto. Existing explicit values remain manual overrides. Backup validation
accepts all three preferences, never the machine-specific `detected_panel_layout` runtime field.
`PanelChromeState::effective_panel_layout()` resolves Auto before rendering, positioning and
native input lifecycle decisions; settings windows inherit the main window's cached detection.
The automatic option displays the detected choice even when another layout is selected.

`src/platform/panel_layout.rs` performs a bounded, local startup probe: Android prefers bottom;
desktop/unknown operating systems prefer caret following. Linux recognizes exact mobile/gaming
session tokens, then valid `/etc/machine-info` `CHASSIS`, DMI chassis type, and DeviceTree chassis
type in that order. A configured desktop/laptop classification overrides contradictory hardware
fallbacks. Unknown metadata, missing/unreadable files, convertibles and detachable devices without
a known posture remain caret-following. Mere touchscreen presence, display size, distribution,
or `XDG_CURRENT_DESKTOP=gamescope` from a nested game do not trigger the bottom layout.
The conservative chassis categories follow the [systemd hostnamed mapping](https://github.com/systemd/systemd/blob/main/src/hostname/hostnamed.c).
No device event stream, device identifier, external command or network request is used. Detection
is not live posture/hot-plug monitoring and does not qualify native mobile window integration.

The GPU layout is separate from Linux caret tracking: `PanelLayoutMode` changes scene ordering
and touch target sizes, while `src/bin/panel/bottom_layout.rs` owns monitor bounds, size/position
requests and restoring the previous floating width. Bottom dock is an overlay, not a reserved
desktop work area or a native Wayland input-method surface.

An expanded native keyboard may remain after submission only for the already-woken public
host/context. Context changes, privacy, focus loss, disconnect and local editing clear that
retention; empty fields alone do not authorize it. Asynchronous window growth waits for a
size acknowledgement before drawing the enlarged page, with the existing bounded resize
deadline preventing a permanent wait on a clamping window manager.

`bash scripts/test-linux-apps.sh bottom-layout` exercises actual settings clicks and restart
persistence, all native candidate slots, pagination, screen-key continuation after commit,
orb restoration, folding and switching back to caret following on an owned 1024×768 Xvfb
display. Run both the default and `SUZAKU_GTK3_QA_SYNC_MODE=1` transports; screenshots/logs are
retained with `SUZAKU_APP_QA_KEEP=1`. `candidates` remains the separate full caret-follow gate.
`auto-layout` starts without saved settings in a private Phosh-labelled session and uses the
production detector. Its 36 workflows include actual manual override, restarting with that
override, restoring Auto, restarting again, and the shared bottom-layout input checks.
Each layout now includes three private offline-pack byte-budget workflows: Unicode
single-entry overflow, aggregate-frame overflow and JSON escaping. They install with the
adjacent `suzaku_tool` before host startup, then verify real drawing, number adoption,
Backspace undo and exact card-click submission. Follow/bottom/auto counts are
`T + 20` / `T + 29` / `T + 32`, where `T` is the complete observed English card
count. The earlier 26/33/36 checkpoint used six cards; 0.8.6's installed default
follow-caret check has ten cards and 30 workflows. These numbers do not establish
new bottom/auto acceptance; the counts below remain historical validation records.
Both transports of all three suites are included in the Linux CI workflow. This does not qualify
physical touch, GNOME Shell/native Wayland positioning, or install the changed binaries into a
desktop session.

Local validation on 2026-10-03 passed the all-features regression, final panel regression,
strict Clippy, ShellCheck, 17 private native UI checks, and both IBus transports for all
30 bottom-layout plus 23 follow-caret workflows. First failures exposed hidden-window
fixture clicks, overlapping collapsed controls, an over-constrained vertical hit tolerance,
and candidate rendering before the resize acknowledgement; the fixes retain the original
timeouts and assertions. The follow-restore fixture now moves to a genuinely different
vertical caret anchor rather than requiring movement from an already edge-clamped position.

The automatic-layout follow-up passed 1,079 non-ignored all-features tests, strict Clippy,
ShellCheck, the 17 opt-in native UI checks, both transports of the 33 Auto and 23 caret-follow
workflows, and the 30 manual-bottom workflows with the default transport. The desktop fixture
resolved this machine to `follow-caret` via `linux-dmi-non-mobile`; the private Phosh fixture
resolved to `bottom-dock` while preserving explicit overrides across process restarts.
An extra `cargo check --locked --no-default-features --all-targets` did not pass: unchanged
core icon/platform modules still import GPU types and `winit` without feature guards. This
existing feature-boundary issue is not repaired by the layout work; the validated build uses
`--all-features`. Its first failure is retained separately from the passing regression logs.

## Refactor Policy

This repository is intentionally being kept in a refactor-friendly early state.

Current codebase rules that the project is trying to hold:

- keep modules small enough to reason about
- separate shared IME logic from platform lifecycles
- separate system-host code from debug companion UI
- let README describe the current codebase, not the historical prototype

## Current Refactor Snapshot

This repository was recently cleaned up in two important ways:

- the GPU scene builder no longer carries the old unused `*.inc` and `*_macro` fragments
- the top-level README now describes the current architecture instead of the original single-file prototype story

That makes the current structure a better base for continued platform work, especially Android and system-host integration.
