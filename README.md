<h1 align="center">nudge</h1>

<p align="center">
  <strong>Keep your Microsoft Teams status green, without ever moving your mouse.</strong><br>
  A tiny, dependency-free command-line tool for macOS, Linux and Windows.
</p>

<p align="center">
  <a href="https://github.com/hubdestro/nudge/actions/workflows/ci.yml"><img src="https://github.com/hubdestro/nudge/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/hubdestro/nudge/releases/latest"><img src="https://img.shields.io/github/v/release/hubdestro/nudge?sort=semver" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg" alt="MIT License"></a>
  <img src="https://img.shields.io/badge/tested%20on-macOS-blue?logo=apple" alt="Tested on macOS">
  <img src="https://img.shields.io/badge/Windows%20%7C%20Linux-untested-orange" alt="Windows and Linux untested">
</p>

```bash
curl -fsSL https://raw.githubusercontent.com/hubdestro/nudge/main/install.sh | sh
nudge start
```

> [!IMPORTANT]
> **Nudge has only been tested on macOS** (macOS 26, Apple Silicon). The Windows and
> Linux versions are fully implemented and compile cleanly in CI, but no one has verified
> them on a real machine yet. If you try them, please
> [report what happens](https://github.com/hubdestro/nudge/issues/new?template=platform-report.md),
> whether it worked or not.

---

## The problem

Microsoft Teams marks you **Away after about 5 minutes** without keyboard or mouse input.
That happens when you're reading a long document, thinking through a design, sitting in a
meeting room, or grabbing a coffee. There's no setting to change it.

The usual fix is a "mouse jiggler". Jigglers move your cursor every few seconds, **even
while you're working**. Your pointer drifts mid-click, text selections break, and some
press keys you didn't press.

## The nudge approach

Nudge solves the same problem without the side effects.

| | Typical mouse jiggler | **nudge** |
|---|---|---|
| Moves your cursor | Yes, every few seconds | **Never.** It sends a *zero-distance* mouse event |
| Acts while you're working | Yes, it fights your hand | **Never.** It waits until you've been idle |
| Fake keypresses | Often (Shift, F15…) | **None** |
| Dependencies | Homebrew, `cliclick`, Python, Node… | **None.** One ~600 KB binary |
| Platforms | Usually one | macOS, Windows, Linux |
| Configurable | Rarely | Idle threshold, schedule, timers, autostart |
| Respects a locked screen | Rarely | Yes, it pauses when you lock |
| Keeps the machine awake | Sometimes | Yes, no sleep and no screensaver while active |

### Benefits

- **Zero interference.** While you type or move the mouse, Nudge does nothing. It acts
  only after you've been idle for a set time (default 60 seconds).
- **Your cursor stays put.** The OS sees "user activity", but your pointer doesn't move
  a single pixel. Nothing can change under your hand.
- **Lightweight.** It's a single native binary with no runtime. It checks idle time every
  5 seconds and sleeps the rest of the time, with no noticeable CPU or battery cost.
- **Developer-friendly.** It's a plain CLI: `start`, `stop`, `status`, `log -f`, and
  `config set`. It works from scripts, dotfiles and SSH sessions.
- **Respects your boundaries.** A work-hours schedule ("only 09:00–18:00, Mon–Fri"),
  timers ("keep me green for 2 hours"), and automatic pause when you lock the screen.
- **Transparent.** MIT-licensed, small codebase, no network access, no telemetry. Read
  every line in an afternoon.

## Quick start

```bash
# 1. Install (macOS / Linux)
curl -fsSL https://raw.githubusercontent.com/hubdestro/nudge/main/install.sh | sh

# 2. Check permissions (macOS: opens the Accessibility prompt for your terminal)
nudge doctor

# 3. Start
nudge start
```

That's it. Nudge runs in the background until you `nudge stop`.

## Installation

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/hubdestro/nudge/main/install.sh | sh
```

This downloads the right prebuilt binary for your OS and CPU into `~/.local/bin`. If
there's no binary for your platform, it builds from source with `cargo`.

### Windows (PowerShell) — untested

```powershell
irm https://raw.githubusercontent.com/hubdestro/nudge/main/install.ps1 | iex
```

This installs `nudge.exe` into `%LOCALAPPDATA%\Programs\nudge` and adds it to your PATH.

### From source

```bash
cargo install --git https://github.com/hubdestro/nudge
```

Requires Rust 1.82 or newer. You can also download a binary directly from the
[releases page](https://github.com/hubdestro/nudge/releases).

### macOS: grant Accessibility access (one time)

macOS only delivers synthetic input to other apps when the sending app has
**Accessibility** access. Grant it to the terminal you run `nudge` from (Terminal,
iTerm2, Warp, VS Code…):

**System Settings → Privacy & Security → Accessibility → enable your terminal**

`nudge doctor` checks this and opens the system prompt for you. Without this permission,
Nudge still keeps your Mac awake, but Teams will see you as idle.

## Usage

```bash
nudge start               # start in the background
nudge for 2h              # keep green for 2 hours, then pause automatically
nudge status              # what it's doing right now
nudge pause               # stay running, but stop covering for you
nudge resume              # cover for you again (also: nudge resume --for 30m)
nudge stop                # stop the background process
nudge log -f              # follow the activity log
```

```
$ nudge status
● nudge is running (pid 35215) — active — covering for you
  mode        on until 13:36 (1h 59m left)
  uptime      12m 5s
  last nudge  11:48:02 (7 since start)
  idle now    4m 12s
  settings    nudge after 1m idle, every 1m; display awake on; pause when locked on
  schedule    09:00–18:00, Mon–Fri (inside now)
  autostart   on
```

### Configuration

Every setting is changed from the CLI and applies **immediately** to the running process,
with no restart needed.

```bash
nudge config                          # show all settings
nudge config set idle 2m              # start covering after 2 minutes of idle time
nudge config set interval 45s         # nudge every 45s while you're away
nudge config set schedule 09:00-18:00 # only during work hours (overnight works: 22:00-06:00)
nudge config set days mon-fri         # or: every-day, weekends, mon,wed,fri
nudge config set schedule off         # active at any time again
nudge config set display off          # let the display sleep
nudge config set lock-pause off       # keep covering even when the screen is locked
nudge config reset                    # back to defaults
nudge config edit                     # open the JSON file in $EDITOR
```

| Key | Values | Default | Description |
|---|---|---|---|
| `idle` | duration, 10s–1h | `1m` | Start nudging after this much idle time |
| `interval` | duration, 10s–1h | `1m` | How often to nudge while you're away |
| `schedule` | `on` · `off` · `HH:MM-HH:MM` | `off` | Only keep you green during work hours |
| `days` | `mon-fri` · `every-day` · `sat,sun` … | `mon-fri` | Days the schedule applies to |
| `display` | `on` · `off` | `on` | Keep the display awake while active |
| `lock-pause` | `on` · `off` | `on` | Pause while the screen is locked |
| `method` | `auto` · `input` · `activity` | `auto` | How to nudge (leave on `auto`) |

Durations accept `90`, `90s`, `2m`, `1h30m` and `2 hours`.

### Start at login

```bash
nudge autostart on    # macOS LaunchAgent · Linux XDG autostart · Windows Run key
nudge autostart off
```

On macOS, a login-started Nudge runs on its own (not inside a terminal), so grant
Accessibility to the `nudge` binary itself. `nudge autostart on` prints its path.

### All commands

| Command | Description |
|---|---|
| `nudge start [--for <dur>]` | Start in the background, or switch it back on |
| `nudge stop` · `restart` | Stop / restart the background process |
| `nudge status` | Current state, idle time, settings |
| `nudge pause` · `resume [--for <dur>]` | Temporarily stop / restart covering for you |
| `nudge for <dur>` | Keep green for a while, then pause |
| `nudge log [-f] [-n N]` | Show or follow the activity log |
| `nudge config …` | Show / change settings (see above) |
| `nudge autostart on\|off` | Start at login |
| `nudge doctor` | Check permissions and platform support |
| `nudge selftest` | Show that a nudge resets the idle clock apps read |
| `nudge run` | Run in the foreground (Ctrl+C to stop) |
| `nudge uninstall` | Stop and remove everything, including the binary |

## How it works

```
every 5 seconds:
  paused, outside work hours, or screen locked?  ── yes ─▶ release everything, do nothing
  keyboard/mouse used within the idle threshold?  ── yes ─▶ keep the machine awake, do nothing else
  otherwise, at most once per interval            ──────▶ send a zero-distance mouse event
```

Apps such as Teams decide you're Away by asking the OS how long it's been since the last
input event. A zero-distance mouse event counts as input, so the idle clock resets, but
the pointer doesn't move. Because Nudge only acts *after* you've gone idle, it can never
collide with your own input.

| | macOS | Windows | Linux (X11) |
|---|---|---|---|
| Idle time | `CGEventSourceSecondsSinceLastEventType` | `GetLastInputInfo` | XScreenSaver extension |
| Nudge | zero-distance `CGEventPost` | zero-distance `SendInput` | zero-distance XTEST motion |
| Keep awake | IOKit power assertion | `SetThreadExecutionState` | `systemd-inhibit` |
| Screen lock | `CGSessionCopyCurrentDictionary` | `OpenInputDesktop` | `loginctl` `LockedHint` |
| Autostart | `~/Library/LaunchAgents` | `HKCU\…\Run` | `~/.config/autostart` |

**What we measured on macOS.** The power-management call that `caffeinate -u` uses
(`IOPMAssertionDeclareUserActivity`) keeps the display on, but resets *none* of the idle
clocks apps read. A posted zero-distance mouse event resets all of them, provided the
sender has Accessibility access. Run `nudge selftest` to see this on your own machine.

## Platform support

| Platform | Status | Notes |
|---|---|---|
| **macOS** 11+ (Apple Silicon & Intel) | ✅ **Tested** on macOS 26, Apple Silicon | Needs Accessibility access for your terminal |
| **Windows** 10 / 11 | ⚠️ Implemented, **untested** | Builds in CI; [help us test](#contributing) |
| **Linux**, X11 session | ⚠️ Implemented, **untested** | Builds in CI; [help us test](#contributing) |
| **Linux**, Wayland session | ❌ Limited | Keeps the system awake, but can't nudge (Wayland forbids reading global idle time and injecting input) |

## FAQ

<details>
<summary><strong>Will it interfere with my typing or clicking?</strong></summary>

No. Nudge does nothing while you're active. It acts only after you've been idle for the
configured threshold (default 60s), and even then it sends a mouse event that moves the
pointer zero pixels.
</details>

<details>
<summary><strong>Does it work with Slack, Zoom, Google Chat…?</strong></summary>

Most likely. Nudge doesn't talk to Teams directly. It resets the operating system's idle
clock, which is what most presence-aware apps read. Teams is the only app it has been
tested with.
</details>

<details>
<summary><strong>Does it use the network or collect data?</strong></summary>

No. Nudge makes no network connections and has no telemetry. Its only files are its
settings and a local log in one folder (`nudge config path` shows where).
</details>

<details>
<summary><strong>What about battery life?</strong></summary>

Nudge wakes every 5 seconds to read one number from the OS. Its own cost is negligible.
While active it does keep the machine (and, by default, the display) awake. Use
`nudge config set display off` or a work-hours schedule to save power.
</details>

<details>
<summary><strong>It says "Accessibility access missing" on macOS</strong></summary>

Grant Accessibility to the app you run `nudge` from (System Settings → Privacy & Security
→ Accessibility), then quit and reopen that app. Some apps (for example terminals embedded
in other apps) run under a helper process, so the permission must go to the real terminal.
Using Terminal.app or iTerm2 is the simplest option.
</details>

<details>
<summary><strong>How do I uninstall it?</strong></summary>

`nudge uninstall`. This stops the background process, removes autostart, deletes the
settings and log, and removes the binary.
</details>

## Contributing

Contributions are welcome. The most valuable contribution right now is **testing on
Windows and Linux**:

1. Install nudge and run `nudge doctor` and `nudge selftest`.
2. Run `nudge start`, step away for 6+ minutes, and check whether Teams stays Available.
3. [Open a platform report](https://github.com/hubdestro/nudge/issues/new?template=platform-report.md)
   with the output, whether it worked or not.

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup.

## Responsible use

Nudge is a convenience tool: it keeps your machine awake and your status accurate while
you're reading, thinking, or briefly away from your desk. It is not meant to misrepresent
your availability. Check your employer's policies on presence and activity tools. You are
responsible for how you use it.

## Acknowledgements

Inspired by [stay-active](https://github.com/sleekhost/stay-active), which showed how
useful a simple command for this can be.

## License

[MIT](LICENSE) © Hubdestro
