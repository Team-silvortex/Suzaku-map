//! Read-only report events never enter settings editing or input dispatch.
use super::report_view::{PointerSource, ReportSnapshot, ReportView};
use super::{PanelInteractionState, PanelState, PanelWindowKind};
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

impl PanelState {
    pub(super) fn refresh_settings_after_report(&mut self, latest: &super::PanelChromeState) {
        let rebuild = self.chrome.font_face != latest.font_face
            || self.chrome.text_smoothing != latest.text_smoothing;
        self.chrome = restored_settings_chrome(&self.chrome, latest);
        if rebuild {
            self.rebuild_font_atlas();
        }
        self.apply_window_decorations();
    }

    pub(super) fn begin_report_details(
        &mut self,
        snapshot: ReportSnapshot,
        return_to_settings: bool,
    ) {
        debug_assert_eq!(self.kind, PanelWindowKind::Settings);
        self.chrome.cancel_shortcut_recording();
        self.text_input.clear();
        self.interaction = PanelInteractionState::default();
        self.report_view = Some(ReportView::new(snapshot));
        self.report_return_to_settings = return_to_settings;
        self.report_close_key = None;
        self.invalidate_pointer_scene();
        self.sync_text_input_state();
        self.window.set_cursor(winit::window::CursorIcon::Default);
        self.window.request_redraw();
    }

    pub(super) fn end_report_details(&mut self) {
        if self.report_view.take().is_none() {
            return;
        }
        self.report_close_key = None;
        self.report_return_to_settings = false;
        self.interaction = PanelInteractionState::default();
        self.invalidate_pointer_scene();
        self.sync_text_input_permission();
        self.window.request_redraw();
    }

    /// Escape only completes on its matching real release. Closing on key-down
    /// would let auto-repeat reach the restored settings or another window.
    pub(super) fn handle_report_key(
        &mut self,
        key: PhysicalKey,
        state: ElementState,
        repeat: bool,
    ) -> bool {
        let Some(view) = self.report_view.as_mut() else {
            return false;
        };
        if state == ElementState::Released {
            self.text_input.consume_owned_hold(key, false);
            if self.report_close_key == Some(key) {
                self.report_close_key = None;
                return true;
            }
            return false;
        }
        // Keep ownership across return to settings without disabling the
        // report's own paging repeats or Escape's matching-release close.
        self.text_input.hold_owned_key(key);
        let changed = match key {
            PhysicalKey::Code(KeyCode::Escape) => {
                if !repeat {
                    self.report_close_key = Some(key);
                }
                false
            }
            PhysicalKey::Code(KeyCode::PageDown | KeyCode::ArrowDown) => view.page(true),
            PhysicalKey::Code(KeyCode::PageUp | KeyCode::ArrowUp) => view.page(false),
            PhysicalKey::Code(KeyCode::Home) => view.first(),
            PhysicalKey::Code(KeyCode::End) => view.last(),
            _ => false,
        };
        if changed {
            self.window.request_redraw();
        }
        false
    }

