import {
  Check,
  ChevronDown,
  Circle,
  Copy,
  FolderOpen,
  Laptop,
  MoreHorizontal,
  RefreshCw,
  ShieldCheck,
  Trash2,
  Unplug,
  UserPlus,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";

import type {
  DevicePlatform,
  DeviceView,
  HistoryRetention,
  PairingCodeView,
  PlatformCapabilitiesView,
  SettingsPatch,
  SettingsView,
} from "../api/types";
import { IconButton } from "../components/IconButton";
import { ModalDialog } from "../components/ModalDialog";
import { PageHeader } from "../components/PageHeader";
import { Switch } from "../components/Switch";
import { formatRelative } from "../lib/format";

interface SettingsPageProps {
  appVersion: string | null;
  settings: SettingsView;
  devices: DeviceView[];
  pairingCode: PairingCodeView | null;
  capabilities: PlatformCapabilitiesView;
  onGenerateCode: () => void;
  onCopyCode: (code: string) => void;
  onJoin: (code: string) => void;
  onCheckForUpdates: () => Promise<void>;
  onUpdate: (patch: SettingsPatch) => void;
  onSelectDirectory: () => void;
  onOpenDirectory: () => void;
  onPauseDevice: (device: DeviceView, paused: boolean) => void;
  onRevoke: (device: DeviceView) => void;
}

const retentionOptions: Array<[HistoryRetention, string]> = [
  ["none", "不保存"],
  ["oneDay", "1 天"],
  ["sevenDays", "7 天"],
  ["thirtyDays", "30 天"],
  ["forever", "永久"],
];

export function SettingsPage({
  appVersion,
  settings,
  devices,
  pairingCode,
  capabilities,
  onGenerateCode,
  onCopyCode,
  onJoin,
  onCheckForUpdates,
  onUpdate,
  onSelectDirectory,
  onOpenDirectory,
  onPauseDevice,
  onRevoke,
}: SettingsPageProps) {
  const [joinCode, setJoinCode] = useState("");
  const [clock, setClock] = useState(() => Date.now());
  const [deviceMenu, setDeviceMenu] = useState<string | null>(null);
  const [editingName, setEditingName] = useState(false);
  const [deviceName, setDeviceName] = useState(settings.deviceName);
  const [joinDialogOpen, setJoinDialogOpen] = useState(false);
  const [checkingForUpdates, setCheckingForUpdates] = useState(false);
  const joinInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!pairingCode) return;
    const interval = window.setInterval(() => setClock(Date.now()), 250);
    return () => window.clearInterval(interval);
  }, [pairingCode]);
  const remaining = pairingCode
    ? Math.min(60, Math.max(0, Math.ceil((new Date(pairingCode.expiresAt).getTime() - clock) / 1000)))
    : 0;

  const online = useMemo(() => devices.filter((device) => device.connectionState === "online"), [devices]);
  const offline = useMemo(() => devices.filter((device) => device.connectionState === "offline"), [devices]);

  return (
    <section className="page settings-page" aria-labelledby="settings-title">
      <PageHeader eyebrow="LOCAL TRUST" title="设置" />
      <div className="page-scroll settings-reading-width">
        <SettingsSection title="添加设备">
          <div className="pairing-panel">
            <div className="pairing-panel-main">
              <div className="pairing-label">
                <ShieldCheck aria-hidden="true" size={17} />
                <span>一次性同步码</span>
              </div>
              {pairingCode && remaining > 0 ? (
                <div className="pairing-code-wrap">
                  <strong className="pairing-code" aria-label={`同步码 ${pairingCode.code}`}>{pairingCode.code}</strong>
                  <span className="countdown">00:{String(remaining).padStart(2, "0")}</span>
                </div>
              ) : (
                <div className="pairing-idle">
                  <strong>尚未开放配对</strong>
                  <span>生成后 60 秒内有效</span>
                </div>
              )}
            </div>
            <p>在另一台设备输入此码。使用一次、超时或退出配对流程后立即失效。</p>
            <div className="pairing-actions">
              {pairingCode && remaining > 0 ? (
                <button className="button button--secondary" onClick={() => onCopyCode(pairingCode.code)} type="button">
                  <Copy size={15} />复制
                </button>
              ) : null}
              <button className="button button--primary" onClick={onGenerateCode} type="button">
                <RefreshCw size={15} />{pairingCode && remaining > 0 ? "刷新" : "生成同步码"}
              </button>
              <button
                className="button button--secondary"
                onClick={() => setJoinDialogOpen(true)}
                type="button"
              >
                <UserPlus size={15} />加入
              </button>
            </div>
          </div>
        </SettingsSection>

        <SettingsSection title="我的设备">
          <DeviceGroup
            devices={online}
            label="在线"
            menuDevice={deviceMenu}
            onMenu={setDeviceMenu}
            onPauseDevice={onPauseDevice}
            onRevoke={onRevoke}
          />
          <DeviceGroup
            devices={offline}
            label="离线"
            menuDevice={deviceMenu}
            onMenu={setDeviceMenu}
            onPauseDevice={onPauseDevice}
            onRevoke={onRevoke}
          />
        </SettingsSection>

        <SettingsSection title="文件接收">
          <SettingRow label="接收目录">
            <code className="path-value" title={settings.receiveDirectory}>{settings.receiveDirectory}</code>
            <button className="button button--secondary button--small" onClick={onSelectDirectory} type="button">更改</button>
            <IconButton icon={<FolderOpen size={16} />} label="打开接收目录" onClick={onOpenDirectory} />
          </SettingRow>
        </SettingsSection>

        <SettingsSection title="同步与历史">
          <SettingRow description="开启后，此设备发送并接收单条历史的删除与撤销。" label="删除同步">
            <Switch checked={settings.deleteSyncEnabled} label="删除同步" onChange={(value) => onUpdate({ deleteSyncEnabled: value })} />
          </SettingRow>
          <SettingRow description="开启后，此设备发送并接收收藏状态变更。" label="收藏同步">
            <Switch checked={settings.favoriteSyncEnabled} label="收藏同步" onChange={(value) => onUpdate({ favoriteSyncEnabled: value })} />
          </SettingRow>
          <SettingRow description="收藏条目不受自动清理影响。" label="保存历史">
            <label className="select-control">
              <select
                aria-label="历史保留时间"
                onChange={(event) => onUpdate({ historyRetention: event.target.value as HistoryRetention })}
                value={settings.historyRetention}
              >
                {retentionOptions.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
              </select>
              <ChevronDown aria-hidden="true" size={14} />
            </label>
          </SettingRow>
        </SettingsSection>

        <SettingsSection title="启动与后台">
          <SettingRow label="开机时启动 SyncHalo">
            <Switch
              checked={settings.launchAtStartup}
              disabled={!capabilities.supportsAutostart}
              label="开机时启动 SyncHalo"
              onChange={(value) => onUpdate({ launchAtStartup: value })}
            />
          </SettingRow>
          <SettingRow label="关闭窗口后留在系统托盘">
            <Switch
              checked={settings.keepInTray}
              disabled={!capabilities.supportsTray}
              label="关闭窗口后留在系统托盘"
              onChange={(value) => onUpdate({ keepInTray: value })}
            />
          </SettingRow>
          <SettingRow description="启动后及每 30 分钟检查；开启后自动下载并等待确认安装，关闭后只提醒。" label="自动更新">
            <Switch
              checked={settings.automaticUpdatesEnabled}
              label="自动更新"
              onChange={(value) => onUpdate({ automaticUpdatesEnabled: value })}
            />
          </SettingRow>
          <SettingRow description="立即向 GitHub Releases 查询当前平台的签名更新。" label="检查更新">
            <button
              aria-busy={checkingForUpdates}
              className="button button--secondary button--small"
              disabled={checkingForUpdates}
              onClick={() => {
                setCheckingForUpdates(true);
                void onCheckForUpdates().finally(() => setCheckingForUpdates(false));
              }}
              type="button"
            >
              <RefreshCw className={checkingForUpdates ? "loading-spinner" : undefined} size={14} />
              {checkingForUpdates ? "正在检查…" : "检查更新"}
            </button>
          </SettingRow>
        </SettingsSection>

        <SettingsSection title="当前设备">
          <SettingRow label="设备名称">
            {editingName ? (
              <form
                className="inline-edit"
                onSubmit={(event) => {
                  event.preventDefault();
                  onUpdate({ deviceName });
                  setEditingName(false);
                }}
              >
                <input autoFocus maxLength={64} onChange={(event) => setDeviceName(event.target.value)} value={deviceName} />
                <button className="button button--primary button--small" type="submit"><Check size={14} />保存</button>
              </form>
            ) : (
              <>
                <strong className="setting-value">{settings.deviceName}</strong>
                <button
                  className="button button--secondary button--small"
                  onClick={() => {
                    setDeviceName(settings.deviceName);
                    setEditingName(true);
                  }}
                  type="button"
                >
                  重命名
                </button>
              </>
            )}
          </SettingRow>
        </SettingsSection>

        <p className="version-line">SyncHalo{appVersion ? ` ${appVersion}` : ""}</p>
      </div>

      {joinDialogOpen ? (
        <ModalDialog
          actions={
            <>
              <button
                className="button button--secondary"
                onClick={() => setJoinDialogOpen(false)}
                type="button"
              >
                取消
              </button>
              <button
                className="button button--primary"
                disabled={joinCode.replace(/\D/g, "").length !== 6}
                form="join-device-form"
                type="submit"
              >
                加入设备
              </button>
            </>
          }
          className="join-device-dialog"
          initialFocusRef={joinInputRef}
          onClose={() => setJoinDialogOpen(false)}
          title="加入另一台设备"
        >
          <form
            className="join-dialog-form"
            id="join-device-form"
            onSubmit={(event) => {
              event.preventDefault();
              onJoin(joinCode);
              setJoinDialogOpen(false);
              setJoinCode("");
            }}
          >
            <label htmlFor="join-code-dialog">输入一次性同步码</label>
            <input
              autoComplete="one-time-code"
              id="join-code-dialog"
              inputMode="numeric"
              maxLength={7}
              onChange={(event) => setJoinCode(formatPairingInput(event.target.value))}
              placeholder="000 000"
              ref={joinInputRef}
              value={joinCode}
            />
            <p>请在另一台设备生成同步码，并在 60 秒内输入。</p>
          </form>
        </ModalDialog>
      ) : null}
    </section>
  );
}

