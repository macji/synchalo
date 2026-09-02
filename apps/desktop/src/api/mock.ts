import type { AppSnapshot, ClipboardItemView, TransferView } from "./types";

const now = Date.now();
const id = (suffix: string) => `018f0f00-0000-7000-8000-${suffix.padStart(12, "0")}`;

export const mockSnapshot: AppSnapshot = {
  currentDeviceId: id("1"),
  syncStatus: {
    state: "healthy",
    label: "同步正常",
    onlineCount: 2,
    offlineCount: 1,
    clipboardCapability: "full",
  },
  devices: [
    {
      id: id("1"),
      name: "Jason 的 MacBook Air",
      platform: "macos",
      connectionState: "online",
      isCurrent: true,
      address: null,
      lastSeenAt: new Date(now).toISOString(),
      lastSyncAt: new Date(now - 14_000).toISOString(),
      paused: false,
    },
    {
      id: id("2"),
      name: "Studio Ubuntu",
      platform: "linux",
      connectionState: "online",
      isCurrent: false,
      address: "192.168.1.18:53317",
      lastSeenAt: new Date(now - 20_000).toISOString(),
      lastSyncAt: new Date(now - 82_000).toISOString(),
      paused: false,
    },
    {
      id: id("3"),
      name: "Desk Pi",
      platform: "linux",
      connectionState: "online",
      isCurrent: false,
      address: "192.168.1.31:53317",
      lastSeenAt: new Date(now - 43_000).toISOString(),
      lastSyncAt: new Date(now - 2_000_000).toISOString(),
      paused: false,
    },
    {
      id: id("4"),
      name: "Office Ubuntu",
      platform: "linux",
      connectionState: "offline",
      isCurrent: false,
      address: null,
      lastSeenAt: new Date(now - 86_400_000).toISOString(),
      lastSyncAt: new Date(now - 86_600_000).toISOString(),
      paused: false,
    },
  ],
  clipboardHistory: [
    {
      id: id("101"),
      content: "cargo test --workspace",
      contentHash: "dca3b4",
      sourceDeviceId: id("1"),
      sourceDeviceName: "Jason 的 MacBook Air",
      direction: "local",
      createdAt: new Date(now - 60_000).toISOString(),
      hlc: { physicalMs: now - 60_000, logical: 0 },
      pinned: false,
    },
    {
      id: id("102"),
      content: "https://github.com/tauri-apps/tauri",
      contentHash: "38f1aa",
      sourceDeviceId: id("2"),
      sourceDeviceName: "Studio Ubuntu",
      direction: "received",
      createdAt: new Date(now - 3_600_000).toISOString(),
      hlc: { physicalMs: now - 3_600_000, logical: 0 },
      pinned: true,
    },
    {
      id: id("103"),
      content: "会议结论：MVP 首发覆盖 macOS 和 Ubuntu ARM64。\n文件流不经过 WebView，历史正文在本地加密。",
      contentHash: "a1043e",
      sourceDeviceId: id("3"),
      sourceDeviceName: "Desk Pi",
      direction: "received",
      createdAt: new Date(now - 7_200_000).toISOString(),
      hlc: { physicalMs: now - 7_200_000, logical: 0 },
      pinned: false,
    },
    {
      id: id("104"),
      content: "export RUST_LOG=synchalo=debug",
      contentHash: "439ca1",
      sourceDeviceId: id("1"),
      sourceDeviceName: "Jason 的 MacBook Air",
      direction: "local",
      createdAt: new Date(now - 90_000_000).toISOString(),
      hlc: { physicalMs: now - 90_000_000, logical: 0 },
      pinned: false,
    },
  ],
  clipboardHistoryTotal: 4,
  fileHistory: [
    {
      id: id("201"),
      fileName: "SyncHalo-design.zip",
      fileSize: 2_400_000_000,
      direction: "sending",
      state: "transferring",
      progress: 0.67,
      createdAt: new Date(now - 70_000).toISOString(),
      sourceDeviceName: "Jason 的 MacBook Air",
      targets: [
        {
          deviceId: id("2"),
          deviceName: "Studio Ubuntu",
          state: "transferring",
          progress: 0.67,
          bytesPerSecond: 82_000_000,
          error: null,
        },
        {
          deviceId: id("3"),
          deviceName: "Desk Pi",
          state: "completed",
          progress: 1,
          bytesPerSecond: null,
          error: null,
        },
      ],
      bytesPerSecond: 82_000_000,
      etaSeconds: 10,
      displayPath: "/Users/jason/Downloads/SyncHalo-design.zip",
      error: null,
      contentHash: "a".repeat(64),
      sourceModifiedUnixMs: now - 70_000,
      pinned: false,
    },
    {
      id: id("202"),
      fileName: "notes.pdf",
      fileSize: 8_200_000,
      direction: "receiving",
      state: "completed",
      progress: 1,
      createdAt: new Date(now - 4_600_000).toISOString(),
      sourceDeviceName: "Studio Ubuntu",
      targets: [],
      bytesPerSecond: null,
      etaSeconds: null,
      displayPath: "/Users/jason/Downloads/notes.pdf",
      error: null,
      contentHash: "b".repeat(64),
      sourceModifiedUnixMs: null,
      pinned: true,
    },
    {
      id: id("203"),
      fileName: "dataset.tar",
      fileSize: 12_800_000_000,
      direction: "sending",
      state: "failed",
      progress: 0,
      createdAt: new Date(now - 8_000_000).toISOString(),
      sourceDeviceName: "Jason 的 MacBook Air",
      targets: [
        {
          deviceId: id("4"),
          deviceName: "Office Ubuntu",
          state: "failed",
          progress: 0,
          bytesPerSecond: null,
          error: "目标设备离线，文件未发送",
        },
      ],
      bytesPerSecond: null,
      etaSeconds: null,
      displayPath: "/Users/jason/Downloads/dataset.tar",
      error: "目标设备离线，文件未发送",
      contentHash: "c".repeat(64),
      sourceModifiedUnixMs: now - 8_000_000,
      pinned: false,
    },
  ],
  fileHistoryTotal: 3,
  settings: {
    deviceName: "Jason 的 MacBook Air",
    receiveDirectory: "~/Downloads",
    clipboardSyncEnabled: true,
    historyRetention: "sevenDays",
    launchAtStartup: false,
    keepInTray: true,
    notificationsEnabled: true,
  },
  pairingCode: null,
  capabilities: {
    platform: "macos",
    architecture: "aarch64",
    clipboard: "full",
    supportsTray: true,
    supportsAutostart: true,
  },
};

