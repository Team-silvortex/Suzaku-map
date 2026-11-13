use super::PanelState;
use crate::helpers::point_in_rect;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use suzaku_map::ime::gpu::{InputMode, InteractionKind};
use suzaku_map::panel_support::{recognize_handwriting_candidates, summarize_handwriting_strokes};

const HANDWRITING_EDGE_PADDING: f32 = 4.0;
const HANDWRITING_MIN_POINT_DISTANCE_DEFAULT: f32 = 1.2;
const HANDWRITING_INTERPOLATION_STEP_DEFAULT: f32 = 3.2;
const HANDWRITING_SMOOTH_ALPHA_DEFAULT: f32 = 0.22;
const HANDWRITING_SPEED_REFERENCE_DEFAULT: f32 = 16.0;
const HANDWRITING_SPEED_MIN_DEFAULT: f32 = 0.55;
const HANDWRITING_SPEED_MAX_DEFAULT: f32 = 2.0;
const HANDWRITING_INTERPOLATION_STEP_MIN_DEFAULT: f32 = 2.0;
const HANDWRITING_INTERPOLATION_STEP_MAX_DEFAULT: f32 = 6.8;
const HANDWRITING_MIN_DISTANCE_MIN_DEFAULT: f32 = 0.78;
const HANDWRITING_MIN_DISTANCE_MAX_DEFAULT: f32 = 2.0;
const HANDWRITING_SMOOTH_ALPHA_MIN_DEFAULT: f32 = 0.14;
const HANDWRITING_SMOOTH_ALPHA_MAX_DEFAULT: f32 = 0.34;
const HANDWRITING_SAMPLE_INTERVAL_TOUCH_MS_DEFAULT: u64 = 8;
const HANDWRITING_SAMPLE_INTERVAL_MOUSE_MS_DEFAULT: u64 = 4;
const HANDWRITING_TOUCH_SMOOTH_FACTOR_DEFAULT: f32 = 1.05;
const HANDWRITING_TOUCH_MIN_DISTANCE_SCALE_DEFAULT: f32 = 1.18;
const HANDWRITING_TOUCH_INTERPOLATION_SCALE_DEFAULT: f32 = 0.9;
const HANDWRITING_TOUCH_SAMPLE_DISTANCE_SCALE_DEFAULT: f32 = 0.85;

#[derive(Clone, Copy)]
struct HandwritingSamplingParams {
    edge_padding: f32,
    min_point_distance: f32,
    interpolation_step: f32,
    smooth_alpha: f32,
    speed_reference: f32,
    speed_ratio_min: f32,
    speed_ratio_max: f32,
    interpolation_step_min: f32,
    interpolation_step_max: f32,
    min_distance_min: f32,
    min_distance_max: f32,
    smooth_alpha_min: f32,
    smooth_alpha_max: f32,
    sample_interval_touch_ms: u64,
    sample_interval_mouse_ms: u64,
    touch_smooth_factor: f32,
    touch_min_distance_scale: f32,
    touch_interpolation_scale: f32,
    touch_sample_distance_scale: f32,
}

