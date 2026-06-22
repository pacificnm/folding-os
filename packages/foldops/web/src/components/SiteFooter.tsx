import { Link } from "react-router-dom";
import type { ReactNode } from "react";
import {
  ADMIN_NAV_LINKS,
  EXTERNAL_SITE_LINKS,
  SITE_NAV_LINKS,
} from "../siteLinks";

interface SiteFooterProps {
  note?: ReactNode;
  compact?: boolean;
}

function FooterSeparator() {
  return <span className="site-footer-sep" aria-hidden="true">·</span>;
}

function FooterNav({
  ariaLabel,
  children,
}: {
  ariaLabel: string;
  children: ReactNode;
}) {
  return (
    <nav className="site-footer-nav" aria-label={ariaLabel}>
      {children}
    </nav>
  );
}

function InternalLinks({
  links,
}: {
  links: readonly { href: string; label: string }[];
}) {
  return links.map((link, index) => (
    <span key={link.href} className="site-footer-link-wrap">
      {index > 0 && <FooterSeparator />}
      <Link to={link.href}>{link.label}</Link>
    </span>
  ));
}

function ExternalLinks({
  links,
}: {
  links: readonly { href: string; label: string }[];
}) {
  return links.map((link, index) => (
    <span key={link.href} className="site-footer-link-wrap">
      {index > 0 && <FooterSeparator />}
      <a href={link.href} target="_blank" rel="noopener noreferrer">
        {link.label}
      </a>
    </span>
  ));
}

export function SiteFooter({ note, compact = false }: SiteFooterProps) {
  return (
    <footer
      className={`footer site-footer${compact ? " site-footer--compact" : ""}`}
    >
      <p className="site-footer-brand">FoldOps · Folding@home farm monitor</p>

      <FooterNav ariaLabel="FoldOps navigation">
        <InternalLinks links={SITE_NAV_LINKS} />
      </FooterNav>

      {!compact && (
        <FooterNav ariaLabel="Admin sections">
          <InternalLinks links={ADMIN_NAV_LINKS} />
        </FooterNav>
      )}

      <FooterNav ariaLabel="Project links">
        <ExternalLinks links={EXTERNAL_SITE_LINKS} />
      </FooterNav>

      {note && <p className="site-footer-note">{note}</p>}
    </footer>
  );
}
