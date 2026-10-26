//! Windows: SetThreadExecutionState + GetLastInputInfo + SendInput.

use crate::config::NudgeMethod;
use std::time::Duration;
use windows_sys::Win32::System::Power::{
    SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};
use windows_sys::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, SwitchDesktop, DESKTOP_SWITCHDESKTOP,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, SendInput, INPUT, INPUT_0, INPUT_MOUSE, LASTINPUTINFO, MOUSEEVENTF_MOVE,
    MOUSEINPUT,
};

pub struct Platform {
    awake: bool,
}

impl Platform {
    pub fn new() -> Self {
        Platform { awake: false }
    }

    /// Time since the last input (includes injected input, so our own nudges reset it;
    /// the engine accounts for that).
    pub fn idle_time(&self) -> Duration {
        let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return Duration::ZERO;
        }
        // Both are 32-bit millisecond tick counts; wrapping_sub handles the 49.7-day rollover.
        let ms = unsafe { GetTickCount() }.wrapping_sub(info.dwTime);
        Duration::from_millis(ms as u64)
    }

    pub fn idle_report(&self) -> Vec<(&'static str, Duration, bool)> {
        vec![("GetLastInputInfo (apps read)", self.idle_time(), true)]
    }

    /// The input desktop can't be opened or switched to while the lock screen is up.
    pub fn screen_locked(&self) -> bool {
        unsafe {
            let desk = OpenInputDesktop(0, 0, DESKTOP_SWITCHDESKTOP);
            if desk.is_null() {
                return true;
            }
            let ok = SwitchDesktop(desk);
            CloseDesktop(desk);
            ok == 0
        }
    }

    pub fn nudge(&mut self, _method: NudgeMethod) -> Result<&'static str, String> {
        // Windows has no "declare activity" API that resets the idle timer apps read,
        // so every method injects a relative mouse move of (0, 0): the cursor stays put.
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: 0, dwFlags: MOUSEEVENTF_MOVE, time: 0, dwExtraInfo: 0 },
            },
        };
        let sent = unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
        if sent == 1 {
            Ok("sent zero-distance mouse move")
        } else {
            Err(format!("SendInput failed: {}", std::io::Error::last_os_error()))
        }
    }

    pub fn hold_awake(&mut self, display: bool) {
        let mut flags = ES_CONTINUOUS | ES_SYSTEM_REQUIRED;
        if display {
            flags |= ES_DISPLAY_REQUIRED;
        }
        // Must be called from the long-lived main thread; it stays in effect until reset.
        unsafe { SetThreadExecutionState(flags) };
        self.awake = true;
    }

    pub fn release_awake(&mut self) {
        if self.awake {
            unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
            self.awake = false;
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
