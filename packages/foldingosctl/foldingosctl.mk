################################################################################
#
# foldingosctl
#
################################################################################

FOLDINGOSCTL_VERSION = 0.1.0
FOLDINGOSCTL_SITE = $(TOPDIR)/../../../packages/foldingosctl
FOLDINGOSCTL_SITE_METHOD = local
FOLDINGOSCTL_LICENSE = GPL-3.0-only

# Local cargo-package builds use --offline; refresh packages/foldingosctl/VENDOR
# with `cargo vendor --locked VENDOR` when Cargo.lock changes.

$(eval $(cargo-package))
