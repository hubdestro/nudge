mod app;
mod autostart;
mod config;
mod daemon;
mod duration;
mod engine;
mod paths;
mod platform;
mod process;
mod schedule;
mod state;

use chrono::Local;
use config::{Config, NudgeMethod, KEYS};
use daemon::Started;
use engine::Mode;
use state::{Control, Runtime};
use std::io::IsTerminal;
use std::time::Duration;

const HELP: &str = "\
nudge — keep your chat status green without ever moving your mouse

USAGE
  nudge start [--for <duration>]   Start in the background (or switch it back on)
  nudge stop                       Stop the background process
  nudge restart                    Stop, then start
  nudge status                     What it's doing right now
  nudge pause                      Stay running but stop covering for you
  nudge resume [--for <duration>]  Start covering again
  nudge for <duration>             Keep green for a while, e.g. `nudge for 2h`
  nudge log [-f] [-n <lines>]      Show the activity log (-f to follow)

  nudge config                     Show all settings
  nudge config set <key> <value>   Change a setting (applies immediately)
  nudge config get <key>           Show one setting
  nudge config reset               Restore the defaults
  nudge config edit | path         Open / locate the settings file

  nudge autostart on|off           Start automatically when you log in
  nudge doctor                     Check permissions and platform support
  nudge selftest [input|activity]  Prove a nudge resets the idle clock apps read
  nudge run                        Run in the foreground (Ctrl+C to stop)
  nudge uninstall [--yes]          Stop, remove autostart, settings, logs and the binary
  nudge --version

Nudge never acts while you're using your computer: it only nudges after you've
been idle for a while, with a zero-distance mouse event. Your cursor never moves.

EXAMPLES
  nudge start
  nudge config set idle 2m
  nudge config set schedule 09:00-18:00
  nudge config set days mon-fri
  nudge for 90m";

fn main() {
    // Exit quietly when output is piped into something that closes early (`| head`).
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if let Err(e) = dispatch(&args) {
        eprintln!("{} {e}", paint("error:", RED));
        std::process::exit(1);
    }
}

fn dispatch(args: &[&str]) -> Result<(), String> {
    match args {
        [] | ["help" | "-h" | "--help"] => println!("{HELP}"),
        ["-V" | "--version" | "version"] => println!("nudge {}", env!("CARGO_PKG_VERSION")),
        ["start", rest @ ..] => start(timer_flag(rest)?)?,
        ["for", dur] => start(Some(duration::parse(dur)?))?,
        ["stop"] => stop()?,
        ["restart"] => {
            daemon::stop()?;
            start(None)?;
        }
        ["status"] => status(),
        ["pause"] => {
            if daemon::set_mode(Mode::Off)? {
                println!("Paused. Resume with `nudge resume`.");
            } else {
                println!("Not running.");
            }
        }
        ["resume", rest @ ..] => {
            let mode = mode_for(timer_flag(rest)?);
            if daemon::set_mode(mode)? {
                println!("{}", describe_on(mode));
            } else {
                println!("Not running. Start it with `nudge start`.");
            }
        }
        ["log", rest @ ..] => log(rest)?,
        ["config", rest @ ..] => config(rest)?,
        ["autostart", rest @ ..] => autostart(rest)?,
        ["doctor"] => doctor(),
        ["selftest", rest @ ..] => selftest(rest.first().copied())?,
        ["run", rest @ ..] => daemon::run(rest.contains(&"--background"))?,
        ["uninstall", rest @ ..] => uninstall(rest.contains(&"--yes") || rest.contains(&"-y"))?,
        [cmd, ..] => return Err(format!("unknown command {cmd:?}. See `nudge help`.")),
    }
    Ok(())
}

// ---------- output helpers ----------

const GREEN: &str = "32";
const YELLOW: &str = "33";
const RED: &str = "31";
const DIM: &str = "2";
const BOLD: &str = "1";

