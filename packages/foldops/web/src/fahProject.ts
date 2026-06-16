import type { FahProjectInfo } from "./types";

function htmlToPlain(html: string): string {
  return html
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&#39;/g, "'")
    .replace(/&quot;/g, '"')
    .replace(/&amp;/g, "&")
    .replace(/&nbsp;/g, " ")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function pickString(value: unknown): string | null {
  if (typeof value === "number" && !Number.isNaN(value)) {
    return String(value);
  }
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

/** Normalize raw FAH API JSON (same shape as supervisor proxy). */
export function normalizeFahProject(
  raw: Record<string, unknown>,
  projectId: number,
): FahProjectInfo {
  const descriptionHtml = pickString(raw.description) ?? pickString(raw.mdescription);
  return {
    project: projectId,
    manager: pickString(raw.manager),
    cause: pickString(raw.cause),
    institution: pickString(raw.institution),
    description: descriptionHtml ? htmlToPlain(descriptionHtml) : null,
    projectRange: pickString(raw.projects),
    modified: pickString(raw.modified),
    statsUrl: `https://stats.foldingathome.org/project/${projectId}`,
  };
}

export function hasProjectDetails(info: FahProjectInfo): boolean {
  return Boolean(
    info.cause ||
      info.manager ||
      info.institution ||
      info.description ||
      info.projectRange,
  );
}
