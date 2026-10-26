//! Glue between the engine and the OS: reads idle state, applies decisions.

use crate::config::Config;
use crate::engine::{Decision, Engine, Inputs, Mode, Status};
use crate::platform::Platform;
use chrono::Local;
use std::time::{Duration, Instant};

/// How often Nudge wakes up to check idle time.
pub const TICK: Duration = Duration::from_secs(5);

pub struct Nudger {
    pub config: Config,
    pub engine: Engine,
    platform: Platform,
    started: Instant,
    /// `Some(display)` while a keep-awake assertion is held.
    awake_held: Option<bool>,
    pub last: Decision,
    pub last_error: Option<String>,
}

impl Nudger {
    pub fn new(config: Config, mode: Mode) -> Self {
        Nudger {
            config,
            engine: Engine::new(mode),
            platform: Platform::new(),
            started: Instant::now(),
            awake_held: None,
            last: Decision { status: Status::Paused, nudge: false, keep_awake: false, timer_ended: false },
            last_error: None,
        }
    }

    pub fn tick(&mut self) -> Decision {
        let locked = self.config.pause_when_locked && self.platform.screen_locked();
        let inputs = Inputs {
            wall: Local::now().naive_local(),
            mono: self.started.elapsed(),
            idle: self.platform.idle_time(),
            locked,
        };
        let d = self.engine.tick(&self.config, &inputs);
        self.sync_awake(d.keep_awake);
        if d.nudge {
            self.last_error = self.platform.nudge(self.config.nudge_method).err();
        }
        self.last = d;
        d
    }

    fn sync_awake(&mut self, want: bool) {
        let target = want.then_some(self.config.keep_display_awake);
        if target == self.awake_held {
            return;
        }
        self.platform.release_awake();
        if let Some(display) = target {
            self.platform.hold_awake(display);
        }
        self.awake_held = target;
    }

    pub fn shutdown(&mut self) {
        self.sync_awake(false);
    }

    pub fn idle_time(&self) -> Duration {
        self.platform.idle_time()
    }

    pub fn input_permission(&self) -> Option<bool> {
        self.platform.input_permission()
    }

    pub fn request_input_permission(&self) {
        self.platform.request_input_permission();
    }
}

impl Drop for Nudger {
    fn drop(&mut self) {
        self.shutdown();
    }
}
