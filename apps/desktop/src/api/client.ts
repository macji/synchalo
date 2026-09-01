import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";

import { cloneMockSnapshot, makeMockTransfer } from "./mock";
import type {
  AppSnapshot,
  ClipboardHistoryPage,
  ClipboardItemView,
  DeviceView,
  FileDragDropEvent,
  PairingCodeView,
  PairingRequestView,
  Route,
  SettingsPatch,
  SettingsView,
  SyncStatusView,
  TransferView,
  TransferHistoryFilter,
  TransferHistoryPage,
  Unlisten,
  UserFacingError,
} from "./types";

const isTauri = "__TAURI_INTERNALS__" in window;
const mock = cloneMockSnapshot();
for (let index = mock.clipboardHistory.length; index < 205; index += 1) {
  const createdAt = Date.now() - (index + 1) * 90_000;
  mock.clipboardHistory.push({
    id: globalThis.crypto?.randomUUID?.() ?? `018f0f00-0000-7000-8000-${String(index).padStart(12, "0")}`,
    content: `历史记录 #${index + 1} · SyncHalo 分页数据`,
    contentHash: `mock-${index}`,
    sourceDeviceId: mock.currentDeviceId,
    sourceDeviceName: mock.settings.deviceName,
    direction: "local",
    createdAt: new Date(createdAt).toISOString(),
    hlc: { physicalMs: createdAt, logical: 0 },
    pinned: index % 50 === 0,
  });
}
mock.clipboardHistoryTotal = mock.clipboardHistory.length;
for (let index = mock.fileHistory.length; index < 205; index += 1) {
  const transfer = makeMockTransfer(`历史文件 #${index + 1}.zip`);
  transfer.createdAt = new Date(Date.now() - (index + 1) * 120_000).toISOString();
  transfer.state = "completed";
  transfer.progress = 1;
  transfer.pinned = index % 50 === 0;
  mock.fileHistory.push(transfer);
}
mock.fileHistoryTotal = mock.fileHistory.length;

function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(name, args).catch((error: unknown) => Promise.reject(normalizeError(error)));
}

function normalizeError(error: unknown): UserFacingError {
  if (typeof error === "object" && error !== null && "message" in error) {
    const candidate = error as Partial<UserFacingError>;
    return {
      code: candidate.code ?? "INTERNAL",
      message: String(candidate.message),
      detail: candidate.detail ?? null,
      recoverable: candidate.recoverable ?? true,
    };
  }
  return {
    code: "INTERNAL",
    message: typeof error === "string" ? error : "操作失败",
    detail: null,
    recoverable: true,
  };
}

