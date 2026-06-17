################################################################################
#
# foldingosctl-rust
#
################################################################################

FOLDINGOSCTL_RUST_VERSION = 0.1.0
FOLDINGOSCTL_RUST_SITE = $(TOPDIR)/../../../packages/foldingosctl/rust
FOLDINGOSCTL_RUST_SITE_METHOD = local
FOLDINGOSCTL_RUST_LICENSE = GPL-3.0-only

# Local cargo-package builds use --offline; refresh packages/foldingosctl/rust/VENDOR
# and .cargo/config.toml with `cargo vendor --locked VENDOR` when Cargo.lock changes.

$(eval $(cargo-package))
