//! Read-only presentation of a local engine's prediction state, never a request.
use super::UiLanguage;
use crate::{ime::PredictionStatus, prediction::PredictionError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PredictionNotice {
    Waiting,
    Received,
    Unavailable,
    Error(PredictionError),
}

impl PredictionNotice {
    pub fn from_state(status: PredictionStatus, error: Option<&PredictionError>) -> Option<Self> {
        match status {
            PredictionStatus::Disabled | PredictionStatus::Idle => None,
            PredictionStatus::Pending => Some(Self::Waiting),
            PredictionStatus::Ready => Some(Self::Received),
            PredictionStatus::Unavailable => {
                Some(error.cloned().map(Self::Error).unwrap_or(Self::Unavailable))
            }
        }
    }

    pub fn render(&self, ui: UiLanguage) -> String {
        match self {
            Self::Waiting => ui
                .tr("Waiting for model result; local input remains available")
                .into(),
            // Ready includes local corroboration, not necessarily a new AI row.
            Self::Received => ui
                .tr("Model response received; local candidates may be unchanged")
                .into(),
            Self::Unavailable => ui.tr("Model unavailable; local candidates retained").into(),
            Self::Error(error) => ui.diagnostic(&error.to_string()).into_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_current_active_states_expose_a_notice() {
        for status in [PredictionStatus::Idle, PredictionStatus::Disabled] {
            assert_eq!(
                PredictionNotice::from_state(status, Some(&PredictionError::Timeout)),
                None
            );
        }
        assert_eq!(
            PredictionNotice::from_state(
                PredictionStatus::Pending,
                Some(&PredictionError::Timeout)
            ),
            Some(PredictionNotice::Waiting)
        );
        assert_eq!(
            PredictionNotice::from_state(PredictionStatus::Ready, None),
            Some(PredictionNotice::Received)
        );
        assert_eq!(
            PredictionNotice::from_state(PredictionStatus::Unavailable, None),
            Some(PredictionNotice::Unavailable)
        );
    }

    #[test]
    fn typed_errors_use_existing_exact_eight_language_diagnostics() {
        for ui in UiLanguage::ALL
            .into_iter()
            .filter(|ui| *ui != UiLanguage::System)
        {
            for error in [
                PredictionError::Timeout,
                PredictionError::MissingCredentials,
                PredictionError::CloudConsentRequired,
                PredictionError::HttpStatus(401),
                PredictionError::HttpStatus(404),
                PredictionError::HttpStatus(429),
                PredictionError::InvalidResponse,
                PredictionError::NoCandidates,
            ] {
                let expected = ui.diagnostic(&error.to_string()).into_owned();
                let notice =
                    PredictionNotice::from_state(PredictionStatus::Unavailable, Some(&error))
                        .unwrap();
                assert_eq!(notice.render(ui), expected);
            }
            let missing = PredictionNotice::Error(PredictionError::MissingCredentials).render(ui);
            assert_ne!(
                missing,
                PredictionNotice::Error(PredictionError::HttpStatus(401)).render(ui)
            );
            assert_ne!(
                missing,
                PredictionNotice::Error(PredictionError::CloudConsentRequired).render(ui)
            );
            assert!(!PredictionNotice::Received.render(ui).contains("AI"));
        }
    }
}
