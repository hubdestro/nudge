//! The decision engine: pure logic, no OS calls, fully unit-tested.
//!
//! The one rule that matters: **while you're using the computer, Nudge does nothing.**
//! It only nudges once you've been idle for `idle_threshold`, and then at most once per
//! `nudge_interval`.

use crate::config::Config;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Whether the user turned Nudge on, and for how long.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Off,
    On,
    Until(NaiveDateTime),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Turned off by the user, or the timer ran out.
    Paused,
    /// Turned on, but the schedule says it's outside work hours.
    OutsideSchedule,
    /// Turned on, but the screen is locked and `pause_when_locked` is set.
    ScreenLocked,
    /// You're at the keyboard; Nudge stays out of the way.
    UserActive,
    /// You're away; Nudge is keeping your status green.
    Covering,
}

impl Status {
    pub fn describe(self) -> &'static str {
        match self {
            Status::Paused => "paused",
            Status::OutsideSchedule => "waiting — outside work hours",
            Status::ScreenLocked => "paused — screen locked",
            Status::UserActive => "active — you're here",
            Status::Covering => "active — covering for you",
        }
    }
}

pub struct Inputs {
    /// Local wall-clock time (for the schedule and timers).
    pub wall: NaiveDateTime,
    /// Monotonic time since Nudge started (for rate limiting).
    pub mono: Duration,
    /// Time since the last keyboard/mouse input, as reported by the OS.
    pub idle: Duration,
    /// Screen is locked and the user wants Nudge to pause when it is.
    pub locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub status: Status,
    /// Send one nudge now.
    pub nudge: bool,
    /// Hold the keep-awake (no sleep / no screen saver) assertion.
    pub keep_awake: bool,
    /// The "keep green for…" timer ran out on this tick.
    pub timer_ended: bool,
}

/// How far idle time may lag behind "time since our nudge" and still count as
/// "no input since our nudge" (covers measurement jitter within a tick).
const OWN_NUDGE_SLACK: Duration = Duration::from_secs(1);

pub struct Engine {
    pub mode: Mode,
    last_nudge: Option<Duration>,
}

impl Engine {
    pub fn new(mode: Mode) -> Self {
        Engine { mode, last_nudge: None }
    }

