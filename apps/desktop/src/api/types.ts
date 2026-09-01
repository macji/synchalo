export type Route = "clipboard" | "files" | "settings";
export type DevicePlatform = "macos" | "linux" | "unknown";
export type DeviceConnectionState = "online" | "offline" | "nearby";
export type SyncState = "healthy" | "paused" | "offline" | "limited";
export type ClipboardCapability = "full" | "appActiveOnly" | "manual" | "unsupported";
export type ClipboardDirection = "local" | "received";
export type TransferDirection = "sending" | "receiving";
export type TransferState =
  | "queued"
  | "waitingForDevice"
  | "transferring"
  | "verifying"
  | "completed"
  | "failed"
  | "cancelled";
export type TransferHistoryFilter = "all" | "sending" | "receiving" | "active" | "failed";
export type HistoryRetention = "none" | "oneDay" | "sevenDays" | "thirtyDays" | "forever";
export type FileDragDropEvent =
  | { type: "enter"; paths: string[] }
  | { type: "over" }
  | { type: "drop"; paths: string[] }
  | { type: "leave" };

export interface HlcTimestamp {
  physicalMs: number;
  logical: number;
}

export interface DeviceView {
  id: string;
  name: string;
  platform: DevicePlatform;
  connectionState: DeviceConnectionState;
  isCurrent: boolean;
  address: string | null;
  lastSeenAt: string | null;
  lastSyncAt: string | null;
  paused: boolean;
}

export interface SyncStatusView {
  state: SyncState;
  label: string;
  onlineCount: number;
  offlineCount: number;
  clipboardCapability: ClipboardCapability;
}

export interface ClipboardItemView {
  id: string;
  content: string;
  contentHash: string;
  sourceDeviceId: string;
  sourceDeviceName: string;
  direction: ClipboardDirection;
  createdAt: string;
  hlc: HlcTimestamp;
  pinned: boolean;
}

export interface ClipboardHistoryPage {
  items: ClipboardItemView[];
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
}

export interface TransferTargetView {
  deviceId: string;
  deviceName: string;
  state: TransferState;
  progress: number;
  bytesPerSecond: number | null;
  error: string | null;
}

export interface TransferView {
  id: string;
  fileName: string;
  fileSize: number;
  direction: TransferDirection;
  state: TransferState;
  progress: number;
  createdAt: string;
  sourceDeviceName: string | null;
  targets: TransferTargetView[];
  bytesPerSecond: number | null;
  etaSeconds: number | null;
  displayPath: string | null;
  error: string | null;
  contentHash: string | null;
  sourceModifiedUnixMs: number | null;
  pinned: boolean;
}

export interface TransferHistoryPage {
  items: TransferView[];
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
}

export interface SettingsView {
  deviceName: string;
  receiveDirectory: string;
  clipboardSyncEnabled: boolean;
  historyRetention: HistoryRetention;
  launchAtStartup: boolean;
  keepInTray: boolean;
  notificationsEnabled: boolean;
}

export type SettingsPatch = Partial<SettingsView>;

export interface PairingCodeView {
  code: string;
  expiresAt: string;
}

export interface PairingRequestView {
  requestId: string;
  deviceId: string;
  deviceName: string;
  platform: DevicePlatform;
}

export interface PlatformCapabilitiesView {
  platform: DevicePlatform;
  architecture: string;
  clipboard: ClipboardCapability;
  supportsTray: boolean;
  supportsAutostart: boolean;
}

export interface AppSnapshot {
  currentDeviceId: string;
  syncStatus: SyncStatusView;
  devices: DeviceView[];
  clipboardHistory: ClipboardItemView[];
  clipboardHistoryTotal: number;
  fileHistory: TransferView[];
  fileHistoryTotal: number;
  settings: SettingsView;
  pairingCode: PairingCodeView | null;
  capabilities: PlatformCapabilitiesView;
}

export interface UserFacingError {
  code: string;
  message: string;
  detail: string | null;
  recoverable: boolean;
}

export type Unlisten = () => void;
