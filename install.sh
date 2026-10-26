#!/bin/sh
# Nudge installer for macOS and Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/hubdestro/nudge/main/install.sh | sh
#
# Environment overrides:
#   NUDGE_REPO          GitHub repo to download from   (default: hubdestro/nudge)
#   NUDGE_VERSION       release tag, e.g. v0.1.0       (default: latest)
#   NUDGE_INSTALL_DIR   where to put the binary        (default: ~/.local/bin)
#   NUDGE_DOWNLOAD_URL  full archive URL (skips the GitHub lookup; file:// works)
set -eu

REPO="${NUDGE_REPO:-hubdestro/nudge}"
VERSION="${NUDGE_VERSION:-latest}"
INSTALL_DIR="${NUDGE_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-musl ;;
    *) die "unsupported OS $(uname -s). On Windows, use install.ps1." ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) die "unsupported CPU $(uname -m)" ;;
esac
target="$arch-$os"

if [ -n "${NUDGE_DOWNLOAD_URL:-}" ]; then
    url="$NUDGE_DOWNLOAD_URL"
elif [ "$VERSION" = latest ]; then
    url="https://github.com/$REPO/releases/latest/download/nudge-$target.tar.gz"
else
    url="https://github.com/$REPO/releases/download/$VERSION/nudge-$target.tar.gz"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "Downloading nudge for $target…"
if curl -fsSL "$url" -o "$tmp/nudge.tar.gz" 2>/dev/null; then
    tar -xzf "$tmp/nudge.tar.gz" -C "$tmp"
    [ -f "$tmp/nudge" ] || die "archive did not contain a nudge binary"
    mkdir -p "$INSTALL_DIR"
    # Stop a running copy before replacing it.
    if [ -x "$INSTALL_DIR/nudge" ]; then "$INSTALL_DIR/nudge" stop >/dev/null 2>&1 || true; fi
    install -m 755 "$tmp/nudge" "$INSTALL_DIR/nudge"
    bin="$INSTALL_DIR/nudge"
elif command -v cargo >/dev/null 2>&1; then
    say "No prebuilt binary found; building from source with cargo (about a minute)…"
    cargo install --quiet --locked --git "https://github.com/$REPO" nudge
    bin="$(command -v nudge || echo "$HOME/.cargo/bin/nudge")"
else
    die "could not download $url, and cargo is not installed to build from source (https://rustup.rs)"
fi

say "Installed $("$bin" --version) to $bin"

case ":$PATH:" in
    *":$(dirname "$bin"):"*) ;;
    *)
        shell_rc="$HOME/.profile"
        case "${SHELL:-}" in
            */zsh) shell_rc="$HOME/.zshrc" ;;
            */bash) shell_rc="$HOME/.bashrc" ;;
        esac
        say ""
        say "$(dirname "$bin") is not on your PATH. Add it with:"
        say "  echo 'export PATH=\"$(dirname "$bin"):\$PATH\"' >> $shell_rc && . $shell_rc"
        ;;
esac

say ""
say "Get started:"
say "  nudge start      # keep your status green"
say "  nudge status"
say "  nudge doctor     # check permissions"
if [ "$os" = apple-darwin ]; then
    say ""
    say "macOS: give your terminal app Accessibility access so nudges reach Teams"
    say "(System Settings → Privacy & Security → Accessibility). \`nudge doctor\` opens the prompt."
fi
