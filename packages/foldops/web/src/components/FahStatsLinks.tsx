import type { MouseEvent } from "react";
import { resolveFahStats } from "../fahStats";

interface FahStatsLinksProps {
  donor?: string | null;
  team?: string | null;
  className?: string;
  /** Stop click from bubbling (e.g. inside a tile Link). */
  stopPropagation?: boolean;
  compact?: boolean;
}

export function FahStatsLinks({
  donor,
  team,
  className = "",
  stopPropagation = false,
  compact = false,
}: FahStatsLinksProps) {
  const stats = resolveFahStats(donor, team);
  if (!stats.donorUrl && !stats.teamUrl) return null;

  const onClick = stopPropagation
    ? (e: MouseEvent) => e.stopPropagation()
    : undefined;

  return (
    <span
      className={`fah-stats-links${compact ? " fah-stats-links--compact" : ""}${className ? ` ${className}` : ""}`}
      onClick={onClick}
      onKeyDown={stopPropagation ? (e) => e.stopPropagation() : undefined}
    >
      {stats.donorUrl && (
        <a
          href={stats.donorUrl}
          target="_blank"
          rel="noopener noreferrer"
          title={`FAH donor stats: ${stats.donor}`}
        >
          {compact ? "Donor" : "Donor stats"}
        </a>
      )}
      {stats.donorUrl && stats.teamUrl && (
        <span className="fah-stats-sep">·</span>
      )}
      {stats.teamUrl && (
        <a
          href={stats.teamUrl}
          target="_blank"
          rel="noopener noreferrer"
          title={`FAH team stats: ${stats.team}`}
        >
          {compact ? "Team" : "Team stats"}
        </a>
      )}
    </span>
  );
}
