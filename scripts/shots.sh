#!/bin/sh
# Regenerate docs/screenshots/ from tapes/*.tape.
# Needs vhs (plus its runtime deps ttyd and ffmpeg):
#   https://github.com/charmbracelet/vhs
set -eu
cd "$(dirname "$0")/.."

if ! command -v vhs >/dev/null 2>&1; then
    echo "vhs not found — install it with one of:" >&2
    if command -v brew >/dev/null 2>&1; then
        echo "  brew install vhs                                  # pulls in ttyd + ffmpeg" >&2
    fi
    if command -v go >/dev/null 2>&1; then
        echo "  go install github.com/charmbracelet/vhs@latest    # also needs ttyd + ffmpeg on PATH" >&2
    fi
    if command -v pacman >/dev/null 2>&1; then
        echo "  pacman -S vhs" >&2
    fi
    if command -v nix-env >/dev/null 2>&1; then
        echo "  nix-env -iA nixpkgs.vhs" >&2
    fi
    echo "  or grab a release: https://github.com/charmbracelet/vhs/releases" >&2
    exit 1
fi

# vhs shells out to these at record time; missing ones fail halfway through
# a render, so check up front.
for dep in ttyd ffmpeg; do
    command -v "$dep" >/dev/null 2>&1 || {
        echo "vhs needs $dep on PATH — install it (brew install $dep) and retry" >&2
        exit 1
    }
done

cargo build --release --locked
mkdir -p docs/screenshots
for tape in tapes/*.tape; do
    echo "» $tape"
    vhs "$tape"
done

echo "done — docs/screenshots/:"
ls -l docs/screenshots/
