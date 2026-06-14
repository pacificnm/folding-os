#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CACHE_DIR="${PROJECT_ROOT}/build/cache/ipxe"
IPXE_URL="http://boot.ipxe.org/x86_64-efi/ipxe.efi"
IPXE_FILE="${CACHE_DIR}/ipxe.efi"

mkdir -p "${CACHE_DIR}"

if [ ! -f "${IPXE_FILE}" ]; then
  echo "Downloading iPXE UEFI bootstrap loader"
  wget -O "${IPXE_FILE}" "${IPXE_URL}"
else
  echo "Using cached iPXE UEFI bootstrap loader"
fi

sha256sum "${IPXE_FILE}" > "${IPXE_FILE}.sha256"
echo "iPXE bootstrap ready:"
echo "${IPXE_FILE}"
