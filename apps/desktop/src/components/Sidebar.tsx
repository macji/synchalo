import { Clipboard, Files, Pause, Play, Settings } from "lucide-react";

import type { Route, SyncStatusView } from "../api/types";
import { HaloMark } from "./HaloMark";

interface SidebarProps {
  route: Route;
  status: SyncStatusView;
  onNavigate: (route: Route) => void;
  onPause: (paused: boolean) => void;
}

const items: Array<{ route: Route; label: string; icon: typeof Clipboard; shortcut: string }> = [
  { route: "clipboard", label: "粘贴板", icon: Clipboard, shortcut: "⌘1" },
  { route: "files", label: "同步文件", icon: Files, shortcut: "⌘2" },
  { route: "settings", label: "设置", icon: Settings, shortcut: "⌘," },
];

export function Sidebar({ route, status, onNavigate, onPause }: SidebarProps) {
  const paused = status.state === "paused";
  return (
    <aside className="sidebar" aria-label="SyncHalo 侧栏">
      <div className="brand-block">
        <div className="brand-lockup">
          <HaloMark />
          <span className="brand-name">SyncHalo</span>
        </div>
        <div className={`sync-indicator sync-indicator--${status.state}`}>
          <span className="status-dot" aria-hidden="true" />
          <span>{status.label}</span>
        </div>
      </div>

      <nav aria-label="主导航" className="primary-nav">
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
      <div className="device-summary" aria-label="设备连接摘要">
        <div>
          <span className="status-dot status-dot--online" aria-hidden="true" />
          <span>{status.onlineCount} 台在线</span>
        </div>
        <div>
          <span className="status-dot status-dot--offline" aria-hidden="true" />
          <span>{status.offlineCount} 台离线</span>
        </div>
      </div>
      <button className="sidebar-control" onClick={() => onPause(!paused)} type="button">
        {paused ? <Play aria-hidden="true" size={16} /> : <Pause aria-hidden="true" size={16} />}
        {paused ? "恢复同步" : "暂停同步"}
      </button>
    </aside>
  );
}
