import { Link } from "react-router-dom";
import type { ReactNode } from "react";

interface PageLayoutProps {
  eyebrow: string;
  title: string;
  badge?: ReactNode;
  headerAside?: ReactNode;
  backLink?: { href: string; label: string };
  footer?: ReactNode;
  children: ReactNode;
}

export function PageLayout({
  eyebrow,
  title,
  badge,
  headerAside,
  backLink,
  footer = "FoldOps · Folding@home farm monitor",
  children,
}: PageLayoutProps) {
  return (
    <div className="page-shell">
      <div className="app">
        <nav className="breadcrumb" aria-label="Breadcrumb">
          {backLink ? (
            <Link to={backLink.href}>{backLink.label}</Link>
          ) : (
            <span aria-hidden="true">&nbsp;</span>
          )}
        </nav>

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

        <footer className="footer">{footer}</footer>
      </div>
    </div>
  );
}
