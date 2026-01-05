//! Session management for PIN-based authentication

use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Session configuration
pub struct SessionConfig {
    /// Auto-lock timeout in seconds (0 = disabled)
    pub auto_lock_timeout_secs: u64,
    /// Max failed PIN attempts before lockout
    pub max_failed_attempts: u32,
    /// Lockout duration in seconds
    pub lockout_duration_secs: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            auto_lock_timeout_secs: 300, // 5 minutes
            max_failed_attempts: 3,
            lockout_duration_secs: 30,
        }
    }
}

/// Session state
pub struct SessionManager {
    config: SessionConfig,
    state: RwLock<SessionState>,
}

struct SessionState {
    is_authenticated: bool,
    last_activity: Option<Instant>,
    failed_attempts: u32,
    lockout_until: Option<Instant>,
}

impl SessionManager {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            state: RwLock::new(SessionState {
                is_authenticated: false,
                last_activity: None,
                failed_attempts: 0,
                lockout_until: None,
            }),
        }
    }

    /// Check if session is authenticated and not expired
    pub async fn is_authenticated(&self) -> bool {
        let state = self.state.read().await;

        if !state.is_authenticated {
            return false;
        }

        // Check auto-lock timeout
        if self.config.auto_lock_timeout_secs > 0 {
            if let Some(last) = state.last_activity {
                let timeout = Duration::from_secs(self.config.auto_lock_timeout_secs);
                if last.elapsed() > timeout {
                    return false;
                }
            }
        }

        true
    }

    /// Check if currently locked out due to failed attempts
    pub async fn is_locked_out(&self) -> bool {
        let state = self.state.read().await;

        if let Some(until) = state.lockout_until {
            if Instant::now() < until {
                return true;
            }
        }

        false
    }

    /// Get remaining lockout time in seconds
    pub async fn lockout_remaining_secs(&self) -> u64 {
        let state = self.state.read().await;

        if let Some(until) = state.lockout_until {
            let now = Instant::now();
            if now < until {
                return (until - now).as_secs();
            }
        }

        0
    }

    /// Attempt to authenticate with PIN
    pub async fn authenticate(&self, _pin: &str, verify_fn: impl FnOnce(&str) -> bool) -> Result<(), AuthError> {
        // Check lockout
        if self.is_locked_out().await {
            let remaining = self.lockout_remaining_secs().await;
            return Err(AuthError::LockedOut(remaining));
        }

        let mut state = self.state.write().await;

        // Verify PIN (actual verification delegated to caller)
        if verify_fn(_pin) {
            state.is_authenticated = true;
            state.last_activity = Some(Instant::now());
            state.failed_attempts = 0;
            state.lockout_until = None;
            Ok(())
        } else {
            state.failed_attempts += 1;

            if state.failed_attempts >= self.config.max_failed_attempts {
                state.lockout_until = Some(
                    Instant::now() + Duration::from_secs(self.config.lockout_duration_secs)
                );
                Err(AuthError::TooManyAttempts)
            } else {
                Err(AuthError::InvalidPin(
                    self.config.max_failed_attempts - state.failed_attempts
                ))
            }
        }
    }

    /// Update last activity timestamp
    pub async fn update_activity(&self) {
        let mut state = self.state.write().await;
        if state.is_authenticated {
            state.last_activity = Some(Instant::now());
        }
    }

    /// Lock the session
    pub async fn lock(&self) {
        let mut state = self.state.write().await;
        state.is_authenticated = false;
        state.last_activity = None;
    }
}

/// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    /// Invalid PIN with remaining attempts
    InvalidPin(u32),
    /// Too many failed attempts
    TooManyAttempts,
    /// Locked out with remaining seconds
    LockedOut(u64),
}
