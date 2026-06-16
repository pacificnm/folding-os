import type { FahProjectInfo } from "../types";

interface ProjectInfoPanelProps {
  projectId: string;
  run: number | null;
  clone: number | null;
  gen: number | null;
  info: FahProjectInfo | null;
  loading: boolean;
  error: string | null;
}

export function ProjectInfoPanel({
  projectId,
  run,
  clone,
  gen,
  info,
  loading,
  error,
}: ProjectInfoPanelProps) {
  const wuLabel =
    run != null
      ? `Run ${run} · Clone ${clone ?? "?"} · Gen ${gen ?? "?"}`
      : null;

  return (
    <section className="project-panel" aria-labelledby="project-panel-title">
      <div className="project-panel-head">
        <div>
          <h2 id="project-panel-title" className="project-panel-title">
            Project {projectId}
          </h2>
          {wuLabel && <p className="project-panel-wu mono">{wuLabel}</p>}
        </div>
        {info?.statsUrl && (
          <a
            href={info.statsUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="project-panel-link"
          >
            FAH stats ↗
          </a>
        )}
      </div>

      {loading && <p className="project-panel-message">Loading project info…</p>}
      {error && !loading && (
        <p className="project-panel-message project-panel-message--error">
          {error}
        </p>
      )}

      {!loading && !error && info && (
        <>
          <dl className="project-panel-meta">
            {info.cause && (
              <div>
                <dt>Cause</dt>
                <dd>{info.cause}</dd>
              </div>
            )}
            {info.manager && (
              <div>
                <dt>Researcher</dt>
                <dd>{info.manager}</dd>
              </div>
            )}
            {info.institution && (
              <div>
                <dt>Institution</dt>
                <dd>{info.institution}</dd>
              </div>
            )}
            {info.projectRange && (
              <div>
                <dt>Project range</dt>
                <dd className="mono">{info.projectRange}</dd>
              </div>
            )}
          </dl>
          {info.description && (
            <p className="project-panel-description">{info.description}</p>
          )}
        </>
      )}

      {!loading && !error && !info && (
        <p className="project-panel-message">
          Could not load details for project {projectId} from Folding@home.
          If the supervisor was recently updated, restart{" "}
          <span className="mono">foldops-supervisor</span> so{" "}
          <span className="mono">/api/projects</span> is available.
        </p>
      )}
    </section>
  );
}
