# Privacy boundaries

An active input method necessarily processes keystrokes, preedit and selected candidates.
These are implementation boundaries, not a security certification.

## Local data

Drafts, candidate lists, translations, handwriting/voice drafts and short committed context remain
in memory. Suzaku does not persist verbatim input history, but the candidate preferences described
below are sensitive learned data. Backups include recognized settings and key-variable
names, not key values, drafts, model weights or transcripts. Backups are unencrypted and may reveal
provider addresses and preferences; review before sharing. See [data management](linux-packaging-data.md).

Optional offline packs are explicitly imported vocabulary, not learned input history. Their manifests
and text persist unencrypted in the user's `suzaku/lexicons/registry.json` data directory, outside
settings backups. Pack tools do not collect drafts or access model services; example collections are
authored by the project. Third-party content/author/license claims are unverified: review packs and
never include private records in packages you distribute. Pack validation is not signature verification.
Once a pack's suggestion is adopted into an ordinary draft, existing opt-in provider rules apply to
that draft exactly as for manually typed text; importing vocabulary does not grant cloud consent.

Since 0.8.8, the Linux/IBus host persists candidate preferences by default, separately from settings
and dictionaries. At most 2,048 language/query/candidate fingerprints and decaying counts are saved
in a versioned file capped at 512 KiB, together with a save timestamp and the fingerprint key.
**The key is in the same file: HMAC fingerprints are not encryption or anonymization.** Someone who
can read it can test guesses of common input. The file contains no draft text, pending adoptions or
undo history, but must still be treated as sensitive data. Counts decay with a 24-hour half-life,
including time while the host is stopped. The cache only reorders existing suggestions; it neither
generates text nor sends its contents to a model.

The default file is `$XDG_STATE_HOME/suzaku-ime/preferences-v1.json`, or
`$HOME/.local/state/suzaku-ime/preferences-v1.json` when unset. An absolute `SUZAKU_IME_CONFIG`
instead places it in `candidate-preferences/preferences-v1.json` next to that configuration.
Linux storage requires owned private directories and single-link regular files (new directories
0700, files 0600); visible symlinks and unsafe permissions are rejected. A bounded background worker
owns file I/O. Invalid files, contention, startup timeout or detected external changes are reported
without silently replacing the existing file; basic input can continue with memory-only learning.
Normal exit allows a bounded save, but a crash, OOM or stalled filesystem can lose recent changes.
Interrupted saves may leave private `.suzaku-preferences-<pid>-<sequence>.tmp` snapshots; a later
disk-mode start or explicit clear validates and removes that bounded staging set without restoring it.

Private fields neither read nor update preferences. Pending adoptions are discarded on cancellation,
editing away the chosen prefix, focus/language/provider/privacy boundaries, or cache clear.
A number/Shift+Enter/Space adoption is not learned until its text survives to a successful engine
commit. Native delivery still uses IBus's normal commit signal, not an application-level receipt.
Unhinted masked fields have the same limitations described below.

The tray's **Clear candidate preferences (memory + local file)** action and
`suzaku-tool preferences clear` discard confirmed and pending learning without changing a live
selection. Completion is reported only after file deletion and parent-directory synchronization;
an accepted request is not yet completion. Timeout/disconnection means unconfirmed: refresh the
status, rather than assuming deletion or repeatedly clearing. A refresh must match the same host
and clear operation; failures remain visible. Explicit clear can remove a corrupt but otherwise
valid private file; unsafe permissions, a competing writer or an external conflict can prevent it.
Older queued saves cannot recreate a cleared file, but later confirmed choices can start learning
again. Deletion is not secure erasure and cannot remove copies in filesystem snapshots or backups.

Set `SUZAKU_PREFERENCE_MODE=memory` in the host's launch environment before startup to disable
preference file reads/writes while retaining session-only learning. This does not delete an existing
file, and the tray's **Clear candidate preferences (memory only)** action in that mode clears only
RAM. To remove the local file, use the confirmed disk-mode clear before switching modes or uninstalling.
Preferences are excluded from Suzaku's settings backup/restore and exports, and upgrades and uninstall
do not delete them. Other backup tools may still copy the state directory. See
[data management](linux-packaging-data.md#候选偏好文件) for paths and controls.

The native companion channel contains active preedit/candidates, not surrounding application text
or accumulated committed history. Password/PIN and declared numeric/decimal/phone fields bypass
composition and panel injection. Private hints suppress model requests and companion snapshots.
Applications must report purposes/hints correctly; switch input methods when unsure about a field.
For example, local testing of Zenity 4.0.1 `--entry --hide-text` found that it masks text visually
but reports an ordinary unhinted field to IBus, unlike its `--password` dialog. Visual masking
alone does not guarantee private handling; disabling the model also does not hide local companion
drafts. See the [GTK application audit](bug-audit-gtk-apps-2026-09-23.md).
The tested Chrome 153 X11/GTK path and Qt 5.15.13/6.4.2 IBus plugins also do not convey numeric
field purposes. 0.6.2 passes digits directly while the draft is empty, without
creating candidates; this does not mark the whole field private. Letters, explicit Alt+digits
and panel actions can still use ordinary-field routing when purposes are missing. Standard
password keyboard paths passed isolated tests, but this does not qualify arbitrary masked
controls or companion injection. See the [compatibility fixes](bug-audit-cross-app-fixes-2026-09-23.md).

0.6.1 also rejects the independent panel's direct text sends into PRIVATE
fields, while keeping native local conversion available. Changing the active field's private/bypass
policy invalidates its old target identity, including delayed direct sends; a brief round trip back
to public input does not revive them. Ordinary hint updates and stale events from another engine
do not change the current target. See the [content-type audit](bug-audit-content-type-2026-09-23.md).
0.6.1 also invalidates old targets after a successful input-language change, even
when intermediate language snapshots are coalesced away. Same-language aliases, provider-only
reloads and failed changes keep the target. See the [language-target audit](bug-audit-language-target-2026-09-23.md).

Native focus/language/privacy changes clear session context and pending keyboard replay. Uncertain
sends are never retried automatically. Source/recovery text can remain visible until explicitly
handled, its native context changes or the process ends.

## Optional providers

Suggestions require opt-in. Requests include language, raw draft, local conversion and up to 160
characters of committed context within the current focus session. Explicit translation uses the
source draft and language choices. Providers may log, retain or forward data under their own policies,
including through a local gateway.

Local discovery checks fixed loopback endpoints without downloading models, reading model files or
scanning the network. Cloud use requires an explicit HTTPS endpoint/model and consent. Credentials
come from named process environment variables; never put values in URLs, settings or CLI arguments.
Changing provider identity through the CLI or restoring a backup revokes cloud consent. Disabling
requests cannot retract data already sent. See [provider configuration](model-providers.md).

## Reports

Use synthetic text. Review screenshots, paths, usernames, logs and settings before sharing.
Automated fixtures use synthetic inputs and private sessions. A model probe can send synthetic
samples to the configured provider and incur charges. Follow [SECURITY.md](../SECURITY.md) for
security reports, without putting passwords, tokens or real drafts in public issues.
