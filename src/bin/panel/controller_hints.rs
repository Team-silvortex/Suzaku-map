use super::{PanelState, PanelWindowKind};
use crate::render::PanelOverlay;
use std::time::{Duration, Instant};
use suzaku_map::ime::gpu::{
    CandidateQuad, DisplayTextScale, InteractionKind, InteractiveTarget, LlmModelPreset,
    RenderScene, TextAlign, TextBlock, TextLayout, TextRole, ThemePreset, VirtualKeyboardKey,
    VoiceCaptureState,
};

const TOOLTIP_DELAY: Duration = Duration::from_millis(600);

#[derive(Default)]
pub(super) struct HoverTooltipState {
    target: Option<InteractiveTarget>,
    reveal_at: Option<Instant>,
    visible: bool,
}

impl HoverTooltipState {
    pub(super) fn track(&mut self, target: Option<InteractiveTarget>, now: Instant) -> bool {
        if self.target == target {
            return false;
        }
        let was_visible = self.visible;
        self.target = target;
        self.visible = false;
        self.reveal_at = target.map(|_| now + TOOLTIP_DELAY);
        was_visible
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        self.reveal_at
    }

    pub(super) fn advance(&mut self, now: Instant) -> bool {
        if self.reveal_at.is_some_and(|deadline| now >= deadline) {
            self.reveal_at = None;
            self.visible = self.target.is_some();
            return self.visible;
        }
        false
    }

    pub(super) fn visible_target(&self) -> Option<InteractiveTarget> {
        self.target.filter(|_| self.visible)
    }

    // Keep the target so clicking/typing does not immediately rearm the same tooltip.
    pub(super) fn dismiss(&mut self) -> bool {
        self.reveal_at = None;
        std::mem::take(&mut self.visible)
    }

    pub(super) fn clear(&mut self) -> bool {
        let was_visible = self.visible;
        *self = Self::default();
        was_visible
    }
}

