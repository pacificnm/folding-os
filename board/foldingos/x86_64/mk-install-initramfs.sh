#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:?target directory argument is required}"
BOARD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STAGING_DIR="${STAGING_DIR:?STAGING_DIR is required}"
HOST_DIR="${HOST_DIR:?HOST_DIR is required}"
READELF="${HOST_DIR}/bin/x86_64-linux-readelf"

TARGET_LIB_DIRS=(
  "${TARGET_DIR}/lib"
  "${TARGET_DIR}/lib64"
  "${TARGET_DIR}/usr/lib"
)

INITRAMFS_ROOT="${BUILD_DIR}/foldingos-install-initramfs"
OUTPUT="${TARGET_DIR}/usr/share/foldingos/boot/install-initramfs.cpio.gz"

rm -rf "${INITRAMFS_ROOT}"
mkdir -p "${INITRAMFS_ROOT}"/{bin,sbin,usr/bin,lib,lib64,etc,proc,sys,dev,run}

install -m 0755 "${BOARD_DIR}/install-initramfs/init" "${INITRAMFS_ROOT}/init"

find_target_library() {
  local soname="$1"
  local directory
  for directory in "${TARGET_LIB_DIRS[@]}"; do
    if [ -f "${directory}/${soname}" ]; then
      printf '%s\n' "${directory}/${soname}"
      return 0
    fi
  done
  return 1
}

install_target_library() {
  local source="$1"
  local base
  local mode=0644

  base="$(basename "${source}")"
  case "${base}" in
    ld-linux*.so.2)
      install -D -m 0755 "${source}" "${INITRAMFS_ROOT}/lib64/${base}"
      return
      ;;
  esac

  # Buildroot x86_64 glibc uses slibdir=/lib64. Install runtime libraries
  # there so the initramfs dynamic linker can resolve DT_NEEDED entries.
  install -D -m "${mode}" "${source}" "${INITRAMFS_ROOT}/lib64/${base}"
  install -D -m "${mode}" "${source}" "${INITRAMFS_ROOT}/lib/${base}"
}

copy_needed_closure() {
  local binary="$1"
  local queue=("${binary}")
  local queued=" ${binary} "
  local current
  local soname
  local dependency

  while [ "${#queue[@]}" -gt 0 ]; do
    current="${queue[0]}"
    queue=("${queue[@]:1}")

    while read -r soname; do
      soname="${soname//[\[\]]/}"
      [ -n "${soname}" ] || continue

      dependency="$(find_target_library "${soname}")" || {
        echo "ERROR: unable to locate ${soname} required by ${current}" >&2
        exit 1
      }

      install_target_library "${dependency}"

      case " ${queued} " in
        *" ${dependency} "*) continue ;;
      esac
      queued="${queued}${dependency} "
      queue+=("${dependency}")
    done < <("${READELF}" -d "${current}" 2>/dev/null | awk '/NEEDED/ {print $5}')
  done
}

copy_with_deps() {
  local binary="$1"
  local destination="$2"
  local directory
  directory="$(dirname "${destination}")"
  mkdir -p "${INITRAMFS_ROOT}${directory}"
  install -m 0755 "${binary}" "${INITRAMFS_ROOT}${destination}"
  copy_needed_closure "${binary}"
}

copy_dynamic_linker() {
  local linker="${TARGET_DIR}/lib64/ld-linux-x86-64.so.2"
  if [ ! -e "${linker}" ]; then
    linker="$(find "${TARGET_DIR}/lib" -name 'ld-linux*.so.2' -print -quit)"
  fi
  if [ -z "${linker}" ] || [ ! -e "${linker}" ]; then
    echo "ERROR: unable to locate target dynamic linker for install initramfs" >&2
    exit 1
  fi
  install -D -m 0755 "${linker}" "${INITRAMFS_ROOT}/lib64/ld-linux-x86-64.so.2"
}

verify_initramfs_binary() {
  local binary="${INITRAMFS_ROOT}/$1"
  local soname
  local dependency
  local installed

  while read -r soname; do
    soname="${soname//[\[\]]/}"
    [ -n "${soname}" ] || continue
    installed="$(find "${INITRAMFS_ROOT}/lib64" "${INITRAMFS_ROOT}/lib" "${INITRAMFS_ROOT}/usr/lib" \
      -name "${soname}" -print -quit || true)"
    if [ -z "${installed}" ]; then
      dependency="$(find_target_library "${soname}")" || true
      echo "ERROR: initramfs is missing ${soname} required by $1" >&2
      if [ -n "${dependency}" ]; then
        echo "Target provides ${dependency} but it was not installed into the initramfs." >&2
      fi
      exit 1
    fi
  done < <("${READELF}" -d "${binary}" 2>/dev/null | awk '/NEEDED/ {print $5}')
}

copy_dynamic_linker

