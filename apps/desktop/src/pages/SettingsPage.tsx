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
  LanguagePreference,
  PairingCodeView,
  PlatformCapabilitiesView,
  SettingsPatch,
  SettingsView,
} from "../api/types";
import { IconButton } from "../components/IconButton";
import { ModalDialog } from "../components/ModalDialog";
import { PageHeader } from "../components/PageHeader";
import { Switch } from "../components/Switch";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/messages";
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
  onRefreshDevices: () => void;
  onUpdate: (patch: SettingsPatch) => void;
  onSelectDirectory: () => void;
  onOpenDirectory: () => void;
  onPauseDevice: (device: DeviceView, paused: boolean) => void;
  onRevoke: (device: DeviceView) => void;
  refreshingDevices: boolean;
}

const retentionOptions: Array<[HistoryRetention, MessageKey]> = [
  ["none", "settings.retention.none"],
  ["oneDay", "settings.retention.oneDay"],
  ["sevenDays", "settings.retention.sevenDays"],
  ["thirtyDays", "settings.retention.thirtyDays"],
  ["forever", "settings.retention.forever"],
];

const languageOptions: Array<[LanguagePreference, MessageKey]> = [
  ["system", "language.system"],
  ["en", "language.en"],
  ["zh-cn", "language.zhCN"],
  ["zh-tw", "language.zhTW"],
  ["ja", "language.ja"],
  ["ko", "language.ko"],
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
  onRefreshDevices,
  onUpdate,
  onSelectDirectory,
  onOpenDirectory,
  onPauseDevice,
  onRevoke,
  refreshingDevices,
}: SettingsPageProps) {
  const { t } = useI18n();
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
      <PageHeader eyebrow="LOCAL TRUST" title={t("settings.title")} />
      <div className="page-scroll settings-reading-width">
        <SettingsSection title={t("language.section")}>
          <SettingRow description={t("language.description")} label={t("language.label")}>
            <label className="select-control">
              <select
                aria-label={t("language.select")}
                onChange={(event) => onUpdate({ language: event.target.value as LanguagePreference })}
                value={settings.language}
              >
                {languageOptions.map(([value, label]) => (
                  <option key={value} value={value}>{t(label)}</option>
                ))}
              </select>
              <ChevronDown aria-hidden="true" size={14} />
            </label>
          </SettingRow>
        </SettingsSection>

        <SettingsSection title={t("settings.addDevice")}>
          <div className="pairing-panel">
            <div className="pairing-panel-main">
              <div className="pairing-label">
                <ShieldCheck aria-hidden="true" size={17} />
                <span>{t("settings.oneTimeCode")}</span>
              </div>
              {pairingCode && remaining > 0 ? (
                <div className="pairing-code-wrap">
                  <strong className="pairing-code" aria-label={t("settings.codeLabel", { code: pairingCode.code })}>{pairingCode.code}</strong>
                  <span className="countdown">00:{String(remaining).padStart(2, "0")}</span>
                </div>
              ) : (
                <div className="pairing-idle">
                  <strong>{t("settings.pairingIdle")}</strong>
                  <span>{t("settings.codeValidity")}</span>
                </div>
              )}
            </div>
            <p>{t("settings.pairingHint")}</p>
            <div className="pairing-actions">
              {pairingCode && remaining > 0 ? (
                <button className="button button--secondary" onClick={() => onCopyCode(pairingCode.code)} type="button">
                  <Copy size={15} />{t("common.copy")}
                </button>
              ) : null}
              <button className="button button--primary" onClick={onGenerateCode} type="button">
                <RefreshCw size={15} />{pairingCode && remaining > 0 ? t("common.refresh") : t("settings.generateCode")}
              </button>
              <button
                className="button button--secondary"
                onClick={() => setJoinDialogOpen(true)}
                type="button"
              >
                <UserPlus size={15} />{t("settings.join")}
              </button>
            </div>
          </div>
        </SettingsSection>

        <SettingsSection
          action={
            <IconButton
              disabled={refreshingDevices}
              icon={
                <RefreshCw
                  className={refreshingDevices ? "loading-spinner" : undefined}
                  size={15}
                />
              }
              label={t("files.refreshDevices")}
              onClick={onRefreshDevices}
            />
          }
          title={t("settings.myDevices")}
        >
          <DeviceGroup
            devices={online}
            label={t("common.online")}
            menuDevice={deviceMenu}
            onMenu={setDeviceMenu}
            onPauseDevice={onPauseDevice}
            onRevoke={onRevoke}
          />
          <DeviceGroup
            devices={offline}
            label={t("common.offline")}
            menuDevice={deviceMenu}
            onMenu={setDeviceMenu}
            onPauseDevice={onPauseDevice}
            onRevoke={onRevoke}
          />
        </SettingsSection>

        <SettingsSection title={t("settings.fileReceiving")}>
          <SettingRow label={t("settings.receiveDirectory")}>
            <code className="path-value" title={settings.receiveDirectory}>{settings.receiveDirectory}</code>
            <button className="button button--secondary button--small" onClick={onSelectDirectory} type="button">{t("settings.change")}</button>
            <IconButton icon={<FolderOpen size={16} />} label={t("settings.openDirectory")} onClick={onOpenDirectory} />
          </SettingRow>
        </SettingsSection>

        <SettingsSection title={t("settings.syncHistory")}>
          <SettingRow description={t("settings.deleteSyncDescription")} label={t("settings.deleteSync")}>
            <Switch checked={settings.deleteSyncEnabled} label={t("settings.deleteSync")} onChange={(value) => onUpdate({ deleteSyncEnabled: value })} />
          </SettingRow>
          <SettingRow description={t("settings.favoriteSyncDescription")} label={t("settings.favoriteSync")}>
            <Switch checked={settings.favoriteSyncEnabled} label={t("settings.favoriteSync")} onChange={(value) => onUpdate({ favoriteSyncEnabled: value })} />
          </SettingRow>
          <SettingRow description={t("settings.saveHistoryDescription")} label={t("settings.saveHistory")}>
            <label className="select-control">
              <select
                aria-label={t("settings.retentionLabel")}
                onChange={(event) => onUpdate({ historyRetention: event.target.value as HistoryRetention })}
                value={settings.historyRetention}
              >
                {retentionOptions.map(([value, label]) => <option key={value} value={value}>{t(label)}</option>)}
              </select>
              <ChevronDown aria-hidden="true" size={14} />
            </label>
          </SettingRow>
        </SettingsSection>

        <SettingsSection title={t("settings.startupBackground")}>
          <SettingRow label={t("settings.launchAtStartup")}>
            <Switch
              checked={settings.launchAtStartup}
              disabled={!capabilities.supportsAutostart}
              label={t("settings.launchAtStartup")}
              onChange={(value) => onUpdate({ launchAtStartup: value })}
            />
          </SettingRow>
          <SettingRow label={t("settings.keepInTray")}>
            <Switch
              checked={settings.keepInTray}
              disabled={!capabilities.supportsTray}
              label={t("settings.keepInTray")}
              onChange={(value) => onUpdate({ keepInTray: value })}
            />
          </SettingRow>
          <SettingRow
            description={capabilities.platform === "linux"
              ? t("settings.updateLinuxDescription")
              : t("settings.updateDescription")}
            label={t("settings.automaticUpdates")}
          >
            <Switch
              checked={settings.automaticUpdatesEnabled}
              label={t("settings.automaticUpdates")}
              onChange={(value) => onUpdate({ automaticUpdatesEnabled: value })}
            />
          </SettingRow>
          <SettingRow description={t("settings.checkUpdatesDescription")} label={t("settings.checkUpdates")}>
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
              {checkingForUpdates ? t("update.checking") : t("settings.checkUpdates")}
            </button>
          </SettingRow>
        </SettingsSection>

        <SettingsSection title={t("settings.currentDevice")}>
          <SettingRow label={t("settings.deviceName")}>
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
                <button className="button button--primary button--small" type="submit"><Check size={14} />{t("common.save")}</button>
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
                  {t("settings.rename")}
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
                {t("common.cancel")}
              </button>
              <button
                className="button button--primary"
                disabled={joinCode.replace(/\D/g, "").length !== 6}
                form="join-device-form"
                type="submit"
              >
                {t("settings.joinSubmit")}
              </button>
            </>
          }
          className="join-device-dialog"
          initialFocusRef={joinInputRef}
          onClose={() => setJoinDialogOpen(false)}
          title={t("settings.joinDevice")}
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
            <label htmlFor="join-code-dialog">{t("settings.enterCode")}</label>
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
            <p>{t("settings.enterCodeHint")}</p>
          </form>
        </ModalDialog>
      ) : null}
    </section>
  );
}

