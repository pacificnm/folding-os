# Ensure rustup/cargo from the standard install location is on PATH.
# shellcheck shell=bash

rust_host_ensure_cargo() {
  if command -v cargo >/dev/null 2>&1; then
    return 0
  fi

  if [ -f "${HOME}/.cargo/env" ]; then
    # shellcheck disable=SC1091
    source "${HOME}/.cargo/env"
  elif [ -x "${HOME}/.cargo/bin/cargo" ]; then
    PATH="${HOME}/.cargo/bin:${PATH}"
    export PATH
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo not found" >&2
    echo "Install Rust (rustup) or add ~/.cargo/bin to PATH:" >&2
    echo "  source \"\$HOME/.cargo/env\"" >&2
    exit 1
  fi
}
