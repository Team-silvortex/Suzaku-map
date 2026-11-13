//! Text-field input is separate from panel commands and input-mode tabs.
use super::{PanelState, PanelWindowKind};
use std::collections::BTreeSet;
use suzaku_map::ime::gpu::{InteractionKind, PanelChromeState};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::Ime,
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
};

#[derive(Default)]
pub(super) struct TextInputState {
    allowed: bool,
    preedit: String,
    cursor: usize,
    cursor_area: Option<[f32; 4]>,
    /// Consume every recorded known key until its own real release, even after
    /// saving, cancelling or releasing modifiers. KeyCode is a finite set; native
    /// unidentified codes are never retained. IME cleanup must not clear holds.
    recording_holds: BTreeSet<KeyCode>,
}

impl TextInputState {
    pub(super) fn composing(&self) -> bool {
        !self.preedit.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.preedit.clear();
        self.cursor = 0;
    }

    pub(super) fn clear_shortcut_hold(&mut self) {
        self.recording_holds.clear();
    }

    pub(super) fn hold_recorded_key(&mut self, key: PhysicalKey) {
        if let PhysicalKey::Code(code) = key
            && !matches!(
                code,
                KeyCode::ControlLeft
                    | KeyCode::ControlRight
                    | KeyCode::AltLeft
                    | KeyCode::AltRight
                    | KeyCode::ShiftLeft
                    | KeyCode::ShiftRight
                    | KeyCode::SuperLeft
                    | KeyCode::SuperRight
            )
        {
            self.recording_holds.insert(code);
        }
    }

    pub(super) fn consume_recorded_hold(&mut self, key: PhysicalKey, pressed: bool) -> bool {
        let PhysicalKey::Code(code) = key else {
            return false;
        };
        if pressed {
            self.recording_holds.contains(&code)
        } else {
            self.recording_holds.remove(&code)
        }
    }

    fn set_preedit(&mut self, text: String, cursor: Option<(usize, usize)>) {
        // winit supplies a byte offset, while PanelChromeState uses characters.
        let byte_offset = cursor
            .map_or(text.len(), |(start, _)| start)
            .min(text.len());
        self.cursor = text
            .char_indices()
            .take_while(|(offset, _)| *offset < byte_offset)
            .count();
        self.preedit = text;
    }

