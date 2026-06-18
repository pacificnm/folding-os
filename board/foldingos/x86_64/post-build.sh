#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:?target directory argument is required}"
BOARD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${BOARD_DIR}/../../.." && pwd)"

mkdir -p \
  "${TARGET_DIR}/boot/efi" \
  "${TARGET_DIR}/data" \
  "${TARGET_DIR}/etc/systemd/network" \
  "${TARGET_DIR}/usr/lib"

cat > "${TARGET_DIR}/usr/lib/os-release" <<'EOF'
NAME="FoldingOS"
ID=foldingos
VERSION="0.1.0"
VERSION_ID="0.1.0"
PRETTY_NAME="FoldingOS 0.1.0"
EOF
ln -snf ../usr/lib/os-release "${TARGET_DIR}/etc/os-release"

mkdir -p "${TARGET_DIR}/usr/share/foldingos"
BUILD_REVISION="unknown"
if command -v git >/dev/null 2>&1 && [ -d "${PROJECT_ROOT}/.git" ]; then
  BUILD_REVISION="$(git -C "${PROJECT_ROOT}" rev-parse HEAD 2>/dev/null || echo unknown)"
fi
printf '%s\n' "${BUILD_REVISION}" > "${TARGET_DIR}/usr/share/foldingos/build-revision"

# systemd-resolved owns the runtime resolver file.
ln -snf /run/systemd/resolve/stub-resolv.conf "${TARGET_DIR}/etc/resolv.conf"

# Host keys are generated on the node and must not be embedded in the image.
rm -f "${TARGET_DIR}"/etc/ssh/ssh_host_*_key*
chmod 0440 "${TARGET_DIR}/etc/sudoers.d/foldingos-admin"
if [ -f "${TARGET_DIR}/etc/sudoers.d/foldops-recovery" ]; then
  chmod 0440 "${TARGET_DIR}/etc/sudoers.d/foldops-recovery"
fi

FOLDOPS_MANIFEST="${PROJECT_ROOT}/overlay/usr/share/foldingos/manifests/foldops.toml"
if [ ! -f "${FOLDOPS_MANIFEST}" ]; then
  echo "ERROR: Missing embedded FoldOps manifest: ${FOLDOPS_MANIFEST}" >&2
  exit 1
fi
FOLDOPS_MANIFEST_RELEASE="$(grep '^manifest_release' "${FOLDOPS_MANIFEST}" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
FOLDOPS_BUNDLE_DIR="${PROJECT_ROOT}/build/output/foldops/${FOLDOPS_MANIFEST_RELEASE}"
if [ ! -d "${FOLDOPS_BUNDLE_DIR}" ]; then
  echo "ERROR: Missing FoldOps bundles for embedded manifest ${FOLDOPS_MANIFEST_RELEASE}" >&2
  echo "Run ./scripts/build — it builds FoldOps bundles before the OS image." >&2
  exit 1
fi
FOLDOPS_CACHE="${TARGET_DIR}/usr/share/foldingos/cache/foldops/${FOLDOPS_MANIFEST_RELEASE}"
mkdir -p "${FOLDOPS_CACHE}"
for bundle in foldops-agent-x86_64.tar.zst foldops-supervisor-x86_64.tar.zst foldops-web-x86_64.tar.zst; do
  if [ ! -f "${FOLDOPS_BUNDLE_DIR}/${bundle}" ]; then
    echo "ERROR: Missing FoldOps bundle: ${FOLDOPS_BUNDLE_DIR}/${bundle}" >&2
    exit 1
  fi
  install -D -m 0644 "${FOLDOPS_BUNDLE_DIR}/${bundle}" "${FOLDOPS_CACHE}/${bundle}"
done
echo "Embedded FoldOps bootstrap cache ${FOLDOPS_MANIFEST_RELEASE} into image rootfs"

mkdir -p "${TARGET_DIR}/etc/systemd/system/local-fs.target.wants"
ln -snf \
  /usr/lib/systemd/system/foldingos-data-expand.service \
  "${TARGET_DIR}/etc/systemd/system/local-fs.target.wants/foldingos-data-expand.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-persistent-dirs.service \
  "${TARGET_DIR}/etc/systemd/system/local-fs.target.wants/foldingos-persistent-dirs.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-journal-dir.service \
  "${TARGET_DIR}/etc/systemd/system/local-fs.target.wants/foldingos-journal-dir.service"
ln -snf \
  /usr/lib/systemd/system/var-log-journal.mount \
  "${TARGET_DIR}/etc/systemd/system/local-fs.target.wants/var-log-journal.mount"

mkdir -p \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants" \
  "${TARGET_DIR}/etc/systemd/system/network-online.target.wants" \
  "${TARGET_DIR}/etc/systemd/system/sockets.target.wants" \
  "${TARGET_DIR}/etc/systemd/system/sysinit.target.wants" \
  "${TARGET_DIR}/etc/systemd/system/timers.target.wants"
