import type { ActiveAlert } from "../types";

interface AlertBannerProps {
  alerts: ActiveAlert[];
  className?: string;
}

export function AlertBanner({ alerts, className = "" }: AlertBannerProps) {
  if (alerts.length === 0) return null;

  const critical = alerts.filter((a) => a.severity === "critical");
  const warning = alerts.filter(
    (a) => a.severity === "warning" && !critical.some((c) => c.id === a.id),
  );
  const shown = [...critical, ...warning].slice(0, 4);
  const extra = alerts.length - shown.length;
  const tone =
    critical.length > 0 ? "critical" : warning.length > 0 ? "warning" : "info";

  return (
    <div
      className={`alert-banner alert-banner--${tone}${className ? ` ${className}` : ""}`}
      role="alert"
    >
      <p className="alert-banner-title">
        {alerts.length} active alert{alerts.length === 1 ? "" : "s"}
      </p>
      <ul className="alert-banner-list">
        {shown.map((a) => (
          <li key={a.id} className={`alert-banner-item alert-banner-item--${a.severity}`}>
            {a.message}
          </li>
        ))}
        {extra > 0 && (
          <li className="alert-banner-item alert-banner-item--more">
            +{extra} more
          </li>
        )}
      </ul>
    </div>
  );
}
