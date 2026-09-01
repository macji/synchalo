import type { ReactNode } from "react";

interface PageHeaderProps {
  title: string;
  eyebrow?: string;
  actions?: ReactNode;
}
export function PageHeader({ title, eyebrow, actions }: PageHeaderProps) {
  return (
    <header className="page-header">
      <div>
        {eyebrow ? <p className="page-eyebrow">{eyebrow}</p> : null}
        <h1>{title}</h1>
      </div>
      {actions ? <div className="page-actions">{actions}</div> : null}
    </header>
  );
}
