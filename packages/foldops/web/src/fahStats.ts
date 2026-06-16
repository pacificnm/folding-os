export function fahDonorStatsUrl(donor: string): string {
  return `https://stats.foldingathome.org/donor/${encodeURIComponent(donor.trim())}`;
}

export function fahTeamStatsUrl(team: string): string {
  return `https://stats.foldingathome.org/team/${encodeURIComponent(team.trim())}`;
}

export interface FahStatsLinks {
  donor: string | null;
  team: string | null;
  donorUrl: string | null;
  teamUrl: string | null;
}

export function resolveFahStats(
  donor: string | null | undefined,
  team: string | null | undefined,
): FahStatsLinks {
  const d = donor?.trim() || null;
  const t = team?.trim() || null;
  return {
    donor: d,
    team: t,
    donorUrl: d ? fahDonorStatsUrl(d) : null,
    teamUrl: t ? fahTeamStatsUrl(t) : null,
  };
}
