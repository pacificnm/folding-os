export interface FahConfigSnapshot {
  configUsername?: string | null;
  configTeam?: number | null;
  configPasskeyConfigured?: boolean | null;
  configCpus?: number | null;
  effectiveCpus?: number | null;
  statsDonor?: string | null;
  statsTeam?: string | null;
}

export function isFahConfigured(fah: FahConfigSnapshot | null | undefined): boolean {
  if (!fah) {
    return false;
  }
  if (fah.configPasskeyConfigured) {
    return true;
  }
  const username = fah.configUsername?.trim();
  if (username && username !== "Anonymous") {
    return true;
  }
  if (fah.configTeam != null && fah.configTeam !== 0) {
    return true;
  }
  return false;
}

export function displayConfiguredDonor(
  fah: FahConfigSnapshot | null | undefined,
): string {
  const username = fah?.configUsername?.trim();
  if (username && username !== "Anonymous") {
    return username;
  }
  const statsDonor = fah?.statsDonor?.trim();
  if (statsDonor) {
    return statsDonor;
  }
  return "—";
}

export function displayConfiguredTeam(
  fah: FahConfigSnapshot | null | undefined,
): string {
  if (fah?.configTeam != null) {
    return String(fah.configTeam);
  }
  if (fah?.statsTeam != null) {
    return String(fah.statsTeam);
  }
  return "—";
}

export function displayConfiguredToken(
  fah: FahConfigSnapshot | null | undefined,
): string {
  if (fah?.configPasskeyConfigured) {
    return "Set";
  }
  return "—";
}

export function displayConfiguredCpus(
  fah: FahConfigSnapshot | null | undefined,
): string {
  if (fah?.configCpus == null) {
    return "—";
  }
  if (fah.configCpus === 0) {
    return "Automatic";
  }
  if (fah.configCpus > 0) {
    return String(fah.configCpus);
  }
  return "—";
}

export function displayEffectiveCpus(
  fah: FahConfigSnapshot | null | undefined,
): string {
  if (fah?.effectiveCpus != null && fah.effectiveCpus > 0) {
    return String(fah.effectiveCpus);
  }
  if (fah?.configCpus != null && fah.configCpus > 0) {
    return String(fah.configCpus);
  }
  return "—";
}

export function fahCpuPolicyDrift(
  fah: FahConfigSnapshot | null | undefined,
): boolean {
  const configured = fah?.configCpus;
  const effective = fah?.effectiveCpus;
  if (configured == null || effective == null || configured <= 0 || effective <= 0) {
    return false;
  }
  return configured !== effective;
}
