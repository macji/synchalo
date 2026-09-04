import { getCurrentWindow } from "@tauri-apps/api/window";

export type WindowAction = "close" | "maximize" | "minimize";

export async function runWindowAction(action: WindowAction) {
  if (!("__TAURI_INTERNALS__" in window)) return;
  const appWindow = getCurrentWindow();
  if (action === "close") await appWindow.close();
  else if (action === "minimize") await appWindow.minimize();
  else await appWindow.toggleMaximize();
}

export function toggleWindowMaximize() {
  if (navigator.userAgent.toLowerCase().includes("mac")) return Promise.resolve();
  return runWindowAction("maximize");
}
