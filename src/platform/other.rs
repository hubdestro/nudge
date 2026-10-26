//! Fallback for unsupported platforms: compiles and runs the test suite, does nothing.

use crate::config::NudgeMethod;
use std::time::Duration;

pub struct Platform;

impl Platform {
    pub fn new() -> Self {
        Platform
    }

    pub fn idle_time(&self) -> Duration {
        Duration::ZERO
    }

    pub fn idle_report(&self) -> Vec<(&'static str, Duration, bool)> {
        vec![("unsupported platform", Duration::ZERO, true)]
    }

    pub fn screen_locked(&self) -> bool {
        false
    }

    pub fn nudge(&mut self, _method: NudgeMethod) -> Result<&'static str, String> {
        Err("nudging is not supported on this platform".into())
    }

    pub fn hold_awake(&mut self, _display: bool) {}

    pub fn release_awake(&mut self) {}

    pub fn input_permission(&self) -> Option<bool> {
        None
    }

    pub fn request_input_permission(&self) {}
}
