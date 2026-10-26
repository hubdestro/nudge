//! Files shared between the CLI and the background process.
//!
//! - `control.json`: written by the CLI (`start`, `pause`, `for 2h`…), watched by the daemon.
//! - `runtime.json`: written by the daemon, read by `nudge status`.

use crate::engine::{Mode, Status};
use crate::paths;
use chrono::NaiveDateTime;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Control {
    pub mode: Mode,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Runtime {
    pub pid: u32,
    pub started: NaiveDateTime,
    pub status: Status,
    pub mode: Mode,
    pub nudges: u64,
    pub last_nudge: Option<NaiveDateTime>,
    pub last_error: Option<String>,
}

fn read<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Write-then-rename so readers never see a half-written file.
fn write<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(value).expect("serializes"))?;
    std::fs::rename(tmp, path)
}

impl Control {
    pub fn load() -> Option<Control> {
        read(&paths::control())
    }

    pub fn save(&self) -> std::io::Result<()> {
        write(&paths::control(), self)
    }
}

impl Runtime {
    pub fn load() -> Option<Runtime> {
        read(&paths::runtime())
    }

    pub fn save(&self) -> std::io::Result<()> {
        write(&paths::runtime(), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn control_json_shape() {
        let until = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap().and_hms_opt(15, 30, 0).unwrap();
        let cases = [
            (Mode::On, r#"{"mode":"on"}"#),
            (Mode::Off, r#"{"mode":"off"}"#),
            (Mode::Until(until), r#"{"mode":{"until":"2026-10-26T15:30:00"}}"#),
        ];
        for (mode, json) in cases {
            assert_eq!(serde_json::to_string(&Control { mode }).unwrap(), json);
            assert_eq!(serde_json::from_str::<Control>(json).unwrap().mode, mode);
        }
    }
}