fn paint(s: &str, code: &str) -> String {
    if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn row(label: &str, value: impl std::fmt::Display) {
    println!("  {:<12}{value}", paint(label, DIM));
}

// ---------- start / stop ----------

fn timer_flag(rest: &[&str]) -> Result<Option<Duration>, String> {
    match rest {
        [] => Ok(None),
        ["--for", d] => Ok(Some(duration::parse(d)?)),
        _ => Err("expected `--for <duration>`, e.g. --for 2h".into()),
    }
}

fn mode_for(timer: Option<Duration>) -> Mode {
    match timer {
        None => Mode::On,
        Some(d) => Mode::Until(Local::now().naive_local() + chrono::Duration::seconds(d.as_secs() as i64)),
    }
}

fn describe_on(mode: Mode) -> String {
    match mode {
        Mode::Until(t) => format!("Keeping you green until {}.", t.format("%H:%M")),
        _ => "Keeping you green until you stop it.".into(),
    }
}

fn start(timer: Option<Duration>) -> Result<(), String> {
    let mode = mode_for(timer);
    match daemon::start(mode)? {
        Started::New(pid) => println!("{} Started in the background (pid {pid}). {}", paint("●", GREEN), describe_on(mode)),
        Started::AlreadyRunning(pid) => println!("{} Already running (pid {pid}). {}", paint("●", GREEN), describe_on(mode)),
    }
    let p = platform::Platform::new();
    if p.input_permission() == Some(false) {
        println!(
            "{} This terminal app needs Accessibility access for nudges to reach Teams. Run `nudge doctor`.",
            paint("!", YELLOW)
        );
    }
    Ok(())
}

fn stop() -> Result<(), String> {
    match daemon::stop()? {
        Some(pid) => println!("Stopped (pid {pid})."),
        None => println!("Not running."),
    }
    Ok(())
}

// ---------- status ----------

fn status() {
    let (config, warning) = Config::load();
    let p = platform::Platform::new();
    let now = Local::now().naive_local();
    match daemon::running_pid() {
        Some(pid) => {
            let rt = Runtime::load().filter(|r| r.pid == pid);
            let status = rt.as_ref().map_or("starting…", |r| r.status.describe());
            let dot = match rt.as_ref().map(|r| r.status) {
                Some(engine::Status::UserActive | engine::Status::Covering) => paint("●", GREEN),
                _ => paint("●", YELLOW),
            };
            println!("{dot} {} (pid {pid}) — {status}", paint("nudge is running", BOLD));
            let mode = Control::load().map_or(Mode::On, |c| c.mode);
            row(
                "mode",
                match mode {
                    Mode::On => "on".to_string(),
                    Mode::Off => "paused (`nudge resume` to continue)".to_string(),
                    Mode::Until(t) => {
                        let left = (t - now).num_seconds().max(0) as u64;
                        format!("on until {} ({} left)", t.format("%H:%M"), duration::format(Duration::from_secs(left)))
                    }
                },
            );
            if let Some(rt) = &rt {
                let up = (now - rt.started).num_seconds().max(0) as u64;
                row("uptime", duration::format(Duration::from_secs(up)));
                match rt.last_nudge {
                    Some(t) => row("last nudge", format!("{} ({} since start)", t.format("%H:%M:%S"), rt.nudges)),
                    None => row("last nudge", "none yet — you've been active"),
                }
                if let Some(e) = &rt.last_error {
                    row("problem", paint(e, RED));
                }
            }
        }
        None => println!("{} {} — start it with `nudge start`", paint("○", DIM), paint("nudge is not running", BOLD)),
    }
    row("idle now", duration::format(p.idle_time()));
    row(
        "settings",
        format!(
            "nudge after {} idle, every {}; display awake {}; pause when locked {}",
            duration::format(config.idle_threshold()),
            duration::format(config.nudge_interval()),
            if config.keep_display_awake { "on" } else { "off" },
            if config.pause_when_locked { "on" } else { "off" },
        ),
    );
    let s = &config.schedule;
    if s.enabled {
        let inside = if s.contains(now) { "inside now" } else { "outside now" };
        row("schedule", format!("{} ({inside})", s.summary()));
    } else {
        row("schedule", "off — active at any time");
    }
    row("autostart", if autostart::is_enabled() { "on" } else { "off" });
    if p.input_permission() == Some(false) {
        row("warning", paint("this terminal lacks Accessibility access — run `nudge doctor`", YELLOW));
    }
    if let Some(w) = warning {
        row("warning", paint(&w, YELLOW));
    }
}

// ---------- log ----------

fn log(rest: &[&str]) -> Result<(), String> {
    let mut follow = false;
    let mut lines = 20usize;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "-f" | "--follow" => follow = true,
            "-n" | "--lines" => {
                lines = it.next().and_then(|n| n.parse().ok()).ok_or("expected a number after -n")?;
            }
            other => return Err(format!("unknown option {other:?}")),
        }
    }
    let path = paths::log();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    if text.is_empty() && !follow {
        println!("No activity yet. Start with `nudge start`.");
        return Ok(());
    }
    let all: Vec<&str> = text.lines().collect();
    for line in &all[all.len().saturating_sub(lines)..] {
        println!("{line}");
    }
    if follow {
        use std::io::{Read, Seek, SeekFrom};
        let mut pos = text.len() as u64;
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let Ok(mut f) = std::fs::File::open(&path) else { continue };
            let len = f.metadata().map(|m| m.len()).unwrap_or(0);
            if len < pos {
                pos = 0; // rotated
            }
            if len > pos {
                let mut buf = String::new();
                f.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
                f.read_to_string(&mut buf).map_err(|e| e.to_string())?;
                print!("{buf}");
                pos = len;
            }
        }
    }
    Ok(())
}

