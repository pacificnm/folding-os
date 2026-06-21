import { Link, Outlet, useLocation } from "react-router-dom";
import { PageLayout } from "../../components/PageLayout";

const NAV = [
  { href: "/admin/folding", label: "Folding@Home" },
  { href: "/admin/machines", label: "Network Install" },
  { href: "/admin/software", label: "Software Updates" },
  { href: "/admin/services", label: "Services" },
  { href: "/admin/logs", label: "Logs" },
  { href: "/admin/recovery", label: "Backup" },
  { href: "/admin/settings/alerts", label: "Alerts" },
] as const;

export function AdminLayout() {
  const location = useLocation();
  const isActive = (href: string) =>
    location.pathname === href || location.pathname.startsWith(`${href}/`);

  return (
    <PageLayout
      backLink={{ href: "/dashboard", label: "← Farm dashboard" }}
      eyebrow="Settings"
      title="Supervisor admin"
      footer={
        <>
          <Link to="/dashboard">Farm dashboard</Link>
        </>
      }
    >
      <nav className="admin-nav" aria-label="Settings sections">
        {NAV.map((item) => (
          <Link
            key={item.href}
            to={item.href}
            className={
              isActive(item.href)
                ? "admin-nav-link admin-nav-link--active"
                : "admin-nav-link"
            }
          >
            {item.label}
          </Link>
        ))}
      </nav>
      <Outlet />
    </PageLayout>
  );
}
