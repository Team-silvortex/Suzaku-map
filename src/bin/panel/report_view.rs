//! Bounded, read-only report pages. No model, host, file, clipboard or editing I/O.
use suzaku_map::{
    ime::gpu::{
        CandidateQuad, PanelChromeState, RenderScene, TextAlign, TextBlock, TextLayout, TextRole,
        TextSection,
    },
    ui::{UiLanguage, model_setup::ModelSetupGuide},
};
use unicode_width::UnicodeWidthChar;

const DISPLAY_LIMIT: usize = 2 * 1024 * 1024;
const MAX_COLUMNS: usize = 120;
const MAX_ROWS: usize = 32;
const MAX_PAGE_CHARS: usize = 2048;
const INPUT_METHOD_ERROR_LIMIT: usize = 4096;
const LIMIT_NOTICE: &str =
    "Report exceeds the 2 MiB display limit; only a bounded preview is available.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ReportSnapshot {
    InputMethodError(String),
    ModelSetup(ModelSetupGuide),
    #[cfg(target_os = "linux")]
    Data(crate::tray_data::DataReport),
}

impl ReportSnapshot {
    pub(super) fn render(&self, ui: UiLanguage) -> String {
        match self {
            Self::InputMethodError(error) => ui
                .tr("Input method error: {}")
                .replace("{}", &ui.diagnostic(error)),
            Self::ModelSetup(guide) => guide.render(ui),
            #[cfg(target_os = "linux")]
            Self::Data(report) => report.render(ui),
        }
    }

    // Bound temporary display copies before formatting; retain the original
    // snapshot even when its raw parameter exceeds the display budget.
    fn bounded(&self) -> (Self, bool) {
        let mut limited = false;
        let mut guide_limited = false;
        let mut copy = |raw: &str| {
            let mut end = raw.len().min(DISPLAY_LIMIT);
            while !raw.is_char_boundary(end) {
                end -= 1;
            }
            limited |= end != raw.len();
            raw[..end].to_owned()
        };
        let bounded = match self {
            Self::InputMethodError(error) => Self::InputMethodError(copy(error)),
            Self::ModelSetup(guide) => {
                let (guide, truncated) = guide.bounded(DISPLAY_LIMIT);
                guide_limited = truncated;
                Self::ModelSetup(guide)
            }
            #[cfg(target_os = "linux")]
            Self::Data(report) => {
                use crate::tray_data::{DataFailure, DataReport};
                Self::Data(match report {
                    DataReport::BackupSaved(raw) => DataReport::BackupSaved(copy(raw)),
                    DataReport::FolderOpened(raw) => DataReport::FolderOpened(copy(raw)),
                    DataReport::Failed(DataFailure::DataFolders(raw)) => {
                        DataReport::Failed(DataFailure::DataFolders(copy(raw)))
                    }
                    DataReport::Failed(DataFailure::Backup(raw)) => {
                        DataReport::Failed(DataFailure::Backup(copy(raw)))
                    }
                    DataReport::Failed(DataFailure::OpenFolder(raw)) => {
                        DataReport::Failed(DataFailure::OpenFolder(copy(raw)))
                    }
                    DataReport::Failed(
                        DataFailure::InvalidSettingsFolder
                        | DataFailure::ManagerUnavailable
                        | DataFailure::WorkerStart
                        | DataFailure::Panicked,
                    ) => report.clone(),
                })
            }
        };
        (bounded, limited || guide_limited)
    }
}

fn visible_escape(source: &str) -> (String, bool) {
    visible_escape_bounded(source, DISPLAY_LIMIT)
}

fn visible_escape_bounded(source: &str, limit: usize) -> (String, bool) {
    let mut escaped = String::with_capacity(source.len().min(limit));
    for ch in source.chars() {
        if UnicodeWidthChar::width(ch) == Some(0) {
            let token = format!("\\u{{{:x}}}", ch as u32);
            if escaped.len() + token.len() > limit {
                return (escaped, true);
            }
            escaped.push_str(&token);
        } else {
            let token = ch.escape_debug();
            let bytes: usize = token.clone().map(char::len_utf8).sum();
            if escaped.len() + bytes > limit {
                return (escaped, true);
            }
            escaped.extend(token);
        }
    }
    (escaped, false)
}