export const api = {
  isTauri,

  async getAppState(): Promise<AppSnapshot> {
    if (isTauri) return command("get_app_state");
    const snapshot = structuredClone(mock);
    snapshot.clipboardHistory = snapshot.clipboardHistory.slice(0, 100);
    snapshot.clipboardHistoryTotal = mock.clipboardHistory.length;
    snapshot.fileHistory = snapshot.fileHistory.slice(0, 100);
    snapshot.fileHistoryTotal = mock.fileHistory.length;
    return snapshot;
  },

  async listClipboardHistory(
    query = "",
    favoritesOnly = false,
    page = 1,
  ): Promise<ClipboardHistoryPage> {
    if (isTauri) return command("list_clipboard_history", { query, favoritesOnly, page });
    const normalized = query.trim().toLocaleLowerCase();
    const matching = mock.clipboardHistory.filter(
      (item) =>
        (!favoritesOnly || item.pinned) &&
        item.content.toLocaleLowerCase().includes(normalized),
    );
    const pageSize = 100;
    const totalPages = Math.max(1, Math.ceil(matching.length / pageSize));
    const safePage = Math.min(Math.max(1, page), totalPages);
    const offset = (safePage - 1) * pageSize;
    return {
      items: structuredClone(matching.slice(offset, offset + pageSize)),
      page: safePage,
      pageSize,
      totalItems: matching.length,
      totalPages,
    };
  },

  async listFileHistory(
    query = "",
    favoritesOnly = false,
    filter: TransferHistoryFilter = "all",
    page = 1,
  ): Promise<TransferHistoryPage> {
    if (isTauri) return command("list_file_history", { query, favoritesOnly, filter, page });
    const normalized = query.trim().toLocaleLowerCase();
    const matching = mock.fileHistory.filter((transfer) => {
      const matchesQuery = transfer.fileName.toLocaleLowerCase().includes(normalized);
      const matchesFavorite = !favoritesOnly || transfer.pinned;
      const matchesFilter =
        filter === "all" ||
        (filter === "sending" && transfer.direction === "sending") ||
        (filter === "receiving" && transfer.direction === "receiving") ||
        (filter === "failed" && transfer.state === "failed") ||
        (filter === "active" &&
          ["queued", "waitingForDevice", "transferring", "verifying"].includes(transfer.state));
      return matchesQuery && matchesFavorite && matchesFilter;
    });
    const pageSize = 100;
    const totalPages = Math.max(1, Math.ceil(matching.length / pageSize));
    const safePage = Math.min(Math.max(1, page), totalPages);
    const offset = (safePage - 1) * pageSize;
    return {
      items: structuredClone(matching.slice(offset, offset + pageSize)),
      page: safePage,
      pageSize,
      totalItems: matching.length,
      totalPages,
    };
  },

  async copyHistoryItem(id: string): Promise<ClipboardItemView> {
    if (isTauri) return command("copy_history_item", { id });
    const item = mock.clipboardHistory.find((candidate) => candidate.id === id);
    if (!item) throw normalizeError("历史记录不存在");
    return structuredClone(item);
  },

  async deleteClipboardItem(id: string): Promise<boolean> {
    if (isTauri) return command("delete_clipboard_item", { id });
    const before = mock.clipboardHistory.length;
    mock.clipboardHistory = mock.clipboardHistory.filter((item) => item.id !== id);
    mock.clipboardHistoryTotal = mock.clipboardHistory.length;
    return mock.clipboardHistory.length !== before;
  },

  async restoreClipboardItem(item: ClipboardItemView): Promise<void> {
    if (isTauri) return command("restore_clipboard_item", { item });
    mock.clipboardHistory = [item, ...mock.clipboardHistory.filter((entry) => entry.id !== item.id)];
    mock.clipboardHistoryTotal = mock.clipboardHistory.length;
  },

  async clearClipboardHistory(): Promise<number> {
    if (isTauri) return command("clear_clipboard_history");
    const removed = mock.clipboardHistory.filter((item) => !item.pinned).length;
    mock.clipboardHistory = mock.clipboardHistory.filter((item) => item.pinned);
    mock.clipboardHistoryTotal = mock.clipboardHistory.length;
    return removed;
  },

  async setClipboardPinned(id: string, pinned: boolean): Promise<boolean> {
    if (isTauri) return command("set_clipboard_pinned", { id, pinned });
    const item = mock.clipboardHistory.find((entry) => entry.id === id);
    if (!item) return false;
    item.pinned = pinned;
    return true;
  },

  async generatePairingCode(): Promise<PairingCodeView> {
    if (isTauri) return command("generate_pairing_code");
    const code = { code: "482 913", expiresAt: new Date(Date.now() + 60_000).toISOString() };
    mock.pairingCode = code;
    return code;
  },

  async copyPairingCode(): Promise<PairingCodeView> {
    if (isTauri) return command("copy_pairing_code");
    if (!mock.pairingCode) throw normalizeError("同步码已失效");
    await navigator.clipboard?.writeText?.(mock.pairingCode.code.replace(" ", ""));
    return structuredClone(mock.pairingCode);
  },

  async respondToPairing(requestId: string, accepted: boolean): Promise<void> {
    if (isTauri) return command("respond_to_pairing", { requestId, accepted });
  },

  async joinWithCode(code: string): Promise<DeviceView> {
    if (isTauri) return command("join_with_code", { code });
    if (code.replace(/\D/g, "").length !== 6) throw normalizeError("请输入 6 位同步码");
    return structuredClone(mock.devices[1]);
  },

  async revokeDevice(id: string): Promise<boolean> {
    if (isTauri) return command("revoke_device", { id });
    const before = mock.devices.length;
    mock.devices = mock.devices.filter((device) => device.id !== id || device.isCurrent);
    return mock.devices.length !== before;
  },

  async setDevicePaused(id: string, paused: boolean): Promise<DeviceView> {
    if (isTauri) return command("set_device_paused", { id, paused });
    const device = mock.devices.find((entry) => entry.id === id);
    if (!device) throw normalizeError("设备不存在");
    device.paused = paused;
    return structuredClone(device);
  },

  async pauseSync(paused: boolean): Promise<SyncStatusView> {
    if (isTauri) return command("pause_sync", { paused });
    mock.syncStatus.state = paused ? "paused" : "healthy";
    mock.syncStatus.label = paused ? "同步已暂停" : "同步正常";
    return structuredClone(mock.syncStatus);
  },

  async updateSettings(patch: SettingsPatch): Promise<SettingsView> {
    if (isTauri) return command("update_settings", { patch });
    mock.settings = { ...mock.settings, ...patch };
    return structuredClone(mock.settings);
  },

  async selectReceiveDirectory(): Promise<SettingsView | null> {
    if (isTauri) return command("select_receive_directory");
    mock.settings.receiveDirectory = "~/Downloads/SyncHalo";
    return structuredClone(mock.settings);
  },

  async selectFiles(targetIds?: string[]): Promise<TransferView[]> {
    if (isTauri) return command("select_files", { targetIds });
    const transfer = makeMockTransfer("release-arm64.deb", targetIds);
    mock.fileHistory = [transfer, ...mock.fileHistory];
    mock.fileHistoryTotal = mock.fileHistory.length;
    return [structuredClone(transfer)];
  },

  async pasteFiles(targetIds?: string[]): Promise<TransferView[]> {
    if (isTauri) return command("paste_files", { targetIds });
    const transfer = makeMockTransfer("clipboard-file.zip", targetIds);
    mock.fileHistory = [transfer, ...mock.fileHistory];
    mock.fileHistoryTotal = mock.fileHistory.length;
    return [structuredClone(transfer)];
  },

  async enqueueFiles(paths: string[], targetIds?: string[]): Promise<TransferView[]> {
    if (isTauri) return command("enqueue_files", { paths, targetIds });
    const transfers = paths.map((path) =>
      makeMockTransfer(path.split(/[\\/]/).pop() || path, targetIds),
    );
    mock.fileHistory = [...transfers, ...mock.fileHistory];
    mock.fileHistoryTotal = mock.fileHistory.length;
    return structuredClone(transfers);
  },

  async resyncTransfer(id: string, targetIds?: string[]): Promise<TransferView[]> {
    if (isTauri) return command("resync_transfer", { id, targetIds });
    const original = mock.fileHistory.find((item) => item.id === id);
    if (!original) throw normalizeError("文件任务不存在");
    const transfer = makeMockTransfer(original.fileName, targetIds);
    mock.fileHistory = [transfer, ...mock.fileHistory];
    mock.fileHistoryTotal = mock.fileHistory.length;
    return [structuredClone(transfer)];
  },

  async setTransferPinned(id: string, pinned: boolean): Promise<TransferView> {
    if (isTauri) return command("set_transfer_pinned", { id, pinned });
    const transfer = mock.fileHistory.find((item) => item.id === id);
    if (!transfer) throw normalizeError("文件任务不存在");
    transfer.pinned = pinned;
    return structuredClone(transfer);
  },

  async retryTransfer(id: string): Promise<TransferView> {
    return updateMockTransfer("retry_transfer", id, "queued");
  },

  async cancelTransfer(id: string): Promise<TransferView> {
    return updateMockTransfer("cancel_transfer", id, "cancelled");
  },

  async deleteTransfer(id: string): Promise<boolean> {
    if (isTauri) return command("delete_transfer", { id });
    const before = mock.fileHistory.length;
    mock.fileHistory = mock.fileHistory.filter((item) => item.id !== id);
    mock.fileHistoryTotal = mock.fileHistory.length;
    return mock.fileHistory.length !== before;
  },

  async openTransfer(id: string): Promise<void> {
    if (isTauri) return command("open_transfer", { id });
  },

  async revealTransfer(id: string): Promise<void> {
    if (isTauri) return command("reveal_transfer", { id });
  },

  async openReceiveDirectory(): Promise<void> {
    if (isTauri) return command("open_receive_directory");
  },

  async onClipboardAdded(callback: (item: ClipboardItemView) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://clipboard-added", callback);
  },

  async onClipboardDeleted(callback: (id: string) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://clipboard-deleted", callback);
  },

  async onDevicesChanged(callback: (devices: DeviceView[]) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://devices-changed", callback);
  },

  async onPairingCodeChanged(callback: (code: PairingCodeView | null) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://pairing-code-changed", callback);
  },

  async onPairingRequested(callback: (request: PairingRequestView) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://pairing-requested", callback);
  },

  async onSettingsChanged(callback: (settings: SettingsView) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://settings-changed", callback);
  },

  async onTransferChanged(callback: (transfer: TransferView) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://transfer-changed", callback);
  },

  async onSyncStatusChanged(callback: (status: SyncStatusView) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://sync-status-changed", callback);
  },

  async onUserError(callback: (error: UserFacingError) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://user-error", callback);
  },

  async onNavigate(callback: (route: Route) => void): Promise<Unlisten> {
    return listenWhenTauri("synchalo://navigate", callback);
  },

  async onFileDragDrop(callback: (event: FileDragDropEvent) => void): Promise<Unlisten> {
    if (!isTauri) return () => undefined;
    return getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter") callback({ type: "enter", paths: payload.paths });
      else if (payload.type === "drop") callback({ type: "drop", paths: payload.paths });
      else if (payload.type === "over") callback({ type: "over" });
      else callback({ type: "leave" });
    });
  },
};

async function listenWhenTauri<T>(eventName: string, callback: (value: T) => void): Promise<Unlisten> {
  if (!isTauri) return () => undefined;
  return listen<T>(eventName, (event) => callback(event.payload));
}

async function updateMockTransfer(
  commandName: "retry_transfer" | "cancel_transfer",
  id: string,
  state: TransferView["state"],
): Promise<TransferView> {
  if (isTauri) return command(commandName, { id });
  const transfer = mock.fileHistory.find((item) => item.id === id);
  if (!transfer) throw normalizeError("文件任务不存在");
  transfer.state = state;
  transfer.error = null;
  return structuredClone(transfer);
}
