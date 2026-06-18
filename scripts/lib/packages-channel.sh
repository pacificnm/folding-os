# Shared helpers for packages.folding-os.com publication scripts.
# shellcheck shell=bash

packages_channel_defaults() {
  R2_REMOTE="${R2_REMOTE:-r2}"
  FOLDOPS_R2_BUCKET="${FOLDOPS_R2_BUCKET:-foldops-packages}"
  FOLDOPS_R2_PREFIX="${FOLDOPS_R2_PREFIX:-foldops}"
  TOOLS_R2_PREFIX="${TOOLS_R2_PREFIX:-foldingos-tools}"
  PACKAGES_PUBLIC_BASE="${PACKAGES_PUBLIC_BASE:-https://packages.folding-os.com}"
  PACKAGES_MINIMUM_FOLDINGOS_VERSION="${PACKAGES_MINIMUM_FOLDINGOS_VERSION:-0.1.0}"
}

packages_require_rclone() {
  if ! command -v rclone >/dev/null 2>&1; then
    echo "ERROR: rclone not found" >&2
    exit 1
  fi
}

packages_read_manifest_minimum() {
  local manifest_path="$1"
  local value
  value="$(grep -E '^minimum_foldingos_version[[:space:]]*=' "${manifest_path}" \
    | head -1 \
    | sed -E 's/^minimum_foldingos_version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/')"
  if [ -z "${value}" ]; then
    echo "${PACKAGES_MINIMUM_FOLDINGOS_VERSION}"
  else
    echo "${value}"
  fi
}

packages_fetch_index() {
  local channel="$1"
  local bucket="$2"
  local prefix="$3"
  local remote="${R2_REMOTE}:${bucket}/${prefix}/index.json"
  local tmp

  tmp="$(mktemp)"
  if rclone copyto "${remote}" "${tmp}" 2>/dev/null && [ -s "${tmp}" ]; then
    cat "${tmp}"
  elif [ "${channel}" = "foldops" ]; then
    printf '%s\n' '{"schema_version":1,"channel":"foldops","releases":[]}'
  else
    printf '%s\n' '{"schema_version":1,"channel":"foldingos-tools","releases":[]}'
  fi
  rm -f "${tmp}"
}

packages_upload_index() {
  local bucket="$1"
  local prefix="$2"
  local index_file="$3"
  local remote="${R2_REMOTE}:${bucket}/${prefix}/index.json"

  if [ "${DRY_RUN:-0}" -eq 1 ]; then
    echo "Would upload index: ${remote}"
    cat "${index_file}"
    return 0
  fi

  rclone copyto "${index_file}" "${remote}"
}

packages_refresh_foldops_index() {
  local manifest_release="$1"
  local manifest_path="$2"
  local minimum_version
  local manifest_url
  local current_index
  local updated_index
  local index_file

  minimum_version="$(packages_read_manifest_minimum "${manifest_path}")"
  manifest_url="${PACKAGES_PUBLIC_BASE}/${FOLDOPS_R2_PREFIX}/${manifest_release}/manifest.toml"
  index_file="$(mktemp)"

  if [ "${DRY_RUN:-0}" -eq 1 ]; then
    echo "Would refresh FoldOps index: ${R2_REMOTE}:${FOLDOPS_R2_BUCKET}/${FOLDOPS_R2_PREFIX}/index.json"
    current_index='{"schema_version":1,"channel":"foldops","releases":[]}'
  else
    current_index="$(packages_fetch_index foldops "${FOLDOPS_R2_BUCKET}" "${FOLDOPS_R2_PREFIX}")"
  fi

  updated_index="$(
    printf '%s' "${current_index}" \
      | python3 "${PROJECT_ROOT}/scripts/lib/packages-index.py" merge-foldops \
          "${manifest_release}" \
          "${manifest_url}" \
          "${minimum_version}"
  )"
  printf '%s' "${updated_index}" > "${index_file}"
  packages_upload_index "${FOLDOPS_R2_BUCKET}" "${FOLDOPS_R2_PREFIX}" "${index_file}"
  rm -f "${index_file}"
}

packages_refresh_tools_index() {
  local tools_version="$1"
  local binary_url
  local sha256_url
  local current_index
  local updated_index
  local index_file

  binary_url="${PACKAGES_PUBLIC_BASE}/${TOOLS_R2_PREFIX}/${tools_version}/foldingosctl-x86_64"
  sha256_url="${PACKAGES_PUBLIC_BASE}/${TOOLS_R2_PREFIX}/${tools_version}/SHA256SUMS"
  index_file="$(mktemp)"

  if [ "${DRY_RUN:-0}" -eq 1 ]; then
    echo "Would refresh tools index: ${R2_REMOTE}:${FOLDOPS_R2_BUCKET}/${TOOLS_R2_PREFIX}/index.json"
    current_index='{"schema_version":1,"channel":"foldingos-tools","releases":[]}'
  else
    current_index="$(packages_fetch_index foldingos-tools "${FOLDOPS_R2_BUCKET}" "${TOOLS_R2_PREFIX}")"
  fi

  updated_index="$(
    printf '%s' "${current_index}" \
      | python3 "${PROJECT_ROOT}/scripts/lib/packages-index.py" merge-tools \
          "${tools_version}" \
          "${binary_url}" \
          "${sha256_url}" \
          "${PACKAGES_MINIMUM_FOLDINGOS_VERSION}"
  )"
  printf '%s' "${updated_index}" > "${index_file}"
  packages_upload_index "${FOLDOPS_R2_BUCKET}" "${TOOLS_R2_PREFIX}" "${index_file}"
  rm -f "${index_file}"
}