fn cells(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(1).clamp(1, 2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PointerSource {
    Mouse,
    Touch(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Previous,
    Next,
    Close,
}

impl Button {
    fn label(self) -> &'static str {
        match self {
            Self::Previous => "Previous page",
            Self::Next => "Next page",
            Self::Close => "Close details",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    button: Button,
    rect: [f32; 4],
}

#[derive(Clone, Debug)]
struct Geometry {
    generation: u64,
    targets: Vec<Target>,
    body: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
struct Press {
    target: Target,
    source: PointerSource,
    generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GridKey {
    width: u32,
    height: u32,
    columns: usize,
    rows: usize,
    language: UiLanguage,
    text_scale: suzaku_map::ime::gpu::DisplayTextScale,
}

struct TextCache {
    language: UiLanguage,
    escaped: String,
    limited: bool,
}

#[derive(Clone, Copy)]
struct BodyGrid {
    rect: [f32; 4],
    columns: usize,
    cell_width: f32,
    row_height: f32,
    px: f32,
}

pub(super) struct ReportView {
    snapshot: ReportSnapshot,
    cache: Option<TextCache>,
    key: Option<GridKey>,
    starts: Vec<u32>,
    page: usize,
    generation: u64,
    prepared: Option<Geometry>,
    presented: Option<Geometry>,
    press: Option<Press>,
    input_method_error: Option<String>,
    input_method_error_ticks: u8,
}

impl ReportView {
    pub(super) fn new(snapshot: ReportSnapshot) -> Self {
        Self {
            snapshot,
            cache: None,
            key: None,
            starts: vec![0],
            page: 0,
            generation: 0,
            prepared: None,
            presented: None,
            press: None,
            input_method_error: None,
            input_method_error_ticks: 0,
        }
    }

    pub(super) fn show_input_method_error(&mut self, error: &str) {
        let truncated = error.len() > INPUT_METHOD_ERROR_LIMIT;
        let mut end = error.len().min(if truncated {
            INPUT_METHOD_ERROR_LIMIT - '…'.len_utf8()
        } else {
            INPUT_METHOD_ERROR_LIMIT
        });
        while !error.is_char_boundary(end) {
            end -= 1;
        }
        let mut notice = error[..end].to_owned();
        if truncated {
            notice.push('…');
        }
        self.input_method_error = Some(notice);
        self.input_method_error_ticks = 180;
    }

    pub(super) fn has_input_method_error(&self) -> bool {
        self.input_method_error.is_some()
    }

    pub(super) fn advance_input_method_error(&mut self) {
        let (ticks, clear) = super::advance_commit_feedback_state(self.input_method_error_ticks);
        self.input_method_error_ticks = ticks;
        if clear {
            self.input_method_error = None;
        }
    }

    pub(super) fn input_method_error_feedback(&self, ui: UiLanguage) -> Option<(String, [f32; 4])> {
        let error = self.input_method_error.as_deref()?;
        let body = self.prepared.as_ref()?.body;
        if body.iter().any(|value| !value.is_finite()) || body[2] <= 0.0 || body[3] <= 0.0 {
            return None;
        }
        let text = ui
            .tr("Input method error: {}")
            .replace("{}", &ui.diagnostic(error));
        let (mut text, limited) =
            visible_escape_bounded(&text, INPUT_METHOD_ERROR_LIMIT - '…'.len_utf8());
        if limited {
            text.push('…');
        }
        Some((text, body))
    }

    pub(super) fn page_index(&self) -> usize {
        self.page
    }
    pub(super) fn title_key(&self) -> &'static str {
        match &self.snapshot {
            ReportSnapshot::ModelSetup(_) => "Configuration path and guide",
            _ => "Operation details",
        }
    }
    pub(super) fn page_count(&self) -> usize {
        self.starts.len()
    }

    fn go(&mut self, page: usize) -> bool {
        let page = page.min(self.page_count().saturating_sub(1));
        if page == self.page {
            return false;
        }
        self.page = page;
        self.invalidate();
        true
    }

    pub(super) fn page(&mut self, forward: bool) -> bool {
        self.go(if forward {
            self.page.saturating_add(1)
        } else {
            self.page.saturating_sub(1)
        })
    }
    pub(super) fn first(&mut self) -> bool {
        self.go(0)
    }
    pub(super) fn last(&mut self) -> bool {
        self.go(self.page_count().saturating_sub(1))
    }

    pub(super) fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.prepared = None;
        self.presented = None;
        self.press = None;
    }

    pub(super) fn unpresent(&mut self) {
        self.presented = None;
    }

    pub(super) fn presented(&mut self) {
        self.presented = self.prepared.clone();
    }

    pub(super) fn press(&mut self, point: [f32; 2], source: PointerSource) -> bool {
        if self.press.take().is_some() {
            return false;
        }
        let Some(geometry) = &self.presented else {
            return false;
        };
        let Some(target) = geometry
            .targets
            .iter()
            .find(|target| inside(point, target.rect))
            .copied()
        else {
            return false;
        };
        self.press = Some(Press {
            target,
            source,
            generation: geometry.generation,
        });
        true
    }

    pub(super) fn release(&mut self, point: [f32; 2], source: PointerSource) -> bool {
        let Some(press) = self.press.take() else {
            return false;
        };
        let Some(presented) = &self.presented else {
            return false;
        };
        if press.source != source
            || press.generation != presented.generation
            || !presented.targets.contains(&press.target)
            || !inside(point, press.target.rect)
        {
            return false;
        }
        match press.target.button {
            Button::Close => true,
            Button::Previous => {
                self.page(false);
                false
            }
            Button::Next => {
                self.page(true);
                false
            }
        }
    }

    #[cfg(test)]
    pub(super) fn test_snapshot(&self) -> &ReportSnapshot {
        &self.snapshot
    }

    #[cfg(test)]
    pub(super) fn test_button_rect(&self, forward: Option<bool>) -> Option<[f32; 4]> {
        let button = match forward {
            None => Button::Close,
            Some(false) => Button::Previous,
            Some(true) => Button::Next,
        };
        self.presented
            .as_ref()?
            .targets
            .iter()
            .find(|target| target.button == button)
            .map(|target| target.rect)
    }

    fn prepare_text(&mut self, language: UiLanguage) {
        if self
            .cache
            .as_ref()
            .is_some_and(|cache| cache.language == language)
        {
            return;
        }
        let (bounded, limited) = self.snapshot.bounded();
        let (escaped, escaped_limit) = visible_escape(&bounded.render(language));
        self.cache = Some(TextCache {
            language,
            escaped,
            limited: limited || escaped_limit,
        });
        self.key = None;
        self.page = 0;
        self.invalidate();
    }

    pub(super) fn scene(
        &mut self,
        chrome: &PanelChromeState,
        width: f32,
        height: f32,
    ) -> RenderScene {
        let mut scene = empty_scene();
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            self.invalidate();
            return scene;
        }
        let ui = chrome.ui_language.resolved();
        self.prepare_text(ui);
        let cache = self.cache.as_ref().expect("prepared report text");
        let scale = (width / 640.0).clamp(0.86, 1.08);
        let px = chrome.text_scale.label_pixel_size() * scale;
        let (surface, text, border) = chrome.theme_preset.tooltip_colors();
        let margin = 8.0 * scale;
        let usable_width = (width - 2.0 * margin).max(0.0);
        // The resource warning is mandatory. Measure it at the actual width
        // once, with no line limit that can truncate this fixed catalog text.
        // Its retained glyphs are only translated/re-spaced below, not reflowed.
        let warning = cache.limited.then(|| {
            TextBlock {
                text: ui.tr(LIMIT_NOTICE).into(),
                origin: [0.0, 0.0],
                max_width: usable_width,
                pixel_size: px,
                letter_spacing: 0.0,
                line_gap: 0.0,
                max_lines: ui.tr(LIMIT_NOTICE).chars().count().max(1),
                color: text,
                align: TextAlign::Left,
                role: TextRole::HeaderStatus,
            }
            .layout()
        });
        let warning_rows = warning.as_ref().map_or(0, |layout| layout.lines.len());
        let full_line = px * 7.0 + 3.0 * scale;
        let full_gap = 6.0 * scale;
        let full_warning = if warning_rows == 0 {
            0.0
        } else {
            full_line * warning_rows as f32 - 3.0 * scale
        };
        let full_controls_y =
            height - margin - full_line * 3.0 - full_gap * 2.0 - (full_line * 2.0 + 12.0 * scale);
        let full_body_top = margin + full_line * 4.0 + full_warning + full_gap * 2.0;
        // Keep the ordinary help where it fits. Otherwise remove only optional
        // chrome and inter-line gaps, retaining the selected text size, warning,
        // controls and at least one visible body row in valid Settings windows.
        let compact = full_controls_y - full_gap - full_body_top + 0.0001 < full_line;
        let line_gap = if compact { 0.0 } else { 3.0 * scale };
        let line = px * 7.0 + line_gap;
        let gap = if compact { 4.0 } else { 6.0 } * scale;
        let button_height = line * 2.0 + if compact { 4.0 } else { 12.0 } * scale;
        let footer_lines = if compact { 1.0 } else { 3.0 };
        let controls_y = (height
            - margin
            - line * footer_lines
            - gap * (if compact { 1.0 } else { 2.0 })
            - button_height)
            .max(margin);
        let title_lines = if compact { 1.0 } else { 2.0 };
        let hint_lines = if compact { 0.0 } else { 2.0 };
        let warning_height = if warning_rows == 0 {
            0.0
        } else {
            line * warning_rows as f32 - line_gap
        };
        let body_top = margin + line * (title_lines + hint_lines) + warning_height + gap * 2.0;
        let body = [
            margin,
            body_top,
            usable_width,
            (controls_y - gap - body_top).max(0.0),
        ];
        let cell_width = px * 7.0;
        let columns = ((body[2] / cell_width).floor() as usize).min(MAX_COLUMNS);
        let rows = ((body[3] / line).floor() as usize).min(MAX_ROWS);
        let key = GridKey {
            width: width.to_bits(),
            height: height.to_bits(),
            columns,
            rows,
            language: ui,
            text_scale: chrome.text_scale,
        };
        if self.key != Some(key) {
            let anchor = self.starts.get(self.page).copied().unwrap_or(0);
            self.starts = page_starts(&cache.escaped, columns, rows);
            // Keep the old first character reachable in the current page when
            // resizing or learning warning-font metrics changes the row count.
            self.page = self
                .starts
                .partition_point(|start| *start <= anchor)
                .saturating_sub(1);
            self.key = Some(key);
            self.invalidate();
        }
        let cache = self.cache.as_ref().expect("report text remains cached");
        scene.quads.push(CandidateQuad::rounded(
            [0.0, 0.0, width, height],
            chrome.theme_preset.page_background(),
            0.0,
        ));
        let viewport = [0.0, 0.0, width, height];
        label(
            &mut scene,
            ui.tr(self.title_key()),
            [margin, margin, usable_width, line * title_lines],
            px,
            line_gap,
            text,
            viewport,
        );
        if !compact {
            label(
                &mut scene,
                ui.tr("Control characters are shown as escapes"),
                [
                    margin,
                    margin + line * title_lines,
                    usable_width,
                    line * hint_lines,
                ],
                px,
                line_gap,
                text,
                viewport,
            );
        }
        if let Some(warning) = warning {
            place_notice(
                &mut scene,
                warning,
                [
                    margin,
                    margin + line * (title_lines + hint_lines),
                    usable_width,
                    warning_height,
                ],
                px,
                line_gap,
                viewport,
            );
        }
        if columns >= 2 && rows > 0 {
            let start = self.starts[self.page_index()] as usize;
            let end = self
                .starts
                .get(self.page_index() + 1)
                .map_or(cache.escaped.len(), |offset| *offset as usize);
            body_text(
                &mut scene,
                &cache.escaped[start..end],
                BodyGrid {
                    rect: body,
                    columns,
                    cell_width,
                    row_height: line,
                    px,
                },
                text,
            );
        }
        let button_width = ((usable_width - gap * 2.0) / 3.0).max(0.0);
        let mut targets = Vec::new();
        for (index, button) in [Button::Previous, Button::Next, Button::Close]
            .into_iter()
            .enumerate()
        {
            let rect = clipped(
                [
                    margin + index as f32 * (button_width + gap),
                    controls_y,
                    button_width,
                    button_height,
                ],
                viewport,
            );
            if rect[2] <= 0.0 || rect[3] <= 0.0 {
                continue;
            }
            scene
                .quads
                .push(CandidateQuad::rounded(rect, surface, 5.0 * scale));
            scene
                .quads
                .push(CandidateQuad::outline(rect, border, 5.0 * scale, 1.0));
            label(
                &mut scene,
                ui.tr(button.label()),
                rect,
                px,
                line_gap,
                text,
                viewport,
            );
            let enabled = match button {
                Button::Previous => self.page_index() > 0,
                Button::Next => self.page_index() + 1 < self.page_count(),
                Button::Close => true,
            };
            if enabled {
                targets.push(Target { button, rect });
            }
        }
        let page = ui.tr("Page {}").replace(
            "{}",
            &format!("{} / {}", self.page_index() + 1, self.page_count()),
        );
        let page_y = controls_y + button_height + gap;
        label(
            &mut scene,
            &page,
            [margin, page_y, usable_width, line],
            px,
            line_gap,
            text,
            viewport,
        );
        if !compact {
            let help = format!(
                "{} · {}",
                ui.tr("Esc closes"),
                ui.tr("PageUp / PageDown changes pages")
            );
            label(
                &mut scene,
                &help,
                [margin, page_y + line + gap, usable_width, line * 2.0],
                px,
                line_gap,
                text,
                viewport,
            );
        }
        self.prepared = Some(Geometry {
            generation: self.generation,
            targets,
            body,
        });
        scene
    }
}

fn page_starts(text: &str, columns: usize, rows: usize) -> Vec<u32> {
    let mut starts = vec![0];
    if columns < 2 || rows == 0 {
        return starts;
    }
    let (mut column, mut row, mut count) = (0, 0, 0);
    for (offset, ch) in text.char_indices() {
        let width = cells(ch);
        if column + width > columns {
            column = 0;
            row += 1;
        }
        if row >= rows || count == MAX_PAGE_CHARS {
            starts.push(offset as u32);
            column = 0;
            row = 0;
            count = 0;
        }
        column += width;
        count += 1;
    }
    starts
}

fn inside(point: [f32; 2], rect: [f32; 4]) -> bool {
    point[0] >= rect[0]
        && point[1] >= rect[1]
        && point[0] <= rect[0] + rect[2]
        && point[1] <= rect[1] + rect[3]
}

fn clipped(rect: [f32; 4], clip: [f32; 4]) -> [f32; 4] {
    let x = rect[0].max(clip[0]);
    let y = rect[1].max(clip[1]);
    [
        x,
        y,
        ((rect[0] + rect[2]).min(clip[0] + clip[2]) - x).max(0.0),
        ((rect[1] + rect[3]).min(clip[1] + clip[3]) - y).max(0.0),
    ]
}

fn place_notice(
    scene: &mut RenderScene,
    mut layout: TextLayout,
    rect: [f32; 4],
    px: f32,
    line_gap: f32,
    viewport: [f32; 4],
) {
    let clip = clipped(rect, viewport);
    if clip[2] <= 0.0 || clip[3] <= 0.0 {
        return;
    }
    let position = |glyph: &mut [f32; 4]| {
        let row = ((glyph[1] / (px * 7.0)) + 0.0001).floor();
        glyph[0] += rect[0];
        glyph[1] += rect[1] + row * line_gap;
    };
    for quad in &mut layout.quads {
        position(&mut quad.rect);
        quad.clip_rect = Some(clip);
    }
    for glyph in &mut layout.atlas_glyphs {
        position(&mut glyph.rect);
        glyph.clip_rect = Some(clip);
    }
    layout.bounds[0] += rect[0];
    layout.bounds[1] += rect[1];
    layout.bounds[3] += layout.lines.len().saturating_sub(1) as f32 * line_gap;
    scene.text_quads.extend(layout.quads.iter().copied());
    scene
        .atlas_glyphs
        .extend(layout.atlas_glyphs.iter().cloned());
    scene.text_sections.push(TextSection {
        role: TextRole::HeaderStatus,
        layouts: vec![layout],
    });
}

fn label(
    scene: &mut RenderScene,
    text: &str,
    rect: [f32; 4],
    px: f32,
    line_gap: f32,
    color: [f32; 4],
    viewport: [f32; 4],
) {
    let rect = clipped(rect, viewport);
    if rect[2] <= 0.0 || rect[3] < px * 7.0 {
        return;
    }
    let mut layout = TextBlock {
        text: text.into(),
        origin: [rect[0], rect[1]],
        max_width: rect[2],
        pixel_size: px,
        letter_spacing: 0.0,
        line_gap,
        max_lines: (((rect[3] + line_gap) / (px * 7.0 + line_gap)) + 0.0001).floor() as usize,
        color,
        align: TextAlign::Left,
        role: TextRole::HeaderStatus,
    }
    .layout();
    for quad in &mut layout.quads {
        quad.clip_rect = Some(rect);
    }
    for glyph in &mut layout.atlas_glyphs {
        glyph.clip_rect = Some(rect);
    }
    scene.text_quads.extend(layout.quads.iter().copied());
    scene
        .atlas_glyphs
        .extend(layout.atlas_glyphs.iter().cloned());
    scene.text_sections.push(TextSection {
        role: TextRole::HeaderStatus,
        layouts: vec![layout],
    });
}

fn body_text(scene: &mut RenderScene, text: &str, grid: BodyGrid, color: [f32; 4]) {
    let BodyGrid {
        rect: body,
        columns,
        cell_width,
        row_height,
        px,
    } = grid;
    let (mut column, mut row) = (0, 0);
    for ch in text.chars().take(MAX_PAGE_CHARS) {
        let width = cells(ch);
        if column + width > columns {
            column = 0;
            row += 1;
        }
        let cell = [
            body[0] + column as f32 * cell_width,
            body[1] + row as f32 * row_height,
            width as f32 * cell_width,
            row_height,
        ];
        let clip = clipped(cell, body);
        if ch != ' ' && clip[2] > 0.0 && clip[3] > 0.0 {
            // A wide actual font is clipped, never replaced by an ellipsis or
            // shrunk to fit a grid cell. Paging itself never uses font advances.
            let mut layout = TextBlock {
                text: ch.to_string(),
                origin: [cell[0], cell[1]],
                max_width: px * 32.0,
                pixel_size: px,
                letter_spacing: 0.0,
                line_gap: 0.0,
                max_lines: 1,
                color,
                align: TextAlign::Left,
                role: TextRole::HeaderStatus,
            }
            .layout();
            for quad in &mut layout.quads {
                quad.clip_rect = Some(clip);
            }
            for glyph in &mut layout.atlas_glyphs {
                glyph.clip_rect = Some(clip);
            }
            scene.text_quads.extend(layout.quads.iter().copied());
            scene
                .atlas_glyphs
                .extend(layout.atlas_glyphs.iter().cloned());
        }
        column += width;
    }
}

fn empty_scene() -> RenderScene {
    RenderScene {
        quads: Vec::new(),
        text_quads: Vec::new(),
        atlas_glyphs: Vec::new(),
        text_sections: Vec::new(),
        hit_targets: Vec::new(),
        interactive_targets: Vec::new(),
        sentence_candidate_truncated: Vec::new(),
        next_token_candidate_truncated: Vec::new(),
        handwriting_candidate_truncated: Vec::new(),
        settings_option_truncated: Vec::new(),
        settings_scroll_metadata: None,
        settings_focus_targets: Vec::new(),
        labels: Vec::new(),
        selected_label: None,
        draft_text: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Arc};
    use suzaku_map::ime::gpu::{DisplayTextScale, FontLayoutMetrics, with_font_metrics};

    fn languages() -> impl Iterator<Item = UiLanguage> {
        UiLanguage::ALL
            .into_iter()
            .filter(|ui| *ui != UiLanguage::System)
    }
    fn center(rect: [f32; 4]) -> [f32; 2] {
        [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0]
    }
    fn view() -> ReportView {
        ReportView::new(ReportSnapshot::InputMethodError(
            "Font: Settings_日_ß:{} ".repeat(200),
        ))
    }
    fn decode(source: &str) -> String {
        let mut chars = source.chars();
        let mut decoded = String::new();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                decoded.push(ch);
                continue;
            }
            decoded.push(match chars.next().expect("escape suffix") {
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '0' => '\0',
                'u' => {
                    assert_eq!(chars.next(), Some('{'));
                    let mut hex = String::new();
                    loop {
                        let next = chars.next().expect("unicode escape terminator");
                        if next == '}' {
                            break;
                        }
                        hex.push(next);
                    }
                    char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap()
                }
                other => panic!("unknown escape {other}"),
            });
        }
        decoded
    }

    #[test]
    fn new_input_errors_are_bounded_at_unicode_boundaries_and_escape_display_controls() {
        let chrome = PanelChromeState::default();
        for raw in ["日".repeat(2048), "\u{202e}\0\n\t".repeat(2048)] {
            let mut report = view();
            report.show_input_method_error(&raw);
            let stored = report.input_method_error.as_ref().unwrap();
            assert!(stored.len() <= INPUT_METHOD_ERROR_LIMIT);
            assert!(stored.ends_with('…'));
            assert!(raw.starts_with(stored.strip_suffix('…').unwrap()));
            report.scene(&chrome, 640.0, 500.0);
            let (text, _) = report
                .input_method_error_feedback(UiLanguage::English)
                .unwrap();
            assert!(text.len() <= INPUT_METHOD_ERROR_LIMIT);
            assert!(!text.chars().any(char::is_control));
            assert!(!text.contains('\u{202e}'));
        }
        for ui in languages() {
            let mut report = view();
            report.show_input_method_error("Font: Settings");
            report.scene(&chrome, 640.0, 500.0);
            assert_eq!(
                report.input_method_error_feedback(ui).unwrap().0,
                ui.tr("Input method error: {}")
                    .replace("{}", "Font: Settings")
            );
        }
    }

    #[test]
    fn input_error_lifetime_never_changes_report_layout_or_pressed_button_identity() {
        let mut report = view();
        let chrome = PanelChromeState::default();
        report.scene(&chrome, 640.0, 500.0);
        assert!(report.page(true));
        let before = report.scene(&chrome, 640.0, 500.0);
        report.presented();
        let source = report.snapshot.clone();
        let page = report.page_index();
        let starts = report.starts.clone();
        let generation = report.generation;
        let close = center(report.test_button_rect(None).unwrap());
        assert!(report.press(close, PointerSource::Mouse));
        assert!(!report.has_input_method_error());
        report.show_input_method_error("new report-only error");
        assert_eq!(report.scene(&chrome, 640.0, 500.0), before);
        report.presented();
        let (_, body) = report
            .input_method_error_feedback(UiLanguage::English)
            .unwrap();
        for target in &report.presented.as_ref().unwrap().targets {
            assert!(body[1] + body[3] <= target.rect[1]);
        }
        for _ in 0..179 {
            report.advance_input_method_error();
        }
        assert!(report.has_input_method_error());
        report.advance_input_method_error();
        assert!(!report.has_input_method_error());
        assert!(
            report
                .input_method_error_feedback(UiLanguage::English)
                .is_none()
        );
        report.advance_input_method_error();
        assert_eq!(report.snapshot, source);
        assert_eq!(report.page_index(), page);
        assert_eq!(report.starts, starts);
        assert_eq!(report.generation, generation);
        assert!(report.release(close, PointerSource::Mouse));
    }

    #[test]
    fn input_error_feedback_requires_a_current_nonempty_report_body() {
        let mut report = view();
        let chrome = PanelChromeState::default();
        report.show_input_method_error("retained while the viewport is unavailable");
        assert!(
            report
                .input_method_error_feedback(UiLanguage::English)
                .is_none()
        );
        report.scene(&chrome, 640.0, 500.0);
        report.presented();
        assert!(
            report
                .input_method_error_feedback(UiLanguage::English)
                .is_some()
        );
        for (width, height) in [(400.0, 1.0), (400.0, 0.0), (f32::NAN, 320.0)] {
            report.scene(&chrome, width, height);
            assert!(
                report
                    .input_method_error_feedback(UiLanguage::English)
                    .is_none()
            );
            assert!(report.has_input_method_error());
        }
        report.scene(&chrome, 640.0, 500.0);
        assert!(
            report
                .input_method_error_feedback(UiLanguage::English)
                .is_some()
        );
    }

    #[test]
    fn snapshot_localization_never_reinterprets_opaque_parameters() {
        for ui in languages() {
            for raw in ["Font: Settings", "Settings", "raw_日_ß:{}\nmodel settings"] {
                let snapshot = ReportSnapshot::InputMethodError(raw.into());
                assert_eq!(
                    snapshot.render(ui),
                    ui.tr("Input method error: {}").replace("{}", raw)
                );
            }
            let snapshot = ReportSnapshot::InputMethodError("无法读取 IBus 状态，请重试".into());
            assert_eq!(
                snapshot.render(ui),
                ui.tr("Input method error: {}")
                    .replace("{}", ui.tr("Cannot read IBus status; retry"))
            );
            #[cfg(target_os = "linux")]
            {
                let report = crate::tray_data::DataReport::Failed(
                    crate::tray_data::DataFailure::Backup("Font: Settings".into()),
                );
                assert_eq!(
                    ReportSnapshot::Data(report.clone()).render(ui),
                    report.render(ui)
                );
            }
        }
    }

    #[test]
    fn model_setup_snapshot_localizes_and_exposes_only_read_only_report_controls() {
        let guide = ModelSetupGuide {
            settings_path: Some(std::path::PathBuf::from(
                "/SYNTHETIC/Font: Settings_日_ß:{}/settings.json",
            )),
        };
        for ui_language in languages() {
            let snapshot = ReportSnapshot::ModelSetup(guide.clone());
            assert_eq!(snapshot.render(ui_language), guide.render(ui_language));
            assert!(
                snapshot
                    .render(ui_language)
                    .contains("/SYNTHETIC/Font: Settings_日_ß:{}/settings.json")
            );
            let chrome = PanelChromeState {
                ui_language,
                settings_search_query: "retained search".into(),
                seed_text: "retained draft".into(),
                settings_keyboard_focus: Some(
                    suzaku_map::ime::gpu::InteractionKind::OpenModelSetup,
                ),
                ..Default::default()
            };
            let before = chrome.clone();
            let mut report = ReportView::new(snapshot.clone());
            assert_eq!(report.title_key(), "Configuration path and guide");
            let scene = report.scene(&chrome, 420.0, 380.0);
            assert!(scene.interactive_targets.is_empty());
            assert!(scene.hit_targets.is_empty());
            assert!(scene.settings_focus_targets.is_empty());
            assert!(scene.draft_text.is_empty());
            assert_eq!(chrome, before);
            assert_eq!(report.snapshot, snapshot);
            assert!(report.test_button_rect(None).is_none());
            report.presented();
            assert!(report.test_button_rect(None).is_some());
        }
    }

    #[test]
    fn model_setup_paths_are_bounded_without_replacing_the_original_snapshot() {
        let snapshot = ReportSnapshot::ModelSetup(ModelSetupGuide {
            settings_path: Some(std::path::PathBuf::from(format!(
                "/SYNTHETIC/{}",
                "日".repeat(DISPLAY_LIMIT / 3 + 9)
            ))),
        });
        let (bounded, limited) = snapshot.bounded();
        assert!(limited);
        let ReportSnapshot::ModelSetup(guide) = bounded else {
            panic!("the bounded snapshot must remain a model guide")
        };
        assert!(
            guide
                .settings_path
                .as_ref()
                .unwrap()
                .as_os_str()
                .as_encoded_bytes()
                .len()
                <= DISPLAY_LIMIT
        );
        let mut report = ReportView::new(snapshot.clone());
        report.scene(&PanelChromeState::default(), 420.0, 380.0);
        let cache = report.cache.as_ref().unwrap();
        assert!(cache.limited);
        assert!(cache.escaped.len() <= DISPLAY_LIMIT);
        assert_eq!(report.snapshot, snapshot);
    }

    #[test]
    fn visible_escapes_preserve_spaces_controls_unicode_and_literal_backslashes_reversibly() {
        let raw = "  C:\\work\\Settings\n\tFont: Settings\r\0\u{1b}\u{7f} 日本語🦀 e\u{301} \"'  ";
        let (escaped, limited) = visible_escape(raw);
        assert!(!limited);
        assert_eq!(decode(&escaped), raw);
        assert!(escaped.starts_with("  C:\\\\work\\\\Settings"));
        assert!(!escaped.chars().any(char::is_control));
        assert_eq!(cells('日'), 2);
        assert_eq!(cells('a'), 1);
    }

    #[test]
    fn complete_text_and_tail_are_reachable_without_breaking_utf8_or_losing_spaces() {
        let raw = format!("{}END_末尾_{}", "  日a🦀\\x\n\t ".repeat(300), "Settings");
        let mut report = ReportView::new(ReportSnapshot::InputMethodError(raw));
        report.scene(&PanelChromeState::default(), 420.0, 380.0);
        assert!(report.page_count() > 1);
        let cache = report.cache.as_ref().unwrap();
        let mut collected = String::new();
        for (index, &start) in report.starts.iter().enumerate() {
            let end = report
                .starts
                .get(index + 1)
                .map_or(cache.escaped.len(), |end| *end as usize);
            let page = &cache.escaped[start as usize..end];
            assert!(page.chars().count() <= MAX_PAGE_CHARS);
            assert!(!page.is_empty());
            collected.push_str(page);
        }
        assert_eq!(collected, cache.escaped);
        assert!(collected.ends_with("END_末尾_Settings"));
        assert!(report.last());
        assert_eq!(report.page_index(), report.page_count() - 1);
        assert!(!report.page(true));
        assert!(report.first());
        assert_eq!(report.page_index(), 0);
        assert!(!report.page(false));
    }

    #[test]
    fn reflow_preserves_the_current_byte_anchor_and_invalidates_old_presses() {
        let mut report = view();
        let chrome = PanelChromeState {
            ui_language: UiLanguage::English,
            ..Default::default()
        };
        report.scene(&chrome, 520.0, 540.0);
        report.page(true);
        report.page(true);
        report.scene(&chrome, 520.0, 540.0);
        for (width, height) in [(400.0, 220.0), (960.0, 420.0), (640.0, 360.0)] {
            report.presented();
            let old_close = center(report.test_button_rect(None).unwrap());
            assert!(report.press(old_close, PointerSource::Mouse));
            let anchor = report.starts[report.page];
            let starts = report.starts.clone();
            report.scene(&chrome, width, height);
            assert_ne!(report.starts, starts, "the fixture must actually reflow");
            let start = report.starts[report.page];
            let end = report
                .starts
                .get(report.page + 1)
                .copied()
                .unwrap_or(report.cache.as_ref().unwrap().escaped.len() as u32);
            assert!(
                start <= anchor && anchor < end,
                "the old first byte stays on screen"
            );
            report.presented();
            assert!(!report.release(old_close, PointerSource::Mouse));
        }
    }

    #[test]
    fn warning_font_learning_keeps_the_reading_anchor_when_rows_change() {
        let mut report = view();
        let chrome = PanelChromeState {
            ui_language: UiLanguage::English,
            ..Default::default()
        };
        report.prepare_text(UiLanguage::English);
        report.cache.as_mut().unwrap().limited = true;
        let metrics = |advance| {
            Arc::new(
                LIMIT_NOTICE
                    .chars()
                    .map(|ch| (ch, advance))
                    .collect::<HashMap<_, _>>(),
            )
        };
        with_font_metrics(metrics(3.0), || report.scene(&chrome, 640.0, 500.0));
        report.page(true);
        report.page(true);
        let anchor = report.starts[report.page];
        let old_rows = report.key.unwrap().rows;
        with_font_metrics(metrics(9.0), || report.scene(&chrome, 640.0, 500.0));
        assert_ne!(
            report.key.unwrap().rows,
            old_rows,
            "warning must change the body budget"
        );
        let start = report.starts[report.page];
        let end = report
            .starts
            .get(report.page + 1)
            .copied()
            .unwrap_or(report.cache.as_ref().unwrap().escaped.len() as u32);
        assert!(start <= anchor && anchor < end);
    }

    #[test]
    fn eight_languages_three_sizes_have_only_private_in_bounds_controls() {
        for ui_language in languages() {
            for text_scale in [
                DisplayTextScale::Small,
                DisplayTextScale::Medium,
                DisplayTextScale::Large,
            ] {
                for (width, height) in [(240.0, 300.0), (420.0, 380.0), (900.0, 540.0)] {
                    let chrome = PanelChromeState {
                        ui_language,
                        text_scale,
                        ..Default::default()
                    };
                    let mut report = view();
                    let scene = report.scene(&chrome, width, height);
                    assert!(scene.interactive_targets.is_empty());
                    assert!(scene.hit_targets.is_empty());
                    assert!(scene.settings_focus_targets.is_empty());
                    assert!(scene.draft_text.is_empty());
                    assert!(scene.selected_label.is_none());
                    assert!(
                        report.test_button_rect(None).is_none(),
                        "measurement is not presentation"
                    );
                    report.presented();
                    let rect = report.test_button_rect(None).unwrap();
                    assert!(rect[0] >= 0.0 && rect[1] >= 0.0);
                    assert!(rect[0] + rect[2] <= width && rect[1] + rect[3] <= height);
                    for glyph in &scene.atlas_glyphs {
                        let clip = glyph.clip_rect.expect("all report text is clipped");
                        assert!(clip[0] >= 0.0 && clip[1] >= 0.0);
                        assert!(clip[0] + clip[2] <= width + 0.01);
                        assert!(clip[1] + clip[3] <= height + 0.01);
                    }
                }
            }
        }
    }

    #[test]
    fn font_learning_does_not_change_page_boundaries_or_shrink_body_glyphs() {
        let mut report = view();
        let chrome = PanelChromeState {
            text_scale: DisplayTextScale::Large,
            ..Default::default()
        };
        let narrow: FontLayoutMetrics = Arc::new(HashMap::from([('F', 1.5), ('日', 5.0)]));
        let wide: FontLayoutMetrics = Arc::new(HashMap::from([('F', 13.0), ('日', 14.0)]));
        with_font_metrics(narrow, || report.scene(&chrome, 420.0, 380.0));
        let starts = report.starts.clone();
        report.page(true);
        let page = report.page_index();
        let scene = with_font_metrics(wide, || report.scene(&chrome, 420.0, 380.0));
        assert_eq!(report.starts, starts);
        assert_eq!(report.page_index(), page);
        let px = chrome.text_scale.label_pixel_size() * (420.0_f32 / 640.0).clamp(0.86, 1.08);
        for glyph in scene.atlas_glyphs.iter().filter(|glyph| glyph.ch == '日') {
            assert!((glyph.rect[3] - px * 7.0).abs() < 0.001);
            assert!(glyph.clip_rect.unwrap()[2] <= px * 14.0 + 0.01);
        }
    }

    #[test]
    fn clicks_require_a_successful_presented_frame_and_matching_complete_source() {
        let chrome = PanelChromeState::default();
        let mut report = view();
        report.scene(&chrome, 520.0, 540.0);
        let close = center(
            report
                .prepared
                .as_ref()
                .unwrap()
                .targets
                .iter()
                .find(|target| target.button == Button::Close)
                .unwrap()
                .rect,
        );
        assert!(!report.press(close, PointerSource::Mouse));
        report.presented();
        assert!(report.press(close, PointerSource::Mouse));
        report.unpresent();
        report.scene(&chrome, 520.0, 540.0);
        report.presented();
        assert!(
            report.release(close, PointerSource::Mouse),
            "a successful same-geometry redraw retains the press"
        );
        assert!(report.press(close, PointerSource::Mouse));
        report.unpresent();
        assert!(
            !report.release(close, PointerSource::Mouse),
            "failed presentation cannot authorize release"
        );
        report.presented();
        assert!(report.press(close, PointerSource::Touch(7)));
        assert!(!report.release(close, PointerSource::Touch(8)));
        assert!(report.press(close, PointerSource::Mouse));
        report.invalidate();
        report.scene(&chrome, 520.0, 540.0);
        report.presented();
        assert!(!report.release(close, PointerSource::Mouse));
        assert!(report.press(close, PointerSource::Mouse));
        report.scene(&chrome, 420.0, 380.0);
        report.presented();
        assert!(
            !report.release(close, PointerSource::Mouse),
            "resize invalidates stale press geometry"
        );
    }

    #[test]
    fn page_buttons_navigate_only_on_release_and_old_rects_cannot_close_after_paging() {
        let mut report = view();
        let chrome = PanelChromeState::default();
        report.scene(&chrome, 420.0, 380.0);
        report.presented();
        assert!(report.test_button_rect(Some(false)).is_none());
        let next = center(report.test_button_rect(Some(true)).unwrap());
        assert!(report.press(next, PointerSource::Mouse));
        assert_eq!(report.page_index(), 0);
        assert!(!report.release(next, PointerSource::Mouse));
        assert_eq!(report.page_index(), 1);
        assert!(
            !report.press(next, PointerSource::Mouse),
            "new page needs presentation"
        );
        report.scene(&chrome, 420.0, 380.0);
        report.presented();
        let close = center(report.test_button_rect(None).unwrap());
        assert!(report.press(close, PointerSource::Touch(1)));
        assert!(
            !report.press(close, PointerSource::Touch(2)),
            "a second source cancels rather than steals a gesture"
        );
        assert!(!report.release(close, PointerSource::Touch(1)));
    }

    #[test]
    fn display_limits_are_explicit_bounded_and_leave_the_original_report_unchanged() {
        let raw = "\\".repeat(DISPLAY_LIMIT + 13);
        let mut report = ReportView::new(ReportSnapshot::InputMethodError(raw));
        let scene = report.scene(
            &PanelChromeState {
                ui_language: UiLanguage::English,
                ..Default::default()
            },
            420.0,
            460.0,
        );
        let cache = report.cache.as_ref().unwrap();
        assert!(cache.limited);
        assert!(cache.escaped.len() <= DISPLAY_LIMIT);
        assert_eq!(
            report.snapshot,
            ReportSnapshot::InputMethodError("\\".repeat(DISPLAY_LIMIT + 13))
        );
        assert!(
            scene
                .text_sections
                .iter()
                .flat_map(|section| &section.layouts)
                .any(|layout| layout.lines.join(" ").starts_with("Report exceeds"))
        );
        assert!(
            report
                .starts
                .iter()
                .all(|offset| cache.escaped.is_char_boundary(*offset as usize))
        );
        let mut small = view();
        for (width, height) in [(10.0, 10.0), (f32::NAN, 100.0), (100.0, f32::INFINITY)] {
            small.scene(&PanelChromeState::default(), width, height);
            small.presented();
            assert!(small.test_button_rect(None).is_none());
        }
    }

    #[test]
    fn minimum_settings_size_keeps_body_controls_and_the_complete_limit_warning() {
        for ui_language in languages() {
            for text_scale in [
                DisplayTextScale::Small,
                DisplayTextScale::Medium,
                DisplayTextScale::Large,
            ] {
                for (width, height) in [
                    (400.0, 220.0),
                    (640.0, 220.0),
                    (960.0, 220.0),
                    (400.0, 360.0),
                    (960.0, 360.0),
                    (960.0, 420.0),
                ] {
                    for limited in [false, true] {
                        let chrome = PanelChromeState {
                            ui_language,
                            text_scale,
                            ..Default::default()
                        };
                        let mut report = view();
                        report.prepare_text(ui_language);
                        if limited {
                            let cache = report.cache.as_mut().unwrap();
                            cache.limited = true;
                            cache.escaped = "日 bounded preview tail".into();
                        }
                        let scene = report.scene(&chrome, width, height);
                        let grid = report.key.unwrap();
                        assert!(
                            grid.rows >= 1 && grid.columns >= 2,
                            "{ui_language:?}/{text_scale:?}/{width}x{height}/limited={limited}"
                        );
                        let chrome_glyphs: usize = scene
                            .text_sections
                            .iter()
                            .flat_map(|section| &section.layouts)
                            .map(|layout| layout.atlas_glyphs.len())
                            .sum();
                        assert!(
                            scene.atlas_glyphs.len() > chrome_glyphs,
                            "body glyphs must actually be rendered"
                        );
                        if limited {
                            let warning = ui_language
                                .tr(LIMIT_NOTICE)
                                .chars()
                                .filter(|ch| !ch.is_whitespace())
                                .collect::<String>();
                            let layout = scene
                            .text_sections
                            .iter()
                            .flat_map(|section| &section.layouts)
                                .find(|layout| {
                                    layout
                                        .lines
                                        .iter()
                                        .flat_map(|line| line.chars())
                                        .filter(|ch| !ch.is_whitespace())
                                        .collect::<String>()
                                        == warning
                                })
                            .unwrap_or_else(|| {
                                panic!("complete warning missing: {ui_language:?}/{text_scale:?}/{width}x{height}")
                            });
                            assert!(
                                !layout.truncated,
                                "the display-limit warning is not an optional ellipsis label"
                            );
                            for glyph in &layout.atlas_glyphs {
                                let clip = glyph.clip_rect.unwrap();
                                assert!(glyph.rect[1] >= clip[1] - 0.01);
                                assert!(glyph.rect[1] + glyph.rect[3] <= clip[1] + clip[3] + 0.01);
                            }
                        }
                        report.presented();
                        let close = report.test_button_rect(None).unwrap();
                        assert!(close[0] >= 0.0 && close[1] >= 0.0);
                        assert!(close[0] + close[2] <= width && close[1] + close[3] <= height);
                        assert!(report.press(center(close), PointerSource::Mouse));
                        assert!(report.release(center(close), PointerSource::Mouse));
                    }
                }
            }
        }
    }
}
