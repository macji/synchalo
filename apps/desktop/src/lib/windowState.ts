import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

import type { DevicePlatform } from "../api/types";

export function useLinuxWindowMaximized(platform: DevicePlatform): boolean {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (platform !== "linux" || !("__TAURI_INTERNALS__" in window)) return;

    const appWindow = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const refresh = async () => {
      try {
        const nextMaximized = await appWindow.isMaximized();
        if (!disposed) setMaximized(nextMaximized);
      } catch {
        // Keep the windowed presentation if the compositor cannot report this state.
      }
    };

    void refresh();
    void appWindow.onResized(() => {
      void refresh();
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    }).catch(() => {
      // The initial state still provides a safe fallback when resize events are unavailable.
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [platform]);

  return maximized;
}