    pub fn tick(&mut self, cfg: &Config, i: &Inputs) -> Decision {
        let mut timer_ended = false;
        if let Mode::Until(end) = self.mode {
            if i.wall >= end {
                self.mode = Mode::Off;
                timer_ended = true;
            }
        }
        let idle_decision = |status| Decision { status, nudge: false, keep_awake: false, timer_ended };

        if self.mode == Mode::Off {
            self.last_nudge = None;
            return idle_decision(Status::Paused);
        }
        if cfg.schedule.enabled && !cfg.schedule.contains(i.wall) {
            self.last_nudge = None;
            return idle_decision(Status::OutsideSchedule);
        }
        if i.locked {
            self.last_nudge = None;
            return idle_decision(Status::ScreenLocked);
        }

        // Some idle sources (e.g. Windows' GetLastInputInfo) are reset by our own nudge.
        // If there has been no input since our last nudge, the user is still away.
        let since_nudge = self.last_nudge.map(|t| i.mono.saturating_sub(t));
        let only_our_input = since_nudge.is_some_and(|s| i.idle + OWN_NUDGE_SLACK >= s);

        if i.idle < cfg.idle_threshold() && !only_our_input {
            self.last_nudge = None;
            return Decision { status: Status::UserActive, nudge: false, keep_awake: true, timer_ended };
        }

        let due = since_nudge.is_none_or(|s| s >= cfg.nudge_interval());
        if due {
            self.last_nudge = Some(i.mono);
        }
        Decision { status: Status::Covering, nudge: due, keep_awake: true, timer_ended }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    const TICK: u64 = 5;

    fn noon() -> NaiveDateTime {
        // Monday
        NaiveDate::from_ymd_opt(2026, 10, 26).unwrap().and_hms_opt(12, 0, 0).unwrap()
    }

    /// Simulates the OS idle clock across ticks.
    struct Sim {
        engine: Engine,
        cfg: Config,
        t: u64,
        last_input: u64,
        /// Whether our nudge resets the OS idle clock (true on Windows).
        nudge_resets_idle: bool,
        locked: bool,
        nudges: Vec<u64>,
    }

    impl Sim {
        fn new() -> Self {
            Sim {
                engine: Engine::new(Mode::On),
                cfg: Config::default(),
                t: 0,
                last_input: 0,
                nudge_resets_idle: false,
                locked: false,
                nudges: vec![],
            }
        }

        fn inputs(&self) -> Inputs {
            Inputs {
                wall: noon() + chrono::Duration::seconds(self.t as i64),
                mono: Duration::from_secs(self.t),
                idle: Duration::from_secs(self.t - self.last_input),
                locked: self.locked,
            }
        }

        fn tick(&mut self) -> Decision {
            let d = self.engine.tick(&self.cfg, &self.inputs());
            if d.nudge {
                self.nudges.push(self.t);
                if self.nudge_resets_idle {
                    self.last_input = self.t;
                }
            }
            d
        }

        /// Run for `secs`, ticking every TICK seconds. `typing` = user input every tick.
        fn run(&mut self, secs: u64, typing: bool) -> Vec<Decision> {
            let mut out = vec![];
            let end = self.t + secs;
            while self.t < end {
                self.t += TICK;
                if typing {
                    self.last_input = self.t;
                }
                out.push(self.tick());
            }
            out
        }
    }

    #[test]
    fn never_nudges_while_user_is_working() {
        for resets in [false, true] {
            let mut s = Sim::new();
            s.nudge_resets_idle = resets;
            let ds = s.run(8 * 3600, true);
            assert!(s.nudges.is_empty());
            assert!(ds.iter().all(|d| d.status == Status::UserActive && d.keep_awake));
        }
    }

    #[test]
    fn brief_pauses_below_threshold_do_not_nudge() {
        let mut s = Sim::new();
        for _ in 0..100 {
            s.run(30, true);
            s.run(55, false); // reading something for <60s
        }
        assert!(s.nudges.is_empty());
    }

    #[test]
    fn nudges_after_threshold_then_every_interval() {
        for resets in [false, true] {
            let mut s = Sim::new();
            s.nudge_resets_idle = resets;
            s.run(10, true);
            let ds = s.run(600, false);
            // last input at t=10; threshold 60s → first nudge at t=70, then every 60s.
            assert_eq!(s.nudges, vec![70, 130, 190, 250, 310, 370, 430, 490, 550, 610], "resets={resets}");
            assert!(ds.iter().filter(|d| d.status == Status::Covering).count() > 100);
            // Once covering, status stays Covering even when our nudge resets the idle clock.
            let first = ds.iter().position(|d| d.nudge).unwrap();
            assert!(ds[first..].iter().all(|d| d.status == Status::Covering), "resets={resets}");
        }
    }

    #[test]
    fn returning_user_is_detected_immediately() {
        for resets in [false, true] {
            let mut s = Sim::new();
            s.nudge_resets_idle = resets;
            s.run(300, false);
            assert!(!s.nudges.is_empty());
            let before = s.nudges.len();
            let ds = s.run(120, true);
            assert!(ds.iter().all(|d| d.status == Status::UserActive), "resets={resets}");
            assert_eq!(s.nudges.len(), before);
        }
    }

    #[test]
    fn off_and_locked_and_schedule_hold_nothing() {
        let mut s = Sim::new();
        s.engine.mode = Mode::Off;
        assert!(s.run(600, false).iter().all(|d| d.status == Status::Paused && !d.keep_awake));

        let mut s = Sim::new();
        s.locked = true;
        assert!(s.run(600, false).iter().all(|d| d.status == Status::ScreenLocked && !d.keep_awake));
        assert!(s.nudges.is_empty());

        let mut s = Sim::new();
        s.cfg.schedule.enabled = true;
        s.cfg.schedule.start = "13:00".into();
        s.cfg.schedule.end = "14:00".into();
        let ds = s.run(600, false);
        assert!(ds.iter().all(|d| d.status == Status::OutsideSchedule && !d.keep_awake));
        assert!(s.nudges.is_empty());
    }

    #[test]
    fn timer_turns_itself_off() {
        let mut s = Sim::new();
        s.engine.mode = Mode::Until(noon() + chrono::Duration::minutes(10));
        let ds = s.run(900, false);
        let ended: Vec<_> = ds.iter().filter(|d| d.timer_ended).collect();
        assert_eq!(ended.len(), 1);
        assert_eq!(s.engine.mode, Mode::Off);
        assert!(s.nudges.iter().all(|&t| t < 600));
        assert_eq!(ds.last().unwrap().status, Status::Paused);
    }

    #[test]
    fn unlocking_resumes_cleanly() {
        let mut s = Sim::new();
        s.run(300, false);
        s.locked = true;
        s.run(600, false);
        let n = s.nudges.len();
        s.locked = false;
        s.tick();
        // Still idle past the threshold after unlock → nudge right away.
        assert_eq!(s.nudges.len(), n + 1);
    }
}
