#!/usr/bin/env bash
# Fork-only (leftos/patches): run one cargo command on a WSL ext4 copy of the
# working tree, so the Unix-only tests and Linux lints run from Windows.
#
#   wsl -d Ubuntu -u root -- bash tools/wsl-ci.sh clippy --workspace --all-targets -- -D warnings
#
# Run it from the tree's root. The copy lives in $HOME/herdr-wsl/<key>/tree and
# builds into $HOME/herdr-wsl/<key>/target, where <key> is the tree's last two
# path parts, so two worktrees never share a copy or a target folder.
set -euo pipefail

key=$(basename "$(dirname "$PWD")")-$(basename "$PWD")
key=${key//[^A-Za-z0-9._-]/_}
base="$HOME/herdr-wsl/$key"
pidfile="$base/cargo.pid"
mkdir -p "$base/tree"

# A gate kill ends wsl.exe but not what it started in the VM. End the earlier
# run's cargo (recorded below, its command line names no path) and every
# compiler or test process whose command line names this key's folders.
if [[ -f $pidfile ]]; then
  old=$(<"$pidfile")
  if [[ $old =~ ^[0-9]+$ ]] && kill -0 "$old" 2>/dev/null; then
    pkill -TERM -P "$old" || true
    kill -TERM "$old" || true
  fi
fi
pkill -TERM -f "herdr-wsl/$key/" || true

rsync -a --delete --exclude target --exclude .git --exclude .tmp "$PWD/" "$base/tree/"

# shellcheck source=/dev/null
source "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$base/target"
cd "$base/tree"
echo $$ >"$pidfile"
exec cargo "$@"
