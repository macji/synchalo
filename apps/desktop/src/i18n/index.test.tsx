import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "../App";
import { api } from "../api/client";
import type { LanguagePreference } from "../api/types";
import { I18nProvider, resolveLocale } from ".";

describe("internationalization", () => {
  beforeEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.spyOn(window.navigator, "language", "get").mockReturnValue("zh-CN");
    vi.spyOn(window.navigator, "languages", "get").mockReturnValue(["zh-CN"]);
  });

  it("maps supported system locales and falls back to English", () => {
    expect(resolveLocale("system", ["zh-Hans-CN"])).toBe("zh-cn");
    expect(resolveLocale("system", ["zh-Hant-TW"])).toBe("zh-tw");
    expect(resolveLocale("system", ["ja-JP"])).toBe("ja");
    expect(resolveLocale("system", ["ko-KR"])).toBe("ko");
    expect(resolveLocale("system", ["fr-FR", "en-GB"])).toBe("en");
    expect(resolveLocale("system", ["fr-FR"])).toBe("en");
  });

  it.each([
    ["en", "Settings"],
    ["zh-cn", "设置"],
    ["zh-tw", "設定"],
    ["ja", "設定"],
    ["ko", "설정"],
  ] satisfies Array<[LanguagePreference, string]>) (
    "renders the %s locale from the persisted setting",
    async (language, settingsLabel) => {
      vi.spyOn(api, "getAppState").mockImplementation(async () => {
        const snapshot = await vi.importActual<typeof import("../api/mock")>("../api/mock");
        return structuredClone({ ...snapshot.mockSnapshot, settings: { ...snapshot.mockSnapshot.settings, language } });
      });

      render(
        <I18nProvider>
          <App />
        </I18nProvider>,
      );

      expect(await screen.findByRole("button", { name: new RegExp(settingsLabel) })).toBeInTheDocument();
      await waitFor(() => expect(document.documentElement.lang).toBe(language));
    },
  );

  it("switches language immediately and sends the preference to settings persistence", async () => {
    const getAppState = vi.spyOn(api, "getAppState");
    const update = vi.spyOn(api, "updateSettings");
    render(
      <I18nProvider>
        <App />
      </I18nProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: /设置/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "界面语言" }), {
      target: { value: "en" },
    });

    await waitFor(() => expect(update).toHaveBeenCalledWith({ language: "en" }));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
    expect(getAppState).toHaveBeenCalledOnce();
  });
});
