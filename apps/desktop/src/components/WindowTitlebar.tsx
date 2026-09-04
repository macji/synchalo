import { Clipboard, Files, Minus, Settings, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseEvent } from "react";

import type { DevicePlatform, Route } from "../api/types";
import { useI18n } from "../i18n";

interface WindowTitlebarProps {
  platform: DevicePlatform;
  route: Route;
}

export function WindowTitlebar({ platform, route }: WindowTitlebarProps) {
  const { t } = useI18n();
  const routeMeta = {
    clipboard: { icon: Clipboard, label: t("sidebar.clipboard") },
    files: { icon: Files, label: t("sidebar.files") },
    settings: { icon: Settings, label: t("sidebar.settings") },
  }[route];
  const RouteIcon = routeMeta.icon;
  const isMac = platform === "macos";

  return (
    <header className="window-titlebar" data-platform={isMac ? "macos" : "standard"}>
      <div className="window-titlebar-leading" data-tauri-drag-region>
        {isMac ? (
          <div className="window-controls window-controls--mac">
            <button
              aria-label={t("window.close")}
              className="traffic-light traffic-light--close"
              onClick={() => void runWindowAction("close")}
              title={t("window.close")}
              type="button"
            >
              <X aria-hidden="true" size={8} strokeWidth={2.6} />
            </button>
            <button
              aria-label={t("window.minimize")}
              className="traffic-light traffic-light--minimize"
              onClick={() => void runWindowAction("minimize")}
              title={t("window.minimize")}
              type="button"
            >
              <Minus aria-hidden="true" size={8} strokeWidth={2.6} />
            </button>
            <button
              aria-label={t("window.maximize")}
              className="traffic-light traffic-light--maximize"
              onClick={() => void runWindowAction("maximize")}
              title={t("window.maximize")}
              type="button"
            >
              <span aria-hidden="true" className="traffic-light-expand" />
            </button>
          </div>
        ) : null}
      </div>

      <div
        className="window-titlebar-main"
        data-tauri-drag-region
        onDoubleClick={handleTitlebarDoubleClick}
      >
        <div className="window-route-title" data-tauri-drag-region>
          <RouteIcon aria-hidden="true" size={16} strokeWidth={1.8} />
          <span data-tauri-drag-region>{routeMeta.label}</span>
        </div>

        {!isMac ? (
          <div className="window-controls window-controls--standard">
            <button
              aria-label={t("window.minimize")}
              onClick={() => void runWindowAction("minimize")}
              title={t("window.minimize")}
              type="button"
            >
              <Minus aria-hidden="true" size={15} />
            </button>
            <button
              aria-label={t("window.maximize")}
              onClick={() => void runWindowAction("maximize")}
              title={t("window.maximize")}
              type="button"
            >
              <Square aria-hidden="true" size={12} />
            </button>
            <button
              aria-label={t("window.close")}
              className="window-control-close"
              onClick={() => void runWindowAction("close")}
              title={t("window.close")}
              type="button"
            >
              <X aria-hidden="true" size={15} />
            </button>
          </div>
        ) : null}
      </div>
    </header>
  );
}

function handleTitlebarDoubleClick(event: MouseEvent<HTMLDivElement>) {
  if ((event.target as HTMLElement).closest("button")) return;
  void runWindowAction("maximize");
}

async function runWindowAction(action: "close" | "maximize" | "minimize") {
  if (!("__TAURI_INTERNALS__" in window)) return;
  const appWindow = getCurrentWindow();
  if (action === "close") await appWindow.close();
  else if (action === "minimize") await appWindow.minimize();
  else await appWindow.toggleMaximize();
}
