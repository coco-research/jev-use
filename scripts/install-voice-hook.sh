#!/bin/sh
# install-voice-hook - build the hook and put it where the wrapper expects it.
#
#   scripts/install-voice-hook.sh              install to $HOME/.local/bin
#   PREFIX=/usr/local/bin scripts/install-voice-hook.sh
#
# Nothing here is machine-specific: it builds from source, installs into your own bin
# directory, and then prints what to put in CoCo Voice's settings. It does NOT edit that
# app's settings, because switching paste_method to external_script replaces typing - the
# kind of change you should make deliberately, with the rollback in front of you.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PREFIX=${PREFIX:-$HOME/.local/bin}
BIN=jev-voice-hook

say() { printf '%s\n' "$*"; }

command -v cargo >/dev/null 2>&1 || {
  say "cargo is not on PATH. Install Rust first: https://rustup.rs"
  exit 1
}

say "building $BIN (release)..."
cargo build --release --manifest-path "$ROOT/Cargo.toml" -p jev-voice

mkdir -p "$PREFIX"
install -m 0755 "$ROOT/target/release/$BIN" "$PREFIX/$BIN"
say "installed  $PREFIX/$BIN"

# Say it by running it, not by claiming it.
if "$PREFIX/$BIN" --dry-run "Emma, open a new tab" >/dev/null 2>&1; then
  say "smoke test  ok (decides without touching anything)"
else
  say "smoke test  FAILED - run $PREFIX/$BIN --dry-run 'Emma, open a new tab'"
  exit 1
fi

cat <<EOF

Next, in CoCo Voice (settings_store.json, or the app's settings UI):

  "paste_method": "external_script",
  "external_script_path": "$ROOT/scripts/voice-hook"

Read the warning at the top of scripts/voice-hook before switching it on: that setting
Optional, if your driver is not jev-use on PATH:

  "paste_method": "ctrl_v",
  "external_script_path": null

Optional, if your driver is not `jev-use` on PATH:

  JEV_USE_BIN=/path/to/your/driver     # what a command is handed to
  JEV_VOICE_MODE=classify              # skip the wake word
  JEV_VOICE_TRIGGER=computer           # change the wake word
EOF