impl HandwritingSamplingParams {
    fn from_env() -> Self {
        let speed_ratio_min = read_handwriting_env_f32(
            "SUZAKU_HANDWRITING_SPEED_RATIO_MIN",
            HANDWRITING_SPEED_MIN_DEFAULT,
        )
        .max(0.1);
        let speed_ratio_max = read_handwriting_env_f32(
            "SUZAKU_HANDWRITING_SPEED_RATIO_MAX",
            HANDWRITING_SPEED_MAX_DEFAULT,
        )
        .max(speed_ratio_min);
        let interpolation_step_min = read_handwriting_env_f32(
            "SUZAKU_HANDWRITING_INTERPOLATION_STEP_MIN",
            HANDWRITING_INTERPOLATION_STEP_MIN_DEFAULT,
        )
        .max(0.1);
        let interpolation_step_max = read_handwriting_env_f32(
            "SUZAKU_HANDWRITING_INTERPOLATION_STEP_MAX",
            HANDWRITING_INTERPOLATION_STEP_MAX_DEFAULT,
        )
        .max(interpolation_step_min);
        let min_distance_min = read_handwriting_env_f32(
            "SUZAKU_HANDWRITING_MIN_DISTANCE_MIN",
            HANDWRITING_MIN_DISTANCE_MIN_DEFAULT,
        )
        .max(0.01);
        let min_distance_max = read_handwriting_env_f32(
            "SUZAKU_HANDWRITING_MIN_DISTANCE_MAX",
            HANDWRITING_MIN_DISTANCE_MAX_DEFAULT,
        )
        .max(min_distance_min);
        let smooth_alpha_min = read_handwriting_env_f32(
            "SUZAKU_HANDWRITING_SMOOTH_ALPHA_MIN",
            HANDWRITING_SMOOTH_ALPHA_MIN_DEFAULT,
        )
        .clamp(0.0, 1.0);
        let smooth_alpha_max = read_handwriting_env_f32(
            "SUZAKU_HANDWRITING_SMOOTH_ALPHA_MAX",
            HANDWRITING_SMOOTH_ALPHA_MAX_DEFAULT,
        )
        .clamp(0.0, 1.0)
        .max(smooth_alpha_min);
        Self {
            edge_padding: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_EDGE_PADDING",
                HANDWRITING_EDGE_PADDING,
            )
            .max(0.0),
            min_point_distance: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_MIN_POINT_DISTANCE",
                HANDWRITING_MIN_POINT_DISTANCE_DEFAULT,
            )
            .max(0.1),
            interpolation_step: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_INTERPOLATION_STEP",
                HANDWRITING_INTERPOLATION_STEP_DEFAULT,
            )
            .max(0.1),
            smooth_alpha: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_SMOOTH_ALPHA",
                HANDWRITING_SMOOTH_ALPHA_DEFAULT,
            )
            .clamp(0.0, 1.0),
            speed_reference: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_SPEED_REFERENCE",
                HANDWRITING_SPEED_REFERENCE_DEFAULT,
            )
            .max(0.01),
            speed_ratio_min: speed_ratio_min.max(0.1),
            speed_ratio_max,
            interpolation_step_min,
            interpolation_step_max,
            min_distance_min: min_distance_min.max(0.01),
            min_distance_max,
            smooth_alpha_min,
            smooth_alpha_max,
            sample_interval_touch_ms: read_handwriting_env_u64(
                "SUZAKU_HANDWRITING_TOUCH_SAMPLE_MS",
                HANDWRITING_SAMPLE_INTERVAL_TOUCH_MS_DEFAULT,
            )
            .max(1),
            sample_interval_mouse_ms: read_handwriting_env_u64(
                "SUZAKU_HANDWRITING_MOUSE_SAMPLE_MS",
                HANDWRITING_SAMPLE_INTERVAL_MOUSE_MS_DEFAULT,
            )
            .max(1),
            touch_smooth_factor: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_TOUCH_SMOOTH_FACTOR",
                HANDWRITING_TOUCH_SMOOTH_FACTOR_DEFAULT,
            )
            .max(0.0),
            touch_min_distance_scale: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_TOUCH_MIN_DISTANCE_SCALE",
                HANDWRITING_TOUCH_MIN_DISTANCE_SCALE_DEFAULT,
            )
            .max(0.0),
            touch_interpolation_scale: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_TOUCH_INTERPOLATION_SCALE",
                HANDWRITING_TOUCH_INTERPOLATION_SCALE_DEFAULT,
            )
            .max(0.0),
            touch_sample_distance_scale: read_handwriting_env_f32(
                "SUZAKU_HANDWRITING_TOUCH_SAMPLE_DISTANCE_SCALE",
                HANDWRITING_TOUCH_SAMPLE_DISTANCE_SCALE_DEFAULT,
            )
            .max(0.0),
        }
    }
}

fn handwriting_sampling_params() -> &'static HandwritingSamplingParams {
    static PARAMS: OnceLock<HandwritingSamplingParams> = OnceLock::new();
    PARAMS.get_or_init(HandwritingSamplingParams::from_env)
}

fn read_handwriting_env_f32(key: &str, default_value: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(default_value)
}

fn read_handwriting_env_u64(key: &str, default_value: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default_value)
}

