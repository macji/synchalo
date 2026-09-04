import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../i18n";
import { PageHeader } from "./PageHeader";
import { WindowTitlebar } from "./WindowTitlebar";

const windowActions = vi.hoisted(() => ({
  close: vi.fn(() => Promise.resolve()),
  isMaximized: vi.fn(() => Promise.resolve(false)),
  minimize: vi.fn(() => Promise.resolve()),
  onResized: vi.fn(() => Promise.resolve(vi.fn())),
  toggleMaximize: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowActions,
}));

describe("WindowTitlebar", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
  });

  afterEach(() => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("renders macOS traffic lights and invokes each native window action", async () => {
    const { container } = render(
      <I18nProvider>
        <WindowTitlebar platform="macos" />
      </I18nProvider>,
    );

    expect(container.querySelector(".window-controls--mac")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close window" }));
    fireEvent.click(screen.getByRole("button", { name: "Minimize window" }));
    fireEvent.click(screen.getByRole("button", { name: "Maximize or restore window" }));

    await waitFor(() => {
      expect(windowActions.close).toHaveBeenCalledOnce();
      expect(windowActions.minimize).toHaveBeenCalledOnce();
      expect(windowActions.toggleMaximize).toHaveBeenCalledOnce();
    });
  });

  it("uses standard custom controls on Linux and supports titlebar double-click", async () => {
    const { container } = render(
      <I18nProvider>
        <WindowTitlebar platform="linux" />
        <PageHeader title="Settings" />
      </I18nProvider>,
    );

    expect(container.querySelector(".window-controls--standard")).toBeInTheDocument();
    expect(container.querySelector(".traffic-light")).not.toBeInTheDocument();
    fireEvent.doubleClick(screen.getByRole("heading", { name: "Settings" }));
    await waitFor(() => expect(windowActions.toggleMaximize).toHaveBeenCalledOnce());
  });
});
