//! Candidate gesture regressions on an isolated Xvfb, never the desktop session.
use super::*;
use std::sync::{Mutex, mpsc};
use suzaku_map::ime::CommitOptions;
use suzaku_map::languages::llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest};
use winit::platform::x11::EventLoopBuilderExtX11;

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_candidate_clicks_and_async_refresh_are_safe() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    assert!(std::env::var("DISPLAY").unwrap().split('.').next().unwrap() != ":0");
    let mut builder = EventLoop::<()>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let mut probe = CandidateProbe { completed: false };
    event_loop.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct CandidateProbe {
    completed: bool,
}

impl ApplicationHandler for CandidateProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    panel_window_attributes()
                        .with_title("Synthetic candidate QA")
                        .with_inner_size(LogicalSize::new(900.0, 480.0)),
                )
                .unwrap(),
        );
        let mut state = pollster::block_on(PanelState::new(window)).unwrap();
        state.runs_without_window_focus = true;
        state.chrome.llm_enabled = false;
        state.chrome.input_modes_expanded = false;
        check_editable_keyboard_choices(&mut state);
        check_number_choice_history(&mut state);
        check_held_rewind_history_boundaries(&mut state);
        check_rapid_completion_gestures(&mut state);
        for touch in [false, true] {
            reset_composition(&mut state);
            press(&mut state, InteractionKind::SelectNextToken(0), touch);
            // A refresh with identical candidates must not eat a valid click.
            state.refresh_composition_candidates();
            assert_eq!(
                state.interaction.pressed_interaction,
                Some(InteractionKind::SelectNextToken(0))
            );
            state.complete_primary_release(touch);
            assert_eq!(
                state.chrome.seed_text, "hello",
                "first completion click was lost"
            );
            assert_eq!(state.engine.snapshot().seed_text, "hello");
            assert_eq!(state.chrome.composed_tokens, ["hello"]);
            let completed = state.engine.snapshot();
            let history = state.completion_history.labels();
            state.complete_primary_release(touch);
            assert_eq!(state.engine.snapshot(), completed);
            assert_eq!(state.chrome.seed_text, "hello");
            assert_eq!(state.completion_history.labels(), history);
            press(&mut state, InteractionKind::RewindNextToken, touch);
            state.complete_primary_release(touch);
            assert_eq!(state.chrome.seed_text, "hel");
            assert!(state.chrome.composed_tokens.is_empty());
        }
        check_held_completion_boundaries(&mut state);
        for touch in [false, true] {
            for target in [
                InteractionKind::Candidate(2),
                InteractionKind::SelectNextToken(1),
            ] {
                check_async_refresh(&mut state, target, touch);
            }
        }
        check_standalone_commit_delivery(&mut state);
        println!(
            "PASS: mouse/touch completion and rewind; asynchronous candidate replacement cancels stale presses"
        );
        self.completed = true;
        event_loop.exit();
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

fn reset_composition(state: &mut PanelState) {
    state.engine.configure_prediction(None);
    state.chrome.set_seed_text("hel".into());
    state.chrome.move_caret_to_end();
    state.completion_history.clear();
    state.last_interaction_action = None;
    state.engine.seed("hel");
    state.refresh_composition_candidates();
    state.last_scene = None;
}

