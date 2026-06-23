import { Link } from "react-router-dom";
import type { ReactNode } from "react";
import { SITE_NAV_LINKS } from "../siteLinks";

interface SiteFooterProps {
  note?: ReactNode;
  compact?: boolean;
}

function FooterSeparator() {
  return <span className="site-footer-sep" aria-hidden="true">·</span>;
}

export function SiteFooter({ note, compact = false }: SiteFooterProps) {
  return (
    <footer
      className={`footer site-footer${compact ? " site-footer--compact" : ""}`}
    >
      <nav className="site-footer-nav" aria-label="FoldOps navigation">
        {SITE_NAV_LINKS.map((link, index) => (
          <span key={link.href} className="site-footer-link-wrap">
            {index > 0 && <FooterSeparator />}
            <Link to={link.href}>{link.label}</Link>
          </span>
        ))}
      </nav>

      {note && <p className="site-footer-note">{note}</p>}
    </footer>
  );
}
