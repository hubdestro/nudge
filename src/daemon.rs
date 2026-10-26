//! The background process (`nudge run`) and the commands that manage it.

use crate::app::{Nudger, TICK};
use crate::config::Config;
use crate::engine::{Mode, Status};
use crate::state::{Control, Runtime};
use crate::{duration, paths, process};
use chrono::Local;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Rotate the log once it passes this size.
const MAX_LOG_BYTES: u64 = 512 * 1024;

/// PID of the running background process, if any.
pub fn running_pid() -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(paths::pid()).ok()?.trim().parse().ok()?;
    (pid != std::process::id() && process::is_alive(pid)).then_some(pid)
}

pub enum Started {
    New(u32),
    AlreadyRunning(u32),
}

/// Sets the mode and makes sure the background process is running.
pub fn start(mode: Mode) -> Result<Started, String> {
    std::fs::create_dir_all(paths::home()).map_err(|e| e.to_string())?;
    Control { mode }.save().map_err(|e| format!("could not save state: {e}"))?;
    if let Some(pid) = running_pid() {
        return Ok(Started::AlreadyRunning(pid));
    }
    rotate_log();
    let pid = process::spawn_detached(&paths::log()).map_err(|e| format!("could not start: {e}"))?;
    // Give it a moment, so "started" means it's really up.
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(50));
        if running_pid() == Some(pid) {
            return Ok(Started::New(pid));
        }
        if !process::is_alive(pid) {
            break;
        }
    }
    if process::is_alive(pid) {
        Ok(Started::New(pid))
    } else {
        Err(format!("it exited right away; see the log: {}", paths::log().display()))
    }
}

/// Stops the background process. Returns its PID if one was running.
pub fn stop() -> Result<Option<u32>, String> {
    let Some(pid) = running_pid() else {
        let _ = std::fs::remove_file(paths::pid());
        return Ok(None);
    };
    process::terminate(pid).map_err(|e| format!("could not stop pid {pid}: {e}"))?;
    for _ in 0..40 {
        if !process::is_alive(pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = std::fs::remove_file(paths::pid());
    let _ = std::fs::remove_file(paths::runtime());
    Ok(Some(pid))
}

/// Updates the mode of the running process. Returns false if nothing is running.
pub fn set_mode(mode: Mode) -> Result<bool, String> {
    if running_pid().is_none() {
        return Ok(false);
    }
    Control { mode }.save().map_err(|e| format!("could not save state: {e}"))?;
    Ok(true)
}

fn rotate_log() {
    let log = paths::log();
    if std::fs::metadata(&log).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = std::fs::rename(&log, log.with_extension("log.old"));
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub fn log(msg: &str) {
    println!("{} {msg}", Local::now().format("%Y-%m-%d %H:%M:%S"));
}

fn describe_mode(mode: Mode) -> String {
    match mode {
        Mode::On => "on".into(),
        Mode::Off => "paused".into(),
        Mode::Until(t) => format!("on until {}", t.format("%H:%M")),
    }
}

/// The background loop. `background` = started by `nudge start` (output goes to the log).
pub fn run(background: bool) -> Result<(), String> {
    if let Some(pid) = running_pid() {
        return Err(format!("already running (pid {pid}); use `nudge stop` first"));
    }
    std::fs::create_dir_all(paths::home()).map_err(|e| e.to_string())?;
    std::fs::write(paths::pid(), std::process::id().to_string()).map_err(|e| e.to_string())?;
    if background {
        process::ignore_hangup();
    }

    // Record mtimes before reading, so a change made in between is still picked up.
    let mut config_mtime = mtime(&paths::config());
    let mut control_mtime = mtime(&paths::control());
    let (config, warning) = Config::load();
    if let Some(w) = warning {
        log(&format!("warning: {w}"));
    }
    let mode = Control::load().map_or(Mode::On, |c| c.mode);
    let mut n = Nudger::new(config, mode);
    log(&format!(
        "started v{} (pid {}) — {}; nudging after {} idle, every {}",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        describe_mode(mode),
        duration::format(n.config.idle_threshold()),
        duration::format(n.config.nudge_interval()),
    ));
    if n.input_permission() == Some(false) {
        log("warning: no Accessibility access — nudges won't reach Teams. Run `nudge doctor`.");
        n.request_input_permission();
    }

    let mut runtime = Runtime {
        pid: std::process::id(),
        started: Local::now().naive_local(),
        status: Status::Paused,
        mode,
        nudges: 0,
        last_nudge: None,
        last_error: None,
    };
    let mut last_status: Option<Status> = None;
    let mut last_error: Option<String> = None;

    loop {
        // Pick up `nudge config set …` and `nudge pause/resume/for …` without a restart.
        let m = mtime(&paths::config());
        if m != config_mtime {
            config_mtime = m;
            match Config::load_from(&paths::config()) {
                Ok(c) => {
                    n.config = c;
                    log("settings reloaded");
                }
                Err(e) => log(&format!("warning: keeping previous settings: {e}")),
            }
        }
        let m = mtime(&paths::control());
        if m != control_mtime {
            control_mtime = m;
            if let Some(c) = Control::load() {
                if c.mode != n.engine.mode {
                    n.engine.mode = c.mode;
                    log(&format!("mode: {}", describe_mode(c.mode)));
                }
            }
        }

        let d = n.tick();
        if d.timer_ended {
            log("timer ended — paused");
            let _ = Control { mode: Mode::Off }.save();
            control_mtime = mtime(&paths::control());
        }
        if last_status != Some(d.status) {
            log(d.status.describe());
            last_status = Some(d.status);
        }
        if d.nudge {
            match &n.last_error {
                None => {
                    runtime.nudges += 1;
                    runtime.last_nudge = Some(Local::now().naive_local());
                    log(&format!("nudged (idle {})", duration::format(n.idle_time())));
                }
                Some(e) if last_error.as_ref() != Some(e) => log(&format!("nudge failed: {e}")),
                Some(_) => {}
            }
            last_error = n.last_error.clone();
        }

        let next = Runtime { status: d.status, mode: n.engine.mode, last_error: n.last_error.clone(), ..runtime.clone() };
        if next != runtime || d.nudge {
            runtime = next;
            let _ = runtime.save();
        }
        // Sleep until the next tick, waking early when the CLI changes something.
        for _ in 0..TICK.as_secs() {
            std::thread::sleep(Duration::from_secs(1));
            if mtime(&paths::control()) != control_mtime || mtime(&paths::config()) != config_mtime {
                break;
            }
        }
    }
}
