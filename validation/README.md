# FoldingOS Validation Records

This directory stores committed physical validation records for release gates.

## Appliance foundation

- Template: [appliance-physical.template.json](appliance-physical.template.json)
- Completed record: `appliance-physical-<version>.json`

Create the versioned record only after the procedure in
[doc/physical-validation.md](../doc/physical-validation.md) passes on physical
hardware.

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
