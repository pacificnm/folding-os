import type { ReactNode } from "react";

export interface TabItem {
  id: string;
  label: string;
}

interface TabsProps {
  tabs: TabItem[];
  active: string;
  onChange: (id: string) => void;
  children: ReactNode;
  className?: string;
}

export function Tabs({ tabs, active, onChange, children, className = "" }: TabsProps) {
  return (
    <div className={`tabs${className ? ` ${className}` : ""}`}>
      <div className="tabs-bar" role="tablist">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={active === tab.id}
            className={`tabs-btn ${active === tab.id ? "active" : ""}`}
            onClick={() => onChange(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div className="tabs-panel" role="tabpanel">
        {children}
      </div>
    </div>
  );
}
