# Shared appliance release identity and sync checks.
# shellcheck shell=bash

appliance_release_project_root() {
  if [ -n "${PROJECT_ROOT:-}" ]; then
    printf '%s\n' "${PROJECT_ROOT}"
    return
  fi
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  printf '%s\n' "${script_dir}"
}

appliance_release_overlay_manifest() {
  local root
  root="$(appliance_release_project_root)"
  printf '%s/overlay/usr/share/foldingos/manifests/foldops.toml\n' "${root}"
}

appliance_release_read() {
  local manifest
  manifest="$(appliance_release_overlay_manifest)"
  if [ ! -f "${manifest}" ]; then
    echo "ERROR: missing overlay FoldOps manifest: ${manifest}" >&2
    return 1
  fi
  grep '^manifest_release' "${manifest}" | head -1 | sed 's/.*"\(.*\)".*/\1/'
}

appliance_release_foldops_dir() {
  local root release
  root="$(appliance_release_project_root)"
  release="$(appliance_release_read)"
  printf '%s/build/output/foldops/%s\n' "${root}" "${release}"
}

appliance_release_tools_dir() {
  local root release
  root="$(appliance_release_project_root)"
  release="$(appliance_release_read)"
  printf '%s/build/output/foldingos-tools/%s\n' "${root}" "${release}"
}

appliance_release_image_foldingosctl() {
  local root
  root="$(appliance_release_project_root)"
  printf '%s/build/output/target/usr/bin/foldingosctl\n' "${root}"
}

appliance_release_sha256_file() {
  sha256sum "$1" | awk '{print $1}'
}