fn check_held_completion_boundaries(state: &mut PanelState) {
    let language = state.engine.snapshot().active_language;
    for touch in [false, true] {
        for boundary in [
            "canceled",
            "new press",
            "seed",
            "language",
            "edit",
            "resize",
        ] {
            state.engine.set_language(&language);
            reset_composition(state);
            press(state, InteractionKind::SelectNextToken(0), touch);
            let viewport = state.window.inner_size();
            state.refresh_composition_candidates();
            assert!(state.last_scene.is_none());
            assert_eq!(
                state.interaction.pressed_interaction,
                Some(InteractionKind::SelectNextToken(0))
            );
            assert!(state.interaction.held_completion.is_some());
            match boundary {
                "canceled" | "new press" => {
                    if boundary == "canceled" {
                        state.cancel_primary_interaction();
                        assert!(state.interaction.held_completion.is_none());
                    }
                    // Neither a canceled gesture nor another press may acquire
                    // the old completion without a newly presented scene.
                    state.begin_primary_press(touch);
                    assert!(state.interaction.pressed_interaction.is_none());
                    assert!(state.interaction.held_completion.is_none());
                }
                // Change one identity component while the previous candidate
                // list is still cached. It must not authorize the old release.
                "seed" => {
                    state.chrome.set_seed_text("a different draft".into());
                    state.engine.seed("a different draft");
                }
                "language" => {
                    assert_ne!(language, "zh");
                    state.engine.set_language("zh");
                }
                "edit" => {
                    let edit = &mut state.next_token_completions[0];
                    assert_eq!(edit.seed_after, "hello");
                    edit.seed_after = "help".into();
                    assert_eq!(state.chrome.next_token_candidates[0], edit.label);
                }
                "resize" => {
                    let requested =
                        winit::dpi::PhysicalSize::new(viewport.width + 1, viewport.height);
                    let _ = state.window.request_inner_size(requested);
                    let actual = state.window.inner_size();
                    assert_eq!(actual, requested);
                    state.resize(actual.width, actual.height);
                    assert!(state.interaction.pressed_interaction.is_none());
                    assert!(state.interaction.held_completion.is_none());
                }
                _ => unreachable!(),
            }
            let before = state.engine.snapshot();
            let seed = state.chrome.seed_text.clone();
            let history = state.completion_history.labels();
            state.complete_primary_release(touch);
            assert_eq!(
                state.engine.snapshot(),
                before,
                "stale completion changed engine state: {boundary}, touch={touch}"
            );
            assert_eq!(state.chrome.seed_text, seed);
            assert_eq!(state.completion_history.labels(), history);
            assert!(state.interaction.held_completion.is_none());
            if boundary == "resize" {
                let _ = state.window.request_inner_size(viewport);
                let actual = state.window.inner_size();
                assert_eq!(actual, viewport);
                state.resize(actual.width, actual.height);
            }
        }
    }
    state.engine.set_language(language);
    reset_composition(state);
    println!(
        "PASS: held mouse/touch completions cannot authorize a new press or survive seed, language, edit or viewport changes"
    );
}

fn check_rapid_completion_gestures(state: &mut PanelState) {
    for touch in [false, true] {
        reset_composition(state);
        let committed = state.engine.snapshot().committed_text;
        let mut seeds = vec![state.chrome.seed_text.clone()];
        // Freshly displayed words can occupy the same slot. Each new press is
        // an intentional edit, even inside the settings/commit debounce window.
        for step in 0..2 {
            let edit = state.next_token_completions[0].clone();
            assert_ne!(edit.seed_before, edit.seed_after);
            press(state, InteractionKind::SelectNextToken(0), touch);
            if step > 0 {
                // Deterministic fast-input fixture, independent of slow CI rendering.
                state.last_interaction_action =
                    Some((InteractionKind::SelectNextToken(0), Instant::now()));
            }
            state.complete_primary_release(touch);
            assert_eq!(
                state.chrome.seed_text, edit.seed_after,
                "N17: new word in the same slot was mistaken for a duplicate; touch={touch}, step={step}"
            );
            assert_eq!(state.engine.snapshot().seed_text, edit.seed_after);
            seeds.push(edit.seed_after);
            // A duplicated release WITHOUT a new press must remain a no-op.
            state.complete_primary_release(touch);
            assert_eq!(state.chrome.seed_text, *seeds.last().unwrap());
        }
        assert_eq!(state.chrome.composed_tokens.len(), 2);
        for (step, expected) in seeds[..2].iter().rev().enumerate() {
            press(state, InteractionKind::RewindNextToken, touch);
            if step > 0 {
                state.last_interaction_action =
                    Some((InteractionKind::RewindNextToken, Instant::now()));
            }
            state.complete_primary_release(touch);
            assert_eq!(
                state.chrome.seed_text, *expected,
                "N17: a new undo gesture was discarded; touch={touch}, step={step}"
            );
            assert_eq!(state.engine.snapshot().seed_text, *expected);
            state.complete_primary_release(touch);
            assert_eq!(state.chrome.seed_text, *expected);
        }
        assert!(state.chrome.composed_tokens.is_empty());
        assert_eq!(state.engine.snapshot().committed_text, committed);
        // A new click can reapply the same completion after an intentional undo.
        press(state, InteractionKind::SelectNextToken(0), touch);
        state.complete_primary_release(touch);
        assert_eq!(state.chrome.seed_text, seeds[1]);
        println!(
            "PASS: rapid completion/rewind gestures, duplicate-release suppression and reapply; touch={touch}"
        );
    }
}

