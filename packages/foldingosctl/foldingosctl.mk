################################################################################
#
# foldingosctl
#
################################################################################

FOLDINGOSCTL_VERSION = 0.1.0
FOLDINGOSCTL_SITE = $(TOPDIR)/../../../packages/foldingosctl/src
FOLDINGOSCTL_SITE_METHOD = local
FOLDINGOSCTL_GOMOD = foldingos.local/foldingosctl
FOLDINGOSCTL_LICENSE = GPL-3.0-only
FOLDINGOSCTL_LDFLAGS = -buildid=

$(eval $(golang-package))
