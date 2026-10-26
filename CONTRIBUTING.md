# Contributing to nudge

Thanks for helping! Nudge is intentionally small. The goal is a tool anyone can read and
trust in an afternoon, so please keep changes focused.

## Most wanted: platform testing

Nudge has only been tested on macOS. If you use **Windows** or **Linux (X11)**, a short
report is the most useful thing you can contribute:

```bash
nudge doctor
nudge selftest      # don't touch the keyboard/mouse for 4 seconds
nudge start         # then step away for 6+ minutes: does Teams stay Available?
nudge log
```

Paste the output into a
[platform report](https://github.com/hubdestro/nudge/issues/new?template=platform-report.md).

## Development

Requires Rust 1.82+.

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings

# Check the other platforms compile, from any machine:
rustup target add x86_64-pc-windows-msvc x86_64-unknown-linux-gnu
cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings
cargo clippy --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
```

Use `NUDGE_HOME` to keep test runs away from your real settings:

```bash
NUDGE_HOME=/tmp/nudge-dev cargo run -- start
NUDGE_HOME=/tmp/nudge-dev cargo run -- status
NUDGE_HOME=/tmp/nudge-dev cargo run -- stop
```

## Code map

| File | What it does |
|---|---|
| `src/engine.rs` | Pure decision logic: when to nudge. No OS calls, heavily tested. |
| `src/daemon.rs` | The background process and start/stop/pause. |
| `src/main.rs` | CLI commands and output. |
| `src/config.rs` | Settings file and `config set/get`. |
| `src/schedule.rs` | Work-hours window. |
| `src/state.rs` | Files shared between the CLI and the background process. |
| `src/platform/` | One file per OS: idle time, nudge, keep-awake, lock detection. |

## Ground rules

- **Never act while the user is active.** Any change to `engine.rs` must keep the
  `never_nudges_while_user_is_working` tests passing.
- **Never move the cursor.** Nudges must be zero-distance.
- **No network access, no telemetry.**
- Keep dependencies minimal. Each new crate needs a good reason.
- Run `cargo test` and `cargo clippy` for all three targets before opening a PR.

## Releasing (maintainers)

1. Update `version` in `Cargo.toml` and add an entry to `CHANGELOG.md`.
2. Commit, then tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
3. The release workflow builds binaries for every platform and publishes the release.