fn check_editable_keyboard_choices(state: &mut PanelState) {
    reset_composition(state);
    let committed = state.engine.snapshot().committed_text;
    let word = state
        .engine
        .candidates()
        .iter()
        .position(|candidate| candidate.text == "hello")
        .unwrap();
    state.continue_sentence_candidate(word);
    assert_eq!(state.chrome.seed_text, "hello");
    assert_eq!(state.engine.snapshot().seed_text, "hello");
    assert_eq!(state.engine.snapshot().committed_text, committed);
    for expected in ["hello ", "hello  "] {
        state.continue_composition_with_space();
        assert_eq!(state.chrome.seed_text, expected);
        assert_eq!(state.engine.snapshot().seed_text, expected);
        assert_eq!(state.engine.snapshot().committed_text, committed);
    }
    let before = state.engine.snapshot();
    state.continue_sentence_candidate(usize::MAX);
    assert_eq!(
        state.engine.snapshot(),
        before,
        "an absent candidate slot modified the draft"
    );
    reset_composition(state);
    state.engine.select_candidate(word);
    state.continue_composition_with_space();
    assert_eq!(
        state.chrome.seed_text, "hello ",
        "Space lost an explicit selection"
    );
    assert_eq!(state.engine.snapshot().committed_text, committed);
    reset_composition(state);
    state.continue_composition_with_space();
    assert_eq!(
        state.chrome.seed_text, "hel ",
        "Space changed an unselected literal spelling"
    );
    println!(
        "PASS: panel number choices and repeated spaces retain editable text without committing"
    );
}

fn reset_number_choice_fixture(state: &mut PanelState, language: &str, seed: &str, expanded: bool) {
    state.engine.configure_prediction(None);
    state.sync_manual_seed_base();
    state.engine.set_language(language);
    state.chrome.set_seed_text(seed.into());
    state.chrome.move_caret_to_end();
    state.chrome.blur_input();
    state.chrome.input_modes_expanded = expanded;
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    state.engine.seed(seed);
    state.refresh_composition_candidates();
    state.last_interaction_action = None;
    state.last_scene = None;
}

fn assert_number_choice_draft(state: &PanelState, seed: &str, committed: &str) {
    assert_eq!(state.chrome.seed_text, seed);
    assert_eq!(state.engine.snapshot().seed_text, seed);
    assert_eq!(state.engine.snapshot().committed_text, committed);
    assert_eq!(state.chrome.caret_index, seed.chars().count());
}

fn adopt_number_choice(
    state: &mut PanelState,
    text: &str,
    kind: Option<suzaku_map::ime::candidate_mix::CandidateKind>,
) {
    let index = state
        .engine
        .candidates()
        .iter()
        .position(|candidate| {
            candidate.text == text && kind.is_none_or(|kind| candidate.kind == kind)
        })
        .unwrap_or_else(|| panic!("missing {text:?}/{kind:?}: {:?}", state.engine.candidates()));
    state.continue_sentence_candidate(index);
}

fn rewind_number_choice(state: &mut PanelState, touch: bool, expected: &str, committed: &str) {
    press(state, InteractionKind::RewindNextToken, touch);
    let rect = state
        .last_scene
        .as_ref()
        .unwrap()
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::RewindNextToken)
        .unwrap()
        .rect;
    let viewport = state.window.inner_size();
    assert!(rect[0] >= 0.0 && rect[0] + rect[2] <= viewport.width as f32);
    assert!(rect[1] >= 0.0 && rect[1] + rect[3] <= viewport.height as f32);
    state.complete_primary_release(touch);
    assert_number_choice_draft(state, expected, committed);
    let snapshot = state.engine.snapshot();
    let history = state.completion_history.labels();
    // A duplicated release cannot consume another adoption.
    state.complete_primary_release(touch);
    assert_eq!(state.engine.snapshot(), snapshot);
    assert_eq!(state.completion_history.labels(), history);
}