ln -snf \
  /usr/lib/systemd/system/sshd.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/sshd.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-journal-flush.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-journal-flush.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-identity.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-identity.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-config-validate.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-config-validate.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-ssh-provision.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-ssh-provision.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-installation-role.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-installation-role.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-registry-bootstrap.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-registry-bootstrap.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-provision.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-provision.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-agent-register.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-agent-register.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-agent-version-check.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-agent-version-check.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-agent-apply-update.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-agent-apply-update.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-boot-status.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-boot-status.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-fah-prepare.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-fah-prepare.service"
ln -snf \
  /usr/lib/systemd/system/folding-at-home.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/folding-at-home.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-fah-acquire.timer \
  "${TARGET_DIR}/etc/systemd/system/timers.target.wants/foldingos-fah-acquire.timer"
ln -snf \
  /usr/lib/systemd/system/foldingos-foldops-acquire.timer \
  "${TARGET_DIR}/etc/systemd/system/timers.target.wants/foldingos-foldops-acquire.timer"
ln -snf \
  /usr/lib/systemd/system/foldingos-foldops-provision.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-foldops-provision.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-foldops-serve-https.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-foldops-serve-https.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-foldops-supervisor.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-foldops-supervisor.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-foldops-agent.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-foldops-agent.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-registry-poll.timer \
  "${TARGET_DIR}/etc/systemd/system/timers.target.wants/foldingos-registry-poll.timer"
ln -snf \
  /usr/lib/systemd/system/systemd-networkd.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/systemd-networkd.service"
ln -snf \
  /usr/lib/systemd/system/systemd-networkd-wait-online.service \
  "${TARGET_DIR}/etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service"
ln -snf \
  /usr/lib/systemd/system/systemd-networkd.socket \
  "${TARGET_DIR}/etc/systemd/system/sockets.target.wants/systemd-networkd.socket"
ln -snf \
  /usr/lib/systemd/system/systemd-networkd-varlink.socket \
  "${TARGET_DIR}/etc/systemd/system/sockets.target.wants/systemd-networkd-varlink.socket"
ln -snf \
  /usr/lib/systemd/system/systemd-resolved-varlink.socket \
  "${TARGET_DIR}/etc/systemd/system/sockets.target.wants/systemd-resolved-varlink.socket"
ln -snf \
  /usr/lib/systemd/system/systemd-resolved-monitor.socket \
  "${TARGET_DIR}/etc/systemd/system/sockets.target.wants/systemd-resolved-monitor.socket"
ln -snf \
  /usr/lib/systemd/system/systemd-resolved.service \
  "${TARGET_DIR}/etc/systemd/system/sysinit.target.wants/systemd-resolved.service"
ln -snf \
  /usr/lib/systemd/system/systemd-timesyncd.service \
  "${TARGET_DIR}/etc/systemd/system/sysinit.target.wants/systemd-timesyncd.service"
ln -snf \
  /usr/lib/systemd/system/systemd-networkd.service \
  "${TARGET_DIR}/etc/systemd/system/dbus-org.freedesktop.network1.service"
ln -snf \
  /usr/lib/systemd/system/systemd-resolved.service \
  "${TARGET_DIR}/etc/systemd/system/dbus-org.freedesktop.resolve1.service"
ln -snf \
  /usr/lib/systemd/system/systemd-timesyncd.service \
  "${TARGET_DIR}/etc/systemd/system/dbus-org.freedesktop.timesync1.service"
ln -snf /dev/null \
  "${TARGET_DIR}/etc/systemd/system/systemd-journal-flush.service"
ln -snf \
  /usr/lib/systemd/system/foldingos-provision-boot.service \
  "${TARGET_DIR}/etc/systemd/system/multi-user.target.wants/foldingos-provision-boot.service"

mkdir -p "${TARGET_DIR}/usr/share/foldingos/boot/ipxe"
IPXE_CACHE="${PROJECT_ROOT}/build/cache/ipxe/ipxe.efi"
if [ ! -f "${IPXE_CACHE}" ]; then
  echo "ERROR: Missing cached iPXE bootstrap loader at ${IPXE_CACHE}" >&2
  echo "Run ./scripts/fetch-sources before ./scripts/build." >&2
  exit 1
fi
install -D -m 0644 "${IPXE_CACHE}" \
  "${TARGET_DIR}/usr/share/foldingos/boot/ipxe/ipxe.efi"

if [ -n "${BINARIES_DIR:-}" ] && [ -f "${BINARIES_DIR}/bzImage" ]; then
  install -D -m 0644 "${BINARIES_DIR}/bzImage" \
    "${TARGET_DIR}/usr/share/foldingos/boot/vmlinuz"
  install -D -m 0644 "${BINARIES_DIR}/bzImage" \
    "${BINARIES_DIR}/foldingos-update-vmlinuz"
fi

"${BOARD_DIR}/mk-install-initramfs.sh" "${TARGET_DIR}"
if [ -n "${BINARIES_DIR:-}" ] && [ -f "${TARGET_DIR}/usr/share/foldingos/boot/install-initramfs.cpio.gz" ]; then
  install -D -m 0644 "${TARGET_DIR}/usr/share/foldingos/boot/install-initramfs.cpio.gz" \
    "${BINARIES_DIR}/foldingos-update-initramfs.cpio.gz"
fi
