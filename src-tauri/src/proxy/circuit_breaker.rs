use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

/// Simple circuit breaker for API entries.
/// State is kept in memory only (not persisted).
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    consecutive_failures: Arc<AtomicU32>,
    last_opened_at: Arc<RwLock<Option<Instant>>>,
    recovery_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            last_opened_at: Arc::new(RwLock::new(None)),
            recovery_secs: 60,
        }
    }

    pub fn is_available(&self) -> bool {
        // Check if we should transition from Open to HalfOpen
        let state = self.state.try_read();
        match state {
            Ok(s) => match *s {
                CircuitState::Closed => true,
                CircuitState::HalfOpen => true,
                CircuitState::Open => {
                    // Check if recovery time has passed
                    drop(s);
                    if let Ok(last_open) = self.last_opened_at.try_read() {
                        if let Some(opened_at) = *last_open {
                            if opened_at.elapsed().as_secs() >= self.recovery_secs {
                                // Transition to half-open
                                drop(last_open);
                                if let Ok(mut state) = self.state.try_write() {
                                    *state = CircuitState::HalfOpen;
                                }
                                return true;
                            }
                        }
                    }
                    false
                }
            },
            Err(_) => true, // If we can't read, assume available
        }
    }

    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        if let Ok(mut state) = self.state.try_write() {
            *state = CircuitState::Closed;
        }
    }

    pub fn record_failure(&self, threshold: u32) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures >= threshold {
            if let Ok(mut state) = self.state.try_write() {
                *state = CircuitState::Open;
            }
            if let Ok(mut last_opened) = self.last_opened_at.try_write() {
                *last_opened = Some(Instant::now());
            }
        }
    }

    pub fn get_state(&self) -> CircuitState {
        self.state
            .try_read()
            .map(|s| *s)
            .unwrap_or(CircuitState::Closed)
    }
}