// ---------- config ----------

fn config(rest: &[&str]) -> Result<(), String> {
    let (mut c, warning) = Config::load();
    if let Some(w) = warning {
        eprintln!("{} {w}", paint("warning:", YELLOW));
    }
    match rest {
        [] | ["list" | "show"] => {
            let values: Vec<String> = KEYS.iter().map(|(k, _, _)| c.get(k)).collect::<Result<_, _>>()?;
            let width = values.iter().map(|v| v.chars().count()).max().unwrap_or(0) + 3;
            for ((key, accepted, help), value) in KEYS.iter().zip(&values) {
                let pad = " ".repeat(width - value.chars().count());
                println!("  {}{}{value}{pad}{}", paint(key, BOLD), " ".repeat(12 - key.len()), paint(&format!("{help} [{accepted}]"), DIM));
            }
            println!("\nChange with `nudge config set <key> <value>`. File: {}", paths::config().display());
        }
        ["get", key] => println!("{}", c.get(key)?),
        ["set", key, value @ ..] if !value.is_empty() => {
            c.set(key, &value.join(" "))?;
            c.save().map_err(|e| format!("could not save settings: {e}"))?;
            let applied = if daemon::running_pid().is_some() { " (applied to the running process)" } else { "" };
            println!("{key} = {}{applied}", c.get(key)?);
        }
        ["reset"] => {
            Config::default().save().map_err(|e| e.to_string())?;
            println!("Settings restored to defaults.");
        }
        ["path"] => println!("{}", paths::config().display()),
        ["edit"] => {
            platform::edit_file(&paths::config()).map_err(|e| format!("could not open editor: {e}"))?;
            Config::load_from(&paths::config())?;
            println!("Saved.");
        }
        _ => return Err("usage: nudge config [get <key> | set <key> <value> | reset | edit | path]".into()),
    }
    Ok(())
}

// ---------- autostart ----------

fn autostart(rest: &[&str]) -> Result<(), String> {
    let where_ = autostart::location().map(|p| p.display().to_string()).unwrap_or_default();
    match rest {
        ["on"] => {
            autostart::set(true)?;
            println!("Nudge will start when you log in ({where_}).");
            if cfg!(target_os = "macos") {
                println!(
                    "{} At login Nudge runs on its own, so grant Accessibility to the binary itself:\n  {}",
                    paint("!", YELLOW),
                    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
                );
            }
        }
        ["off"] => {
            autostart::set(false)?;
            println!("Autostart removed.");
        }
        [] | ["status"] => println!("{}", if autostart::is_enabled() { format!("on ({where_})") } else { "off".into() }),
        _ => return Err("usage: nudge autostart on|off|status".into()),
    }
    Ok(())
}

// ---------- doctor / selftest ----------

