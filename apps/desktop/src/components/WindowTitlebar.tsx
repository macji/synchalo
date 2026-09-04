import { Minus, Square, X } from "lucide-react";

import type { DevicePlatform } from "../api/types";
import { useI18n } from "../i18n";
import { runWindowAction } from "../lib/windowControls";

interface WindowTitlebarProps {
  platform: DevicePlatform;
}

export function WindowTitlebar({ platform }: WindowTitlebarProps) {
  const { t } = useI18n();
  if (platform === "macos") return null;

  return (
    <div className="window-controls-layer" data-platform="standard">
      <div className="window-drag-region window-drag-region--sidebar" data-tauri-drag-region>
      </div>

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
    </div>
  );
}