fn clamp_handwriting_point(point: [f32; 2], rect: [f32; 4]) -> [f32; 2] {
    let params = handwriting_sampling_params();
    std::array::from_fn(|axis| {
        // A pending resize can leave less than two paddings of canvas. Collapse
        // only that axis to its centre rather than expanding outside the ink.
        let padding = params.edge_padding.min(rect[axis + 2].max(0.0) * 0.5);
        let lower = rect[axis] + padding;
        // Even at half size, f32 subtraction can round below the same centre.
        let upper = (rect[axis] + rect[axis + 2] - padding).max(lower);
        point[axis].clamp(lower, upper)
    })
}

fn handwriting_speed_profile(
    distance: f32,
    previous_distance: f32,
    is_touch: bool,
) -> (f32, f32, f32) {
    let params = handwriting_sampling_params();
    let speed_hint = (distance.max(0.0) + previous_distance.max(0.0) * 0.65) * 0.5;
    let speed_ratio =
        (speed_hint / params.speed_reference).clamp(params.speed_ratio_min, params.speed_ratio_max);

    let interpolation_scale = if is_touch {
        params.touch_interpolation_scale
    } else {
        1.0
    };
    let interpolation_step = (params.interpolation_step / speed_ratio * interpolation_scale)
        .clamp(params.interpolation_step_min, params.interpolation_step_max);

    let min_distance_scale = if is_touch {
        params.touch_min_distance_scale
    } else {
        1.0
    };
    let min_point_distance = (params.min_point_distance * min_distance_scale / speed_ratio)
        .clamp(params.min_distance_min, params.min_distance_max);

    let smooth_factor = if is_touch {
        params.touch_smooth_factor
    } else {
        1.0
    };
    let smooth_alpha = (params.smooth_alpha * speed_ratio.sqrt() * smooth_factor)
        .clamp(params.smooth_alpha_min, params.smooth_alpha_max);

    (min_point_distance, interpolation_step, smooth_alpha)
}

fn append_handwriting_segment(stroke: &mut Vec<[f32; 2]>, next: [f32; 2], is_touch: bool) {
    let Some(last) = stroke.last().copied() else {
        stroke.push(next);
        return;
    };

    let dx = next[0] - last[0];
    let dy = next[1] - last[1];
    let distance = dx.hypot(dy);
    let previous_distance = stroke
        .get(stroke.len().saturating_sub(2))
        .map(|previous| (last[0] - previous[0]).hypot(last[1] - previous[1]))
        .unwrap_or(0.0);

    let (min_point_distance, interpolation_step, smooth_alpha) =
        handwriting_speed_profile(distance, previous_distance, is_touch);

    if distance < min_point_distance {
        return;
    }

    let segment_count = (distance / interpolation_step).ceil() as usize;
    let segment_count = segment_count.max(1);

    for segment in 1..=segment_count {
        let t = segment as f32 / segment_count as f32;
        let sampled = [last[0] + dx * t, last[1] + dy * t];
        let point = if segment == segment_count {
            sampled
        } else {
            let [prev_x, prev_y] = stroke.last().copied().unwrap_or(last);
            [
                prev_x + (sampled[0] - prev_x) * smooth_alpha,
                prev_y + (sampled[1] - prev_y) * smooth_alpha,
            ]
        };
        let [prev_x, prev_y] = stroke.last().copied().unwrap_or(point);
        if (point[0] - prev_x).hypot(point[1] - prev_y) >= min_point_distance * 0.72 {
            stroke.push(point);
        }
    }
}

impl PanelState {
    fn handwriting_sample_interval(&self) -> Duration {
        let params = handwriting_sampling_params();
        if self.interaction.last_input_was_touch {
            Duration::from_millis(params.sample_interval_touch_ms)
        } else {
            Duration::from_millis(params.sample_interval_mouse_ms)
        }
    }

    fn handwriting_min_sample_distance(&self) -> f32 {
        let params = handwriting_sampling_params();
        let base = params.min_point_distance
            * if self.interaction.last_input_was_touch {
                params.touch_min_distance_scale * params.touch_sample_distance_scale
            } else {
                1.0
            };
        base.max(params.min_distance_min)
            .min(params.min_distance_max)
    }