fn check_number_choice_history(state: &mut PanelState) {
    use suzaku_map::ime::candidate_mix::CandidateKind;

    assert!(!state.native.showing);
    let original_chrome = state.chrome.clone();
    let original_language = state.engine.snapshot().active_language;
    let committed = state.engine.snapshot().committed_text;
    let cases = [
        ("en", "hel", "hello", "hello, how are you?"),
        ("zh-Hans", "nihao", "你好", "你好，很高兴认识你。"),
        ("ja", "nihongo", "日本語", "日本語を勉強しています。"),
    ];

    // First exercise the real default standalone policy. It has no mixed
    // authored-sentence promise and need not attach Word/Sentence kind hints.
    for (language, reading, word, _) in cases {
        reset_number_choice_fixture(state, language, reading, false);
        adopt_number_choice(state, word, None);
        assert_number_choice_draft(state, word, &committed);
        assert_eq!(state.completion_history.labels().len(), 1);
        assert_eq!(state.chrome.composed_tokens.len(), 1);
        if language != "en" {
            assert!(state.chrome.next_token_candidates.is_empty());
        }
        rewind_number_choice(state, false, reading, &committed);
        assert!(state.completion_history.labels().is_empty());
        assert!(state.chrome.composed_tokens.is_empty());
    }

    // A temporary real mixed engine supplies typed builtin Word/Sentence
    // payloads to the same panel adoption path. This is a fixture, not a change
    // to the production standalone candidate policy or the user's host.
    let mut mixed = XRTabletImeEngine::new(EngineConfig::default());
    mixed.enable_ibus_candidate_mix();
    let original_engine = std::mem::replace(&mut state.engine, mixed);
    let committed = state.engine.snapshot().committed_text;
    for (language, reading, word, sentence) in cases {
        // Two representative layouts/input routes, not a costly cross product.
        for (touch, expanded) in [(false, false), (true, true)] {
            reset_number_choice_fixture(state, language, reading, expanded);
            adopt_number_choice(state, word, Some(CandidateKind::Word));
            assert_number_choice_draft(state, word, &committed);
            let history = state.completion_history.labels();
            assert_eq!(history.len(), 1);
            let snapshot = state.engine.snapshot();
            // An unchanged payload is not an edit, irrespective of its kind:
            // English keeps a known word typed as Word after deduplication.
            adopt_number_choice(state, word, None);
            state.continue_sentence_candidate(usize::MAX);
            assert_eq!(state.engine.snapshot(), snapshot);
            assert_eq!(state.completion_history.labels(), history);
            assert_eq!(state.chrome.composed_tokens.len(), 1);
            adopt_number_choice(state, sentence, Some(CandidateKind::Sentence));
            assert_number_choice_draft(state, sentence, &committed);
            assert_eq!(state.completion_history.labels().len(), 2);
            assert_eq!(state.chrome.composed_tokens.len(), 2);
            if language != "en" {
                assert!(state.chrome.next_token_candidates.is_empty());
            }
            rewind_number_choice(state, touch, word, &committed);
            assert_eq!(state.completion_history.labels().len(), 1);
            rewind_number_choice(state, touch, reading, &committed);
            assert!(state.chrome.composed_tokens.is_empty());
            assert!(
                state
                    .current_scene()
                    .interactive_targets
                    .iter()
                    .all(|target| { target.kind != InteractionKind::RewindNextToken })
            );
            // The sentence may also be adopted directly from the original reading.
            adopt_number_choice(state, sentence, Some(CandidateKind::Sentence));
            assert_eq!(state.completion_history.labels().len(), 1);
            rewind_number_choice(state, touch, reading, &committed);
        }

        for boundary in ["typing", "backspace", "space", "new seed", "language"] {
            reset_number_choice_fixture(state, language, reading, false);
            adopt_number_choice(state, word, Some(CandidateKind::Word));
            match boundary {
                "typing" => {
                    state.chrome.focus_input();
                    state.handle_text_input("x");
                    assert_eq!(state.chrome.seed_text, format!("{word}x"));
                }
                "backspace" => {
                    state.backspace_seed();
                    assert_eq!(
                        state.chrome.seed_text,
                        word.chars()
                            .take(word.chars().count() - 1)
                            .collect::<String>()
                    );
                }
                "space" => {
                    // Existing Space semantics remain a manual writing separator.
                    state.continue_composition_with_space();
                    assert_eq!(state.chrome.seed_text, format!("{word} "));
                }
                "new seed" => {
                    state.chrome.set_seed_text("unrelated draft".into());
                    state.chrome.move_caret_to_end();
                    state.sync_manual_seed_base();
                    state.refresh_seed();
                }
                "language" => {
                    // Match the production settings-update cleanup in panel.rs;
                    // directly mutating a naked engine is not a panel gesture.
                    state.sync_manual_seed_base();
                    state
                        .engine
                        .set_language(if language == "ja" { "en" } else { "ja" });
                    state.refresh_composition_candidates();
                }
                _ => unreachable!(),
            }
            assert!(
                state.completion_history.labels().is_empty(),
                "{language}/{boundary}"
            );
            assert!(
                state.chrome.composed_tokens.is_empty(),
                "{language}/{boundary}"
            );
            let seed = state.chrome.seed_text.clone();
            let snapshot = state.engine.snapshot();
            state.rewind_next_token();
            assert_eq!(state.chrome.seed_text, seed, "{language}/{boundary}");
            assert_eq!(state.engine.snapshot(), snapshot, "{language}/{boundary}");
            assert_eq!(snapshot.committed_text, committed);
        }

        reset_number_choice_fixture(state, language, reading, false);
        adopt_number_choice(state, word, Some(CandidateKind::Word));
        let stale_index = state
            .engine
            .candidates()
            .iter()
            .position(|candidate| candidate.text == sentence)
            .unwrap();
        let snapshot = state.engine.snapshot();
        let history = state.completion_history.labels();
        state
            .chrome
            .set_seed_text("a newer draft not yet rebuilt".into());
        state.chrome.move_caret_to_end();
        let caret = state.chrome.caret_index;
        state.continue_sentence_candidate(stale_index);
        assert_eq!(state.chrome.seed_text, "a newer draft not yet rebuilt");
        assert_eq!(state.chrome.caret_index, caret);
        assert_eq!(state.engine.snapshot(), snapshot);
        assert_eq!(state.completion_history.labels(), history);
    }
    state.engine = original_engine;
    state.engine.set_language(original_language);
    state.chrome = original_chrome;
    reset_composition(state);
    println!(
        "PASS: default standalone EN/ZH/JA number adoption undo; typed word/sentence fixture chains, mouse/touch Back, no-op and manual-edit boundaries"
    );
}