copy_with_deps "${TARGET_DIR}/bin/busybox" /bin/busybox
ln -sf busybox "${INITRAMFS_ROOT}/bin/sh"
ln -sf busybox "${INITRAMFS_ROOT}/bin/cat"
ln -sf busybox "${INITRAMFS_ROOT}/bin/mkdir"

copy_with_deps "${TARGET_DIR}/usr/bin/foldingosctl" /usr/bin/foldingosctl
copy_with_deps "${TARGET_DIR}/usr/bin/lsblk" /usr/bin/lsblk
copy_with_deps "${TARGET_DIR}/usr/bin/findmnt" /usr/bin/findmnt
copy_with_deps "${TARGET_DIR}/sbin/sgdisk" /sbin/sgdisk
copy_with_deps "${TARGET_DIR}/usr/bin/losetup" /usr/bin/losetup
copy_with_deps "${TARGET_DIR}/bin/mount" /bin/mount
copy_with_deps "${TARGET_DIR}/bin/umount" /bin/umount
copy_with_deps "${TARGET_DIR}/bin/sync" /bin/sync
copy_with_deps "${TARGET_DIR}/usr/bin/ssh-keygen" /usr/bin/ssh-keygen
if [ -x "${TARGET_DIR}/sbin/partprobe" ]; then
  copy_with_deps "${TARGET_DIR}/sbin/partprobe" /sbin/partprobe
fi
ln -sf busybox "${INITRAMFS_ROOT}/bin/dd"
ln -sf ../bin/busybox "${INITRAMFS_ROOT}/sbin/reboot"

verify_initramfs_binary bin/busybox
verify_initramfs_binary usr/bin/foldingosctl
verify_initramfs_binary usr/bin/losetup

mkdir -p "${INITRAMFS_ROOT}/etc" "${INITRAMFS_ROOT}/root"
printf 'passwd: files\n' > "${INITRAMFS_ROOT}/etc/nsswitch.conf"
printf 'group: files\n' >> "${INITRAMFS_ROOT}/etc/nsswitch.conf"
printf 'hosts: files dns\n' >> "${INITRAMFS_ROOT}/etc/nsswitch.conf"
printf 'root:x:0:0:root:/root:/bin/sh\n' > "${INITRAMFS_ROOT}/etc/passwd"
printf 'root:x:0:\n' > "${INITRAMFS_ROOT}/etc/group"

[ -x "${INITRAMFS_ROOT}/lib64/ld-linux-x86-64.so.2" ] || {
  echo "ERROR: initramfs dynamic linker is missing or not executable" >&2
  exit 1
}
for required in lib64/libc.so.6 lib64/libresolv.so.2 lib/libc.so.6 lib/libresolv.so.2; do
  if [ ! -f "${INITRAMFS_ROOT}/${required}" ]; then
    echo "ERROR: initramfs is missing ${required} required by /bin/sh" >&2
    exit 1
  fi
done

mkdir -p "$(dirname "${OUTPUT}")"
(
  cd "${INITRAMFS_ROOT}"
  find . | LC_ALL=C sort | cpio --quiet -o -H newc | gzip -9 > "${OUTPUT}"
)

if ! zcat "${OUTPUT}" | cpio -t --quiet 2>/dev/null | grep -qx 'init'; then
  echo "ERROR: ${OUTPUT} is missing a root-level init entry" >&2
  echo "Initramfs entries must use relative paths (./init), not absolute build paths." >&2
  zcat "${OUTPUT}" | cpio -t 2>/dev/null | head -10 >&2 || true
  exit 1
fi
if ! zcat "${OUTPUT}" | cpio -t --quiet 2>/dev/null | grep -qx 'bin/sh'; then
  echo "ERROR: ${OUTPUT} is missing bin/sh required to execute the init script" >&2
  exit 1
fi
if ! zcat "${OUTPUT}" | cpio -t --quiet 2>/dev/null | grep -qx 'sbin/reboot'; then
  echo "ERROR: ${OUTPUT} is missing sbin/reboot required to reboot after install" >&2
  exit 1
fi
if ! zcat "${OUTPUT}" | cpio -t --quiet 2>/dev/null | grep -qx 'lib64/libresolv.so.2'; then
  echo "ERROR: ${OUTPUT} is missing lib64/libresolv.so.2 required by /bin/sh" >&2
  exit 1
fi
if ! zcat "${OUTPUT}" | cpio -t --quiet 2>/dev/null | grep -qx 'etc/passwd'; then
  echo "ERROR: ${OUTPUT} is missing etc/passwd required by ssh-keygen" >&2
  exit 1
fi
if zcat "${OUTPUT}" | cpio -t --quiet 2>/dev/null | grep -qx 'lib/x86_64-linux-gnu/libc.so.6'; then
  echo "ERROR: ${OUTPUT} contains host-layout lib/x86_64-linux-gnu/libc.so.6" >&2
  exit 1
fi
