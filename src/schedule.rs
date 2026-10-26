//! Work-hours schedule: when Nudge is allowed to keep you green.

use chrono::{Datelike, NaiveDateTime, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Schedule {
    /// When false, Nudge is active at any time of day.
    pub enabled: bool,
    /// Days the window *starts* on, e.g. `["mon", "tue"]`. Full names work too.
    pub days: Vec<String>,
    /// Start of the window, `HH:MM` (24h, local time).
    pub start: String,
    /// End of the window, `HH:MM`. May be earlier than `start` for overnight shifts.
    pub end: String,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule {
            enabled: false,
            days: ["mon", "tue", "wed", "thu", "fri"].map(String::from).to_vec(),
            start: "09:00".into(),
            end: "18:00".into(),
        }
    }
}

const ALL_DAYS: [Weekday; 7] = [
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
    Weekday::Sat,
    Weekday::Sun,
];

fn parse_time(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

impl Schedule {
    pub fn weekdays(&self) -> Vec<Weekday> {
        self.days.iter().filter_map(|d| d.trim().parse().ok()).collect()
    }

    /// Whether `at` (local wall-clock time) falls inside the schedule window.
    /// An unparsable start/end never blocks Nudge; `validate` reports it instead.
    pub fn contains(&self, at: NaiveDateTime) -> bool {
        let (Some(start), Some(end)) = (parse_time(&self.start), parse_time(&self.end)) else {
            return true;
        };
        let days = self.weekdays();
        let today = at.weekday();
        let t = at.time();
        match start.cmp(&end) {
            Ordering::Equal => days.contains(&today),
            Ordering::Less => days.contains(&today) && t >= start && t < end,
            // Overnight window: the part after midnight belongs to the previous day.
            Ordering::Greater => (days.contains(&today) && t >= start) || (days.contains(&today.pred()) && t < end),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if parse_time(&self.start).is_none() {
            return Err(format!("schedule.start {:?} is not HH:MM", self.start));
        }
        if parse_time(&self.end).is_none() {
            return Err(format!("schedule.end {:?} is not HH:MM", self.end));
        }
        if let Some(bad) = self.days.iter().find(|d| d.trim().parse::<Weekday>().is_err()) {
            return Err(format!("schedule.days contains unknown day {bad:?}"));
        }
        if self.weekdays().is_empty() {
            return Err("schedule.days is empty".into());
        }
        Ok(())
    }

    /// Human-readable summary, e.g. `09:00–18:00, Mon–Fri`.
    pub fn summary(&self) -> String {
        let mut days = self.weekdays();
        days.sort_by_key(|d| d.num_days_from_monday());
        days.dedup();
        let days = if days == ALL_DAYS {
            "every day".to_string()
        } else if days == ALL_DAYS[..5] {
            "Mon–Fri".to_string()
        } else {
            days.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", ")
        };
        format!("{}–{}, {}", self.start.trim(), self.end.trim(), days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    // 2026-10-26 is a Monday.
    fn at(day: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    fn sched(days: &[&str], start: &str, end: &str) -> Schedule {
        Schedule {
            enabled: true,
            days: days.iter().map(|s| s.to_string()).collect(),
            start: start.into(),
            end: end.into(),
        }
    }

    #[test]
    fn weekday_window() {
        let s = Schedule::default();
        assert!(s.contains(at(26, 9, 0)), "start is inclusive");
        assert!(s.contains(at(26, 17, 59)));
        assert!(!s.contains(at(26, 18, 0)), "end is exclusive");
        assert!(!s.contains(at(26, 8, 59)));
        assert!(!s.contains(at(31, 12, 0)), "Saturday");
        assert!(s.contains(at(30, 12, 0)), "Friday");
    }

    #[test]
    fn overnight_window() {
        let s = sched(&["mon"], "22:00", "06:00");
        assert!(s.contains(at(26, 23, 0)), "Monday night");
        assert!(s.contains(at(27, 5, 59)), "early Tuesday belongs to Monday's shift");
        assert!(!s.contains(at(27, 6, 0)));
        assert!(!s.contains(at(27, 23, 0)), "Tuesday's shift is not scheduled");
        assert!(!s.contains(at(26, 3, 0)), "early Monday belongs to Sunday's shift");
    }

    #[test]
    fn equal_start_end_means_all_day() {
        let s = sched(&["Monday"], "00:00", "00:00");
        assert!(s.contains(at(26, 0, 0)));
        assert!(s.contains(at(26, 23, 59)));
        assert!(!s.contains(at(27, 12, 0)));
    }

    #[test]
    fn invalid_times_never_block() {
        let s = sched(&["mon"], "9am", "18:00");
        assert!(s.contains(at(31, 3, 0)));
        assert!(s.validate().is_err());
    }

    #[test]
    fn validation() {
        assert!(Schedule::default().validate().is_ok());
        assert!(sched(&["funday"], "09:00", "17:00").validate().is_err());
        assert!(sched(&[], "09:00", "17:00").validate().is_err());
    }

    #[test]
    fn summaries() {
        assert_eq!(Schedule::default().summary(), "09:00–18:00, Mon–Fri");
        let all = sched(&["sun", "mon", "tue", "wed", "thu", "fri", "sat"], "08:00", "20:00");
        assert_eq!(all.summary(), "08:00–20:00, every day");
        assert_eq!(sched(&["wed", "mon"], "10:00", "12:00").summary(), "10:00–12:00, Mon, Wed");
    }
}