export function cloneMockSnapshot(): AppSnapshot {
  return structuredClone(mockSnapshot);
}

export function makeMockTransfer(fileName: string, targetIds?: string[]): TransferView {
  const targets = mockSnapshot.devices
    .filter((device) => !device.isCurrent && !device.paused)
    .filter((device) =>
      targetIds?.length
        ? targetIds.includes(device.id)
        : device.connectionState === "online",
    )
    .map((device) => ({
      deviceId: device.id,
      deviceName: device.name,
      state: device.connectionState === "online" ? "queued" as const : "failed" as const,
      progress: 0,
      bytesPerSecond: null,
      error: device.connectionState === "online" ? null : "目标设备当前离线，文件未发送",
    }));
  const hasOnlineTarget = targets.some((target) => target.state === "queued");
  const hasOfflineTarget = targets.some((target) => target.state === "failed");
  return {
    id: globalThis.crypto?.randomUUID?.() ?? id(String(Date.now()).slice(-12)),
    fileName,
    fileSize: 12_400_000,
    direction: "sending",
    state: hasOnlineTarget ? "queued" : "failed",
    progress: 0,
    createdAt: new Date().toISOString(),
    sourceDeviceName: mockSnapshot.settings.deviceName,
    targets,
    bytesPerSecond: null,
    etaSeconds: null,
    displayPath: fileName,
    error: hasOfflineTarget
      ? hasOnlineTarget
        ? "部分目标设备当前离线，文件未发送"
        : "目标设备当前离线，文件未发送"
      : targets.length
        ? null
        : "没有可同步的在线设备",
    contentHash: "d".repeat(64),
    sourceModifiedUnixMs: Date.now(),
    pinned: false,
  };
}

export function makeMockClipboard(content: string): ClipboardItemView {
  return {
    id: globalThis.crypto?.randomUUID?.() ?? id(String(Date.now()).slice(-12)),
    content,
    contentHash: String(content.length),
    sourceDeviceId: mockSnapshot.currentDeviceId,
    sourceDeviceName: mockSnapshot.settings.deviceName,
    direction: "local",
    createdAt: new Date().toISOString(),
    hlc: { physicalMs: Date.now(), logical: 0 },
    pinned: false,
  };
}
