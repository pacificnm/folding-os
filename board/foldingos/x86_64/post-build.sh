#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:?target directory argument is required}"

mkdir -p \
  "${TARGET_DIR}/boot/efi" \
  "${TARGET_DIR}/data" \
  "${TARGET_DIR}/etc/systemd/network"

# systemd-resolved owns the runtime resolver file.
ln -snf /run/systemd/resolve/stub-resolv.conf "${TARGET_DIR}/etc/resolv.conf"

# Host keys are generated on the node and must not be embedded in the image.
rm -f "${TARGET_DIR}"/etc/ssh/ssh_host_*_key*

mkdir -p "${TARGET_DIR}/etc/systemd/system/local-fs.target.wants"
ln -snf \
  /usr/lib/systemd/system/foldingos-data-expand.service \
  "${TARGET_DIR}/etc/systemd/system/local-fs.target.wants/foldingos-data-expand.service"
