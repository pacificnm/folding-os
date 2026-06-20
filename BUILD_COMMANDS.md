# FoldingOS Build And Supervisor USB Runbook

Use this checklist from the repository root when preparing a live supervisor
USB image.

This workflow does three things:

1. Builds and pins the FoldOps runtime bundles into the FoldingOS overlay.
2. Publishes those FoldOps bundles to the live packages channel.
3. Optionally builds and publishes a `foldingosctl` tools release without a
   full image rebuild.
4. Builds the FoldingOS image and writes a bootable supervisor USB stick.

Project memory note: `foldingosctl` tools releases are built and published
independently of OS image builds. Use
`./scripts/build-foldingosctl-release --version <tools-version> --sync-overlay`
to produce the tools artifact and update
`overlay/usr/share/foldingos/manifests/tools.json` so the next
`./scripts/build` embeds that tools pin. Publishing the tools artifact itself
does not require `./scripts/build`.

For implementation-agent subsystem orientation, see
[doc/agent-subsystems.md](doc/agent-subsystems.md). That guide maps affected
areas to governing docs, owner paths, and verification commands; this runbook
remains the live supervisor USB workflow.

## Variables

Set these values before running the commands:

```bash
FOLDOPS_RELEASE="0.1.0-<version>"
TOOLS_RELEASE="${FOLDOPS_RELEASE}"
USB_DEVICE="/dev/sdb"
SSH_PUBLIC_KEY="${HOME}/.ssh/id_ed25519.pub"
FOLDOPS_INGEST_TOKEN="/tmp/foldops-ingest-token"
IMAGE="build/output/images/foldingos-x86_64-0.1.0.img"
```

Replace `<version>` with the release suffix being published, for example:

```bash
FOLDOPS_RELEASE="0.1.0-67"
TOOLS_RELEASE="0.1.0-67"
```

Before writing USB media, confirm `USB_DEVICE` is the whole disk for the target
USB stick, not a partition and not the workstation boot disk.

```bash
lsblk
```

## 1. Build And Pin FoldOps Bundles

Build the FoldOps agent, supervisor, and web bundles, then sync the generated
manifest into the FoldingOS overlay:

```bash
./scripts/build-foldops-bundles \
  --manifest-release "${FOLDOPS_RELEASE}" \
  --sync-overlay
```

This updates the embedded FoldOps manifest used by the image bootstrap path.
Commit or intentionally carry the resulting overlay manifest change with the
image build.

## 2. Publish FoldOps Bundles

Publish the same bundle release to the live packages channel:

```bash
./scripts/publish-foldops-bundles "${FOLDOPS_RELEASE}"
```

This requires `rclone` to be configured for the FoldingOS packages bucket.

## 3. Build And Publish foldingosctl Tools

Build and publish a `foldingosctl` tools release when the running fleet needs a
CLI/control-plane fix before the next OS image is rebuilt:

```bash
./scripts/build-foldingosctl-release \
  --version "${TOOLS_RELEASE}" \
  --sync-overlay

./scripts/publish-foldingos-tools "${TOOLS_RELEASE}"
```

`--sync-overlay` writes the bootstrap tools assignment for the next image build.
It is not an image build step and does not require `./scripts/build` before
publishing to `packages.folding-os.com/foldingos-tools/`.

For a dry run through the umbrella publisher:

```bash
./scripts/publish-packages-release \
  --tools "${TOOLS_RELEASE}" \
  --build \
  --dry-run
```

## 4. Build The FoldingOS Image

Build the release image:

```bash
./scripts/build
```

Expected image output:

```text
build/output/images/foldingos-x86_64-0.1.0.img
```

## 5. Write The Supervisor USB

Run the USB writer as root. This is destructive to the selected device.

```bash
sudo ./scripts/make-bootable-usb \
  --ssh-public-key "${SSH_PUBLIC_KEY}" \
  --role supervisor \
  --foldops-ingest-token "${FOLDOPS_INGEST_TOKEN}" \
  "${USB_DEVICE}" \
  "${IMAGE}"
```

The command stages:

- `foldingos-admin` SSH access from the provided public key
- the fixed `supervisor` installation role
- the FoldOps ingest token for supervisor bootstrap

## 6. Flush And Power Off USB

Flush pending writes:

```bash
sync
```

Power off the USB device before removing it:

```bash
udisksctl power-off -b "${USB_DEVICE}"
```

When `udisksctl` completes, remove the USB stick and boot the live supervisor
machine from it.
