//! Where Nudge keeps its files: one directory, easy to find and to delete.

use std::path::PathBuf;

/// `$NUDGE_HOME`, or `<config dir>/nudge`:
/// macOS `~/Library/Application Support/nudge`, Linux `~/.config/nudge`, Windows `%APPDATA%\nudge`.
pub fn home() -> PathBuf {
    if let Some(p) = std::env::var_os("NUDGE_HOME") {
        return PathBuf::from(p);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("nudge")
}

/// User settings.
pub fn config() -> PathBuf {
    home().join("config.json")
}

/// On / off / timer, written by the CLI and watched by the background process.
pub fn control() -> PathBuf {
    home().join("control.json")
}

/// What the background process is doing, written by it for `nudge status`.
pub fn runtime() -> PathBuf {
    home().join("runtime.json")
}

pub fn pid() -> PathBuf {
    home().join("nudge.pid")
}

pub fn log() -> PathBuf {
    home().join("nudge.log")
}
