//! Context-local shortcuts. Adapters own modifier, focus, privacy and Compose checks.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShortcutProfile {
    /// Preserve existing shortcuts and leave all additional Alt chords to the app.
    #[default]
    Standard,
    /// Optional QWERTY home-row aliases; never installs a global key grab.
    HomeRow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ShortcutAction {
    Previous = 1,
    Next = 2,
    PreviousPage = 3,
    NextPage = 4,
    Adopt = 5,
    SwitchLanguage = 6,
}

impl ShortcutAction {
    pub const ALL: [Self; 6] = [
        Self::Previous,
        Self::Next,
        Self::PreviousPage,
        Self::NextPage,
        Self::Adopt,
        Self::SwitchLanguage,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Previous => "previous",
            Self::Next => "next",
            Self::PreviousPage => "previous-page",
            Self::NextPage => "next-page",
            Self::Adopt => "adopt",
            Self::SwitchLanguage => "switch-language",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Previous => "Previous",
            Self::Next => "Next choice",
            Self::PreviousPage => "Page back",
            Self::NextPage => "Page forward",
            Self::Adopt => "Adopt",
            Self::SwitchLanguage => "Switch EN/ZH",
        }
    }

    pub fn index(self) -> usize {
        self as usize - 1
    }
}

/// A deliberately small logical-key protocol, independent of native modifier masks.
/// Modifiers are Ctrl=1, Alt=2 and Shift=4; other modifiers must not be discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortcutChord {
    key: u32,
    modifiers: u32,
}

impl ShortcutChord {
    pub const CTRL: u32 = 1;
    pub const ALT: u32 = 2;
    pub const SHIFT: u32 = 4;

    pub fn from_event(key: u32, modifiers: u32) -> Result<Self, String> {
        let key = char::from_u32(key)
            .filter(|key| key.is_ascii_alphabetic() || *key == ' ')
            .ok_or("快捷键只支持字母 A–Z 或 Space")?
            .to_ascii_lowercase() as u32;
        if modifiers & !(Self::CTRL | Self::ALT | Self::SHIFT) != 0
            || modifiers & (Self::CTRL | Self::ALT) == 0
            || modifiers & (Self::CTRL | Self::ALT) == (Self::CTRL | Self::ALT)
        {
            return Err("快捷键须包含 Ctrl 或 Alt，不支持 Ctrl+Alt、Super 或其他修饰键".into());
        }
        if key == 32 && modifiers & Self::ALT != 0 {
            return Err("Alt+Space 保留给桌面窗口操作".into());
        }
        Ok(Self { key, modifiers })
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.len() > 64 {
            return Err("快捷键组合过长".into());
        }
        let mut parts = raw.split('+').map(str::trim).peekable();
        let mut modifiers = 0;
        let mut key = None;
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                key = if part.eq_ignore_ascii_case("space") {
                    Some(32)
                } else if part.len() == 1 && part.as_bytes()[0].is_ascii_alphabetic() {
                    Some(u32::from(part.as_bytes()[0]))
                } else {
                    None
                };
            } else {
                let bit = match part.to_ascii_lowercase().as_str() {
                    "ctrl" => Self::CTRL,
                    "alt" => Self::ALT,
                    "shift" => Self::SHIFT,
                    _ => return Err("不支持的快捷键修饰键".into()),
                };
                if modifiers & bit != 0 {
                    return Err("快捷键修饰键不能重复".into());
                }
                modifiers |= bit;
            }
        }
        Self::from_event(key.ok_or("快捷键只支持字母 A–Z 或 Space")?, modifiers)
    }

    pub fn display(self) -> String {
        let mut parts = Vec::new();
        for (bit, name) in [
            (Self::CTRL, "Ctrl"),
            (Self::ALT, "Alt"),
            (Self::SHIFT, "Shift"),
        ] {
            if self.modifiers & bit != 0 {
                parts.push(name.to_string());
            }
        }
        parts.push(if self.key == 32 {
            "Space".into()
        } else {
            char::from_u32(self.key)
                .unwrap()
                .to_ascii_uppercase()
                .to_string()
        });
        parts.join("+")
    }
}

/// Optional aliases for candidate actions, and an optional replacement language chord.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShortcutBindings {
    chords: [Option<ShortcutChord>; 6],
}

impl ShortcutBindings {
    pub fn get(self, action: ShortcutAction) -> Option<ShortcutChord> {
        self.chords[action.index()]
    }

    pub fn with_binding(mut self, action: ShortcutAction, chord: Option<ShortcutChord>) -> Self {
        self.chords[action.index()] = chord;
        self
    }

