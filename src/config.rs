//! User settings, stored as JSON and edited with `nudge config set <key> <value>`.

use crate::duration;
use crate::paths;
use crate::schedule::Schedule;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum NudgeMethod {
    /// Recommended: `input`, plus `activity` on macOS.
    #[default]
    Auto,
    /// macOS only: declare user activity through the power-management API. Keeps the
    /// display awake but does not reset the idle clock chat apps read.
    Activity,
    /// Post a zero-distance mouse move: the cursor stays exactly where it is.
    /// On macOS this reaches apps only with Accessibility access.
    Input,
}

impl NudgeMethod {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "auto" => Ok(NudgeMethod::Auto),
            "activity" => Ok(NudgeMethod::Activity),
            "input" => Ok(NudgeMethod::Input),
            _ => Err(format!("invalid method {s:?} (expected auto, input or activity)")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            NudgeMethod::Auto => "auto",
            NudgeMethod::Activity => "activity",
            NudgeMethod::Input => "input",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Nudge only after you've been idle this long. While you're working, Nudge does nothing.
    pub idle_threshold_secs: u64,
    /// While you're away, nudge this often. Teams marks you Away after ~5 minutes.
    pub nudge_interval_secs: u64,
    /// Keep the display on (not just the system) while active.
    pub keep_display_awake: bool,
    /// Stop covering for you when you lock the screen.
    pub pause_when_locked: bool,
    pub nudge_method: NudgeMethod,
    pub schedule: Schedule,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            idle_threshold_secs: 60,
            nudge_interval_secs: 60,
            keep_display_awake: true,
            pause_when_locked: true,
            nudge_method: NudgeMethod::Auto,
            schedule: Schedule::default(),
        }
    }
}

const MIN_SECS: u64 = 10;
const MAX_SECS: u64 = 3600;

/// Settings reachable from `nudge config`: key, accepted values, description.
pub const KEYS: [(&str, &str, &str); 7] = [
    ("idle", "<duration>", "start nudging after this much idle time (10s–1h)"),
    ("interval", "<duration>", "while you're away, nudge this often (10s–1h)"),
    ("schedule", "on | off | HH:MM-HH:MM", "only keep you green during work hours"),
    ("days", "mon-fri | every-day | mon,wed,fri", "days the work-hours window applies to"),
    ("display", "on | off", "keep the display awake while active"),
    ("lock-pause", "on | off", "pause while the screen is locked"),
    ("method", "auto | input | activity", "how to nudge (auto is right for almost everyone)"),
];

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.trim().to_lowercase().as_str() {
        "on" | "true" | "yes" | "1" | "enable" | "enabled" => Ok(true),
        "off" | "false" | "no" | "0" | "disable" | "disabled" => Ok(false),
        _ => Err(format!("invalid value {s:?} (expected on or off)")),
    }
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

const WEEK: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// `mon-fri`, `weekdays`, `every-day`, `sat,sun`, `mon-wed,fri`.
fn parse_days(s: &str) -> Result<Vec<String>, String> {
    let s = s.trim().to_lowercase();
    match s.as_str() {
        "weekdays" => return Ok(WEEK[..5].iter().map(|d| d.to_string()).collect()),
        "weekends" | "weekend" => return Ok(vec!["sat".into(), "sun".into()]),
        "every-day" | "everyday" | "daily" | "all" => return Ok(WEEK.iter().map(|d| d.to_string()).collect()),
        _ => {}
    }
    let index = |d: &str| -> Result<usize, String> {
        let wd: chrono::Weekday = d.trim().parse().map_err(|_| format!("unknown day {d:?}"))?;
        Ok(wd.num_days_from_monday() as usize)
    };
    let mut days = [false; 7];
    for part in s.split(',').filter(|p| !p.trim().is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (index(a)?, index(b)?);
                let mut i = a;
                loop {
                    days[i] = true;
                    if i == b {
                        break;
                    }
                    i = (i + 1) % 7;
                }
            }
            None => days[index(part)?] = true,
        }
    }
    let out: Vec<String> = WEEK.iter().zip(days).filter(|(_, on)| *on).map(|(d, _)| d.to_string()).collect();
    if out.is_empty() {
        return Err("no days given".into());
    }
    Ok(out)
}

impl Config {
    /// Loads the config, writing the defaults on first run so there's a file to edit.
    /// A broken file is left untouched and the defaults are used.
    pub fn load() -> (Config, Option<String>) {
        let path = paths::config();
        if !path.exists() {
            let config = Config::default();
            let warning = config
                .save_to(&path)
                .err()
                .map(|e| format!("could not write {}: {e}", path.display()));
            return (config, warning);
        }
        match Self::load_from(&path) {
            Ok(config) => {
                let warning = config.schedule.validate().err();
                (config, warning)
            }
            Err(e) => (Config::default(), Some(e)),
        }
    }

