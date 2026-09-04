import { createContext, useContext } from "react";

import type { LanguagePreference, UserFacingError } from "../api/types";
import { en, ja, ko, zhCN, zhTW } from "./messages";
import type { MessageKey, Messages } from "./messages";

export type SupportedLocale = Exclude<LanguagePreference, "system">;
export type TranslationValues = Record<string, string | number>;
export type Translate = (key: MessageKey, values?: TranslationValues) => string;

export interface I18nContextValue {
  locale: SupportedLocale;
  preference: LanguagePreference;
  setPreference: (preference: LanguagePreference) => void;
  t: Translate;
}

export const messageSets: Record<SupportedLocale, Messages> = {
  en,
  "zh-cn": zhCN,
  "zh-tw": zhTW,
  ja,
  ko,
};

export const I18nContext = createContext<I18nContextValue | null>(null);

export function useI18n(): I18nContextValue {
  const value = useContext(I18nContext);
  if (!value) throw new Error("useI18n must be used inside I18nProvider");
  return value;
}

export function resolveLocale(
  preference: LanguagePreference,
  systemLanguages: readonly string[] = browserLanguages(),
): SupportedLocale {
  if (preference !== "system") return preference;
  for (const language of systemLanguages) {
    const normalized = language.toLowerCase().replace("_", "-");
    if (normalized.startsWith("zh")) {
      if (
        normalized.includes("hant") ||
        normalized.includes("-tw") ||
        normalized.includes("-hk") ||
        normalized.includes("-mo")
      ) {
        return "zh-tw";
      }
      return "zh-cn";
    }
    if (normalized.startsWith("ja")) return "ja";
    if (normalized.startsWith("ko")) return "ko";
    if (normalized.startsWith("en")) return "en";
  }
  return "en";
}

export function localizeError(error: UserFacingError, t: Translate): string {
  const keyByCode: Partial<Record<string, MessageKey>> = {
    INVALID_INPUT: "errors.invalidInput",
    INVALID_PAIRING_CODE: "errors.invalidInput",
    NO_SYNC_DEVICES: "errors.noSyncDevices",
    STORAGE_UNAVAILABLE: "errors.storageUnavailable",
    CLIPBOARD_UNAVAILABLE: "errors.clipboardUnavailable",
    NETWORK_UNREACHABLE: "errors.networkUnreachable",
    MDNS_UNAVAILABLE: "errors.networkUnreachable",
    SYNC_SPACE_MISMATCH: "errors.syncSpaceMismatch",
    TRANSFER_FAILED: "errors.transferFailed",
    SOURCE_FILE_CHANGED: "errors.transferFailed",
    DISK_FULL: "errors.diskFull",
    PERMISSION_DENIED: "errors.permissionDenied",
    SOURCE_FILE_MISSING: "errors.sourceFileMissing",
    INTERNAL: "errors.internal",
  };
  return t(keyByCode[error.code] ?? "errors.default");
}

export function localizeTransferError(message: string, t: Translate): string {
  const normalized = message.toLowerCase();
  if (normalized.includes("no sync") || message.includes("没有可同步")) {
    return t("errors.noSyncDevices");
  }
  if (normalized.includes("offline") || message.includes("离线")) {
    return t("errors.networkUnreachable");
  }
  if (normalized.includes("space") || message.includes("空间不足")) {
    return t("errors.diskFull");
  }
  return t("errors.transferFailed");
}

export function interpolate(template: string, values?: TranslationValues): string {
  if (!values) return template;
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    Object.prototype.hasOwnProperty.call(values, key) ? String(values[key]) : match,
  );
}

function browserLanguages(): readonly string[] {
  if (typeof navigator === "undefined") return ["en"];
  return navigator.languages?.length ? navigator.languages : [navigator.language];
}
