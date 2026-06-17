import { useState } from "react";
import { Tabs, type TabItem } from "./Tabs";
import { ErrorsLogViewer, LogViewer } from "./LogViewer";
import type { MachineSummary } from "../types";

const LOG_TABS: TabItem[] = [
  { id: "fah", label: "FAH client" },
  { id: "work", label: "Work unit" },
  { id: "errors", label: "Errors" },
];

interface MachineLogsPanelProps {
  hostname: string;
  machine: MachineSummary | null;
}

export function MachineLogsPanel({ hostname, machine }: MachineLogsPanelProps) {
  const [logTab, setLogTab] = useState("fah");
  const online = machine?.online ?? false;
  const recentErrors =
    machine?.latest?.payload?.fah?.recentErrors ?? [];
  const cachedFah = machine?.latest?.payload?.logs?.fah ?? [];

  return (
    <Tabs
      tabs={LOG_TABS}
      active={logTab}
      onChange={setLogTab}
      className="machine-logs-tabs"
    >
      {logTab === "fah" && (
        <LogViewer hostname={hostname} source="fah" online={online} />
      )}
      {logTab === "work" && (
        <LogViewer hostname={hostname} source="work" online={online} />
      )}
      {logTab === "errors" && (
        <ErrorsLogViewer
          hostname={hostname}
          recentErrors={recentErrors}
          fahLines={cachedFah}
          online={online}
        />
      )}
    </Tabs>
  );
}
