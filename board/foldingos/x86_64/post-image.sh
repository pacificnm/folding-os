#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BOARD_DIR="${PROJECT_ROOT}/board/foldingos/x86_64"
EFI_DIR="${BINARIES_DIR}/efi-part"
DATA_IMAGE="${BINARIES_DIR}/data.ext4"

mkdir -p \
  "${EFI_DIR}/EFI/BOOT" \
  "${EFI_DIR}/boot/grub" \
  "${EFI_DIR}/foldingos/provision"

if [ -f "${EFI_DIR}/EFI/BOOT/bootx64.efi" ]; then
  mv "${EFI_DIR}/EFI/BOOT/bootx64.efi" "${EFI_DIR}/EFI/BOOT/BOOTX64.EFI"
fi
install -m 0644 "${BOARD_DIR}/grub.cfg" "${EFI_DIR}/EFI/BOOT/grub.cfg"
install -m 0644 "${BOARD_DIR}/grub.cfg" "${EFI_DIR}/boot/grub/grub.cfg"

rm -f "${DATA_IMAGE}"
"${HOST_DIR}/sbin/mkfs.ext4" \
  -F \
  -L FOLDINGOS_DATA \
  -U 464f4c44-494e-474f-5344-415441000001 \
  -m 0 \
  "${DATA_IMAGE}" \
  1534M

support/scripts/genimage.sh -c "${BOARD_DIR}/genimage.cfg"
mv "${BINARIES_DIR}/foldingos-x86_64.img" \
  "${BINARIES_DIR}/foldingos-x86_64-0.1.0.img"
