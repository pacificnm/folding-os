import { Link, Outlet, useLocation } from "react-router-dom";
import { ADMIN_SECTIONS, buildAdminBreadcrumbs } from "../../adminBreadcrumbs";
import { PageLayout } from "../../components/PageLayout";

export function AdminLayout() {
  const location = useLocation();
  const isActive = (href: string) =>
    location.pathname === href || location.pathname.startsWith(`${href}/`);

  return (
    <PageLayout
      breadcrumbs={buildAdminBreadcrumbs(location.pathname)}
      eyebrow="Settings"
      title="Supervisor admin"
    >
      <nav className="admin-nav" aria-label="Settings sections">
        {ADMIN_SECTIONS.map((item) => (
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
