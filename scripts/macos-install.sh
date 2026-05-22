#!/usr/bin/env bash
set -euo pipefail

open_app=1
install_dir="/Applications"
pause_on_exit="auto"
log_file=""

usage() {
  cat <<'USAGE'
usage: scripts/macos-install.sh [--install-dir PATH] [--no-open] [--pause-on-exit|--no-pause]

Build an unsigned release naite.app bundle, install it, and open it.

Options:
  --install-dir PATH  Install destination directory. Defaults to /Applications.
  --no-open           Install only; do not launch the app.
  --pause-on-exit     Wait for Return before exiting when running in a terminal.
  --no-pause          Do not wait before exiting.
  -h, --help          Show this help.
USAGE
}

has_controlling_terminal() {
  (: </dev/tty >/dev/tty) 2>/dev/null
}

should_pause_on_exit() {
  [[ "${CI:-}" == "true" ]] && return 1
  [[ "${NAITE_INSTALL_NO_PAUSE:-}" == "1" ]] && return 1

  case "$pause_on_exit" in
    yes)
      return 0
      ;;
    no)
      return 1
      ;;
    auto)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

pause_in_terminal() {
  if [[ -t 0 && -t 1 ]]; then
    printf '\nnaite installer finished with exit code %s. Press Return to close this terminal...' "$1"
    IFS= read -r _
    return $?
  fi

  if has_controlling_terminal; then
    printf '\nnaite installer finished with exit code %s. Press Return to close this terminal...' "$1" >/dev/tty
    IFS= read -r _ </dev/tty
    return $?
  fi

  return 1
}

pause_in_dialog() {
  command -v osascript >/dev/null 2>&1 || return 1
  [[ "$(uname -s)" == "Darwin" ]] || return 1

  message="naite installer finished with exit code $1."
  if [[ -n "$log_file" ]]; then
    message="$message"$'\n\n'"Log: $log_file"
  fi

  osascript - "$message" <<'APPLESCRIPT' >/dev/null 2>&1
on run argv
  display dialog item 1 of argv buttons {"Close"} default button "Close" with title "naite Installer"
end run
APPLESCRIPT
}

pause_before_exit() {
  status=$?
  set +e

  if should_pause_on_exit; then
    pause_in_terminal "$status" || pause_in_dialog "$status"
  fi

  exit "$status"
}

remember_error() {
  local status=$?
  local last_error="line ${BASH_LINENO[0]}: ${BASH_COMMAND} (exit $status)"
  printf '\nerror: naite installer failed at %s\n' "$last_error" >&2
  if [[ -n "$log_file" ]]; then
    printf 'error: full installer log: %s\n' "$log_file" >&2
  fi
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

trap pause_before_exit EXIT
trap remember_error ERR

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-dir)
      if [[ $# -lt 2 || -z "${2:-}" ]]; then
        printf 'error: --install-dir requires a path\n' >&2
        exit 2
      fi
      install_dir="$2"
      shift 2
      ;;
    --no-open)
      open_app=0
      shift
      ;;
    --pause-on-exit)
      pause_on_exit="yes"
      shift
      ;;
    --no-pause)
      pause_on_exit="no"
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      printf 'error: unknown argument: %s\n\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

export PATH="$HOME/.cargo/bin:$PATH"

cd "$repo_root"

mkdir -p "$repo_root/target"
log_file="$repo_root/target/macos-install.log"
: >"$log_file"
exec > >(tee -a "$log_file") 2>&1

printf 'naite installer started. Log: %s\n' "$log_file"
printf 'Repository: %s\n' "$repo_root"
printf 'Install destination: %s\n' "$install_dir"

bundle_path="$(scripts/macos-bundle.sh --release)"
app_path="$install_dir/naite.app"

mkdir -p "$install_dir"
rm -rf "$app_path"
ditto "$bundle_path" "$app_path"

printf 'Installed unsigned app: %s\n' "$app_path"

if [[ "$open_app" -eq 1 ]]; then
  open "$app_path"
fi
