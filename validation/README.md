# FoldingOS Validation Records

This directory stores committed physical validation records for release gates.

## Appliance foundation

- Template: [appliance-physical.template.json](appliance-physical.template.json)
- Completed record: `appliance-physical-<version>.json`

## Network fleet provisioning

- QEMU template: [network-provision-qemu.template.json](network-provision-qemu.template.json)
- QEMU completed record: `network-provision-qemu-<version>.json`
- Physical template: [network-provision-physical.template.json](network-provision-physical.template.json)
- Physical completed record: `network-provision-physical-<version>.json`

Create versioned records only after the procedures in
[doc/physical-validation.md](../doc/physical-validation.md) (foundation) and
[doc/milestone/3-readiness-review.md](../doc/milestone/3-readiness-review.md)
(network provisioning) pass.

Prepare boot media with:

```bash
sudo ./scripts/make-bootable-usb \
  --ssh-public-key /path/to/admin-key.pub \
  /dev/sdX \
  build/output/images/foldingos-x86_64-0.1.0.img
```

Verify a completed record with:

```bash
./scripts/verify-physical-validation-record \
  validation/appliance-physical-0.1.0.json \
  build/output/images/foldingos-x86_64-0.1.0.img
```
