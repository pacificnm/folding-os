import type { ReactNode } from "react";
import type { BreadcrumbItem } from "../adminBreadcrumbs";
import { Breadcrumbs } from "./Breadcrumbs";
import { SiteFooter } from "./SiteFooter";

interface PageLayoutProps {
  eyebrow: string;
  title: string;
  badge?: ReactNode;
  headerAside?: ReactNode;
  breadcrumbs?: BreadcrumbItem[];
  footerNote?: ReactNode;
  children: ReactNode;
}

export function PageLayout({
  eyebrow,
  title,
  badge,
  headerAside,
  breadcrumbs = [],
  footerNote,
  children,
}: PageLayoutProps) {
  return (
    <div className="page-shell">
      <div className="app">
        <Breadcrumbs items={breadcrumbs} />

        <header className="page-header">
          <div className="page-header-main">
            <p className="eyebrow">{eyebrow}</p>
            <div className="page-title-row">
              <h1>{title}</h1>
              {badge}
            </div>
          </div>
          {headerAside && <div className="page-header-aside">{headerAside}</div>}
        </header>

        <main className="page-main">{children}</main>

        <SiteFooter note={footerNote} />
      </div>
    </div>
  );
}
