# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-10-26

First public release. Tested on macOS only.

### Added

- Background process with `start`, `stop`, `restart`, `status`, `pause`, `resume`,
  `for <duration>` and `log [-f]`.
- Zero-distance nudges that never move the cursor and never fire while you're active.
- Configurable idle threshold and interval, work-hours schedule (including overnight
  windows), keep-display-awake, and pause-when-locked, all via `nudge config set`.
  Changes apply without a restart.
- Launch at login (`nudge autostart on`) on macOS, Linux and Windows.
- `nudge doctor`, `nudge selftest` and `nudge uninstall`.
- One-line installers for macOS/Linux (`install.sh`) and Windows (`install.ps1`).
- Prebuilt binaries for macOS (arm64, x64), Linux (x64, arm64; static musl) and Windows
  (x64, arm64).

[0.1.0]: https://github.com/hubdestro/nudge/releases/tag/v0.1.0