fn tooltip_colors(theme: ThemePreset) -> ([f32; 4], [f32; 4], [f32; 4]) {
    match theme {
        ThemePreset::Baihu | ThemePreset::Qinglong | ThemePreset::Xuanwu => theme.tooltip_colors(),
        ThemePreset::Suzaku => (
            suzaku_map::ime::gpu::srgb_color(0xFFFAF4),
            suzaku_map::ime::gpu::srgb_color(0x34272A),
            suzaku_map::ime::gpu::srgb_color(0xC8A580),
        ),
        ThemePreset::Daylight => (
            [0.97, 0.98, 1.0, 1.0],
            [0.18, 0.23, 0.32, 1.0],
            [0.56, 0.66, 0.80, 1.0],
        ),
        ThemePreset::Solarized => (
            [0.98, 0.95, 0.87, 1.0],
            [0.18, 0.16, 0.14, 1.0],
            [0.65, 0.54, 0.39, 1.0],
        ),
        ThemePreset::DeviceDark => (
            [0.12, 0.16, 0.23, 1.0],
            [0.95, 0.97, 1.0, 1.0],
            [0.45, 0.58, 0.74, 1.0],
        ),
        ThemePreset::HighContrast => (
            [0.02, 0.03, 0.05, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            [0.15, 0.86, 1.0, 1.0],
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageOverlayKind {
    Tooltip,
    Feedback,
}

impl MessageOverlayKind {
    fn max_width(self) -> f32 {
        match self {
            Self::Tooltip => 340.0,
            Self::Feedback => 600.0,
        }
    }

    fn max_lines(self) -> usize {
        match self {
            Self::Tooltip => 3,
            Self::Feedback => 6,
        }
    }
}

fn message_overlay_layout(
    text: String,
    anchor: [f32; 4],
    viewport: [f32; 2],
    scale: f32,
    text_scale: DisplayTextScale,
    theme: ThemePreset,
    kind: MessageOverlayKind,
) -> Option<([f32; 4], TextLayout)> {
    if !scale.is_finite()
        || scale <= 0.0
        || viewport
            .iter()
            .any(|size| !size.is_finite() || *size <= 0.0)
        || anchor.iter().any(|value| !value.is_finite())
    {
        return None;
    }
    let margin = 8.0 * scale;
    let padding = [8.0 * scale, 6.0 * scale];
    let available_width = (viewport[0] - margin * 2.0).min(kind.max_width() * scale);
    let available_height = viewport[1] - margin * 2.0;
    if available_width <= padding[0] * 2.0 || available_height <= padding[1] * 2.0 {
        return None;
    }
    let (_, text_color, _) = tooltip_colors(theme);
    let block = TextBlock {
        text,
        origin: [0.0; 2],
        max_width: available_width - padding[0] * 2.0,
        pixel_size: text_scale.label_pixel_size() * scale,
        letter_spacing: -0.1 * scale,
        line_gap: 3.0 * scale,
        max_lines: kind.max_lines(),
        color: text_color,
        align: TextAlign::Left,
        role: TextRole::HeaderStatus,
    };
    let measured = block.layout_in_rect([0.0, 0.0, available_width, available_height], padding);
    let width = (measured.bounds[2] + padding[0] * 2.0 + 1.0)
        .max(72.0 * scale)
        .min(available_width);
    let height = (measured.bounds[3] + padding[1] * 2.0).min(available_height);
    let x = (anchor[0] + anchor[2] * 0.5 - width * 0.5)
        .clamp(margin, (viewport[0] - margin - width).max(margin));
    let below = anchor[1] + anchor[3] + margin;
    let above = anchor[1] - height - margin;
    let y = if below + height <= viewport[1] - margin {
        below
    } else if above >= margin {
        above
    } else if kind == MessageOverlayKind::Tooltip {
        // A hint must not obscure the control it explains in a short viewport.
        return None;
    } else {
        above
    }
    .clamp(margin, (viewport[1] - margin - height).max(margin));
    let rect = [x, y, width, height];
    // Keep the original wrapping and font size when tightening the surface to
    // its visible ink. A line's final advance can exceed its visible width:
    // rewrapping inside an ink-sized box can move the last word to a new line
    // even though the height was measured for the original line count.
    let mut layout = measured;
    let offset = [
        rect[0] + padding[0] - layout.bounds[0],
        rect[1] + padding[1] - layout.bounds[1],
    ];
    let translate = |mut bounds: [f32; 4]| {
        bounds[0] += offset[0];
        bounds[1] += offset[1];
        bounds
    };
    layout.bounds = translate(layout.bounds);
    for quad in &mut layout.quads {
        quad.rect = translate(quad.rect);
        quad.clip_rect = quad.clip_rect.map(translate);
    }
    for glyph in &mut layout.atlas_glyphs {
        glyph.rect = translate(glyph.rect);
        glyph.clip_rect = glyph.clip_rect.map(translate);
    }
    Some((rect, layout))
}

fn message_overlay(
    text: String,
    anchor: [f32; 4],
    viewport: [f32; 2],
    scale: f32,
    text_scale: DisplayTextScale,
    theme: ThemePreset,
    kind: MessageOverlayKind,
) -> Option<PanelOverlay> {
    // Measurement uses the caller's scoped atlas advances, just like the scene
    // underneath; final placement preserves those glyphs and font sizes.
    let (rect, layout) =
        message_overlay_layout(text, anchor, viewport, scale, text_scale, theme, kind)?;
    let [_, _, width, height] = rect;
    let (surface, _, border) = tooltip_colors(theme);
    let stroke = scale.max(1.0).min(width * 0.5).min(height * 0.5);
    Some(PanelOverlay {
        quads: vec![
            CandidateQuad::rounded(rect, surface, 8.0 * scale),
            CandidateQuad::outline(rect, border, 8.0 * scale, stroke),
        ],
        atlas_glyphs: layout.atlas_glyphs,
    })
}

fn hint_overlay(
    text: String,
    anchor: [f32; 4],
    viewport: [f32; 2],
    scale: f32,
    text_scale: DisplayTextScale,
    theme: ThemePreset,
) -> Option<PanelOverlay> {
    message_overlay(
        text,
        anchor,
        viewport,
        scale,
        text_scale,
        theme,
        MessageOverlayKind::Tooltip,
    )
}

fn intersect_overlay_rect(rect: [f32; 4], clip: [f32; 4]) -> [f32; 4] {
    let left = rect[0].max(clip[0]);
    let top = rect[1].max(clip[1]);
    let right = (rect[0] + rect[2]).min(clip[0] + clip[2]);
    let bottom = (rect[1] + rect[3]).min(clip[1] + clip[3]);
    [left, top, (right - left).max(0.0), (bottom - top).max(0.0)]
}

fn visible_feedback_region(region: [f32; 4], viewport: [f32; 2]) -> Option<[f32; 4]> {
    if region.iter().any(|value| !value.is_finite())
        || viewport
            .iter()
            .any(|size| !size.is_finite() || *size <= 0.0)
        || region[2] <= 0.0
        || region[3] <= 0.0
    {
        return None;
    }
    let clipped = intersect_overlay_rect(region, [0.0, 0.0, viewport[0], viewport[1]]);
    (clipped[2] > 0.0 && clipped[3] > 0.0).then_some(clipped)
}

fn translate_feedback_overlay(mut overlay: PanelOverlay, region: [f32; 4]) -> PanelOverlay {
    let translate = |mut rect: [f32; 4]| {
        rect[0] += region[0];
        rect[1] += region[1];
        rect
    };
    for quad in &mut overlay.quads {
        quad.rect = translate(quad.rect);
        quad.clip_rect = Some(quad.clip_rect.map_or(region, |clip| {
            intersect_overlay_rect(translate(clip), region)
        }));
    }
    for glyph in &mut overlay.atlas_glyphs {
        glyph.rect = translate(glyph.rect);
        glyph.clip_rect = Some(glyph.clip_rect.map_or(region, |clip| {
            intersect_overlay_rect(translate(clip), region)
        }));
    }
    overlay
}

fn region_feedback_overlay(
    text: String,
    region: [f32; 4],
    viewport: [f32; 2],
    scale: f32,
    text_scale: DisplayTextScale,
    theme: ThemePreset,
) -> Option<PanelOverlay> {
    let region = visible_feedback_region(region, viewport)?;
    // Layout stays in the caller's live font context. Only the finished shapes,
    // glyphs and any clip rectangles change coordinate systems afterwards.
    let local = message_overlay(
        text,
        [0.0, region[3], region[2], 0.0],
        [region[2], region[3]],
        scale,
        text_scale,
        theme,
        MessageOverlayKind::Feedback,
    )?;
    Some(translate_feedback_overlay(local, region))
}

fn report_feedback_overlay(
    text: String,
    region: [f32; 4],
    viewport: [f32; 2],
    text_scale: DisplayTextScale,
    theme: ThemePreset,
) -> Option<PanelOverlay> {
    let region = visible_feedback_region(region, viewport)?;
    // A valid report can retain only one body row. Do not deduct another
    // tooltip's outer margin and vertical padding from that already-inset row.
    // Always match its selected font size: the generic settings feedback path
    // can silently shrink text even when it returns a nonempty overlay.
    let report_scale = (viewport[0] / 640.0).clamp(0.86, 1.08);
    let pixel_size = text_scale.label_pixel_size() * report_scale;
    let padding = [2.0 * report_scale, 0.0];
    let line_height = pixel_size * 7.0;
    if region[3] + 0.0001 < line_height || region[2] <= padding[0] * 2.0 + pixel_size * 4.65 {
        return None;
    }
    let (surface, text_color, border) = tooltip_colors(theme);
    let layout = TextBlock {
        text,
        origin: [padding[0], 0.0],
        max_width: region[2] - padding[0] * 2.0,
        pixel_size,
        letter_spacing: -0.1 * report_scale,
        line_gap: 0.0,
        max_lines: ((region[3] / line_height + 0.0001).floor() as usize)
            .min(MessageOverlayKind::Feedback.max_lines()),
        color: text_color,
        align: TextAlign::Left,
        role: TextRole::HeaderStatus,
    }
    .layout();
    if layout.atlas_glyphs.is_empty() {
        return None;
    }
    // Measure once at the final width and font size. Keep the small card at
    // the body's bottom without rewrapping or covering the report's controls.
    let height = layout.bounds[3].min(region[3]);
    let rect = [0.0, 0.0, region[2], height];
    let notice_region = [region[0], region[1] + region[3] - height, region[2], height];
    let stroke = report_scale.max(1.0).min(region[2] * 0.5).min(height * 0.5);
    Some(translate_feedback_overlay(
        PanelOverlay {
            quads: vec![
                CandidateQuad::rounded(rect, surface, 4.0 * report_scale),
                CandidateQuad::outline(rect, border, 4.0 * report_scale, stroke),
            ],
            atlas_glyphs: layout.atlas_glyphs,
        },
        notice_region,
    ))
}

fn settings_feedback_region(scene: &RenderScene, viewport: [f32; 2]) -> Option<[f32; 4]> {
    let metadata = scene.settings_scroll_metadata?;
    // The scrollbar track begins at settings_content_top, exactly like the
    // scroll body's own clipping viewport; its height is not the thumb height.
    visible_feedback_region(
        [
            0.0,
            metadata.track_rect[1],
            viewport[0],
            metadata.visible_height,
        ],
        viewport,
    )
}

fn feedback_is_visible(kind: PanelWindowKind, compact: bool, text: &str) -> bool {
    match kind {
        PanelWindowKind::Main => !compact,
        // A delivery confirmation can include user text; do not duplicate it
        // into the settings window. Only its own input-service failures belong here.
        PanelWindowKind::Settings => text.starts_with("Input method error: "),
    }
}

impl PanelState {
    fn tooltip_scale(&self) -> f32 {
        let baseline = if self.kind == PanelWindowKind::Settings {
            520.0
        } else {
            900.0
        };
        (self.renderer.scene_width / baseline).clamp(0.85, 1.55)
    }

    pub(super) fn clear_pointer_hover(&mut self) -> bool {
        self.cursor_position = None;
        let tooltip_visible = self.interaction.tooltip.clear();
        let hovered = self.interaction.hovered_interaction.take().is_some();
        let compact_hovered = std::mem::take(&mut self.interaction.compact_hovered);
        self.update_pointer_cursor();
        tooltip_visible || hovered || compact_hovered
    }

    pub(super) fn tooltip_can_arm(&self) -> bool {
        !self.interaction.last_input_was_touch
            && self.interaction.pressed_interaction.is_none()
            && !self.interaction.panel_dragging
            && !self.interaction.scale_dragging
            && !self.interaction.settings_scroll_dragging
            && !self.interaction.handwriting_dragging
            && !(self.kind == PanelWindowKind::Main && self.chrome.compact_mode)
            && !self
                .last_commit_feedback
                .as_deref()
                .is_some_and(|text| feedback_is_visible(self.kind, self.chrome.compact_mode, text))
    }

    pub(super) fn build_hover_tooltip(&self, scene: &RenderScene) -> Option<PanelOverlay> {
        if !self.tooltip_can_arm() {
            return None;
        }
        let target = self.interaction.tooltip.visible_target()?;
        let (x, y) = self.cursor_position?;
        // A settings reflow or a newly arrived candidate can move the control without a pointer event.
        if scene.hit_interaction(x, y) != Some(target.kind)
            || !scene.interactive_targets.contains(&target)
        {
            return None;
        }
        let text = self.interaction_hint(target.kind)?;
        hint_overlay(
            text,
            target.rect,
            [self.renderer.scene_width, self.renderer.scene_height],
            self.tooltip_scale(),
            self.chrome.text_scale,
            self.chrome.theme_preset,
        )
    }

    pub(super) fn build_report_input_method_error(&self) -> Option<PanelOverlay> {
        let (text, region) = self
            .report_view
            .as_ref()?
            .input_method_error_feedback(self.chrome.ui_language)?;
        report_feedback_overlay(
            text,
            region,
            [self.renderer.scene_width, self.renderer.scene_height],
            self.chrome.text_scale,
            self.chrome.theme_preset,
        )
    }

    pub(super) fn build_commit_feedback(&self, scene: &RenderScene) -> Option<PanelOverlay> {
        let text = self.last_commit_feedback.as_ref()?;
        if !feedback_is_visible(self.kind, self.chrome.compact_mode, text) {
            return None;
        }
        let text = self.chrome.ui_language.feedback(text).into_owned();
        let viewport = [self.renderer.scene_width, self.renderer.scene_height];
        if self.kind == PanelWindowKind::Settings {
            return region_feedback_overlay(
                text,
                settings_feedback_region(scene, viewport)?,
                viewport,
                self.tooltip_scale(),
                self.chrome.text_scale,
                self.chrome.theme_preset,
            );
        }
        message_overlay(
            text,
            [0.0, 0.0, self.renderer.scene_width, 0.0],
            viewport,
            self.tooltip_scale(),
            self.chrome.text_scale,
            self.chrome.theme_preset,
            MessageOverlayKind::Feedback,
        )
    }

    pub(super) fn interaction_hint(&self, kind: InteractionKind) -> Option<String> {
        let ui = self.chrome.ui_language;
        let message: Option<String> = match kind {
            InteractionKind::NativeCandidatePage(forward) => Some(format!(
                "{} · {}",
                ui.tr(if forward { "Page forward" } else { "Page back" }),
                if forward { "PageDown" } else { "PageUp" }
            )),
            InteractionKind::SetUiLanguage(language) => {
                Some(format!("Interface language: {}", language.native_name()))
            }
            InteractionKind::SeedInput => Some(if self.is_focused && self.chrome.input_focused {
                "Type here in any input tab; Enter / Esc finishes editing.".into()
            } else {
                "Click here to type with your keyboard.".into()
            }),
            InteractionKind::ToggleCompactMode => Some("Toggle floating bubble".to_string()),
            InteractionKind::InputModesToggle => Some(if self.chrome.input_modes_expanded {
                "Hide input methods".to_string()
            } else {
                "Show input methods".to_string()
            }),
            InteractionKind::InputModeButton(suzaku_map::ime::gpu::InputMode::VirtualKeyboard) => {
                Some("Virtual keyboard".to_string())
            }
            InteractionKind::InputModeButton(suzaku_map::ime::gpu::InputMode::Dictation) => {
                Some("Voice input".to_string())
            }
            InteractionKind::InputModeButton(suzaku_map::ime::gpu::InputMode::Handwriting) => {
                Some("Handwriting input".to_string())
            }
            // A grab cursor is enough; a viewport-wide tooltip would cover other controls.
            InteractionKind::InputModeButton(suzaku_map::ime::gpu::InputMode::Translation) => {
                Some("Translate the current draft".into())
            }
            InteractionKind::SetTranslationSource(language) => Some(format!(
                "Translate from {}",
                language
                    .map(|l| ui.tr(l.name()))
                    .unwrap_or(ui.tr("automatically detected language"))
            )),
            InteractionKind::SetTranslationTarget(language) => {
                Some(format!("Translate into {}", ui.tr(language.name())))
            }
            InteractionKind::TranslateText => {
                Some("Translate with the configured model; does not commit text".into())
            }
            InteractionKind::CancelTranslation => {
                Some("Discard this translation; already-sent requests cannot be recalled".into())
            }
            InteractionKind::ApplyTranslation => {
                Some("Replace the editable draft with the full translation; Enter sends it".into())
            }
            InteractionKind::TranslationPage(_) => {
                Some("Read another page of the full translation".into())
            }
            InteractionKind::DragWindow => None,
            InteractionKind::ClosePanel => Some("Hide panel to system tray".to_string()),
            InteractionKind::RetrySaveSettings => {
                Some("Check folder permissions, then retry saving settings.".into())
            }
            InteractionKind::SettingsToggle => Some(if self.kind == PanelWindowKind::Settings {
                "Close settings".to_string()
            } else {
                "Panel settings".to_string()
            }),
            InteractionKind::SetTextScale(scale) => Some(format!(
                "Text size: {}",
                ui.tr(display_text_scale_label(scale))
            )),
            InteractionKind::SetCandidateDensity(density) => Some(format!(
                "Candidate density: {}",
                ui.tr(density_label(density))
            )),
            InteractionKind::SetPreviewStyle(style) => Some(format!(
                "Preview style: {}",
                ui.tr(preview_style_label(style))
            )),
            InteractionKind::SetFontFace(face) => Some(format!("Font: {}", font_face_label(face))),
            InteractionKind::SetTextSpacing(spacing) => Some(format!(
                "Text spacing: {}",
                ui.tr(text_spacing_label(spacing))
            )),
            InteractionKind::SetTextSmoothing(smoothing) => Some(format!(
                "Text smoothing: {}",
                ui.tr(smoothing_label(smoothing))
            )),
            InteractionKind::SetThemePreset(theme) => {
                Some(format!("Theme: {}", ui.tr(theme_preset_label(theme))))
            }
            InteractionKind::SetHideSystemTitlebar(hide) => Some(if hide {
                "Hide the system title bar; Suzaku drag and close controls stay available".into()
            } else {
                "Show the system title bar on the panel and settings".into()
            }),
            InteractionKind::SetPanelLayoutMode(mode) => Some(format!(
                "{}: {}",
                ui.tr("Panel Layout"),
                ui.tr(if mode == suzaku_map::ime::gpu::PanelLayoutMode::Auto {
                    if self.chrome.detected_panel_layout
                        == suzaku_map::ime::gpu::PanelLayoutMode::BottomDock
                    {
                        "Auto · Bottom dock"
                    } else {
                        "Auto · Follow caret"
                    }
                } else {
                    mode.label()
                })
            )),
            InteractionKind::DecreaseWindowScale => Some("Shrink window scale".to_string()),
            InteractionKind::DragWindowScale => Some("Drag to resize window".to_string()),
            InteractionKind::IncreaseWindowScale => Some("Enlarge window scale".to_string()),
            InteractionKind::ResetWindowScale => Some("Reset scale to 100%".to_string()),
            InteractionKind::SetVoiceAutoInsert(enabled) => Some(if enabled {
                "Voice auto insert: on".to_string()
            } else {
                "Voice auto insert: off".to_string()
            }),
            InteractionKind::SetLlmEnabled(enabled) => Some(if enabled {
                "LLM suggestions: on".to_string()
            } else {
                "LLM suggestions: off".to_string()
            }),
            InteractionKind::SetLlmModel(model) => {
                Some(format!("LLM model: {}", ui.tr(llm_model_label(model))))
            }
            InteractionKind::OpenModelSetup => Some("Configuration path and guide".into()),
            InteractionKind::SetLlmTemperature(temp) => Some(format!(
                "LLM creativity: {}",
                ui.tr(llm_temperature_label(temp))
            )),
            InteractionKind::SetPointerTapSlopTenths(value) => {
                Some(format!("Tap slop: {:.1}px", value as f32 / 10.0))
            }
            InteractionKind::SetPointerTapMaxMs(value) => Some(format!("Tap timeout: {}ms", value)),
            InteractionKind::SetPointerTargetSlopTenths(value) => {
                Some(format!("Target slop: {:.1}px", value as f32 / 10.0))
            }
            InteractionKind::SettingsSearchInput => None,
            InteractionKind::SetSettingsCategory(category) => {
                Some(format!("{} settings", ui.tr(category.label())))
            }
            InteractionKind::SetShortcutProfile(_) => {
                Some("IBus input shortcuts only. Standard keeps Alt keys for the app.".into())
            }
            InteractionKind::RecordShortcut(_) => {
                Some("Use Ctrl or Alt, optionally Shift, with A–Z or Space. Esc cancels.".into())
            }
            InteractionKind::ClearShortcutBinding(_) => {
                Some("Restore this action to the selected preset.".into())
            }
            InteractionKind::ResetCustomShortcuts => {
                Some("Clear custom keys without changing the selected preset.".into())
            }
            InteractionKind::ShortcutReference(_) => None,
            InteractionKind::SettingsSearchClear => Some("Clear settings search".to_string()),
            InteractionKind::ToggleSettingsSection(section_index) => {
                Some(format!("Toggle settings section {section_index}"))
            }
            InteractionKind::SettingsScrollTrack | InteractionKind::SettingsScrollHandle => None,
            InteractionKind::SelectNextToken(index) => self
                .next_token_completions
                .get(index)
                .map(|edit| format!("Complete word: {}", edit.seed_after)),
            InteractionKind::RewindNextToken => Some("Undo last word completion".to_string()),
            InteractionKind::ToggleVoiceCapture => {
                Some(if self.chrome.voice_state == VoiceCaptureState::Listening {
                    "Stop listening".to_string()
                } else {
                    "Start listening".to_string()
                })
            }
            InteractionKind::OpenVoiceSettings => Some("Open system voice settings".to_string()),
            InteractionKind::RefreshVoicePermissions => {
                Some("Refresh microphone and speech permissions".to_string())
            }
            InteractionKind::CycleVoiceSample => Some("Use next sample transcript".to_string()),
            InteractionKind::InsertVoiceTranscript => {
                Some("Insert transcript into seed input".to_string())
            }
            InteractionKind::ClearVoiceTranscript => Some("Clear captured transcript".to_string()),
            InteractionKind::HandwritingCanvas => None,
            InteractionKind::UndoHandwritingStroke => {
                Some("Undo last handwriting stroke".to_string())
            }
            InteractionKind::ClearHandwriting => Some("Clear handwriting strokes".to_string()),
            InteractionKind::UseHandwritingCandidate(index) => {
                self.chrome.handwriting_candidates.get(index).cloned()
            }
            InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Backspace) => {
                Some("Backspace".to_string())
            }
            InteractionKind::VirtualKeyboardKey(_) => None,
            InteractionKind::Candidate(index) => self
                .chrome
                .sentence_candidate_source_indices
                .iter()
                .position(|candidate_index| *candidate_index == index)
                .and_then(|display_index| {
                    self.chrome.sentence_candidates.get(display_index).cloned()
                })
                .or_else(|| self.chrome.sentence_candidates.get(index).cloned()),
        };
        message.map(|text| {
            if matches!(
                kind,
                InteractionKind::Candidate(_) | InteractionKind::UseHandwritingCandidate(_)
            ) {
                text
            } else {
                ui.message(&text).into_owned()
            }
        })
    }
}

pub(super) fn display_text_scale_label(
    value: suzaku_map::ime::gpu::DisplayTextScale,
) -> &'static str {
    match value {
        suzaku_map::ime::gpu::DisplayTextScale::Small => "Small",
        suzaku_map::ime::gpu::DisplayTextScale::Medium => "Medium",
        suzaku_map::ime::gpu::DisplayTextScale::Large => "Large",
    }
}

pub(super) fn density_label(value: suzaku_map::ime::gpu::CandidateDensity) -> &'static str {
    match value {
        suzaku_map::ime::gpu::CandidateDensity::Compact => "Compact",
        suzaku_map::ime::gpu::CandidateDensity::Cozy => "Cozy",
    }
}

pub(super) fn preview_style_label(value: suzaku_map::ime::gpu::PreviewStyle) -> &'static str {
    match value {
        suzaku_map::ime::gpu::PreviewStyle::Compact => "Compact",
        suzaku_map::ime::gpu::PreviewStyle::Full => "Full",
    }
}

