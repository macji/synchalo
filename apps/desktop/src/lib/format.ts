import type { TransferState } from "../api/types";

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
export function formatTime(value: string): string {
  const date = new Date(value);
  const now = new Date();
  if (isSameDay(date, now)) {
    return new Intl.DateTimeFormat("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }).format(date);
  }
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (isSameDay(date, yesterday)) {
    return `昨天 ${new Intl.DateTimeFormat("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }).format(date)}`;
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

export function historyGroup(value: string): string {
  const date = new Date(value);
  const now = new Date();
  if (isSameDay(date, now)) return "今天";
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (isSameDay(date, yesterday)) return "昨天";
  return new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(date);
}

export function formatRelative(value: string | null): string {
  if (!value) return "从未连接";
  const elapsed = Date.now() - new Date(value).getTime();
  if (elapsed < 60_000) return "刚刚";
  if (elapsed < 3_600_000) return `${Math.floor(elapsed / 60_000)} 分钟前`;
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)} 小时前`;
  return `${Math.floor(elapsed / 86_400_000)} 天前`;
}

export function transferLabel(state: TransferState): string {
  return {
    queued: "已排队",
    transferring: "传输中",
    verifying: "校验中",
    completed: "已完成",
    failed: "失败",
    cancelled: "已取消",
  }[state];
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}