    /// All events are consumed; true asks the app to dismiss this viewer only.
    pub(super) fn handle_report_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) -> bool {
        if self.report_view.is_none() {
            return false;
        }
        if self.native.occluded && super::input::is_pointer_event(&event) {
            return false;
        }
        match event {
            WindowEvent::Resized(size) => {
                self.resize(size.width, size.height);
                self.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.invalidate_pointer_scene();
                self.window.request_redraw();
            }
            WindowEvent::Occluded(occluded) => {
                self.native.occluded = occluded;
                self.invalidate_pointer_scene();
                if !occluded {
                    self.window.request_redraw();
                }
            }
            WindowEvent::Focused(focused) => {
                // Do not activate/clear an IME bridge while reading a report.
                self.is_focused = focused;
                if !focused {
                    self.text_input.clear_shortcut_hold();
                }
                self.modifiers = Default::default();
                self.report_close_key = None;
                self.text_input.clear();
                self.invalidate_pointer_scene();
                self.sync_text_input_state();
                self.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                // Reuse only the surface recovery path, never editing events.
                super::input::handle_panel_window_event(self, event_loop, event, false);
            }
            _ if !self.is_focused => {}
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. }
            | WindowEvent::X11MouseButtonPosition { position, .. } => {
                self.cursor_position = Some((position.x as f32, position.y as f32));
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_position = None;
                self.report_view.as_mut().unwrap().invalidate();
                self.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if let Some((x, y)) = self.cursor_position {
                    let view = self.report_view.as_mut().unwrap();
                    let close = if state == ElementState::Pressed {
                        view.press([x, y], PointerSource::Mouse);
                        false
                    } else {
                        view.release([x, y], PointerSource::Mouse)
                    };
                    self.window.request_redraw();
                    return close;
                }
            }
            WindowEvent::Touch(touch) => {
                let view = self.report_view.as_mut().unwrap();
                let point = [touch.location.x as f32, touch.location.y as f32];
                let source = PointerSource::Touch(touch.device_id, touch.id);
                let close = match touch.phase {
                    TouchPhase::Started => {
                        view.press(point, source);
                        false
                    }
                    TouchPhase::Ended => view.release(point, source),
                    TouchPhase::Cancelled => {
                        view.invalidate();
                        false
                    }
                    TouchPhase::Moved => false,
                };
                self.window.request_redraw();
                return close;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = super::input::wheel_vertical_delta(delta);
                if delta != 0.0 && self.report_view.as_mut().unwrap().page(delta < 0.0) {
                    self.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                return self.handle_report_key(event.physical_key, event.state, event.repeat);
            }
            // IME, text, pinch, settings shortcuts and candidate commands are
            // intentionally inert. The source snapshot is immutable.
            _ => {}
        }
        false
    }
}

fn restored_settings_chrome(
    previous: &super::PanelChromeState,
    latest: &super::PanelChromeState,
) -> super::PanelChromeState {
    // A report is not a settings edit. Never adopt its old configuration back
    // into the main panel when a newer preference arrived while reading.
    let mut restored = latest.clone();
    restored.settings_search_query = previous.settings_search_query.clone();
    restored.settings_category = previous.settings_category;
    restored.settings_search_focused = previous.settings_search_focused;
    restored.settings_keyboard_focus = previous.settings_keyboard_focus;
    restored.settings_collapsed_sections = previous.settings_collapsed_sections.clone();
    restored.settings_scroll_offset = previous.settings_scroll_offset;
    restored
}

#[cfg(test)]
mod tests {
    use super::*;
    use suzaku_map::ime::gpu::{DisplayTextScale, InteractionKind, SettingsCategory};

    #[test]
    fn leaving_a_report_keeps_navigation_but_never_rolls_back_new_preferences() {
        let previous = super::super::PanelChromeState {
            settings_search_query: "原query".into(),
            settings_category: SettingsCategory::Appearance,
            settings_search_focused: true,
            settings_keyboard_focus: Some(InteractionKind::SettingsSearchClear),
            settings_collapsed_sections: vec![true, false],
            settings_scroll_offset: 137.0,
            text_scale: DisplayTextScale::Small,
            ..Default::default()
        };
        let mut latest = previous.clone();
        latest.settings_search_query.clear();
        latest.settings_category = SettingsCategory::Shortcuts;
        latest.settings_search_focused = false;
        latest.settings_keyboard_focus = None;
        latest.settings_collapsed_sections.clear();
        latest.settings_scroll_offset = 0.0;
        latest.text_scale = DisplayTextScale::Large;
        latest.window_scale = 1.55;
        latest.shortcut_edit_generation = 19;
        latest.prediction_edit_generation = [20, 21];
        latest.settings_save_failed = true;
        let restored = restored_settings_chrome(&previous, &latest);
        assert_eq!(
            restored.settings_search_query,
            previous.settings_search_query
        );
        assert_eq!(restored.settings_category, previous.settings_category);
        assert_eq!(
            restored.settings_search_focused,
            previous.settings_search_focused
        );
        assert_eq!(
            restored.settings_keyboard_focus,
            previous.settings_keyboard_focus
        );
        assert_eq!(
            restored.settings_collapsed_sections,
            previous.settings_collapsed_sections
        );
        assert_eq!(
            restored.settings_scroll_offset,
            previous.settings_scroll_offset
        );
        assert_eq!(
            crate::app_state::PersistedDisplaySettings::from(&restored),
            crate::app_state::PersistedDisplaySettings::from(&latest)
        );
        assert_eq!(restored.shortcut_edit_generation, 19);
        assert_eq!(restored.prediction_edit_generation, [20, 21]);
        assert!(restored.settings_save_failed);
    }
}
