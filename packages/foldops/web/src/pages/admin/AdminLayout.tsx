import { Link, Outlet, useLocation } from "react-router-dom";
import { PageLayout } from "../../components/PageLayout";

const NAV = [
  { href: "/admin/machines", label: "Network install" },
  { href: "/admin/folding", label: "Folding@home" },
  { href: "/admin/services", label: "Services" },
  { href: "/admin/logs", label: "Logs" },
  { href: "/admin/software", label: "Software updates" },
  { href: "/admin/recovery", label: "Backup" },
] as const;

export function AdminLayout() {
  const location = useLocation();

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
              location.pathname === item.href
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
