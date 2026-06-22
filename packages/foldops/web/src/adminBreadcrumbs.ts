export interface BreadcrumbItem {
  label: string;
  href?: string;
}

export const ADMIN_SECTIONS = [
  { href: "/admin/folding", label: "Folding@Home", detailPrefix: "/admin/folding/" },
  { href: "/admin/machines", label: "Network install" },
  { href: "/admin/software", label: "Software updates" },
  { href: "/admin/services", label: "Services" },
  { href: "/admin/logs", label: "Logs" },
  { href: "/admin/recovery", label: "Backup" },
  { href: "/admin/settings/alerts", label: "Alert settings" },
] as const;

function decodePathSegment(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function buildAdminBreadcrumbs(pathname: string): BreadcrumbItem[] {
  const crumbs: BreadcrumbItem[] = [
    { label: "Farm dashboard", href: "/dashboard" },
  ];

  if (pathname === "/admin" || pathname === "/admin/") {
    crumbs.push({ label: "Admin" });
    return crumbs;
  }

  crumbs.push({ label: "Admin", href: "/admin" });

  for (const section of ADMIN_SECTIONS) {
    if ("detailPrefix" in section && section.detailPrefix) {
      if (pathname.startsWith(section.detailPrefix)) {
        const detailId = pathname.slice(section.detailPrefix.length);
        if (detailId && !detailId.includes("/")) {
          crumbs.push({ label: section.label, href: section.href });
          crumbs.push({ label: decodePathSegment(detailId) });
          return crumbs;
        }
      }
    }

    if (pathname === section.href || pathname.startsWith(`${section.href}/`)) {
      crumbs.push({ label: section.label });
      return crumbs;
    }
  }

  crumbs.push({ label: "Admin" });
  return crumbs;
}

export function buildDashboardBreadcrumbs(): BreadcrumbItem[] {
  return [
    { label: "Kiosk", href: "/" },
    { label: "Farm dashboard" },
  ];
}

export function buildAlertHistoryBreadcrumbs(): BreadcrumbItem[] {
  return [
    { label: "Farm dashboard", href: "/dashboard" },
    { label: "Alert history" },
  ];
}

export function buildMachineDetailBreadcrumbs(hostname: string): BreadcrumbItem[] {
  return [
    { label: "Farm dashboard", href: "/dashboard" },
    { label: hostname || "Node" },
  ];
}
