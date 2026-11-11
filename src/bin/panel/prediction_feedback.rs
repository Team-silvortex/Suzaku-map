//! UI adapters for the independent engine; native host snapshots stay separate.
use super::PanelWindowKind;
use suzaku_map::{ime::XRTabletImeEngine, ui::PredictionNotice};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Update {
    pub candidates_changed: bool,
    pub state_changed: bool,
}

impl Update {
    pub fn changed(self) -> bool {
        self.candidates_changed || self.state_changed
    }
}

pub(super) fn poll(engine: &mut XRTabletImeEngine) -> Update {
    let before = (
        engine.prediction_status(),
        engine.prediction_error().cloned(),
    );
    let candidates_changed = engine.poll_prediction();
    Update {
        candidates_changed,
        state_changed: before
            != (
                engine.prediction_status(),
                engine.prediction_error().cloned(),
            ),
    }
}

pub(super) fn notice(
    kind: PanelWindowKind,
    native_context: bool,
    report: bool,
    compact: bool,
    engine: &XRTabletImeEngine,
) -> Option<PredictionNotice> {
    if kind != PanelWindowKind::Main || native_context || report || compact {
        return None;
    }
    PredictionNotice::from_state(engine.prediction_status(), engine.prediction_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Mutex, mpsc},
        time::{Duration, Instant},
    };
    use suzaku_map::{
        ime::{EngineConfig, PredictionStatus},
        prediction::{
            PredictionCancellation, PredictionError, PredictionProvider, PredictionRequest,
            PredictionResponse,
        },
    };

    struct Controlled {
        entered: mpsc::Sender<()>,
        reply: Mutex<mpsc::Receiver<PredictionError>>,
    }

    impl PredictionProvider for Controlled {
        fn predict(
            &self,
            _: &PredictionRequest,
            _: &PredictionCancellation,
        ) -> Result<PredictionResponse, PredictionError> {
            self.entered.send(()).unwrap();
            Err(self
                .reply
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap())
        }
    }

    #[test]
    fn render_local_notices_do_not_persist_or_create_settings_mirror_churn() {
        use suzaku_map::ime::gpu::PanelChromeState;
        let main = PanelChromeState::default();
        let mut settings = main.clone();
        let persisted = super::super::app_state::PersistedDisplaySettings::from(&main);
        for notice in [
            PredictionNotice::Waiting,
            PredictionNotice::Received,
            PredictionNotice::Error(PredictionError::Timeout),
        ] {
            let mut render_chrome = main.clone();
            render_chrome.prediction_notice = Some(notice);
            assert_eq!(
                super::super::app_state::PersistedDisplaySettings::from(&render_chrome),
                persisted
            );
            assert!(main.prediction_notice.is_none());
            assert!(!super::super::mirror_settings_chrome(&mut settings, &main));
            assert!(!super::super::mirror_settings_chrome(&mut settings, &main));
            assert!(settings.prediction_notice.is_none());
        }
    }

    #[test]
    fn a_failed_current_request_changes_feedback_without_changing_candidates_once() {
        for error in [
            PredictionError::Timeout,
            PredictionError::MissingCredentials,
            PredictionError::CloudConsentRequired,
            PredictionError::HttpStatus(404),
            PredictionError::HttpStatus(429),
            PredictionError::NoCandidates,
        ] {
            let (entered, entered_rx) = mpsc::channel();
            let (reply, reply_rx) = mpsc::channel();
            let mut engine = XRTabletImeEngine::new(EngineConfig::default());
            engine.set_preferences_enabled(false);
            engine.enable_ibus_candidate_mix();
            engine.configure_prediction_provider(Some(Arc::new(Controlled {
                entered,
                reply: Mutex::new(reply_rx),
            })));
            engine.seed("hel");
            entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let before = engine.snapshot();
            let candidates = engine.candidates().to_vec();
            assert_eq!(
                notice(PanelWindowKind::Main, false, false, false, &engine),
                Some(PredictionNotice::Waiting)
            );
            reply.send(error.clone()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            let update = loop {
                let update = poll(&mut engine);
                if update.changed() {
                    break update;
                }
                assert!(
                    Instant::now() < deadline,
                    "no feedback update for {error:?}"
                );
                std::thread::sleep(Duration::from_millis(2));
            };
            assert!(!update.candidates_changed);
            assert!(update.state_changed);
            assert_eq!(engine.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(engine.prediction_error(), Some(&error));
            assert_eq!(engine.snapshot(), before);
            assert_eq!(engine.candidates(), candidates);
            assert_eq!(
                notice(PanelWindowKind::Main, false, false, false, &engine),
                Some(PredictionNotice::Error(error))
            );
            assert!(
                !poll(&mut engine).changed(),
                "the same error must not continually redraw"
            );
            for (kind, native, report, compact) in [
                (PanelWindowKind::Main, true, false, false),
                (PanelWindowKind::Settings, false, false, false),
                (PanelWindowKind::Main, false, true, false),
                (PanelWindowKind::Main, false, false, true),
            ] {
                assert_eq!(notice(kind, native, report, compact, &engine), None);
            }
            engine.cancel_prediction();
            assert_eq!(
                notice(PanelWindowKind::Main, false, false, false, &engine),
                None
            );
            assert!(engine.prediction_error().is_none());
        }
    }
}