function SettingsSection({
  title,
  action,
  children,
}: {
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="settings-section">
      <div className="section-intro">
        <h2>{title}</h2>
        {action}
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
  const { t } = useI18n();
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
              {platformLabel(device.platform, t("common.unknownPlatform"))}
              {device.address ? ` · ${device.address.split(":")[0]}` : ""}
              {device.paused ? ` · ${t("common.paused")}` : ""}
              {!device.isCurrent && device.connectionState === "offline"
                ? ` · ${t("settings.lastOnline", { time: formatRelative(device.lastSeenAt, t) })}`
                : ""}
            </span>
          </div>
          {device.isCurrent ? (
            <span className="current-device-badge">{t("common.currentDevice")}</span>
          ) : (
            <div className="device-menu-wrap">
              <IconButton
                icon={<MoreHorizontal size={17} />}
                label={t("settings.manageDevice", { name: device.name })}
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
                    <Unplug size={15} />{device.paused ? t("settings.resumeDevice") : t("settings.pauseDevice")}
                  </button>
                  <button className="is-danger" onClick={() => onRevoke(device)} type="button"><Trash2 size={15} />{t("settings.revokeDevice")}</button>
                </div>
              ) : null}
            </div>
          )}
        </div>
      )) : (
        <div className="device-empty"><Circle size={12} />{t("settings.noDevices", { status: label })}</div>
      )}
    </div>
  );
}

function formatPairingInput(value: string): string {
  const digits = value.replace(/\D/g, "").slice(0, 6);
  return digits.length > 3 ? `${digits.slice(0, 3)} ${digits.slice(3)}` : digits;
}

function platformLabel(platform: DevicePlatform, unknown: string): string {
  return platform === "macos" ? "macOS" : platform === "linux" ? "Ubuntu / Linux" : unknown;
}
