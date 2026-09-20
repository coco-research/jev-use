#!/bin/sh
# install-voice-hook - build the hook, install it, and set up the settings file.
#
#   scripts/install-voice-hook.sh                     installs to $HOME/.local/bin
#   PREFIX=/usr/local/bin scripts/install-voice-hook.sh
#
# Nothing here is machine-specific: it builds from source, installs into your own bin
# directory, writes a settings file you own, and prints what to put in CoCo Voice. It does
# NOT edit that app's settings, because switching paste_method to external_script replaces
# typing - the kind of change you should make deliberately, with the rollback in front of
# you.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PREFIX=${PREFIX:-$HOME/.local/bin}

say() { printf '%s\n' "$*"; }

command -v cargo >/dev/null 2>&1 || {
  say "cargo is not on PATH. Install Rust first: https://rustup.rs"
  exit 1
}

say "building (release)..."
cargo build --release --manifest-path "$ROOT/Cargo.toml" -p jev-voice -p jev-config

mkdir -p "$PREFIX"
install -m 0755 "$ROOT/target/release/jev-voice-hook" "$PREFIX/jev-voice-hook"
install -m 0755 "$ROOT/target/release/jev-config" "$PREFIX/jev-config"
say "installed  $PREFIX/jev-voice-hook"
say "installed  $PREFIX/jev-config"

# Settings: write the file if it is missing, then hand over to the real tool. This script
# deliberately does not know the schema - jev-config owns it, so there is one place where
# a new setting has to be added.
if [ -x "$PREFIX/jev-config" ]; then
  "$PREFIX/jev-config" init || true
fi

# Say it by running it, not by claiming it.
if "$PREFIX/jev-voice-hook" --dry-run "Emma, open a new tab" >/dev/null 2>&1; then
  say "smoke test  ok (decides without touching anything)"
else
  say "smoke test  FAILED - run $PREFIX/jev-voice-hook --dry-run 'Emma, open a new tab'"
  exit 1
fi

cat <<EOF

Next, in CoCo Voice (settings_store.json, or the app's settings UI):

  "paste_method": "external_script",
  "external_script_path": "$ROOT/scripts/voice-hook"

Read the warning at the top of scripts/voice-hook before switching it on: that setting
replaces typing, so dictation goes through the hook from then on. To go back:

  "paste_method": "ctrl_v",
  "external_script_path": null

Then set up what is yours, in one place:

  $PREFIX/jev-config              the menu: mode, wake word, driver, log, wallets
  $PREFIX/jev-config doctor       check the hook, the driver, the app and the wallets

EOF