    fn language_chord(self) -> ShortcutChord {
        self.get(ShortcutAction::SwitchLanguage)
            .unwrap_or(ShortcutChord {
                key: 32,
                modifiers: ShortcutChord::CTRL | ShortcutChord::SHIFT,
            })
    }

    pub fn validate(self, profile: ShortcutProfile) -> Result<(), String> {
        for (index, action) in ShortcutAction::ALL.into_iter().enumerate() {
            let Some(chord) = self.get(action) else {
                continue;
            };
            ShortcutChord::from_event(chord.key, chord.modifiers)?;
            if ShortcutAction::ALL[..index]
                .iter()
                .any(|other| self.get(*other) == Some(chord))
            {
                return Err("不同快捷键操作不能使用相同组合".into());
            }
            if action != ShortcutAction::SwitchLanguage && chord == self.language_chord() {
                return Err("快捷键与当前语言切换组合冲突".into());
            }
            if profile == ShortcutProfile::HomeRow
                && chord.modifiers == ShortcutChord::ALT
                && HOME_ROW_BINDINGS.iter().any(|(key, preset_action, _)| {
                    *key as u32 == chord.key && *preset_action != action
                })
            {
                return Err("快捷键与主键区预设冲突，请修改组合或切换标准方案".into());
            }
        }
        Ok(())
    }

    pub fn to_json(self) -> serde_json::Value {
        let mut fields = serde_json::Map::new();
        for action in ShortcutAction::ALL {
            if let Some(chord) = self.get(action) {
                fields.insert(action.id().into(), chord.display().into());
            }
        }
        serde_json::Value::Object(fields)
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        let fields = value.as_object().ok_or("shortcut_bindings 必须是对象")?;
        let mut bindings = Self::default();
        for (key, value) in fields {
            let action = ShortcutAction::ALL
                .into_iter()
                .find(|action| action.id() == key)
                .ok_or("不支持的快捷键操作")?;
            let chord = ShortcutChord::parse(value.as_str().ok_or("快捷键组合必须是字符串")?)?;
            bindings = bindings.with_binding(action, Some(chord));
        }
        bindings.validate(ShortcutProfile::Standard)?;
        Ok(bindings)
    }

    pub fn action(
        self,
        profile: ShortcutProfile,
        key: u32,
        modifiers: u32,
    ) -> Option<ShortcutAction> {
        // Preserve the fixed Alt+; alias even though punctuation is not customizable.
        if modifiers == ShortcutChord::ALT
            && let Some(action) = char::from_u32(key).and_then(|key| profile.alt_action(key))
        {
            return Some(action);
        }
        let chord = ShortcutChord::from_event(key, modifiers).ok()?;
        if chord == self.language_chord() {
            return Some(ShortcutAction::SwitchLanguage);
        }
        ShortcutAction::ALL
            .into_iter()
            .filter(|action| *action != ShortcutAction::SwitchLanguage)
            .find(|action| self.get(*action) == Some(chord))
    }
}

impl ShortcutProfile {
    pub const ALL: [Self; 2] = [Self::Standard, Self::HomeRow];

    pub fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::HomeRow => "home-row",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard keys",
            Self::HomeRow => "Home row (Alt)",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|profile| profile.id() == id)
    }

    /// Only call for an exact Alt chord in a nonempty, public IBus draft.
    /// Case folding allows Caps Lock, not Shift (checked by the adapter).
    pub fn alt_action(self, key: char) -> Option<ShortcutAction> {
        if self != Self::HomeRow {
            return None;
        }
        match key.to_ascii_lowercase() {
            'k' => Some(ShortcutAction::Previous),
            'j' => Some(ShortcutAction::Next),
            'h' => Some(ShortcutAction::PreviousPage),
            'l' => Some(ShortcutAction::NextPage),
            ';' => Some(ShortcutAction::Adopt),
            _ => None,
        }
    }

    /// Searchable read-only reference, including the unchanged writing-stream keys.
    pub fn reference(self) -> Vec<(&'static str, Vec<&'static str>)> {
        let mut rows = vec![
            ("Scope", vec!["IBus", "Focused input"]),
            ("Switch EN/ZH", vec!["Ctrl+Shift+Space"]),
            ("Previous", vec!["Shift+Tab", "↑"]),
            ("Next choice", vec!["Tab", "↓"]),
            ("Page back", vec!["PageUp"]),
            ("Page forward", vec!["PageDown"]),
            ("Adopt", vec!["1–6", "Shift+Enter"]),
            ("Undo choice", vec!["Backspace"]),
            ("Continue", vec!["Space"]),
            ("Submit", vec!["Enter"]),
            ("Cancel", vec!["Esc"]),
            ("Literal digits", vec!["Alt+0–9", "Numpad"]),
            ("Panel help", vec!["F1"]),
            ("Panel settings", vec!["Ctrl+,"]),
        ];
        if self == Self::HomeRow {
            for ((_, _, key), (_, keys)) in HOME_ROW_BINDINGS.iter().zip(&mut rows[2..7]) {
                keys.push(key);
            }
        }
        rows
    }
}

