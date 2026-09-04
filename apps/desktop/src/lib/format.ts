import type { TransferState } from "../api/types";
import type { SupportedLocale, Translate } from "../i18n";

export function formatBytes(bytes: number): string {
  if (bytes < 1_000) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = "B";
  for (const next of units) {
    value /= 1_000;
    unit = next;
    if (value < 1_000) break;
  }
  const digits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${unit}`;
}
export function formatTime(value: string, locale: SupportedLocale, t: Translate): string {
  const date = new Date(value);
  const now = new Date();
  if (isSameDay(date, now)) {
    return new Intl.DateTimeFormat(locale, {
      hour: "2-digit",
      minute: "2-digit",
    }).format(date);
  }
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (isSameDay(date, yesterday)) {
    return `${t("time.yesterday")} ${new Intl.DateTimeFormat(locale, {
      hour: "2-digit",
      minute: "2-digit",
    }).format(date)}`;
  }
  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export function historyGroup(value: string, locale: SupportedLocale, t: Translate): string {
  const date = new Date(value);
  const now = new Date();
  if (isSameDay(date, now)) return t("time.today");
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (isSameDay(date, yesterday)) return t("time.yesterday");
  return new Intl.DateTimeFormat(locale, { month: "long", day: "numeric" }).format(date);
}

export function formatRelative(value: string | null, t: Translate): string {
  if (!value) return t("time.never");
  const elapsed = Date.now() - new Date(value).getTime();
  if (elapsed < 60_000) return t("time.justNow");
  if (elapsed < 3_600_000) return t("time.minutesAgo", { count: Math.floor(elapsed / 60_000) });
  if (elapsed < 86_400_000) return t("time.hoursAgo", { count: Math.floor(elapsed / 3_600_000) });
  return t("time.daysAgo", { count: Math.floor(elapsed / 86_400_000) });
}

export function transferLabel(state: TransferState, t: Translate): string {
  return t(`files.state.${state}`);
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}
