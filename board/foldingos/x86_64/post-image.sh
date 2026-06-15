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

GRUB_EFI_SRC="${EFI_DIR}/EFI/BOOT/bootx64.efi"
GRUB_EFI_DST="${EFI_DIR}/EFI/BOOT/BOOTX64.EFI"

grub_efi_has_loadenv() {
  local efi="$1"
  strings "${efi}" 2>/dev/null | grep -m1 -E 'loadenv|load_env' >/dev/null
}

if [ -f "${GRUB_EFI_SRC}" ]; then
  mv -f "${GRUB_EFI_SRC}" "${GRUB_EFI_DST}"
elif [ -f "${GRUB_EFI_DST}" ]; then
  if ! grub_efi_has_loadenv "${GRUB_EFI_DST}"; then
    echo "ERROR: ${GRUB_EFI_DST} is missing the loadenv module." >&2
    echo "Rebuild GRUB: cd build/output && make grub2-rebuild" >&2
    echo "Then re-run ./scripts/build" >&2
    exit 1
  fi
else
  echo "ERROR: missing GRUB EFI at ${GRUB_EFI_SRC} or ${GRUB_EFI_DST}." >&2
  echo "Rebuild GRUB: cd build/output && make grub2-rebuild" >&2
  echo "Then re-run ./scripts/build" >&2
  exit 1
fi
install -m 0644 "${BOARD_DIR}/grub.cfg" "${EFI_DIR}/EFI/BOOT/grub.cfg"
install -m 0644 "${BOARD_DIR}/grub.cfg" "${EFI_DIR}/boot/grub/grub.cfg"

mkdir -p "${EFI_DIR}/foldingos/update"
if [ -f "${BINARIES_DIR}/foldingos-update-vmlinuz" ]; then
  install -m 0644 "${BINARIES_DIR}/foldingos-update-vmlinuz" \
    "${EFI_DIR}/foldingos/update/vmlinuz"
fi
if [ -f "${BINARIES_DIR}/foldingos-update-initramfs.cpio.gz" ]; then
  install -m 0644 "${BINARIES_DIR}/foldingos-update-initramfs.cpio.gz" \
    "${EFI_DIR}/foldingos/update/install-initramfs.cpio.gz"
fi
if command -v grub-editenv >/dev/null 2>&1; then
  grub-editenv "${EFI_DIR}/EFI/BOOT/grubenv" create
elif [ -x "${HOST_DIR}/bin/grub-editenv" ]; then
  "${HOST_DIR}/bin/grub-editenv" "${EFI_DIR}/EFI/BOOT/grubenv" create
fi

find "${EFI_DIR}" -exec touch -h -d "@${SOURCE_DATE_EPOCH}" {} +

rm -f "${DATA_IMAGE}"
"${HOST_DIR}/sbin/mkfs.ext4" \
  -F \
  -L FOLDINGOS_DATA \
  -U 464f4c44-494e-474f-5344-415441000001 \
  -E hash_seed=464f4c44-494e-474f-5344-415441000001 \
  -m 0 \
  "${DATA_IMAGE}" \
  1534M

support/scripts/genimage.sh -c "${BOARD_DIR}/genimage.cfg"
mv "${BINARIES_DIR}/foldingos-x86_64.img" \
  "${BINARIES_DIR}/foldingos-x86_64-0.1.0.img"
