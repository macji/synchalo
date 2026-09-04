import { Maximize2, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";

import type { DevicePlatform } from "../api/types";
import { useI18n } from "../i18n";
import { runWindowAction } from "../lib/windowControls";

interface WindowTitlebarProps {
  platform: DevicePlatform;
}

export function WindowTitlebar({ platform }: WindowTitlebarProps) {
  const { t } = useI18n();
  const isMac = platform === "macos";

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    const updateMaximizedState = async () => {
      const maximized = "__TAURI_INTERNALS__" in window
        ? await getCurrentWindow().isMaximized()
        : false;
      if (alive) document.documentElement.dataset.windowMaximized = String(maximized);
    };
    void updateMaximizedState();
    if ("__TAURI_INTERNALS__" in window) {
      void getCurrentWindow().onResized(() => void updateMaximizedState())
        .then((listener) => {
          if (alive) unlisten = listener;
          else listener();
        });
    }
    return () => {
      alive = false;
      unlisten?.();
      delete document.documentElement.dataset.windowMaximized;
    };
  }, []);

  return (
    <div className="window-controls-layer" data-platform={isMac ? "macos" : "standard"}>
      <div className="window-drag-region window-drag-region--sidebar" data-tauri-drag-region>
        {isMac ? (
          <div className="window-controls window-controls--mac">
            <button
              aria-label={t("window.close")}
              className="traffic-light traffic-light--close"
              onClick={() => void runWindowAction("close")}
              title={t("window.close")}
              type="button"
            >
              <X aria-hidden="true" size={9} strokeWidth={3} />
            </button>
            <button
              aria-label={t("window.minimize")}
              className="traffic-light traffic-light--minimize"
              onClick={() => void runWindowAction("minimize")}
              title={t("window.minimize")}
              type="button"
            >
              <Minus aria-hidden="true" size={9} strokeWidth={3} />
            </button>
            <button
              aria-label={t("window.maximize")}
              className="traffic-light traffic-light--maximize"
              onClick={() => void runWindowAction("maximize")}
              title={t("window.maximize")}
              type="button"
            >
              <Maximize2 aria-hidden="true" size={8} strokeWidth={3} />
            </button>
          </div>
        ) : null}
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
  );
}
