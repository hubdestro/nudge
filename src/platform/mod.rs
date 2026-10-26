//! OS integration. Each platform provides the same `Platform` type:
//!
//! - `idle_time()`      time since the last user input
//! - `idle_report()`    every idle source we can read (for `nudge status` / `selftest`)
//! - `screen_locked()`
//! - `nudge(method)`    reset the system idle timer without moving the cursor
//! - `hold_awake(display)` / `release_awake()`
//!
//! Linux supports X11 sessions; on Wayland only keep-awake works.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod other;
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub use other::*;

/// Opens `path` in `$VISUAL` / `$EDITOR`, or the platform's default text editor, and waits.
pub fn edit_file(path: &std::path::Path) -> std::io::Result<()> {
    use std::process::Command;
    let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).ok().filter(|e| !e.trim().is_empty());
    let status = match editor {
        // Allow editors with arguments, e.g. `code --wait`.
        Some(e) => {
            let mut parts = e.split_whitespace();
            Command::new(parts.next().unwrap_or("vi")).args(parts).arg(path).status()?
        }
        None if cfg!(target_os = "macos") => Command::new("open").args(["-W", "-t"]).arg(path).status()?,
        None if cfg!(windows) => Command::new("notepad.exe").arg(path).status()?,
        None => Command::new("vi").arg(path).status()?,
    };
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("editor exited with {status}")))
    }
}