    fn should_sample_handwriting_point(&mut self, next: [f32; 2]) -> bool {
        let now = Instant::now();
        let min_distance = self.handwriting_min_sample_distance();
        let allow_long_move = if self.interaction.last_input_was_touch {
            min_distance * 2.4
        } else {
            min_distance * 2.0
        };
        if let Some(last_point) = self.interaction.handwriting_last_sample_position {
            let moved_distance = (next[0] - last_point[0]).hypot(next[1] - last_point[1]);
            if moved_distance < min_distance {
                if now.duration_since(self.interaction.handwriting_last_sample.unwrap_or(now))
                    < self.handwriting_sample_interval()
                {
                    return false;
                }
            } else if moved_distance < allow_long_move
                && now.duration_since(self.interaction.handwriting_last_sample.unwrap_or(now))
                    < self.handwriting_sample_interval()
            {
                return false;
            }
        }

        self.mark_handwriting_sample_state(next, now);
        true
    }

    fn reset_handwriting_sample_state(&mut self) {
        self.interaction.handwriting_last_sample = None;
        self.interaction.handwriting_last_sample_position = None;
    }

    fn mark_handwriting_sample_state(&mut self, next: [f32; 2], now: Instant) {
        self.interaction.handwriting_last_sample = Some(now);
        self.interaction.handwriting_last_sample_position = Some(next);
    }

    fn begin_handwriting_sample_state(&mut self, point: [f32; 2]) {
        self.mark_handwriting_sample_state(point, Instant::now());
    }

    fn try_complete_handwriting_stroke(&mut self) {
        if !self.interaction.handwriting_dragging {
            return;
        }
        self.interaction.handwriting_dragging = false;
        self.reset_handwriting_sample_state();
        self.refresh_handwriting_candidates();
    }

    fn refresh_handwriting_candidates(&mut self) {
        self.retire_handwriting_candidate_gestures();
        self.handwriting_generation = self.handwriting_generation.wrapping_add(1);
        self.last_handwriting_summary = Some(summarize_handwriting_strokes(
            &self.chrome.handwriting_strokes,
        ));
        self.chrome.handwriting_candidates =
            recognize_handwriting_candidates(&self.chrome.handwriting_strokes);
        self.reconfigure_model_provider();
        self.chrome.handwriting_hint = if self.chrome.handwriting_strokes.is_empty() {
            "Draw a seed word with mouse or touch.".to_string()
        } else if self.chrome.handwriting_candidates.is_empty() {
            "Try a clearer trace, undo a stroke, or tap Clear.".to_string()
        } else {
            "Tap a recognized seed, or undo the last stroke.".to_string()
        };
    }

    fn handwriting_canvas_rect(&mut self) -> Option<[f32; 4]> {
        self.interaction_rect(InteractionKind::HandwritingCanvas)
    }

    pub(super) fn try_begin_handwriting_stroke(&mut self) -> bool {
        if self.chrome.active_input_mode != InputMode::Handwriting
            || self.interaction.pressed_interaction != Some(InteractionKind::HandwritingCanvas)
        {
            return false;
        }
        let Some((x, y)) = self.cursor_position else {
            return false;
        };
        let Some(rect) = self.handwriting_canvas_rect() else {
            return false;
        };
        if !point_in_rect(x, y, rect) {
            return false;
        }

        self.chrome.blur_input();
        self.interaction.handwriting_dragging = true;
        let start_point = clamp_handwriting_point([x, y], rect);
        self.chrome.handwriting_strokes.push(vec![start_point]);
        self.handwriting_generation = self.handwriting_generation.wrapping_add(1);
        self.begin_handwriting_sample_state(start_point);
        self.chrome.handwriting_hint = "Tracing… release to recognize".to_string();
        true
    }

    pub(super) fn extend_handwriting_stroke(&mut self) {
        if !self.interaction.handwriting_dragging
            || self.chrome.active_input_mode != InputMode::Handwriting
        {
            return;
        }
        let Some((x, y)) = self.cursor_position else {
            return;
        };
        let Some(rect) = self.handwriting_canvas_rect() else {
            return;
        };
        let clamped = clamp_handwriting_point([x, y], rect);
        if !self.should_sample_handwriting_point(clamped) {
            return;
        }
        if let Some(stroke) = self.chrome.handwriting_strokes.last_mut() {
            append_handwriting_segment(stroke, clamped, self.interaction.last_input_was_touch);
            self.handwriting_generation = self.handwriting_generation.wrapping_add(1);
        }
    }

