import { Clipboard, Files, Pause, Play, Settings } from "lucide-react";

import type { Route, SyncStatusView } from "../api/types";
import { useI18n } from "../i18n";
import { HaloMark } from "./HaloMark";

interface SidebarProps {
  route: Route;
  status: SyncStatusView;
  onNavigate: (route: Route) => void;
  onPause: (paused: boolean) => void;
}

export function Sidebar({ route, status, onNavigate, onPause }: SidebarProps) {
  const { t } = useI18n();
  const paused = status.state === "paused";
  const items: Array<{ route: Route; label: string; icon: typeof Clipboard; shortcut: string }> = [
    { route: "clipboard", label: t("sidebar.clipboard"), icon: Clipboard, shortcut: "⌘1" },
    { route: "files", label: t("sidebar.files"), icon: Files, shortcut: "⌘2" },
    { route: "settings", label: t("sidebar.settings"), icon: Settings, shortcut: "⌘," },
  ];
  return (
    <aside className="sidebar" aria-label={t("sidebar.label")}>
      <div className="brand-block">
        <div className="brand-lockup">
          <HaloMark />
          <span className="brand-name">SyncHalo</span>
        </div>
        <div className={`sync-indicator sync-indicator--${status.state}`}>
          <span className="status-dot" aria-hidden="true" />
          <span>{t(`sync.${status.state}`)}</span>
        </div>
      </div>

      <nav aria-label={t("sidebar.primaryNavigation")} className="primary-nav">
        {items.map((item) => {
          const Icon = item.icon;
          const active = item.route === route;
          return (
            <button
              aria-current={active ? "page" : undefined}
              className={`nav-item ${active ? "is-active" : ""}`}
              key={item.route}
              onClick={() => onNavigate(item.route)}
              type="button"
            >
              <span className="nav-active-line" aria-hidden="true" />
              <Icon aria-hidden="true" size={18} strokeWidth={1.8} />
              <span>{item.label}</span>
              <kbd>{item.shortcut}</kbd>
            </button>
          );
        })}
      </nav>

      <div className="sidebar-spacer" />
      <div className="device-summary" aria-label={t("sidebar.deviceSummary")}>
        <div>
          <span className="status-dot status-dot--online" aria-hidden="true" />
          <span>{t("sidebar.onlineCount", { count: status.onlineCount })}</span>
        </div>
        <div>
          <span className="status-dot status-dot--offline" aria-hidden="true" />
          <span>{t("sidebar.offlineCount", { count: status.offlineCount })}</span>
        </div>
      </div>
      <button className="sidebar-control" onClick={() => onPause(!paused)} type="button">
        {paused ? <Play aria-hidden="true" size={16} /> : <Pause aria-hidden="true" size={16} />}
        {paused ? t("sidebar.resume") : t("sidebar.pause")}
      </button>
    </aside>
  );
}
