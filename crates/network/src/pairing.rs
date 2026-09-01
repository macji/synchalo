use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use subtle::ConstantTimeEq;
use synchalo_core::{AppError, PairingCodeView};

const MAX_ATTEMPTS_PER_MINUTE: usize = 8;

#[derive(Clone)]
pub struct PairingCodeManager {
    state: Arc<Mutex<PairingState>>,
}

struct PairingState {
    active: Option<ActiveCode>,
    attempts: VecDeque<Instant>,
}

struct ActiveCode {
    code: String,
    expires_instant: Instant,
    expires_at: DateTime<Utc>,
}

impl Default for PairingCodeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PairingCodeManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PairingState {
                active: None,
                attempts: VecDeque::new(),
            })),
        }
    }

    pub fn generate(&self, ttl: Duration) -> Result<PairingCodeView, AppError> {
        let mut random = [0_u8; 4];
        getrandom::fill(&mut random).map_err(|error| AppError::Internal(error.to_string()))?;
        let value = u32::from_le_bytes(random) % 1_000_000;
        let compact = format!("{value:06}");
        let code = format!("{} {}", &compact[..3], &compact[3..]);
        let expires_at = Utc::now()
            + chrono::Duration::from_std(ttl)
                .map_err(|error| AppError::Internal(error.to_string()))?;

        self.state.lock().active = Some(ActiveCode {
            code: compact,
            expires_instant: Instant::now() + ttl,
            expires_at,
        });
        Ok(PairingCodeView { code, expires_at })
    }

    pub fn current(&self) -> Option<PairingCodeView> {
        let mut state = self.state.lock();
        if state
            .active
            .as_ref()
            .is_some_and(|active| Instant::now() >= active.expires_instant)
        {
            state.active = None;
        }
        state.active.as_ref().map(|active| PairingCodeView {
            code: format!("{} {}", &active.code[..3], &active.code[3..]),
            expires_at: active.expires_at,
        })
    }

    pub fn validate_and_consume(&self, candidate: &str) -> Result<bool, AppError> {
        let now = Instant::now();
        let mut state = self.state.lock();
        record_attempt(&mut state, now)?;

        let Some(active) = state.active.as_ref() else {
            return Ok(false);
        };
        if now >= active.expires_instant {
            state.active = None;
            return Ok(false);
        }

        let normalized: String = candidate
            .chars()
            .filter(|char| char.is_ascii_digit())
            .collect();
        let matches =
            normalized.len() == 6 && active.code.as_bytes().ct_eq(normalized.as_bytes()).into();
        if matches {
            state.active = None;
        }
        Ok(matches)
    }

    pub fn invalidate(&self) {
        self.state.lock().active = None;
    }

    pub(crate) fn begin_network_attempt(&self) -> Result<Option<String>, AppError> {
        let now = Instant::now();
        let mut state = self.state.lock();
        record_attempt(&mut state, now)?;
        if state
            .active
            .as_ref()
            .is_some_and(|active| now >= active.expires_instant)
        {
            state.active = None;
        }
        Ok(state.active.as_ref().map(|active| active.code.clone()))
    }

    pub(crate) fn consume_active_code(&self, expected: &str) -> bool {
        let mut state = self.state.lock();
        let matches = state.active.as_ref().is_some_and(|active| {
            active.code.as_bytes().ct_eq(expected.as_bytes()).into()
                && Instant::now() < active.expires_instant
        });
        if matches {
            state.active = None;
        }
        matches
    }
}

fn record_attempt(state: &mut PairingState, now: Instant) -> Result<(), AppError> {
    while state
        .attempts
        .front()
        .is_some_and(|attempt| now.duration_since(*attempt) >= Duration::from_secs(60))
    {
        state.attempts.pop_front();
    }
    if state.attempts.len() >= MAX_ATTEMPTS_PER_MINUTE {
        return Err(AppError::Network(
            "too many pairing attempts; try again later".to_owned(),
        ));
    }
    state.attempts.push_back(now);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_six_digits_and_single_use() {
        let manager = PairingCodeManager::new();
        let view = manager.generate(Duration::from_secs(60)).unwrap();
        let compact = view.code.replace(' ', "");
        assert_eq!(compact.len(), 6);
        assert!(compact.chars().all(|char| char.is_ascii_digit()));
        assert!(manager.validate_and_consume(&compact).unwrap());
        assert!(!manager.validate_and_consume(&compact).unwrap());
    }

    #[test]
    fn pairing_attempts_are_rate_limited() {
        let manager = PairingCodeManager::new();
        let generated = manager.generate(Duration::from_secs(60)).unwrap();
        let candidate = if generated.code.replace(' ', "") == "000000" {
            "111111"
        } else {
            "000000"
        };
        for _ in 0..MAX_ATTEMPTS_PER_MINUTE {
            assert!(!manager.validate_and_consume(candidate).unwrap());
        }
        assert!(manager.validate_and_consume(candidate).is_err());
    }
}