fn press(state: &mut PanelState, target: InteractionKind, touch: bool) {
    crate::native_sync::present_test_frame(state);
    let scene = state
        .last_scene
        .as_ref()
        .expect("presented candidate fixture");
    let rect = scene
        .interactive_targets
        .iter()
        .find(|item| item.kind == target)
        .unwrap_or_else(|| {
            panic!(
                "missing {target:?} for draft {:?}, words {:?}, history {:?}",
                state.chrome.seed_text,
                state.chrome.next_token_candidates,
                state.chrome.composed_tokens
            )
        })
        .rect;
    let (x, y) = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
    assert_eq!(scene.hit_interaction(x, y), Some(target));
    state.cursor_position = Some((x, y));
    state.interaction.last_input_was_touch = touch;
    state.begin_primary_press(touch);
    assert_eq!(state.interaction.pressed_interaction, Some(target));
}

fn check_held_rewind_history_boundaries(state: &mut PanelState) {
    use suzaku_map::ime::companion::{NativeCandidate, NativeComposition};

    assert!(!state.native.showing);
    let original_chrome = state.chrome.clone();
    let original_language = state.engine.snapshot().active_language;
    let original_focus = state.is_focused;
    let original_native = std::mem::take(&mut state.native);
    let committed = state.engine.snapshot().committed_text;
    for touch in [false, true] {
        // Append comes first so the unfixed failure explicitly demonstrates a
        // held Back undoing a NEW keyboard adoption, not merely a lost gesture.
        for boundary in [
            "append next token",
            "append number choice",
            "undo then redo",
            "manual clear then adopt",
            "native round trip",
            "same history refresh",
            "no-op and invalid choices",
        ] {
            reset_number_choice_fixture(state, "en", "hel", false);
            state.is_focused = false;
            adopt_number_choice(state, "hello", None);
            assert_number_choice_draft(state, "hello", &committed);
            let initial_labels = state.completion_history.labels();
            assert_eq!(initial_labels.len(), 1);
            press(state, InteractionKind::RewindNextToken, touch);
            let started_at = state.interaction.press_start_instant.unwrap();
            let original_cursor = state.cursor_position.unwrap();
            let mut fresh_rewind_seed = "hel";
            match boundary {
                "append next token" => {
                    let edit = state.next_token_completions[0].clone();
                    assert_eq!(edit.seed_before, "hello");
                    assert_ne!(edit.seed_after, "hello");
                    // A keyboard route changes history while the Back pointer
                    // stays held. It must not acquire a second pointer press.
                    state.select_next_token(0);
                    assert_number_choice_draft(state, &edit.seed_after, &committed);
                    assert_eq!(state.completion_history.labels().len(), 2);
                    fresh_rewind_seed = "hello";
                }
                "append number choice" => {
                    let index = state
                        .engine
                        .candidates()
                        .iter()
                        .position(|candidate| candidate.text != "hello")
                        .expect("default hello candidates include a different adoption");
                    let text = state.engine.candidates()[index].text.clone();
                    state.continue_sentence_candidate(index);
                    assert_number_choice_draft(state, &text, &committed);
                    assert_eq!(state.completion_history.labels().len(), 2);
                    fresh_rewind_seed = "hello";
                }
                "undo then redo" => {
                    state.rewind_next_token();
                    assert_number_choice_draft(state, "hel", &committed);
                    assert!(state.completion_history.labels().is_empty());
                    adopt_number_choice(state, "hello", None);
                    assert_number_choice_draft(state, "hello", &committed);
                    assert_eq!(state.completion_history.labels(), initial_labels);
                }
                "manual clear then adopt" => {
                    // Real manual deletion clears history; returning to the
                    // exact same reading/output cannot revive the old press.
                    state.backspace_seed();
                    state.backspace_seed();
                    assert_number_choice_draft(state, "hel", &committed);
                    assert!(state.completion_history.labels().is_empty());
                    adopt_number_choice(state, "hello", None);
                    assert_number_choice_draft(state, "hello", &committed);
                    assert_eq!(state.completion_history.labels(), initial_labels);
                }
                "native round trip" => {
                    state.receive_native_frame(Some(NativeComposition {
                        host: "00000000-0000-0000-0000-000000000019".into(),
                        context: 19,
                        revision: 1,
                        focused: true,
                        private: false,
                        language: "en".into(),
                        seed: "external draft".into(),
                        selected: 0,
                        cursor: None,
                        candidates: vec![NativeCandidate {
                            text: "external draft".into(),
                            label: "external draft".into(),
                            ..Default::default()
                        }],
                    }));
                    assert!(state.native.showing);
                    assert_eq!(state.completion_history.labels(), initial_labels);
                    state.receive_native_frame(None);
                    state.leave_native_view();
                    assert!(!state.native.showing);
                    assert_number_choice_draft(state, "hello", &committed);
                    assert_eq!(state.completion_history.labels(), initial_labels);
                }
                "same history refresh" => state.refresh_composition_candidates(),
                "no-op and invalid choices" => {
                    adopt_number_choice(state, "hello", None);
                    state.continue_sentence_candidate(usize::MAX);
                    state.select_next_token(usize::MAX);
                    assert_eq!(state.completion_history.labels(), initial_labels);
                    state.refresh_composition_candidates();
                }
                _ => unreachable!(),
            }
            let changed_history = !matches!(
                boundary,
                "same history refresh" | "no-op and invalid choices"
            );
            let snapshot = state.engine.snapshot();
            let seed = state.chrome.seed_text.clone();
            let labels = state.completion_history.labels();
            // Repaint without a new press: geometry and visible Back identity
            // may be identical even when the underlying history is different.
            crate::native_sync::present_test_frame(state);
            assert_eq!(state.cursor_position, Some(original_cursor));
            assert_eq!(
                state
                    .last_scene
                    .as_ref()
                    .unwrap()
                    .hit_interaction(original_cursor.0, original_cursor.1,),
                Some(InteractionKind::RewindNextToken),
                "Back geometry must remain an eligible target: {boundary}, touch={touch}"
            );
            assert!(
                started_at.elapsed()
                    <= Duration::from_millis(u64::from(state.chrome.pointer_tap_max_ms)),
                "fixture exceeded the unchanged production tap deadline: {boundary}, touch={touch}"
            );
            state.complete_primary_release(touch);
            if changed_history {
                assert_eq!(
                    state.chrome.seed_text, seed,
                    "old held Back undid newer adoption history: {boundary}, touch={touch}"
                );
                assert_eq!(state.engine.snapshot(), snapshot);
                assert_eq!(state.completion_history.labels(), labels);
                assert_eq!(state.chrome.composed_tokens, labels);
                // A canceled old gesture is not a permanent disable: a NEW
                // press acts on the currently displayed undo entry normally.
                rewind_number_choice(state, touch, fresh_rewind_seed, &committed);
            } else {
                assert_number_choice_draft(state, "hel", &committed);
                assert!(state.completion_history.labels().is_empty());
                assert!(state.chrome.composed_tokens.is_empty());
            }
            let after = state.engine.snapshot();
            let labels = state.completion_history.labels();
            state.complete_primary_release(touch);
            assert_eq!(state.engine.snapshot(), after);
            assert_eq!(state.completion_history.labels(), labels);
            assert_eq!(state.engine.snapshot().committed_text, committed);
        }
    }
    state.native = original_native;
    state.is_focused = original_focus;
    state.chrome = original_chrome;
    state.engine.set_language(original_language);
    reset_composition(state);
    println!(
        "PASS: held mouse/touch Back rejects appended, ABA-replaced and native-roundtrip history; identical refresh/no-op stays actionable without extending gesture deadlines"
    );
}

