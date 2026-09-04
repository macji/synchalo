import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../i18n";
import { PageHeader } from "./PageHeader";
import { WindowTitlebar } from "./WindowTitlebar";

const windowActions = vi.hoisted(() => ({
  close: vi.fn(() => Promise.resolve()),
  minimize: vi.fn(() => Promise.resolve()),
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

  it("leaves macOS traffic lights entirely to the native window", () => {
    const { container } = render(
      <I18nProvider>
        <WindowTitlebar platform="macos" />
      </I18nProvider>,
    );

    expect(container).toBeEmptyDOMElement();
    expect(windowActions.close).not.toHaveBeenCalled();
    expect(windowActions.minimize).not.toHaveBeenCalled();
    expect(windowActions.toggleMaximize).not.toHaveBeenCalled();
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