pub(super) fn font_face_label(value: suzaku_map::ime::gpu::FontFaceChoice) -> &'static str {
    match value {
        suzaku_map::ime::gpu::FontFaceChoice::Auto => "Auto",
        suzaku_map::ime::gpu::FontFaceChoice::Monaco => "Monaco",
        suzaku_map::ime::gpu::FontFaceChoice::Menlo => "Menlo",
        suzaku_map::ime::gpu::FontFaceChoice::Geneva => "Geneva",
        suzaku_map::ime::gpu::FontFaceChoice::Helvetica => "Helvetica",
        suzaku_map::ime::gpu::FontFaceChoice::PingFang => "PingFang",
        suzaku_map::ime::gpu::FontFaceChoice::ArialUnicode => "Arial Unicode",
    }
}

pub(super) fn text_spacing_label(value: suzaku_map::ime::gpu::TextSpacing) -> &'static str {
    match value {
        suzaku_map::ime::gpu::TextSpacing::Tight => "Tight",
        suzaku_map::ime::gpu::TextSpacing::Normal => "Normal",
        suzaku_map::ime::gpu::TextSpacing::Relaxed => "Relaxed",
    }
}

pub(super) fn smoothing_label(value: suzaku_map::ime::gpu::TextSmoothing) -> &'static str {
    match value {
        suzaku_map::ime::gpu::TextSmoothing::Sharp => "Sharp",
        suzaku_map::ime::gpu::TextSmoothing::Smooth => "Smooth",
    }
}