    pub(super) fn preview(&self, chrome: &mut PanelChromeState, settings: bool) {
        if !self.composing() {
            return;
        }
        if settings {
            chrome.settings_search_query.push_str(&self.preedit);
        } else if chrome.input_focused {
            let start = chrome.caret_index.min(chrome.seed_text.chars().count());
            chrome.insert_text(&self.preedit);
            chrome.caret_index = start + self.cursor;
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum EditAction<'a> {
    Insert(&'a str),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Finish,
    Candidates,
    Fold,
    Ignore,
}

pub(super) fn text_modifiers_allowed(modifiers: ModifiersState) -> bool {
    !modifiers.super_key()
        && (!modifiers.control_key() || modifiers.alt_key()) // Windows/Linux AltGr
        && (!modifiers.alt_key() || modifiers.control_key() || cfg!(target_os = "macos"))
}

pub(super) fn primary_shortcut_modifier(modifiers: ModifiersState) -> bool {
    !modifiers.alt_key() && (modifiers.control_key() || modifiers.super_key())
}

/// Recording owns every non-synthetic pressed key before panel commands or editors.
/// Modifier-only presses and repeats are consumed without attempting to save them.
pub(super) fn capture_shortcut_recording(
    chrome: &mut PanelChromeState,
    key: &Key,
    modifiers: ModifiersState,
    repeat: bool,
    focused_settings: bool,
    composing: bool,
) -> bool {
    if chrome.shortcut_recording.is_none() {
        return false;
    }
    if !focused_settings {
        chrome.cancel_shortcut_recording();
        return false;
    }
    if composing || matches!(key, Key::Named(NamedKey::Escape)) {
        chrome.cancel_shortcut_recording();
        return true;
    }
    if repeat
        || matches!(
            key,
            Key::Named(
                NamedKey::Control
                    | NamedKey::Alt
                    | NamedKey::AltGraph
                    | NamedKey::Shift
                    | NamedKey::Super
            )
        )
    {
        return true;
    }
    let key = match key {
        Key::Named(NamedKey::Space) => u32::from(' '),
        Key::Character(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => u32::from(ch),
                _ => 0,
            }
        }
        _ => 0,
    };
    // Preserve unsupported Super explicitly; never sanitize AltGr into Ctrl or Alt.
    let modifiers = u32::from(modifiers.control_key())
        | (u32::from(modifiers.alt_key()) << 1)
        | (u32::from(modifiers.shift_key()) << 2)
        | (u32::from(modifiers.super_key()) << 3);
    chrome.record_shortcut_key(key, modifiers);
    true
}

/// Focus-local commands only; never run while an IME owns a preedit.
pub(super) fn settings_shortcut(
    key: &Key,
    modifiers: ModifiersState,
    repeat: bool,
) -> Option<bool> {
    if repeat {
        return None;
    }
    match key {
        Key::Named(NamedKey::F1) if modifiers.is_empty() => Some(true),
        Key::Character(text)
            if text == ","
                && (modifiers == ModifiersState::CONTROL
                    || (cfg!(target_os = "macos") && modifiers == ModifiersState::SUPER)) =>
        {
            Some(false)
        }
        _ => None,
    }
}

fn edit_action<'a>(
    key: &Key,
    text: Option<&'a str>,
    modifiers: ModifiersState,
    repeat: bool,
    composing: bool,
) -> EditAction<'a> {
    if composing || !text_modifiers_allowed(modifiers) {
        return EditAction::Ignore;
    }
    // Modified navigation belongs to the OS/editor, never a panel command.
    if modifiers.control_key() || modifiers.alt_key() || modifiers.super_key() {
        return text.map_or(EditAction::Ignore, EditAction::Insert);
    }
    match key {
        Key::Named(NamedKey::Backspace) => EditAction::Backspace,
        Key::Named(NamedKey::Delete) => EditAction::Delete,
        Key::Named(NamedKey::ArrowLeft) => EditAction::Left,
        Key::Named(NamedKey::ArrowRight) => EditAction::Right,
        Key::Named(NamedKey::Home) => EditAction::Home,
        Key::Named(NamedKey::End) => EditAction::End,
        Key::Named(NamedKey::Enter | NamedKey::Escape) if !repeat => EditAction::Finish,
        Key::Named(NamedKey::ArrowDown) if !repeat => EditAction::Candidates,
        Key::Named(NamedKey::Tab) if !repeat => EditAction::Fold,
        Key::Named(NamedKey::Space) => EditAction::Insert(text.unwrap_or(" ")),
        Key::Character(_) => text.map_or(EditAction::Ignore, EditAction::Insert),
        _ => EditAction::Ignore,
    }
}

impl PanelState {
    // Restore only after leaving the native view. Both an explicit click and a
    // preceding Focused event use this handoff, without another focus request.
    pub(super) fn restore_local_recovery(&mut self, recovery: Option<String>) {
        if let Some(text) = recovery {
            self.chrome.set_seed_text(text);
            self.sync_manual_seed_base();
            self.refresh_seed();
        }
    }

    pub(super) fn begin_text_editing(&mut self) {
        let recovery = self.take_native_typing_draft();
        self.leave_native_view();
        // Window focus alone must not revive a retired field's text. Consume
        // the saved copy only here, after restoring/checking the local base;
        // a newer active draft wins and also retires the older cached copy.
        let recovery = recovery.or(self.take_saved_native_typing_draft());
        match self
            .text_focus
            .request(&self.window, self.runs_without_window_focus)
        {
            Ok(()) => {
                self.restore_local_recovery(recovery);
                self.chrome.focus_input();
                self.chrome.move_caret_to_end();
            }
            Err(_) => {
                // Focus failure must not erase a draft recovered from native typing.
                self.restore_local_recovery(recovery);
                self.chrome.blur_input();
                self.last_commit_feedback =
                    Some("Could not focus the input field. Click it to retry.".into());
                self.commit_feedback_ticks = 180;
            }
        }
    }

    pub(super) fn finish_text_editing(&mut self) {
        self.text_input.clear();
        self.chrome.blur_input();
        self.window.set_ime_allowed(false);
        self.text_input.allowed = false;
        self.text_input.cursor_area = None;
        self.text_focus.release();
    }

    pub(super) fn sync_text_input_state(&mut self) {
        let allowed = self.sync_text_input_permission();
        let settings = self.kind == PanelWindowKind::Settings;
        if allowed {
            let target = if settings {
                InteractionKind::SettingsSearchInput
            } else {
                InteractionKind::SeedInput
            };
            if let Some([x, y, w, h]) = self.interaction_rect(target) {
                let area = [x, y, w, h];
                if self.text_input.cursor_area != Some(area) {
                    self.text_input.cursor_area = Some(area);
                    self.window.set_ime_cursor_area(
                        PhysicalPosition::new(x as f64, (y + h) as f64),
                        PhysicalSize::new(w.max(1.0) as u32, 1),
                    );
                }
            }
        }
    }

