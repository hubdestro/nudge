//! Launch at login: a LaunchAgent on macOS, an XDG autostart entry on Linux,
//! the `Run` registry key on Windows.

#[cfg(target_os = "macos")]
mod imp {
    use std::path::PathBuf;

    const LABEL: &str = "io.github.nudge";

    pub fn location() -> Option<PathBuf> {
        Some(dirs::home_dir()?.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    pub fn is_enabled() -> bool {
        location().is_some_and(|p| p.exists())
    }

    pub fn set(enabled: bool) -> Result<(), String> {
        let path = location().ok_or("no home directory")?;
        if !enabled {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let log = crate::paths::log();
        // launchd runs `nudge run` in the foreground and captures its output into the log.
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>run</string>
        <string>--background</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
            exe = xml_escape(&exe.to_string_lossy()),
            log = xml_escape(&log.to_string_lossy()),
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, plist).map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::path::PathBuf;

    pub fn location() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("autostart").join("nudge.desktop"))
    }

    pub fn is_enabled() -> bool {
        location().is_some_and(|p| p.exists())
    }

    pub fn set(enabled: bool) -> Result<(), String> {
        let path = location().ok_or("no config directory")?;
        if !enabled {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName=Nudge\nComment=Keep your chat status green\nExec=\"{}\" start\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, entry).map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::path::PathBuf;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "Nudge";

    pub fn location() -> Option<PathBuf> {
        Some(PathBuf::from(format!(r"HKCU\{RUN_KEY}\{VALUE}")))
    }

    pub fn is_enabled() -> bool {
        let (key, value) = (wide(RUN_KEY), wide(VALUE));
        let mut size = 0u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        r == ERROR_SUCCESS
    }

    pub fn set(enabled: bool) -> Result<(), String> {
        let (key, value) = (wide(RUN_KEY), wide(VALUE));
        let r = if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let data = wide(&format!("\"{}\" start", exe.display()));
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    value.as_ptr(),
                    REG_SZ,
                    data.as_ptr().cast(),
                    (data.len() * 2) as u32,
                )
            }
        } else {
            match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) } {
                ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
                r => r,
            }
        };
        if r == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(r as i32).to_string())
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod imp {
    use std::path::PathBuf;

    pub fn location() -> Option<PathBuf> {
        None
    }

    pub fn is_enabled() -> bool {
        false
    }

    pub fn set(_enabled: bool) -> Result<(), String> {
        Err("launch at login is not supported on this platform".into())
    }
}

pub use imp::{is_enabled, location, set};