fn doctor() {
    let p = platform::Platform::new();
    let ok = |s: &str| println!("{} {s}", paint("✓", GREEN));
    let warn = |s: &str| println!("{} {s}", paint("!", YELLOW));
    println!("nudge {} on {} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);
    match p.input_permission() {
        Some(true) => ok("Accessibility access granted: nudges reach the idle clock Teams reads"),
        Some(false) => {
            warn("Accessibility access missing: nudges won't reach Teams");
            println!(
                "    Open System Settings → Privacy & Security → Accessibility and enable the app you run\n    `nudge` from (Terminal, iTerm, VS Code…). A permission prompt has been opened for it."
            );
            p.request_input_permission();
        }
        None => {}
    }
    if cfg!(target_os = "linux") {
        if std::env::var_os("DISPLAY").is_some() && p.idle_report().first().is_some_and(|(n, _, _)| n.starts_with("X11 X")) {
            ok("X11 session: idle detection and nudging supported");
        } else {
            warn("No X11 display: on Wayland, Nudge can keep the system awake but can't nudge");
        }
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            warn("Wayland session detected: X11 idle time only covers XWayland apps");
        }
        let inhibit = std::process::Command::new("systemd-inhibit").arg("--version").output().is_ok();
        if inhibit {
            ok("systemd-inhibit available: can prevent sleep");
        } else {
            warn("systemd-inhibit not found: can't prevent sleep");
        }
    }
    for (name, idle, _) in p.idle_report() {
        ok(&format!("idle clock {name}: {}", duration::format(idle)));
    }
    match daemon::running_pid() {
        Some(pid) => ok(&format!("running (pid {pid})")),
        None => warn("not running — `nudge start`"),
    }
    println!("  files: {}", paths::home().display());
}

fn selftest(method: Option<&str>) -> Result<(), String> {
    let method = match method {
        None => Config::load().0.nudge_method,
        Some(m) => NudgeMethod::parse(m)?,
    };
    let mut p = platform::Platform::new();
    const WAIT: u64 = 4;
    println!("Selftest (method {}). Don't touch the keyboard or mouse for {WAIT} seconds…", method.name());
    std::thread::sleep(Duration::from_secs(WAIT));
    let before = p.idle_report();
    let result = p.nudge(method);
    std::thread::sleep(Duration::from_millis(300));
    let after = p.idle_report();

    match &result {
        Ok(what) => println!("nudge: {what}"),
        Err(e) => println!("nudge: FAILED — {e}"),
    }
    let mut app_clock_reset = false;
    let mut any_reset = false;
    for ((name, b, apps_read), (_, a, _)) in before.iter().zip(&after) {
        let reset = a < b && *b >= Duration::from_secs(2);
        any_reset |= reset;
        app_clock_reset |= reset && *apps_read;
        println!(
            "  {name:<34} {:>7} → {:>7}  {}",
            format!("{:.1}s", b.as_secs_f64()),
            format!("{:.1}s", a.as_secs_f64()),
            if reset { paint("reset ✓", GREEN) } else { "unchanged".into() }
        );
    }
    if before.iter().all(|(_, b, _)| *b < Duration::from_secs(2)) {
        return Err("inconclusive: you were active during the test. Run it again without touching anything.".into());
    }
    if result.is_ok() && app_clock_reset {
        println!("{} the idle clock apps read was reset, and the cursor never moved.", paint("PASS:", GREEN));
        return Ok(());
    }
    if any_reset {
        println!("{} only a low-level idle clock was reset; chat apps will still see you as idle.", paint("PARTIAL:", YELLOW));
    } else {
        println!("{} no idle clock was reset.", paint("FAIL:", RED));
    }
    if p.input_permission() == Some(false) {
        println!("Grant Accessibility access to this terminal app (run `nudge doctor`), then retry.");
    }
    std::process::exit(1);
}

// ---------- uninstall ----------

fn uninstall(yes: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let home = paths::home();
    if !yes {
        use std::io::Write;
        println!("This stops Nudge and removes:");
        println!("  {}  (settings and logs)", home.display());
        if autostart::is_enabled() {
            println!("  {}  (autostart)", autostart::location().map(|p| p.display().to_string()).unwrap_or_default());
        }
        println!("  {}", exe.display());
        print!("Continue? [y/N] ");
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer).map_err(|e| e.to_string())?;
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    daemon::stop()?;
    autostart::set(false)?;
    if home.exists() {
        std::fs::remove_dir_all(&home).map_err(|e| format!("could not remove {}: {e}", home.display()))?;
    }
    if cfg!(windows) {
        // A running .exe can't delete itself on Windows.
        println!("Removed settings and autostart. Delete the binary to finish:\n  del \"{}\"", exe.display());
    } else {
        std::fs::remove_file(&exe).map_err(|e| format!("could not remove {}: {e}", exe.display()))?;
        println!("Nudge has been uninstalled.");
    }
    if cfg!(target_os = "macos") {
        println!("You can also remove your terminal from System Settings → Privacy & Security → Accessibility.");
    }
    Ok(())
}