    /// Keep the current IME context until the app commits or rolls back a
    /// main-window settings request. Revoking permission here would erase a
    /// live preedit before a failed auxiliary constructor can restore input.
    pub(super) fn sync_text_input_after_dispatch(&mut self) {
        if self.kind == PanelWindowKind::Main && self.settings_window_request.is_some() {
            return;
        }
        self.sync_text_input_state();
    }

    /// Permission changes must not measure a hidden/restored settings scene:
    /// measuring can clamp navigation before its ordinary frame is requested.
    pub(super) fn sync_text_input_permission(&mut self) -> bool {
        let settings = self.kind == PanelWindowKind::Settings;
        let allowed = self.is_focused
            && self.report_view.is_none()
            && self.chrome.shortcut_recording.is_none()
            && if settings {
                self.chrome.settings_search_focused
            } else {
                self.chrome.input_focused && !self.chrome.compact_mode && !self.chrome.settings_open
            };
        if self.text_input.allowed != allowed {
            self.text_input.allowed = allowed;
            self.text_input.cursor_area = None;
            if !allowed {
                self.text_input.clear();
            }
            self.window.set_ime_allowed(allowed);
        }
        allowed
    }

    pub(super) fn handle_ime_event(&mut self, event: Ime) {
        if self.chrome.shortcut_recording.is_some() {
            if matches!(event, Ime::Preedit(..) | Ime::Commit(_)) {
                self.chrome.cancel_shortcut_recording();
                self.text_input.clear();
                self.last_scene = None;
                self.window.request_redraw();
            }
            return;
        }
        if !self.text_input.allowed {
            return;
        }
        match event {
            Ime::Preedit(text, cursor) => self.text_input.set_preedit(text, cursor),
            Ime::Commit(text) => {
                self.text_input.clear();
                if self.kind == PanelWindowKind::Settings {
                    self.handle_settings_search_text(&text);
                } else {
                    self.handle_text_input(&text);
                }
            }
            Ime::Disabled => self.text_input.clear(),
            Ime::Enabled => {}
        }
        self.last_scene = None;
        self.window.request_redraw();
    }

