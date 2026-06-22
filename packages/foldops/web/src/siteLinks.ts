export const FOLDINGOS_REPO_URL = "https://github.com/pacificnm/folding-os";
export const FOLDINGOS_WIKI_URL = "https://github.com/pacificnm/folding-os/wiki";
export const FOLDING_AT_HOME_URL = "https://foldingathome.org";

export const SITE_NAV_LINKS = [
  { href: "/", label: "Kiosk" },
  { href: "/dashboard", label: "Farm dashboard" },
  { href: "/admin", label: "Admin" },
  { href: "/alerts", label: "Alerts" },
] as const;

export const ADMIN_NAV_LINKS = [
  { href: "/admin/folding", label: "Folding@Home" },
  { href: "/admin/machines", label: "Network install" },
  { href: "/admin/software", label: "Software updates" },
  { href: "/admin/services", label: "Services" },
  { href: "/admin/recovery", label: "Backup" },
  { href: "/admin/settings/alerts", label: "Alert settings" },
] as const;

export const EXTERNAL_SITE_LINKS = [
  { href: FOLDINGOS_REPO_URL, label: "FoldingOS" },
  { href: FOLDING_AT_HOME_URL, label: "Folding@Home" },
  { href: FOLDINGOS_WIKI_URL, label: "Manual" },
] as const;