    pub fn load_from(path: &Path) -> Result<Config, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("invalid config {}: {e}", path.display()))
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&paths::config())
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text = serde_json::to_string_pretty(self).expect("config serializes");
        text.push('\n');
        std::fs::write(path, text)
    }

    pub fn idle_threshold(&self) -> Duration {
        Duration::from_secs(self.idle_threshold_secs.clamp(MIN_SECS, MAX_SECS))
    }

    pub fn nudge_interval(&self) -> Duration {
        Duration::from_secs(self.nudge_interval_secs.clamp(MIN_SECS, MAX_SECS))
    }

    pub fn get(&self, key: &str) -> Result<String, String> {
        Ok(match key {
            "idle" => duration::format(self.idle_threshold()),
            "interval" => duration::format(self.nudge_interval()),
            "schedule" if self.schedule.enabled => self.schedule.summary(),
            "schedule" => format!("off ({} when on)", self.schedule.summary()),
            "days" => self.schedule.days.join(","),
            "display" => on_off(self.keep_display_awake).into(),
            "lock-pause" => on_off(self.pause_when_locked).into(),
            "method" => self.nudge_method.name().into(),
            _ => return Err(unknown_key(key)),
        })
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let secs = |v: &str| -> Result<u64, String> {
            let s = duration::parse(v)?.as_secs();
            if !(MIN_SECS..=MAX_SECS).contains(&s) {
                return Err(format!("{v} is out of range (10s–1h)"));
            }
            Ok(s)
        };
        match key {
            "idle" => self.idle_threshold_secs = secs(value)?,
            "interval" => self.nudge_interval_secs = secs(value)?,
            "schedule" => match parse_bool(value) {
                Ok(on) => self.schedule.enabled = on,
                Err(_) => {
                    let (start, end) = value
                        .split_once('-')
                        .ok_or_else(|| format!("invalid schedule {value:?} (expected on, off or 09:00-18:00)"))?;
                    let mut s = self.schedule.clone();
                    s.start = start.trim().into();
                    s.end = end.trim().into();
                    s.enabled = true;
                    s.validate()?;
                    self.schedule = s;
                }
            },
            "days" => self.schedule.days = parse_days(value)?,
            "display" => self.keep_display_awake = parse_bool(value)?,
            "lock-pause" => self.pause_when_locked = parse_bool(value)?,
            "method" => self.nudge_method = NudgeMethod::parse(value)?,
            _ => return Err(unknown_key(key)),
        }
        Ok(())
    }
}

fn unknown_key(key: &str) -> String {
    let keys: Vec<_> = KEYS.iter().map(|(k, _, _)| *k).collect();
    format!("unknown setting {key:?} (available: {})", keys.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_file_falls_back_to_defaults() {
        let c: Config = serde_json::from_str(r#"{ "idle_threshold_secs": 120 }"#).unwrap();
        assert_eq!(c.idle_threshold_secs, 120);
        assert_eq!(c.nudge_interval_secs, 60);
        assert!(c.keep_display_awake);
        assert_eq!(c.schedule, Schedule::default());
    }

    #[test]
    fn nudge_method_names() {
        let c: Config = serde_json::from_str(r#"{ "nudge_method": "input" }"#).unwrap();
        assert_eq!(c.nudge_method, NudgeMethod::Input);
        assert!(serde_json::from_str::<Config>(r#"{ "nudge_method": "wiggle" }"#).is_err());
    }

    #[test]
    fn durations_are_clamped() {
        let c = Config { idle_threshold_secs: 0, nudge_interval_secs: 999_999, ..Config::default() };
        assert_eq!(c.idle_threshold(), Duration::from_secs(10));
        assert_eq!(c.nudge_interval(), Duration::from_secs(3600));
    }

    #[test]
    fn set_and_get() {
        let mut c = Config::default();
        c.set("idle", "2m").unwrap();
        assert_eq!(c.idle_threshold_secs, 120);
        assert_eq!(c.get("idle").unwrap(), "2m");
        c.set("interval", "90s").unwrap();
        assert_eq!(c.get("interval").unwrap(), "1m 30s");
        c.set("display", "off").unwrap();
        assert!(!c.keep_display_awake);
        c.set("lock-pause", "no").unwrap();
        assert!(!c.pause_when_locked);
        c.set("method", "input").unwrap();
        assert_eq!(c.nudge_method, NudgeMethod::Input);

        assert!(!c.schedule.enabled);
        c.set("schedule", "08:30-17:00").unwrap();
        assert!(c.schedule.enabled);
        assert_eq!(c.get("schedule").unwrap(), "08:30–17:00, Mon–Fri");
        c.set("schedule", "off").unwrap();
        assert!(!c.schedule.enabled);
        assert_eq!(c.schedule.start, "08:30", "turning off keeps the hours");
    }

    #[test]
    fn set_rejects_bad_values() {
        let mut c = Config::default();
        assert!(c.set("idle", "5s").is_err(), "below minimum");
        assert!(c.set("idle", "2h").is_err(), "above maximum");
        assert!(c.set("idle", "soon").is_err());
        assert!(c.set("display", "maybe").is_err());
        assert!(c.set("schedule", "9am-5pm").is_err());
        assert!(c.set("bogus", "1").unwrap_err().contains("available: idle"));
        assert_eq!(c, Config::default(), "failed sets change nothing");
    }

    #[test]
    fn days() {
        let d = |s| parse_days(s).unwrap().join(",");
        assert_eq!(d("mon-fri"), "mon,tue,wed,thu,fri");
        assert_eq!(d("weekdays"), "mon,tue,wed,thu,fri");
        assert_eq!(d("every-day"), "mon,tue,wed,thu,fri,sat,sun");
        assert_eq!(d("sun,sat"), "sat,sun");
        assert_eq!(d("fri-mon"), "mon,fri,sat,sun", "ranges wrap around the week");
        assert_eq!(d("Monday,wed-thu"), "mon,wed,thu");
        assert!(parse_days("funday").is_err());
        assert!(parse_days(",").is_err());
    }

    #[test]
    fn round_trip_through_file() {
        let dir = std::env::temp_dir().join(format!("nudge-test-{}", std::process::id()));
        let path = dir.join("sub").join("config.json");
        let mut c = Config::default();
        c.schedule.enabled = true;
        c.nudge_method = NudgeMethod::Activity;
        c.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path).unwrap(), c);
        std::fs::write(&path, "{ not json").unwrap();
        assert!(Config::load_from(&path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