function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="settings-section">
      <div className="section-intro">
        <h2>{title}</h2>
      </div>
      <div className="section-content">{children}</div>
    </section>
  );
}

function SettingRow({ label, description, children }: { label: string; description?: string; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div className="setting-label">
        <strong>{label}</strong>
        {description ? <span>{description}</span> : null}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

function DeviceGroup({
  label,
  devices,
  menuDevice,
  onMenu,
  onPauseDevice,
  onRevoke,
}: {
  label: string;
  devices: DeviceView[];
  menuDevice: string | null;
  onMenu: (id: string | null) => void;
  onPauseDevice: (device: DeviceView, paused: boolean) => void;
  onRevoke: (device: DeviceView) => void;
}) {
  return (
    <div className="device-group">
      <h3>{label} <span>· {devices.length}</span></h3>
      {devices.length ? devices.map((device) => (
        <div className="device-row" key={device.id}>
          <span className={`status-dot status-dot--${device.connectionState}`} aria-hidden="true" />
          <div className="device-icon"><Laptop aria-hidden="true" size={18} /></div>
          <div className="device-copy">
            <strong>{device.name}</strong>
            <span>
              {platformLabel(device.platform)}
              {device.address ? ` · ${device.address.split(":")[0]}` : ""}
              {device.paused ? " · 已暂停" : ""}
              {!device.isCurrent && device.connectionState === "offline" ? ` · 上次在线 ${formatRelative(device.lastSeenAt)}` : ""}
            </span>
          </div>
          {device.isCurrent ? (
            <span className="current-device-badge">当前设备</span>
          ) : (
            <div className="device-menu-wrap">
              <IconButton
                icon={<MoreHorizontal size={17} />}
                label={`管理 ${device.name}`}
                onClick={() => onMenu(menuDevice === device.id ? null : device.id)}
              />
              {menuDevice === device.id ? (
                <div className="context-menu">
                  <button
                    onClick={() => {
                      onPauseDevice(device, !device.paused);
                      onMenu(null);
                    }}
                    type="button"
                  >
                    <Unplug size={15} />{device.paused ? "恢复向此设备同步" : "暂停向此设备同步"}
                  </button>
                  <button className="is-danger" onClick={() => onRevoke(device)} type="button"><Trash2 size={15} />撤销设备</button>
                </div>
              ) : null}
            </div>
          )}
        </div>
      )) : (
        <div className="device-empty"><Circle size={12} />没有{label}设备</div>
      )}
    </div>
  );
}

function formatPairingInput(value: string): string {
  const digits = value.replace(/\D/g, "").slice(0, 6);
  return digits.length > 3 ? `${digits.slice(0, 3)} ${digits.slice(3)}` : digits;
}

function platformLabel(platform: DevicePlatform): string {
  return platform === "macos" ? "macOS" : platform === "linux" ? "Ubuntu / Linux" : "未知平台";
}