struct GatedProvider {
    started: mpsc::Sender<()>,
    reply: Mutex<mpsc::Receiver<()>>,
}

impl LlmCompletionProvider for GatedProvider {
    fn provider_id(&self) -> &str {
        "candidate-gesture-fixture"
    }

    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        assert_eq!(request.seed_text, "hel");
        self.started.send(()).unwrap();
        self.reply
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        vec![LlmCompletion {
            text: "helipad".into(),
            score_bias: 1.0,
            kind: None,
        }]
    }
}

fn check_async_refresh(state: &mut PanelState, target: InteractionKind, touch: bool) {
    reset_composition(state);
    let (started_tx, started) = mpsc::channel();
    let (reply, reply_rx) = mpsc::channel();
    state
        .engine
        .configure_prediction(Some(Arc::new(GatedProvider {
            started: started_tx,
            reply: Mutex::new(reply_rx),
        })));
    state.refresh_composition_candidates();
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(state.engine.candidates()[2].text, "help");
    press(state, target, touch);
    reply.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !state.engine.poll_prediction() {
        assert!(Instant::now() < deadline, "fixture result did not arrive");
        std::thread::sleep(Duration::from_millis(2));
    }
    state.refresh_composition_candidates();
    assert_eq!(state.engine.candidates()[2].text, "helipad");
    assert!(
        state.interaction.pressed_interaction.is_none(),
        "a replaced candidate retained the old press"
    );
    assert!(!state.interaction.touch_tap_pending);
    let before = state.engine.snapshot();
    state.complete_primary_release(touch);
    assert_eq!(
        state.engine.snapshot(),
        before,
        "release applied a replacement candidate"
    );
    assert_eq!(state.chrome.seed_text, "hel");
    state.engine.configure_prediction(None);
}