    pub(super) fn finish_handwriting_stroke(&mut self) {
        self.try_complete_handwriting_stroke();
    }

    pub(super) fn undo_handwriting_stroke(&mut self) {
        self.interaction.handwriting_dragging = false;
        self.reset_handwriting_sample_state();
        if self.chrome.handwriting_strokes.pop().is_none() {
            self.chrome.handwriting_hint = "Draw a seed word with mouse or touch.".to_string();
            return;
        }
        self.refresh_handwriting_candidates();
    }

    pub(super) fn clear_handwriting(&mut self) {
        self.retire_handwriting_candidate_gestures();
        self.handwriting_generation = self.handwriting_generation.wrapping_add(1);
        self.interaction.handwriting_dragging = false;
        self.reset_handwriting_sample_state();
        self.chrome.handwriting_strokes.clear();
        self.chrome.handwriting_candidates.clear();
        self.chrome.handwriting_hint = "Draw a seed word with mouse or touch.".to_string();
        self.last_handwriting_summary = None;
        self.reconfigure_model_provider();
    }

    fn retire_handwriting_candidate_gestures(&mut self) {
        // Undo can change the source while a mouse/touch candidate press is
        // still held. Even identical recognition belongs to the new strokes.
        if matches!(
            self.interaction.pressed_interaction,
            Some(InteractionKind::UseHandwritingCandidate(_))
        ) {
            self.clear_pressed_interaction();
            self.interaction.touch_tap_pending = false;
        }
        self.interaction.handwriting_candidate_scroll_index = None;
        self.interaction.handwriting_candidate_scroll_started_at = None;
    }

