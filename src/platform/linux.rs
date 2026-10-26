//! Linux (X11): XScreenSaver idle time, XTEST zero-distance motion, systemd-inhibit.
//!
//! Wayland compositors don't let applications read global idle time or inject input,
//! so there Nudge can only keep the system awake. `nudge doctor` explains this.

use crate::config::NudgeMethod;
use std::cell::RefCell;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt as _;
use x11rb::protocol::xproto::{ConnectionExt as _, Window, MOTION_NOTIFY_EVENT};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

pub struct Platform {
    /// Lazily opened, dropped on error and reopened on the next call.
    x: RefCell<Option<(RustConnection, Window)>>,
    /// `systemd-inhibit … sleep infinity`, alive while we hold the inhibitor.
    inhibitor: Option<Child>,
}

impl Platform {
    pub fn new() -> Self {
        Platform { x: RefCell::new(None), inhibitor: None }
    }

    /// Runs `f` with the X11 connection, reconnecting if needed; resets it on failure.
    fn with_x11<T>(&self, f: impl FnOnce(&RustConnection, Window) -> Option<T>) -> Option<T> {
        let mut x = self.x.borrow_mut();
        if x.is_none() {
            let (conn, screen) = x11rb::connect(None).ok()?;
            let root = conn.setup().roots.get(screen)?.root;
            *x = Some((conn, root));
        }
        let (conn, root) = x.as_ref()?;
        let out = f(conn, *root);
        if out.is_none() {
            *x = None;
        }
        out
    }

    fn x11_idle(&self) -> Option<Duration> {
        self.with_x11(|conn, root| {
            let info = conn.screensaver_query_info(root).ok()?.reply().ok()?;
            Some(Duration::from_millis(info.ms_since_user_input as u64))
        })
    }

    /// Time since the last input. Without X11 we can't tell and report zero, which
    /// makes the engine treat you as present and never nudge.
    pub fn idle_time(&self) -> Duration {
        self.x11_idle().unwrap_or(Duration::ZERO)
    }

    pub fn idle_report(&self) -> Vec<(&'static str, Duration, bool)> {
        match self.x11_idle() {
            Some(d) => vec![("X11 XScreenSaver (apps read)", d, true)],
            None => vec![("X11 unavailable (Wayland or no display)", Duration::ZERO, true)],
        }
    }

    pub fn screen_locked(&self) -> bool {
        let session = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| "auto".into());
        Command::new("loginctl")
            .args(["show-session", &session, "-p", "LockedHint", "--value"])
            .stderr(Stdio::null())
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "yes")
    }

    pub fn nudge(&mut self, _method: NudgeMethod) -> Result<&'static str, String> {
        let sent = self.with_x11(|conn, root| {
            let p = conn.query_pointer(root).ok()?.reply().ok()?;
            // Absolute motion to where the pointer already is: zero distance.
            conn.xtest_fake_input(MOTION_NOTIFY_EVENT, 0, x11rb::CURRENT_TIME, root, p.root_x, p.root_y, 0)
                .ok()?;
            conn.flush().ok()
        });
        match sent {
            Some(()) => Ok("sent zero-distance pointer motion (XTEST)"),
            None if std::env::var_os("DISPLAY").is_none() => {
                Err("no X11 display: input can't be simulated on Wayland".into())
            }
            None => Err("XTEST pointer motion failed".into()),
        }
    }

    pub fn hold_awake(&mut self, display: bool) {
        self.release_awake();
        let what = if display { "idle:sleep" } else { "sleep" };
        match Command::new("systemd-inhibit")
            .args([
                &format!("--what={what}"),
                "--who=nudge",
                "--why=Nudge keeps you available",
                "--mode=block",
                "sleep",
                "infinity",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => self.inhibitor = Some(child),
            Err(e) => eprintln!("nudge: systemd-inhibit unavailable ({e}); can't prevent sleep"),
        }
    }

    pub fn release_awake(&mut self) {
        if let Some(mut child) = self.inhibitor.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn input_permission(&self) -> Option<bool> {
        None
    }

    pub fn request_input_permission(&self) {}
}

impl Drop for Platform {
    fn drop(&mut self) {
        self.release_awake();
    }
}