    pub(super) fn handle_editing_key(&mut self, key: &Key, text: Option<&str>, repeat: bool) {
        match edit_action(
            key,
            text,
            self.modifiers,
            repeat,
            self.text_input.composing(),
        ) {
            EditAction::Insert(text) => self.handle_text_input(text),
            EditAction::Backspace => self.backspace_seed(),
            EditAction::Delete => {
                if self.chrome.caret_index < self.chrome.seed_text.chars().count() {
                    self.chrome.move_caret_right();
                    self.backspace_seed();
                }
            }
            EditAction::Left => self.chrome.move_caret_left(),
            EditAction::Right => self.chrome.move_caret_right(),
            EditAction::Home => self.chrome.caret_index = 0,
            EditAction::End => self.chrome.move_caret_to_end(),
            EditAction::Finish => self.finish_text_editing(),
            EditAction::Candidates => self.chrome.blur_input(),
            EditAction::Fold => {
                self.chrome.input_modes_expanded = !self.chrome.input_modes_expanded
            }
            EditAction::Ignore => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suzaku_map::ime::shortcuts::{ShortcutAction, ShortcutBindings};

    #[test]
    fn recorded_physical_key_hold_survives_ime_cleanup_and_modifier_release() {
        let mut input = TextInputState::default();
        input.hold_recorded_key(PhysicalKey::Code(KeyCode::ControlLeft));
        assert!(!input.consume_recorded_hold(PhysicalKey::Code(KeyCode::ControlLeft), true));
        input.hold_recorded_key(PhysicalKey::Code(KeyCode::KeyJ));
        input.clear();
        assert!(!input.consume_recorded_hold(PhysicalKey::Code(KeyCode::ControlLeft), false));
        assert!(input.consume_recorded_hold(PhysicalKey::Code(KeyCode::KeyJ), true));
        assert!(input.consume_recorded_hold(PhysicalKey::Code(KeyCode::KeyJ), false));
        assert!(!input.consume_recorded_hold(PhysicalKey::Code(KeyCode::KeyJ), true));
        input.hold_recorded_key(PhysicalKey::Code(KeyCode::KeyF));
        input.clear_shortcut_hold();
        assert!(!input.consume_recorded_hold(PhysicalKey::Code(KeyCode::KeyF), true));
    }

    #[test]
    fn overlapping_recorded_keys_remain_consumed_after_escape_cancels_recording() {
        let mut chrome = PanelChromeState {
            seed_text: "draft café 😀".into(),
            settings_search_query: "untouched".into(),
            ..Default::default()
        };
        let mut input = TextInputState::default();
        let first = PhysicalKey::Code(KeyCode::KeyA);
        let escape = PhysicalKey::Code(KeyCode::Escape);
        chrome.toggle_shortcut_recording(ShortcutAction::Previous);
        assert!(capture_shortcut_recording(
            &mut chrome,
            &Key::Character("a".into()),
            ModifiersState::empty(),
            false,
            true,
            false,
        ));
        input.hold_recorded_key(first);
        assert_eq!(chrome.shortcut_recording, Some(ShortcutAction::Previous));
        assert!(!chrome.shortcut_recording_error.is_empty());
        assert!(capture_shortcut_recording(
            &mut chrome,
            &Key::Named(NamedKey::Escape),
            ModifiersState::empty(),
            false,
            true,
            false,
        ));
        input.hold_recorded_key(escape);
        assert!(chrome.shortcut_recording.is_none());
        assert!(input.consume_recorded_hold(escape, false));
        let cancelled = chrome.clone();
        assert!(
            input.consume_recorded_hold(first, true),
            "held A repeat must not reach settings search after Escape"
        );
        assert!(input.consume_recorded_hold(first, false));
        assert!(!input.consume_recorded_hold(first, true));
        assert_eq!(chrome, cancelled);
        assert_eq!(chrome.shortcut_bindings, ShortcutBindings::default());
        assert_eq!(chrome.shortcut_edit_generation, 0);
        assert_eq!(chrome.settings_search_query, "untouched");
        assert_eq!(chrome.seed_text, "draft café 😀");
    }

    #[test]
    fn overlapping_recorded_keys_remain_consumed_after_a_valid_binding_is_saved() {
        let mut chrome = PanelChromeState {
            seed_text: "draft café 😀".into(),
            settings_search_query: "untouched".into(),
            ..Default::default()
        };
        let mut input = TextInputState::default();
        let rejected = PhysicalKey::Code(KeyCode::KeyA);
        let accepted = PhysicalKey::Code(KeyCode::KeyK);
        chrome.toggle_shortcut_recording(ShortcutAction::Previous);
        assert!(capture_shortcut_recording(
            &mut chrome,
            &Key::Character("a".into()),
            ModifiersState::empty(),
            false,
            true,
            false,
        ));
        input.hold_recorded_key(rejected);
        assert!(capture_shortcut_recording(
            &mut chrome,
            &Key::Character("k".into()),
            ModifiersState::CONTROL,
            false,
            true,
            false,
        ));
        input.hold_recorded_key(accepted);
        assert!(chrome.shortcut_recording.is_none());
        assert_eq!(
            chrome.shortcut_binding_label(ShortcutAction::Previous),
            "Ctrl+K"
        );
        assert_eq!(chrome.shortcut_edit_generation, 1);
        let saved = chrome.clone();
        // Release the valid key and its modifier before A starts repeating.
        assert!(input.consume_recorded_hold(accepted, false));
        assert!(!input.consume_recorded_hold(PhysicalKey::Code(KeyCode::ControlLeft), false));
        assert!(
            input.consume_recorded_hold(rejected, true),
            "saving another key must not release an earlier consumed key"
        );
        assert!(input.consume_recorded_hold(rejected, false));
        assert!(!input.consume_recorded_hold(rejected, true));
        assert_eq!(chrome, saved);
        assert_eq!(chrome.settings_search_query, "untouched");
        assert_eq!(chrome.seed_text, "draft café 😀");
    }

    #[test]
    fn recording_holds_deduplicate_known_keys_and_ignore_unknown_or_modifier_keys() {
        let mut input = TextInputState::default();
        let first = PhysicalKey::Code(KeyCode::KeyA);
        let second = PhysicalKey::Code(KeyCode::KeyK);
        let unknown = PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified);
        let modifier = PhysicalKey::Code(KeyCode::ControlLeft);
        for key in [first, first, unknown, modifier, second, second] {
            input.hold_recorded_key(key);
        }
        input.clear(); // IME cleanup must not release recording-owned keys.
        assert!(!input.consume_recorded_hold(unknown, true));
        assert!(!input.consume_recorded_hold(unknown, false));
        assert!(!input.consume_recorded_hold(modifier, true));
        assert!(!input.consume_recorded_hold(modifier, false));
        assert!(input.consume_recorded_hold(first, true));
        assert!(input.consume_recorded_hold(first, false));
        assert!(
            !input.consume_recorded_hold(first, false),
            "duplicate presses must not need duplicate releases"
        );
        assert!(input.consume_recorded_hold(second, true));
        assert!(input.consume_recorded_hold(second, false));
        assert!(!input.consume_recorded_hold(second, true));
        input.hold_recorded_key(first);
        input.hold_recorded_key(second);
        input.clear_shortcut_hold(); // Focus/close boundaries still release all holds.
        assert!(!input.consume_recorded_hold(first, true));
        assert!(!input.consume_recorded_hold(second, true));
    }

    #[test]
    fn recording_consumes_panel_commands_repeats_and_invalid_keys_before_editors() {
        for (key, modifiers) in [
            (Key::Named(NamedKey::F1), ModifiersState::empty()),
            (Key::Character(",".into()), ModifiersState::CONTROL),
            (
                Key::Character("+".into()),
                ModifiersState::CONTROL | ModifiersState::SHIFT,
            ),
            (Key::Character("0".into()), ModifiersState::CONTROL),
            (
                Key::Character("x".into()),
                ModifiersState::CONTROL | ModifiersState::ALT,
            ),
            (Key::Character("x".into()), ModifiersState::SUPER),
            (Key::Named(NamedKey::Space), ModifiersState::ALT),
        ] {
            let mut chrome = PanelChromeState {
                seed_text: "draft café 😀".into(),
                settings_search_query: "untouched".into(),
                ..Default::default()
            };
            chrome.toggle_shortcut_recording(ShortcutAction::Previous);
            assert!(capture_shortcut_recording(
                &mut chrome,
                &key,
                modifiers,
                false,
                true,
                false
            ));
            assert_eq!(chrome.shortcut_recording, Some(ShortcutAction::Previous));
            assert_eq!(chrome.shortcut_bindings, ShortcutBindings::default());
            assert_eq!(chrome.shortcut_edit_generation, 0);
            assert_eq!(chrome.seed_text, "draft café 😀");
            assert_eq!(chrome.settings_search_query, "untouched");
            assert!(!chrome.shortcut_recording_error.is_empty());
        }
        let mut chrome = PanelChromeState::default();
        chrome.toggle_shortcut_recording(ShortcutAction::Next);
        for key in [Key::Named(NamedKey::Control), Key::Character("j".into())] {
            assert!(capture_shortcut_recording(
                &mut chrome,
                &key,
                ModifiersState::CONTROL,
                true,
                true,
                false
            ));
            assert_eq!(chrome.shortcut_edit_generation, 0);
            assert!(chrome.shortcut_recording_error.is_empty());
        }
        assert!(capture_shortcut_recording(
            &mut chrome,
            &Key::Character("J".into()),
            ModifiersState::CONTROL,
            false,
            true,
            false
        ));
        assert_eq!(
            chrome.shortcut_binding_label(ShortcutAction::Next),
            "Ctrl+J"
        );
        assert_eq!(chrome.shortcut_edit_generation, 1);
        assert_eq!(chrome.shortcut_recording, None);
    }

    #[test]
    fn escape_preedit_and_wrong_window_cancel_recording_without_a_save() {
        for (key, focused_settings, composing, consumed) in [
            (Key::Named(NamedKey::Escape), true, false, true),
            (Key::Character("x".into()), true, true, true),
            (Key::Character("x".into()), false, false, false),
        ] {
            let mut chrome = PanelChromeState::default();
            chrome.toggle_shortcut_recording(ShortcutAction::Adopt);
            assert_eq!(
                capture_shortcut_recording(
                    &mut chrome,
                    &key,
                    ModifiersState::CONTROL,
                    false,
                    focused_settings,
                    composing
                ),
                consumed
            );
            assert_eq!(chrome.shortcut_recording, None);
            assert_eq!(chrome.shortcut_edit_generation, 0);
            assert_eq!(chrome.shortcut_bindings, ShortcutBindings::default());
        }
    }
    #[test]
    fn help_and_settings_chords_are_exact_and_do_not_repeat_or_steal_altgr() {
        let help = Key::Named(NamedKey::F1);
        let comma = Key::Character(",".into());
        assert_eq!(
            settings_shortcut(&help, ModifiersState::empty(), false),
            Some(true)
        );
        assert_eq!(
            settings_shortcut(&comma, ModifiersState::CONTROL, false),
            Some(false)
        );
        assert_eq!(
            settings_shortcut(&help, ModifiersState::empty(), true),
            None
        );
        assert_eq!(
            settings_shortcut(&comma, ModifiersState::CONTROL, true),
            None
        );
        for modifiers in [
            ModifiersState::ALT,
            ModifiersState::SHIFT,
            ModifiersState::CONTROL | ModifiersState::ALT,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        ] {
            assert_eq!(settings_shortcut(&help, modifiers, false), None);
            assert_eq!(settings_shortcut(&comma, modifiers, false), None);
        }
        assert_eq!(
            settings_shortcut(&comma, ModifiersState::empty(), false),
            None
        );
    }

    #[test]
    fn ordinary_characters_digits_and_repeats_are_always_text_in_an_editor() {
        for text in [
            "1", "2", "3", "4", "v", "n", "i", "d", "r", "z", "é", "日", "😀",
        ] {
            for repeat in [false, true] {
                assert_eq!(
                    edit_action(
                        &Key::Character(text.into()),
                        Some(text),
                        ModifiersState::empty(),
                        repeat,
                        false
                    ),
                    EditAction::Insert(text)
                );
            }
        }
        for repeat in [false, true] {
            assert_eq!(
                edit_action(
                    &Key::Named(NamedKey::Space),
                    Some(" "),
                    ModifiersState::empty(),
                    repeat,
                    false
                ),
                EditAction::Insert(" ")
            );
        }
    }

    #[test]
    fn shortcut_chords_do_not_insert_letters_but_altgr_can_insert_text() {
        let key = Key::Character("v".into());
        for modifiers in [
            ModifiersState::CONTROL,
            ModifiersState::SUPER,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        ] {
            assert_eq!(
                edit_action(&key, Some("v"), modifiers, false, false),
                EditAction::Ignore
            );
        }
        assert_eq!(
            edit_action(
                &key,
                Some("@"),
                ModifiersState::CONTROL | ModifiersState::ALT,
                false,
                false
            ),
            EditAction::Insert("@")
        );
    }

    #[test]
    fn altgr_does_not_trigger_zoom_or_quit_shortcuts() {
        assert!(primary_shortcut_modifier(ModifiersState::CONTROL));
        assert!(primary_shortcut_modifier(
            ModifiersState::SUPER | ModifiersState::SHIFT
        ));
        assert!(!primary_shortcut_modifier(
            ModifiersState::CONTROL | ModifiersState::ALT
        ));
    }

    #[test]
    fn composition_owns_keys_and_finish_is_not_repeatable() {
        assert_eq!(
            edit_action(
                &Key::Named(NamedKey::Backspace),
                None,
                ModifiersState::empty(),
                true,
                false
            ),
            EditAction::Backspace
        );
        for named in [
            NamedKey::Enter,
            NamedKey::Escape,
            NamedKey::Space,
            NamedKey::Backspace,
            NamedKey::ArrowLeft,
        ] {
            assert_eq!(
                edit_action(
                    &Key::Named(named),
                    None,
                    ModifiersState::empty(),
                    false,
                    true
                ),
                EditAction::Ignore
            );
        }
        assert_eq!(
            edit_action(
                &Key::Named(NamedKey::Enter),
                Some("\r"),
                ModifiersState::empty(),
                true,
                false
            ),
            EditAction::Ignore
        );
    }

    #[test]
    fn preedit_is_only_a_preview_and_uses_unicode_cursor_offsets() {
        let mut input = TextInputState::default();
        let chrome = PanelChromeState {
            seed_text: "ab".into(),
            caret_index: 1,
            ..Default::default()
        };
        input.set_preedit("日😀本".into(), Some((7, 7)));
        let mut preview = chrome.clone();
        input.preview(&mut preview, false);
        assert_eq!(preview.seed_text, "a日😀本b");
        assert_eq!(preview.caret_index, 3);
        assert_eq!(chrome.seed_text, "ab");
        input.set_preedit("".into(), None);
        let mut preview = chrome.clone();
        input.preview(&mut preview, false);
        assert_eq!(preview, chrome);
    }
}
