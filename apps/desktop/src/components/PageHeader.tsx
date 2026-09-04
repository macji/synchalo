import type { ReactNode } from "react";

import { toggleWindowMaximize } from "../lib/windowControls";

interface PageHeaderProps {
  title: string;
  eyebrow?: string;
  actions?: ReactNode;
}
export function PageHeader({ title, eyebrow, actions }: PageHeaderProps) {
  return (
    <header
      className="page-header"
      data-tauri-drag-region
      onDoubleClick={(event) => {
        if ((event.target as HTMLElement).closest("button, input, select, label")) return;
        void toggleWindowMaximize();
      }}
    >
      <div data-tauri-drag-region>
        {eyebrow ? <p className="page-eyebrow">{eyebrow}</p> : null}
        <h1 data-tauri-drag-region>{title}</h1>
      </div>
      {actions ? <div className="page-actions">{actions}</div> : null}
    </header>
  );
}