fn check_standalone_commit_delivery(state: &mut PanelState) {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
    use suzaku_map::platform::ime_host_dispatch::current_ime_host_dispatch;
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let host = current_ime_host_dispatch();
        if host.marked_text_roundtrip && host.commit_roundtrip {
            break;
        }
        assert!(
            Instant::now() < until,
            "fixture must also exercise a ready local bridge"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let path = std::path::PathBuf::from(std::env::var_os("SUZAKU_LINUX_IME_SOCKET").unwrap());
    let runtime = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
    assert!(runtime.starts_with(std::env::temp_dir()) && path.starts_with(&runtime));
    assert!(
        !path.exists(),
        "fixture must never replace another host's socket"
    );
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let response = Arc::new(AtomicU8::new(b'1'));
    let requests = Arc::new(Mutex::new(Vec::<String>::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (server_response, server_requests, server_stop) =
        (response.clone(), requests.clone(), stop.clone());
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !server_stop.load(Ordering::Acquire) && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_millis(200)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_millis(200)))
                        .unwrap();
                    let mut text = String::new();
                    if stream.read_to_string(&mut text).is_err() {
                        continue;
                    }
                    if let Some(text) = text.strip_prefix('C') {
                        server_requests.lock().unwrap().push(text.into());
                        let ack = server_response.load(Ordering::Acquire);
                        if ack != 0 {
                            let _ = stream.write_all(&[ack]);
                        }
                    } else {
                        let _ = stream.write_all(b"0");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("fixture accept: {error}"),
            }
        }
    });
    for sentence in [false, true] {
        state.engine.clear_session_context();
        state.native = Default::default();
        state.is_focused = false;
        state.chrome.settings_open = false;
        state.last_commit_attempt = None;
        state
            .chrome
            .set_seed_text("a replacement requiring confirmation".into());
        state.refresh_seed();
        let before = requests.lock().unwrap().len();
        let snapshot = state.engine.snapshot();
        assert!(!state.commit_selected_candidate_to_host(CommitOptions::default()));
        assert_eq!(
            requests.lock().unwrap().len(),
            before,
            "unconfirmed text reached the socket"
        );
        assert_eq!(state.engine.snapshot(), snapshot);
        let commit = |state: &mut PanelState| {
            if sentence {
                state.commit_sentence_candidate(0)
            } else {
                state.commit_selected_candidate_to_host(CommitOptions { force: true })
            }
        };
        for text in ["  first123  ", "second456"] {
            response.store(b'1', Ordering::Release);
            state.chrome.set_seed_text(text.into());
            state.refresh_seed();
            // A separate local bridge can have different provider results or a
            // stale seed. Only the candidate the panel displayed belongs on wire.
            suzaku_map::ime_host::host_bridge_replace_marked_text(
                "wrong-source-candidate",
                suzaku_map::ime::InputSource::OnScreenPanel,
            );
            let before = requests.lock().unwrap().len();
            assert!(commit(state));
            assert_eq!(requests.lock().unwrap()[before..], [text]);
            assert!(
                state.chrome.seed_text.is_empty(),
                "delivered text was copied back into the editable draft"
            );
            assert!(state.engine.snapshot().seed_text.is_empty());
            assert!(!commit(state), "a second click recommitted completed text");
            assert_eq!(requests.lock().unwrap().len(), before + 1);
        }
        for ack in [b'0', 0] {
            response.store(ack, Ordering::Release);
            state.last_commit_attempt = None;
            state.chrome.set_seed_text("  retained789  ".into());
            state.refresh_seed();
            let before = requests.lock().unwrap().len();
            let snapshot = state.engine.snapshot();
            assert!(commit(state));
            assert_eq!(requests.lock().unwrap()[before..], ["  retained789  "]);
            assert_eq!(
                state.engine.snapshot(),
                snapshot,
                "unconfirmed output changed composition or committed context"
            );
            assert_eq!(state.chrome.seed_text, "  retained789  ");
            assert!(
                state
                    .last_commit_feedback
                    .as_ref()
                    .unwrap()
                    .contains("retained")
            );
        }
    }
    stop.store(true, Ordering::Release);
    server.join().unwrap();
    std::fs::remove_file(path).unwrap();
    println!(
        "PASS: standalone word/sentence commits clear only acknowledged drafts and retain rejected/uncertain output without replay"
    );
}