/// Labels shared by the settings reference and shortcut tests. Keys are not text actions.
pub const HOME_ROW_BINDINGS: [(char, ShortcutAction, &str); 5] = [
    ('k', ShortcutAction::Previous, "Alt+K"),
    ('j', ShortcutAction::Next, "Alt+J"),
    ('h', ShortcutAction::PreviousPage, "Alt+H"),
    ('l', ShortcutAction::NextPage, "Alt+L"),
    (';', ShortcutAction::Adopt, "Alt+;"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_chords_have_a_canonical_bounded_logical_key_protocol() {
        for (raw, display, key, modifiers) in [
            (" ctrl + shift + a ", "Ctrl+Shift+A", b'a', 5),
            ("SHIFT+ALT+j", "Alt+Shift+J", b'j', 6),
            ("Ctrl+Space", "Ctrl+Space", b' ', 1),
            ("Ctrl+Shift+Space", "Ctrl+Shift+Space", b' ', 5),
        ] {
            let chord = ShortcutChord::parse(raw).unwrap();
            assert_eq!(chord.display(), display);
            assert_eq!(ShortcutChord::parse(&chord.display()).unwrap(), chord);
            assert_eq!(
                ShortcutChord::from_event(u32::from(key), modifiers).unwrap(),
                chord
            );
            if key.is_ascii_alphabetic() {
                assert_eq!(
                    ShortcutChord::from_event(u32::from(key.to_ascii_uppercase()), modifiers)
                        .unwrap(),
                    chord
                );
            }
        }
        for raw in [
            "",
            "A",
            "Shift+A",
            "Ctrl",
            "Ctrl+",
            "Ctrl+Ctrl+A",
            "Ctrl+Alt+A",
            "Ctrl+Alt+Shift+A",
            "Alt+Space",
            "Alt+Shift+Space",
            "Super+A",
            "Ctrl+Tab",
            "Ctrl+1",
            "Alt+;",
            "Ctrl+é",
            "Ctrl+あ",
            "Ctrl+A+B",
            "Ctrl++A",
            "Ctrl+Space\nA",
        ] {
            assert!(ShortcutChord::parse(raw).is_err(), "{raw:?}");
        }
        assert!(ShortcutChord::parse(&"A".repeat(65)).is_err());
        for (key, modifiers) in [
            (u32::MAX, 1),
            (b'a' as u32, 0),
            (b'a' as u32, 8),
            (b'a' as u32, 9),
            (b'a' as u32, 3),
            (b' ' as u32, 6),
        ] {
            assert!(ShortcutChord::from_event(key, modifiers).is_err());
        }
    }

    #[test]
    fn custom_binding_actions_keep_the_native_abi_and_round_trip_exactly() {
        let ids = [
            "previous",
            "next",
            "previous-page",
            "next-page",
            "adopt",
            "switch-language",
        ];
        let mut bindings = ShortcutBindings::default();
        for (index, (action, id)) in ShortcutAction::ALL.into_iter().zip(ids).enumerate() {
            assert_eq!(action as u32, index as u32 + 1);
            assert_eq!(action.index(), index);
            assert_eq!(action.id(), id);
            assert!(!action.label().is_empty());
            let chord = ShortcutChord::from_event(b'a' as u32 + index as u32, 5).unwrap();
            bindings = bindings.with_binding(action, Some(chord));
            assert_eq!(bindings.get(action), Some(chord));
        }
        bindings.validate(ShortcutProfile::HomeRow).unwrap();
        assert_eq!(
            ShortcutBindings::from_json(&bindings.to_json()).unwrap(),
            bindings
        );
        for (index, action) in ShortcutAction::ALL.into_iter().enumerate() {
            assert_eq!(
                bindings.action(ShortcutProfile::HomeRow, b'A' as u32 + index as u32, 5),
                Some(action)
            );
        }
        assert_eq!(bindings.action(ShortcutProfile::Standard, 32, 5), None);
        let cleared = bindings.with_binding(ShortcutAction::SwitchLanguage, None);
        assert_eq!(
            cleared.action(ShortcutProfile::Standard, 32, 5),
            Some(ShortcutAction::SwitchLanguage)
        );
        assert_eq!(
            cleared.action(ShortcutProfile::Standard, b'f' as u32, 5),
            None
        );
        assert_eq!(ShortcutBindings::default().to_json(), serde_json::json!({}));
    }

    #[test]
    fn custom_bindings_reject_duplicates_and_effective_presets() {
        for raw in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({"unknown":"Ctrl+A"}),
            serde_json::json!({"adopt":null}),
            serde_json::json!({"adopt":6}),
            serde_json::json!({"previous":"Ctrl+A", "next":"ctrl+a"}),
            serde_json::json!({"adopt":"Ctrl+Shift+Space"}),
            serde_json::json!({"adopt":"Ctrl+A", "switch-language":"Ctrl+A"}),
        ] {
            assert!(ShortcutBindings::from_json(&raw).is_err(), "{raw}");
        }
        for (key, preset_action, _) in HOME_ROW_BINDINGS {
            if key == ';' {
                continue;
            }
            for action in ShortcutAction::ALL {
                let bindings = ShortcutBindings::default().with_binding(
                    action,
                    Some(ShortcutChord::parse(&format!("Alt+{key}")).unwrap()),
                );
                bindings.validate(ShortcutProfile::Standard).unwrap();
                assert_eq!(
                    bindings.validate(ShortcutProfile::HomeRow).is_ok(),
                    action == preset_action
                );
            }
        }
        let bindings = ShortcutBindings::from_json(&serde_json::json!({
            "adopt":"Ctrl+Shift+Space", "switch-language":"Ctrl+L"
        }))
        .unwrap();
        assert_eq!(
            bindings.action(ShortcutProfile::Standard, 32, 5),
            Some(ShortcutAction::Adopt)
        );
        assert_eq!(
            bindings.action(ShortcutProfile::Standard, b'l' as u32, 1),
            Some(ShortcutAction::SwitchLanguage)
        );
        assert!(
            bindings
                .with_binding(ShortcutAction::SwitchLanguage, None)
                .validate(ShortcutProfile::Standard)
                .is_err()
        );
        let redundant_default =
            ShortcutBindings::from_json(&serde_json::json!({"switch-language":"Ctrl+Shift+Space"}))
                .unwrap();
        redundant_default
            .validate(ShortcutProfile::HomeRow)
            .unwrap();
    }

    #[test]
    fn custom_aliases_do_not_erase_home_row_or_accept_extra_modifiers() {
        let bindings =
            ShortcutBindings::from_json(&serde_json::json!({"adopt":"Ctrl+Shift+A"})).unwrap();
        for (key, action, _) in HOME_ROW_BINDINGS {
            assert_eq!(
                bindings.action(ShortcutProfile::HomeRow, key as u32, 2),
                Some(action)
            );
            assert_eq!(
                bindings.action(ShortcutProfile::Standard, key as u32, 2),
                None
            );
            for modifiers in [0, 1, 3, 4, 6, 7, 8, 10] {
                assert_eq!(
                    bindings.action(ShortcutProfile::HomeRow, key as u32, modifiers),
                    None
                );
            }
        }
        assert_eq!(
            bindings.action(ShortcutProfile::HomeRow, b'a' as u32, 5),
            Some(ShortcutAction::Adopt)
        );
        for modifiers in [0, 1, 2, 3, 4, 6, 7, 13] {
            assert_eq!(
                bindings.action(ShortcutProfile::HomeRow, b'a' as u32, modifiers),
                None
            );
        }
    }

    #[test]
    fn language_toggle_is_easy_to_find_without_moving_home_row_aliases() {
        for profile in ShortcutProfile::ALL {
            let rows = profile.reference();
            assert_eq!(rows[1], ("Switch EN/ZH", vec!["Ctrl+Shift+Space"]));
            for (label, (_, _, key)) in [
                "Previous",
                "Next choice",
                "Page back",
                "Page forward",
                "Adopt",
            ]
            .into_iter()
            .zip(HOME_ROW_BINDINGS)
            {
                let keys = &rows.iter().find(|(name, _)| *name == label).unwrap().1;
                assert_eq!(keys.contains(&key), profile == ShortcutProfile::HomeRow);
            }
        }
    }

    #[test]
    fn profiles_and_displayed_aliases_match_without_stealing_literal_digits() {
        for profile in ShortcutProfile::ALL {
            assert_eq!(ShortcutProfile::from_id(profile.id()), Some(profile));
        }
        assert_eq!(ShortcutProfile::from_id("vim"), None);
        for (key, action, _) in HOME_ROW_BINDINGS {
            assert_eq!(ShortcutProfile::HomeRow.alt_action(key), Some(action));
            assert_eq!(
                ShortcutProfile::HomeRow.alt_action(key.to_ascii_uppercase()),
                Some(action)
            );
            assert_eq!(ShortcutProfile::Standard.alt_action(key), None);
        }
        for key in "0123456789qacvxz \t\r\u{1b}é中".chars() {
            assert_eq!(ShortcutProfile::HomeRow.alt_action(key), None, "{key:?}");
        }
    }
}
