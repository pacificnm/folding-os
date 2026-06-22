import { Link } from "react-router-dom";
import type { BreadcrumbItem } from "../adminBreadcrumbs";

interface BreadcrumbsProps {
  items: BreadcrumbItem[];
}

export function Breadcrumbs({ items }: BreadcrumbsProps) {
  if (items.length === 0) {
    return <span aria-hidden="true">&nbsp;</span>;
  }

  return (
    <nav className="breadcrumb" aria-label="Breadcrumb">
      <ol className="breadcrumb-list">
        {items.map((item, index) => {
          const isLast = index === items.length - 1;
          const showLink = Boolean(item.href) && !isLast;

          return (
            <li key={`${item.label}-${index}`} className="breadcrumb-item">
              {index > 0 && (
                <span className="breadcrumb-sep" aria-hidden="true">
                  /
                </span>
              )}
              {showLink ? (
                <Link to={item.href!}>{item.label}</Link>
              ) : (
                <span
                  className="breadcrumb-current"
                  aria-current={isLast ? "page" : undefined}
                >
                  {item.label}
                </span>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