pub(super) fn theme_preset_label(value: suzaku_map::ime::gpu::ThemePreset) -> &'static str {
    value.label()
}

pub(super) fn llm_temperature_label(
    value: suzaku_map::ime::gpu::LlmTemperaturePreset,
) -> &'static str {
    match value {
        suzaku_map::ime::gpu::LlmTemperaturePreset::Focused => "Focused",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced => "Balanced",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Expressive => "Expressive",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Custom(_) => "Custom",
    }
}

pub(super) fn llm_model_label(value: LlmModelPreset) -> &'static str {
    match value {
        LlmModelPreset::Configured => "Configured service (suzaku_tool model configure)",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HoverTooltipState, MessageOverlayKind, TOOLTIP_DELAY, feedback_is_visible, hint_overlay,
        message_overlay_layout, region_feedback_overlay, settings_feedback_region,
        translate_feedback_overlay, visible_feedback_region,
    };
    use super::{
        density_label, display_text_scale_label, font_face_label, llm_model_label,
        llm_temperature_label, preview_style_label, smoothing_label, text_spacing_label,
        theme_preset_label,
    };
    use std::time::{Duration, Instant};
    use suzaku_map::ime::gpu::{
        CandidateDensity, DisplayTextScale, FontFaceChoice, LlmModelPreset, LlmTemperaturePreset,
        PreviewStyle, TextSmoothing, TextSpacing, ThemePreset,
    };
    use suzaku_map::ime::gpu::{InteractionKind, InteractiveTarget};
    use suzaku_map::ui::UiLanguage;

    fn target() -> InteractiveTarget {
        InteractiveTarget {
            kind: InteractionKind::SettingsToggle,
            rect: [290.0, 12.0, 22.0, 22.0],
        }
    }

    #[test]
    fn tooltip_reveals_once_after_a_stationary_hover_without_continuous_frames() {
        let now = Instant::now();
        let mut tooltip = HoverTooltipState::default();
        tooltip.track(Some(target()), now);
        assert_eq!(tooltip.deadline(), Some(now + TOOLTIP_DELAY));
        assert!(!tooltip.advance(now + TOOLTIP_DELAY - Duration::from_millis(1)));
        // Motion within the same button keeps the original deadline and anchor.
        tooltip.track(Some(target()), now + Duration::from_millis(200));
        assert_eq!(tooltip.deadline(), Some(now + TOOLTIP_DELAY));
        assert!(tooltip.advance(now + TOOLTIP_DELAY));
        assert_eq!(tooltip.visible_target(), Some(target()));
        assert_eq!(tooltip.deadline(), None);
        assert!(!tooltip.advance(now + TOOLTIP_DELAY * 2));
    }

    #[test]
    fn leaving_or_losing_focus_clears_visible_and_pending_tooltips() {
        for visible in [false, true] {
            let now = Instant::now();
            let mut tooltip = HoverTooltipState::default();
            tooltip.track(Some(target()), now);
            if visible {
                tooltip.advance(now + TOOLTIP_DELAY);
            }
            assert_eq!(tooltip.clear(), visible);
            assert!(tooltip.visible_target().is_none());
            assert!(tooltip.deadline().is_none());
            assert!(!tooltip.advance(now + TOOLTIP_DELAY * 2));
        }
    }

    #[test]
    fn clicking_or_typing_suppresses_the_hint_until_the_pointer_leaves_the_control() {
        let now = Instant::now();
        let mut tooltip = HoverTooltipState::default();
        tooltip.track(Some(target()), now);
        tooltip.advance(now + TOOLTIP_DELAY);
        assert!(tooltip.dismiss());
        tooltip.track(Some(target()), now + TOOLTIP_DELAY * 2);
        assert!(tooltip.deadline().is_none());
        assert!(tooltip.visible_target().is_none());
        tooltip.track(None, now + TOOLTIP_DELAY * 2);
        tooltip.track(Some(target()), now + TOOLTIP_DELAY * 2);
        assert_eq!(tooltip.deadline(), Some(now + TOOLTIP_DELAY * 3));
    }

    #[test]
    fn changing_targets_or_reflowing_buttons_restarts_the_delay() {
        let now = Instant::now();
        let mut tooltip = HoverTooltipState::default();
        tooltip.track(Some(target()), now);
        tooltip.advance(now + TOOLTIP_DELAY);
        let mut moved = target();
        moved.rect[0] -= 40.0;
        assert!(tooltip.track(Some(moved), now + TOOLTIP_DELAY));
        assert!(tooltip.visible_target().is_none());
        assert_eq!(tooltip.deadline(), Some(now + TOOLTIP_DELAY * 2));
    }

    #[test]
    fn tooltip_surfaces_are_opaque_and_text_stays_inside_the_window_at_all_edges() {
        for viewport in [
            [420.0, 300.0],
            [520.0, 340.0],
            [900.0, 480.0],
            [1395.0, 806.0],
        ] {
            for scale in [0.85, 1.0, 1.55] {
                for anchor in [
                    [0.0, 0.0, 24.0, 24.0],
                    [viewport[0] - 24.0, 0.0, 24.0, 24.0],
                    [0.0, viewport[1] - 24.0, 24.0, 24.0],
                    [viewport[0] - 24.0, viewport[1] - 24.0, 24.0, 24.0],
                ] {
                    for theme in [
                        ThemePreset::Daylight,
                        ThemePreset::Solarized,
                        ThemePreset::DeviceDark,
                        ThemePreset::HighContrast,
                    ] {
                        let overlay = hint_overlay("Refresh microphone and speech permissions; long hints must wrap inside the panel".into(), anchor, viewport, scale, DisplayTextScale::Small, theme).unwrap();
                        let rect = overlay.quads[0].rect;
                        assert!(rect[0] >= 0.0 && rect[1] >= 0.0);
                        assert!(
                            rect[0] + rect[2] <= viewport[0] && rect[1] + rect[3] <= viewport[1]
                        );
                        assert!(
                            rect[1] >= anchor[1] + anchor[3] || rect[1] + rect[3] <= anchor[1],
                            "tooltip covers its own control"
                        );
                        assert!(overlay.quads.iter().all(|quad| quad.color[3] == 1.0));
                        for glyph in &overlay.atlas_glyphs {
                            assert!(glyph.rect[0] >= rect[0] && glyph.rect[1] >= rect[1]);
                            assert!(glyph.rect[0] + glyph.rect[2] <= rect[0] + rect[2] + 0.01);
                            assert!(glyph.rect[1] + glyph.rect[3] <= rect[1] + rect[3] + 0.01);
                        }
                    }
                }
            }
        }
        assert!(
            hint_overlay(
                "hint".into(),
                [0.0; 4],
                [10.0, 10.0],
                1.0,
                DisplayTextScale::Small,
                ThemePreset::Daylight
            )
            .is_none()
        );
    }

    fn assert_overlay_layout_is_bounded(
        rect: [f32; 4],
        layout: &suzaku_map::ime::gpu::TextLayout,
        viewport: [f32; 2],
    ) {
        assert!(rect.iter().all(|value| value.is_finite()));
        assert!(rect[0] >= 0.0 && rect[1] >= 0.0);
        assert!(rect[2] > 0.0 && rect[3] > 0.0);
        assert!(rect[0] + rect[2] <= viewport[0] + 0.01);
        assert!(rect[1] + rect[3] <= viewport[1] + 0.01);
        for glyph in &layout.atlas_glyphs {
            assert!(glyph.rect.iter().all(|value| value.is_finite()));
            assert!(glyph.rect[0] >= rect[0] && glyph.rect[1] >= rect[1]);
            assert!(glyph.rect[0] + glyph.rect[2] <= rect[0] + rect[2] + 0.01);
            assert!(glyph.rect[1] + glyph.rect[3] <= rect[1] + rect[3] + 0.01);
        }
    }

    #[test]
    fn input_service_feedback_belongs_to_settings_without_mirroring_user_delivery_text() {
        use super::PanelWindowKind;

        for source in [
            "Input method error: Cannot read IBus status; retry",
            "Input method error: 未知错误_日_ß:{}",
        ] {
            for compact in [false, true] {
                assert!(feedback_is_visible(
                    PanelWindowKind::Settings,
                    compact,
                    source
                ));
                assert_eq!(
                    feedback_is_visible(PanelWindowKind::Main, compact, source),
                    !compact
                );
            }
        }
        for source in [
            "Sent to active app: Input method error: private user text",
            "Delivery not confirmed · draft retained; check target before retrying · target error",
            "Input added to the native composition.",
            "Input method error without the explicit boundary",
            "Settings not saved. Check folder permissions and retry in Settings.",
        ] {
            assert!(!feedback_is_visible(
                PanelWindowKind::Settings,
                false,
                source
            ));
            assert!(feedback_is_visible(PanelWindowKind::Main, false, source));
            assert!(!feedback_is_visible(PanelWindowKind::Main, true, source));
        }
    }

    #[test]
    fn feedback_has_a_wider_six_line_budget_without_changing_the_tooltip_budget() {
        // Label wrapping normalizes all whitespace, including '\n'. Supply
        // enough actual content to exceed both budgets instead of assuming
        // that eight source newlines create eight rendered lines.
        let text = "Read this line before retrying.\n".repeat(32);
        let viewport = [900.0, 480.0];
        let anchor = [0.0, 0.0, viewport[0], 0.0];
        let (hint_rect, hint) = message_overlay_layout(
            text.clone(),
            anchor,
            viewport,
            1.0,
            DisplayTextScale::Medium,
            ThemePreset::Suzaku,
            MessageOverlayKind::Tooltip,
        )
        .unwrap();
        let (feedback_rect, feedback) = message_overlay_layout(
            text,
            anchor,
            viewport,
            1.0,
            DisplayTextScale::Medium,
            ThemePreset::Suzaku,
            MessageOverlayKind::Feedback,
        )
        .unwrap();
        assert_eq!(hint.lines.len(), 3);
        assert_eq!(feedback.lines.len(), 6);
        assert!(hint.truncated && feedback.truncated);
        assert!(hint_rect[2] <= 340.0);
        assert!(feedback_rect[2] <= 600.0);
        assert!(feedback_rect[3] > hint_rect[3]);
        assert_overlay_layout_is_bounded(hint_rect, &hint, viewport);
        assert_overlay_layout_is_bounded(feedback_rect, &feedback, viewport);
        // Preserve the original failing fixture as the opposite control: its
        // natural text fits below the six-line cap and must not be padded or
        // truncated simply to consume the available budget.
        let short_text = "Read this line before retrying.\n".repeat(8);
        let (short_rect, short_feedback) = message_overlay_layout(
            short_text.clone(),
            anchor,
            viewport,
            1.0,
            DisplayTextScale::Medium,
            ThemePreset::Suzaku,
            MessageOverlayKind::Feedback,
        )
        .unwrap();
        assert!(short_feedback.lines.len() < 6);
        assert!(!short_feedback.truncated);
        assert_eq!(
            short_feedback.lines.join(" "),
            short_text.split_whitespace().collect::<Vec<_>>().join(" ")
        );
        assert_overlay_layout_is_bounded(short_rect, &short_feedback, viewport);
        let text =
            "Wider readable feedback keeps more of a long diagnostic on each line. ".repeat(4);
        let (hint_rect, _) = message_overlay_layout(
            text.clone(),
            anchor,
            viewport,
            1.0,
            DisplayTextScale::Medium,
            ThemePreset::Suzaku,
            MessageOverlayKind::Tooltip,
        )
        .unwrap();
        let (feedback_rect, _) = message_overlay_layout(
            text,
            anchor,
            viewport,
            1.0,
            DisplayTextScale::Medium,
            ThemePreset::Suzaku,
            MessageOverlayKind::Feedback,
        )
        .unwrap();
        assert!(feedback_rect[2] > hint_rect[2]);
    }

    #[test]
    fn tooltip_and_feedback_use_the_same_accessible_label_font_sizes() {
        for kind in [MessageOverlayKind::Tooltip, MessageOverlayKind::Feedback] {
            let mut previous_height = 0.0;
            for text_scale in [
                DisplayTextScale::Small,
                DisplayTextScale::Medium,
                DisplayTextScale::Large,
            ] {
                let (rect, layout) = message_overlay_layout(
                    "Readable feedback".into(),
                    [0.0, 0.0, 900.0, 0.0],
                    [900.0, 480.0],
                    1.0,
                    text_scale,
                    ThemePreset::Suzaku,
                    kind,
                )
                .unwrap();
                let glyph_height = layout.atlas_glyphs[0].rect[3];
                assert!((glyph_height - text_scale.label_pixel_size() * 7.0).abs() < 0.001);
                assert!(glyph_height > previous_height);
                previous_height = glyph_height;
                assert_overlay_layout_is_bounded(rect, &layout, [900.0, 480.0]);
            }
        }
    }

    #[test]
    fn long_eight_language_feedback_and_tooltips_stay_inside_three_text_size_viewports() {
        for ui in UiLanguage::ALL
            .into_iter()
            .filter(|ui| *ui != UiLanguage::System)
        {
            let source = format!(
                "Input method error: Cannot control the Suzaku user service: {}",
                "raw_日_ß:{} / Permission denied (os error 13) ".repeat(5)
            );
            let text = ui.feedback(&source).into_owned();
            for text_scale in [
                DisplayTextScale::Small,
                DisplayTextScale::Medium,
                DisplayTextScale::Large,
            ] {
                for viewport in [
                    [420.0, 300.0],
                    [520.0, 340.0],
                    [900.0, 480.0],
                    [1395.0, 806.0],
                ] {
                    for scale in [0.85, 1.0, 1.55] {
                        for kind in [MessageOverlayKind::Tooltip, MessageOverlayKind::Feedback] {
                            let (rect, layout) = message_overlay_layout(
                                text.clone(),
                                [0.0, 0.0, viewport[0], 0.0],
                                viewport,
                                scale,
                                text_scale,
                                ThemePreset::Suzaku,
                                kind,
                            )
                            .unwrap();
                            assert!(
                                !layout.atlas_glyphs.is_empty(),
                                "{ui:?}/{text_scale:?}/{viewport:?}"
                            );
                            assert!(layout.lines.len() <= kind.max_lines());
                            assert!(rect[2] <= kind.max_width() * scale + 0.01);
                            assert_overlay_layout_is_bounded(rect, &layout, viewport);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn tiny_or_invalid_message_viewports_do_not_overflow_or_cover_the_tooltip_control() {
        for kind in [MessageOverlayKind::Tooltip, MessageOverlayKind::Feedback] {
            for viewport in [
                [0.0, 0.0],
                [10.0, 10.0],
                [f32::NAN, 100.0],
                [100.0, f32::INFINITY],
            ] {
                assert!(
                    message_overlay_layout(
                        "Read the retained error".into(),
                        [0.0; 4],
                        viewport,
                        1.0,
                        DisplayTextScale::Large,
                        ThemePreset::Suzaku,
                        kind,
                    )
                    .is_none()
                );
            }
            for viewport in [[48.0, 48.0], [72.0, 72.0], [420.0, 72.0]] {
                for text_scale in [
                    DisplayTextScale::Small,
                    DisplayTextScale::Medium,
                    DisplayTextScale::Large,
                ] {
                    for scale in [0.85, 1.0, 1.55] {
                        let anchor = [0.0, 20.0, 24.0, 24.0];
                        if let Some((rect, layout)) = message_overlay_layout(
                            "Very long feedback must stay bounded. ".repeat(10),
                            anchor,
                            viewport,
                            scale,
                            text_scale,
                            ThemePreset::Suzaku,
                            kind,
                        ) {
                            assert_overlay_layout_is_bounded(rect, &layout, viewport);
                            if kind == MessageOverlayKind::Tooltip {
                                assert!(
                                    rect[1] >= anchor[1] + anchor[3]
                                        || rect[1] + rect[3] <= anchor[1]
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn overlay_measurement_respects_the_current_proportional_font_context() {
        use std::{collections::HashMap, sync::Arc};
        use suzaku_map::ime::gpu::with_font_metrics;

        for kind in [MessageOverlayKind::Tooltip, MessageOverlayKind::Feedback] {
            let measure = || {
                message_overlay_layout(
                    "WWWWWWWW".into(),
                    [0.0, 0.0, 900.0, 0.0],
                    [900.0, 480.0],
                    1.0,
                    DisplayTextScale::Medium,
                    ThemePreset::Suzaku,
                    kind,
                )
                .unwrap()
            };
            let (narrow_rect, narrow) =
                with_font_metrics(Arc::new(HashMap::from([('W', 2.0)])), measure);
            let (wide_rect, wide) =
                with_font_metrics(Arc::new(HashMap::from([('W', 5.8)])), measure);
            assert!(wide_rect[2] > narrow_rect[2]);
            assert!(wide.atlas_glyphs[0].rect[2] > narrow.atlas_glyphs[0].rect[2] * 2.0);
            assert_overlay_layout_is_bounded(narrow_rect, &narrow, [900.0, 480.0]);
            assert_overlay_layout_is_bounded(wide_rect, &wide, [900.0, 480.0]);
        }
    }

    #[test]
    fn short_feedback_keeps_its_final_word_at_large_scale_after_tightening_the_surface() {
        use std::{collections::HashMap, sync::Arc};
        use suzaku_map::ime::gpu::with_font_metrics;

        let text = "Input method error: Font: Settings";
        let expected: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
        for proportional in [false, true] {
            let metrics: HashMap<_, _> = text
                .chars()
                .map(|ch| {
                    let width = if !proportional {
                        3.4
                    } else {
                        match ch {
                            'I' | 'i' | 'l' | 't' | ':' => 1.6,
                            'm' | 'o' | 'S' => 5.8,
                            _ => 3.1,
                        }
                    };
                    (ch, width)
                })
                .collect();
            with_font_metrics(Arc::new(metrics), || {
                for text_scale in [
                    DisplayTextScale::Small,
                    DisplayTextScale::Medium,
                    DisplayTextScale::Large,
                ] {
                    for scale in [0.85, 1.0, 1.55] {
                        let viewport = [900.0, 540.0];
                        let (rect, layout) = message_overlay_layout(
                            text.into(),
                            [0.0, 0.0, viewport[0], 0.0],
                            viewport,
                            scale,
                            text_scale,
                            ThemePreset::Suzaku,
                            MessageOverlayKind::Feedback,
                        )
                        .unwrap();
                        assert!(
                            !layout.truncated,
                            "{proportional}/{text_scale:?}/{scale}: {:?}",
                            layout.lines
                        );
                        assert_eq!(layout.lines, [text]);
                        assert_eq!(
                            layout
                                .atlas_glyphs
                                .iter()
                                .map(|glyph| glyph.ch)
                                .collect::<String>(),
                            expected
                        );
                        assert!(
                            (layout.atlas_glyphs[0].rect[3]
                                - text_scale.label_pixel_size() * scale * 7.0)
                                .abs()
                                < 0.001
                        );
                        assert!(
                            rect[2] < MessageOverlayKind::Feedback.max_width() * scale,
                            "short feedback should not fill the entire width budget"
                        );
                        assert_overlay_layout_is_bounded(rect, &layout, viewport);
                        let region = [0.0, 140.0, viewport[0], 400.0];
                        let regional = region_feedback_overlay(
                            text.into(),
                            region,
                            viewport,
                            scale,
                            text_scale,
                            ThemePreset::Suzaku,
                        )
                        .unwrap();
                        assert_eq!(
                            regional
                                .atlas_glyphs
                                .iter()
                                .map(|glyph| glyph.ch)
                                .collect::<String>(),
                            expected
                        );
                        assert!(
                            regional
                                .atlas_glyphs
                                .iter()
                                .all(|glyph| glyph.clip_rect == Some(region))
                        );
                    }
                }
            });
        }
    }

    #[test]
    fn regional_feedback_translates_shapes_glyphs_and_their_existing_clip_rectangles() {
        use super::PanelOverlay;
        use suzaku_map::ime::gpu::{AtlasGlyph, CandidateQuad};

        let region = [40.0, 120.0, 300.0, 140.0];
        let mut quad = CandidateQuad::rounded([8.0, 20.0, 100.0, 50.0], [1.0; 4], 6.0);
        quad.clip_rect = Some([-10.0, 2.0, 400.0, 250.0]);
        let overlay = PanelOverlay {
            quads: vec![quad],
            atlas_glyphs: vec![
                AtlasGlyph {
                    ch: 'W',
                    rect: [9.0, 22.0, 10.0, 14.0],
                    color: [1.0; 4],
                    clip_rect: Some([4.0, 5.0, 200.0, 100.0]),
                },
                AtlasGlyph {
                    ch: '日',
                    rect: [20.0, 22.0, 12.0, 14.0],
                    color: [1.0; 4],
                    clip_rect: None,
                },
            ],
        };
        let moved = translate_feedback_overlay(overlay, region);
        assert_eq!(moved.quads[0].rect, [48.0, 140.0, 100.0, 50.0]);
        assert_eq!(moved.quads[0].clip_rect, Some([40.0, 122.0, 300.0, 138.0]));
        assert_eq!(moved.atlas_glyphs[0].rect, [49.0, 142.0, 10.0, 14.0]);
        assert_eq!(
            moved.atlas_glyphs[0].clip_rect,
            Some([44.0, 125.0, 200.0, 100.0])
        );
        assert_eq!(moved.atlas_glyphs[1].rect, [60.0, 142.0, 12.0, 14.0]);
        assert_eq!(moved.atlas_glyphs[1].clip_rect, Some(region));
        assert_eq!(
            moved
                .atlas_glyphs
                .iter()
                .map(|glyph| glyph.ch)
                .collect::<String>(),
            "W日"
        );
    }

    #[test]
    fn settings_feedback_uses_the_visible_body_bottom_without_covering_the_header() {
        use suzaku_map::ime::gpu::{PanelChromeState, WgpuCandidateRenderer};

        let viewport = [520.0, 540.0];
        for ui in UiLanguage::ALL
            .into_iter()
            .filter(|ui| *ui != UiLanguage::System)
        {
            for text_scale in [
                DisplayTextScale::Small,
                DisplayTextScale::Medium,
                DisplayTextScale::Large,
            ] {
                let chrome = PanelChromeState {
                    ui_language: ui,
                    text_scale,
                    ..Default::default()
                };
                let mut scene = WgpuCandidateRenderer::new(viewport[0], viewport[1])
                    .build_settings_scene(&chrome, None);
                let metadata = scene.settings_scroll_metadata.unwrap();
                let region = settings_feedback_region(&scene, viewport).unwrap();
                assert_eq!(region[1], metadata.track_rect[1]);
                assert!((region[3] - metadata.visible_height).abs() < 0.01);
                let text = ui
                    .feedback("Input method error: Font: Settings")
                    .into_owned();
                let overlay = region_feedback_overlay(
                    text.clone(),
                    region,
                    viewport,
                    1.0,
                    text_scale,
                    ThemePreset::Suzaku,
                )
                .unwrap();
                let rect = overlay.quads[0].rect;
                assert!(rect[1] >= region[1]);
                assert!((rect[1] + rect[3] - (region[1] + region[3] - 8.0)).abs() < 0.02);
                for glyph in &overlay.atlas_glyphs {
                    assert!(glyph.rect[0] >= rect[0] && glyph.rect[1] >= rect[1]);
                    assert!(glyph.rect[0] + glyph.rect[2] <= rect[0] + rect[2] + 0.01);
                    assert!(glyph.rect[1] + glyph.rect[3] <= rect[1] + rect[3] + 0.01);
                    assert_eq!(glyph.clip_rect, Some(region));
                }
                assert!(
                    overlay
                        .quads
                        .iter()
                        .all(|quad| quad.clip_rect == Some(region))
                );
                let offscreen_region = [-20.0, region[1], 600.0, 800.0];
                let clipped = visible_feedback_region(offscreen_region, viewport).unwrap();
                let clipped_overlay = region_feedback_overlay(
                    text.clone(),
                    offscreen_region,
                    viewport,
                    1.0,
                    text_scale,
                    ThemePreset::Suzaku,
                )
                .unwrap();
                assert_eq!(clipped_overlay.quads[0].clip_rect, Some(clipped));
                assert!(clipped_overlay.quads[0].rect[1] >= region[1]);
                assert!(
                    clipped_overlay.quads[0].rect[1] + clipped_overlay.quads[0].rect[3]
                        <= viewport[1]
                );
                scene.settings_scroll_metadata = None;
                assert!(settings_feedback_region(&scene, viewport).is_none());
                for bad_region in [
                    [0.0, 600.0, 520.0, 100.0],
                    [0.0, region[1], 520.0, 10.0],
                    [0.0, f32::NAN, 520.0, 100.0],
                ] {
                    assert!(
                        region_feedback_overlay(
                            text.clone(),
                            bad_region,
                            viewport,
                            1.0,
                            text_scale,
                            ThemePreset::Suzaku,
                        )
                        .is_none()
                    );
                }
            }
        }
    }

    #[test]
    fn panel_hint_scale_labels_cover_expected_values() {
        assert_eq!(display_text_scale_label(DisplayTextScale::Small), "Small");
        assert_eq!(display_text_scale_label(DisplayTextScale::Medium), "Medium");
        assert_eq!(display_text_scale_label(DisplayTextScale::Large), "Large");
        assert_eq!(density_label(CandidateDensity::Compact), "Compact");
        assert_eq!(density_label(CandidateDensity::Cozy), "Cozy");
    }

    #[test]
    fn panel_hint_style_labels_cover_expected_values() {
        assert_eq!(preview_style_label(PreviewStyle::Compact), "Compact");
        assert_eq!(preview_style_label(PreviewStyle::Full), "Full");
        assert_eq!(text_spacing_label(TextSpacing::Tight), "Tight");
        assert_eq!(text_spacing_label(TextSpacing::Normal), "Normal");
        assert_eq!(text_spacing_label(TextSpacing::Relaxed), "Relaxed");
        assert_eq!(smoothing_label(TextSmoothing::Sharp), "Sharp");
        assert_eq!(smoothing_label(TextSmoothing::Smooth), "Smooth");
    }

    #[test]
    fn panel_hint_font_and_theme_labels_are_stable() {
        assert_eq!(font_face_label(FontFaceChoice::Auto), "Auto");
        assert_eq!(font_face_label(FontFaceChoice::Monaco), "Monaco");
        assert_eq!(
            font_face_label(FontFaceChoice::ArialUnicode),
            "Arial Unicode"
        );
        assert_eq!(theme_preset_label(ThemePreset::Daylight), "Daylight");
        assert_eq!(theme_preset_label(ThemePreset::DeviceDark), "Device Dark");
        assert_eq!(
            theme_preset_label(ThemePreset::HighContrast),
            "High Contrast"
        );
    }

    #[test]
    fn panel_hint_temperature_labels_are_readable() {
        assert_eq!(
            llm_temperature_label(LlmTemperaturePreset::Focused),
            "Focused"
        );
        assert_eq!(
            llm_temperature_label(LlmTemperaturePreset::Balanced),
            "Balanced"
        );
        assert_eq!(
            llm_temperature_label(LlmTemperaturePreset::Expressive),
            "Expressive"
        );
    }

    #[test]
    fn panel_hint_llm_model_label() {
        assert_eq!(
            llm_model_label(LlmModelPreset::Configured),
            "Configured service (suzaku_tool model configure)"
        );
    }
}