    pub(super) fn insert_handwriting_candidate(&mut self, index: usize) {
        let Some(candidate) = self.chrome.handwriting_candidates.get(index).cloned() else {
            return;
        };
        if self.native.showing {
            self.native_insert_text(
                &candidate,
                super::native_sync::NativeInsertion::Handwriting {
                    strokes: self.chrome.handwriting_strokes.clone(),
                    candidates: self.chrome.handwriting_candidates.clone(),
                    generation: self.handwriting_generation,
                },
            );
            return;
        }
        self.chrome.insert_tool_text(&candidate);
        self.chrome.active_input_mode = InputMode::VirtualKeyboard;
        self.chrome.input_modes_expanded = false;
        self.chrome.focus_input();
        self.sync_manual_seed_base();
        self.refresh_seed();
        self.clear_handwriting();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn fresh_cases(test: &str, cases: &[(&str, &[(&str, &str)])]) {
        let mut failures = Vec::new();
        for (case, overrides) in cases {
            // The sampling parameters are cached once. A fresh process gives
            // each case a private environment without mutating parallel tests.
            let output = Command::new(std::env::current_exe().unwrap())
                .args([test, "--exact", "--nocapture", "--test-threads=1"])
                .env_clear()
                .env("LC_ALL", "C.UTF-8")
                .env("SUZAKU_HANDWRITING_TEST_CASE", case)
                .envs(overrides.iter().copied())
                .output()
                .expect("fresh handwriting sampling process");
            let marker = format!("HANDWRITING_CASE_COMPLETE:{case}");
            if !output.status.success()
                || !String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line.ends_with(&marker))
            {
                failures.push(format!(
                    "{case}: {}\n{}\n{}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn small_canvas_and_configured_padding_keep_points_inside() {
        let Ok(case) = std::env::var("SUZAKU_HANDWRITING_TEST_CASE") else {
            fresh_cases(
                "handwriting::tests::small_canvas_and_configured_padding_keep_points_inside",
                &[
                    ("default", &[]),
                    ("tiny", &[]),
                    ("fractional", &[]),
                    (
                        "large_padding",
                        &[("SUZAKU_HANDWRITING_EDGE_PADDING", "1000000")],
                    ),
                    (
                        "max_padding",
                        &[("SUZAKU_HANDWRITING_EDGE_PADDING", "3.4028235e38")],
                    ),
                    ("zero_padding", &[("SUZAKU_HANDWRITING_EDGE_PADDING", "0")]),
                ],
            );
            return;
        };
        let rect = match case.as_str() {
            "tiny" => [10.0, 20.0, 6.0, 2.0],
            "fractional" => [1.1, 10.1, 1.0, 2.1],
            _ => [10.0, 20.0, 100.0, 80.0],
        };
        for point in [[-1000.0, -1000.0], [1000.0, 1000.0], [11.0, 21.0]] {
            let clamped = clamp_handwriting_point(point, rect);
            for axis in 0..2 {
                assert!(clamped[axis].is_finite());
                assert!(clamped[axis] >= rect[axis]);
                assert!(clamped[axis] <= rect[axis] + rect[axis + 2]);
            }
            match case.as_str() {
                "tiny" => assert_eq!(clamped, [13.0, 21.0]),
                "fractional" | "large_padding" | "max_padding" => {
                    assert_eq!(clamped, [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5],)
                }
                "zero_padding" => assert_eq!(
                    clamped,
                    [point[0].clamp(10.0, 110.0), point[1].clamp(20.0, 100.0)],
                ),
                "default" => assert_eq!(
                    clamped,
                    [point[0].clamp(14.0, 106.0), point[1].clamp(24.0, 96.0)],
                ),
                _ => panic!("unknown geometry case"),
            }
        }
        println!("HANDWRITING_CASE_COMPLETE:{case}");
    }

    #[test]
    fn smoothing_limits_stay_ordered_and_are_interpolation_weights() {
        let Ok(case) = std::env::var("SUZAKU_HANDWRITING_TEST_CASE") else {
            fresh_cases(
                "handwriting::tests::smoothing_limits_stay_ordered_and_are_interpolation_weights",
                &[
                    ("default", &[]),
                    (
                        "negative",
                        &[
                            ("SUZAKU_HANDWRITING_SMOOTH_ALPHA_MIN", "-1"),
                            ("SUZAKU_HANDWRITING_SMOOTH_ALPHA_MAX", "-0.5"),
                        ],
                    ),
                    (
                        "high",
                        &[
                            ("SUZAKU_HANDWRITING_SMOOTH_ALPHA_MIN", "2"),
                            ("SUZAKU_HANDWRITING_SMOOTH_ALPHA_MAX", "3"),
                            ("SUZAKU_HANDWRITING_SMOOTH_ALPHA", "1"),
                            ("SUZAKU_HANDWRITING_TOUCH_SMOOTH_FACTOR", "3"),
                        ],
                    ),
                    (
                        "reversed",
                        &[
                            ("SUZAKU_HANDWRITING_SMOOTH_ALPHA_MIN", "0.8"),
                            ("SUZAKU_HANDWRITING_SMOOTH_ALPHA_MAX", "0.2"),
                        ],
                    ),
                ],
            );
            return;
        };
        for touch in [false, true] {
            for (distance, previous) in [(0.0, 0.0), (16.0, 4.0), (1024.0, 1000.0)] {
                let (minimum, step, alpha) = handwriting_speed_profile(distance, previous, touch);
                assert!(minimum.is_finite() && minimum > 0.0);
                assert!(step.is_finite() && step > 0.0);
                assert!((0.0..=1.0).contains(&alpha), "{case}: alpha={alpha}");
                match case.as_str() {
                    "negative" => assert_eq!(alpha, 0.0),
                    "high" => assert_eq!(alpha, 1.0),
                    "reversed" => assert_eq!(alpha, 0.8),
                    "default" => {
                        let ratio = ((distance + previous * 0.65) * 0.5 / 16.0).clamp(0.55, 2.0);
                        assert_eq!(
                            minimum,
                            (1.2 * if touch { 1.18 } else { 1.0 } / ratio).clamp(0.78, 2.0)
                        );
                        assert_eq!(
                            step,
                            (3.2 / ratio * if touch { 0.9 } else { 1.0 }).clamp(2.0, 6.8)
                        );
                        assert_eq!(
                            alpha,
                            (0.22 * ratio.sqrt() * if touch { 1.05 } else { 1.0 })
                                .clamp(0.14, 0.34)
                        );
                    }
                    _ => panic!("unknown smoothing case"),
                }
            }
        }
        println!("HANDWRITING_CASE_COMPLETE:{case}");
    }
}
