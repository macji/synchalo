import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useLinuxWindowMaximized } from "./windowState";

const windowApi = vi.hoisted(() => ({
  isMaximized: vi.fn<() => Promise<boolean>>(),
  onResized: vi.fn<(handler: () => void) => Promise<() => void>>(),
  resizedHandler: null as (() => void) | null,
  unlisten: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: windowApi.isMaximized,
    onResized: windowApi.onResized,
  }),
}));

describe("useLinuxWindowMaximized", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    windowApi.resizedHandler = null;
    windowApi.isMaximized.mockResolvedValue(false);
    windowApi.onResized.mockImplementation(async (handler) => {
      windowApi.resizedHandler = handler;
      return windowApi.unlisten;
    });
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
  });

  afterEach(() => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("tracks compositor maximize changes on Linux", async () => {
    windowApi.isMaximized.mockResolvedValueOnce(true);
    const { result, unmount } = renderHook(() => useLinuxWindowMaximized("linux"));

    await waitFor(() => expect(result.current).toBe(true));
    expect(windowApi.onResized).toHaveBeenCalledOnce();

    windowApi.isMaximized.mockResolvedValueOnce(false);
    act(() => windowApi.resizedHandler?.());
    await waitFor(() => expect(result.current).toBe(false));

    unmount();
    expect(windowApi.unlisten).toHaveBeenCalledOnce();
  });

  it("does not subscribe to native window state on macOS", () => {
    const { result } = renderHook(() => useLinuxWindowMaximized("macos"));

    expect(result.current).toBe(false);
    expect(windowApi.isMaximized).not.toHaveBeenCalled();
    expect(windowApi.onResized).not.toHaveBeenCalled();
  });
});
