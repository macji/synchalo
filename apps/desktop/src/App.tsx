import { AlertTriangle, LoaderCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { api } from "./api/client";
import type {
  AppSnapshot,
  DeviceView,
  Route,
  SettingsPatch,
  TransferHistoryFilter,
  TransferView,
  Unlisten,
  UpdateStatusView,
  UserFacingError,
} from "./api/types";
import { ConfirmDialog, type ConfirmState } from "./components/ConfirmDialog";
import { HaloMark } from "./components/HaloMark";
import { Sidebar } from "./components/Sidebar";
import { ToastRegion, type ToastView } from "./components/ToastRegion";
import { UpdateDialog } from "./components/UpdateDialog";
import { WindowTitlebar } from "./components/WindowTitlebar";
import { localizeError, useI18n } from "./i18n";
import { DeviceOfflineDebouncer } from "./lib/devicePresence";
import { useLinuxWindowMaximized } from "./lib/windowState";
import { ClipboardPage } from "./pages/ClipboardPage";
import { FilesPage } from "./pages/FilesPage";
import { SettingsPage } from "./pages/SettingsPage";

export default function App() {
  const { setPreference, t } = useI18n();
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const windowPlatform = snapshot?.capabilities.platform ?? detectWindowPlatform();
  const linuxWindowMaximized = useLinuxWindowMaximized(windowPlatform);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [deviceOfflineDebouncer] = useState(
    () => new DeviceOfflineDebouncer((devices) => {
      setSnapshot((current) => current ? withDevices(current, devices) : current);
    }),
  );
  const [route, setRoute] = useState<Route>("clipboard");
  const [fatalError, setFatalError] = useState<UserFacingError | null>(null);
  const [toasts, setToasts] = useState<ToastView[]>([]);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);
  const [updatePrompt, setUpdatePrompt] = useState<UpdateStatusView | null>(null);
  const [fileTargetIds, setFileTargetIds] = useState<string[] | null>(null);
  const [nativeFileDragging, setNativeFileDragging] = useState(false);
  const [refreshingDevices, setRefreshingDevices] = useState(false);
  const [fileView, setFileView] = useState<{
    query: string;
    favoritesOnly: boolean;
    filter: TransferHistoryFilter;
    page: number;
    pageSize: number;
    totalPages: number;
  }>({
    query: "",
    favoritesOnly: false,
    filter: "all",
    page: 1,
    pageSize: 100,
    totalPages: 1,
  });
  const [clipboardView, setClipboardView] = useState({
    query: "",
    favoritesOnly: false,
    page: 1,
    pageSize: 100,
    totalPages: 1,
  });
  const toastTimers = useRef(new Map<string, number>());
  const fileTargetIdsRef = useRef<string[] | null>(null);
  const availableFileTargetIdsRef = useRef<string[]>([]);
  const clipboardViewRef = useRef(clipboardView);
  const clipboardRequestRef = useRef(0);
  const refreshClipboardRef = useRef<() => void>(() => undefined);
  const fileViewRef = useRef(fileView);
  const fileRequestRef = useRef(0);
  const refreshFileRef = useRef<() => void>(() => undefined);
  availableFileTargetIdsRef.current = snapshot?.devices
    .filter(
      (device) =>
        !device.isCurrent && !device.paused && device.connectionState === "online",
    )
    .map((device) => device.id) ?? [];
  const selectableFileTargetIds = snapshot?.devices
    .filter(
      (device) =>
        !device.isCurrent && !device.paused && device.connectionState === "online",
    )
    .map((device) => device.id) ?? [];
  fileTargetIdsRef.current = fileTargetIds?.filter((id) =>
    selectableFileTargetIds.includes(id),
  ) ?? null;
  fileViewRef.current = fileView;

  const dismissToast = useCallback((id: string) => {
    const timer = toastTimers.current.get(id);
    if (timer) window.clearTimeout(timer);
    toastTimers.current.delete(id);
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const pushToast = useCallback(
    (toast: Omit<ToastView, "id">, duration = 4_000) => {
      const id = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
      setToasts((current) => [...current.slice(-1), { ...toast, id }]);
      const timer = window.setTimeout(() => dismissToast(id), duration);
      toastTimers.current.set(id, timer);
      return id;
    },
    [dismissToast],
  );

  const reportError = useCallback(
    (error: unknown) => {
      const normalized = normalizeError(error);
      pushToast({ message: localizeError(normalized, t), tone: "warning" }, 6_000);
    },
    [pushToast, t],
  );

  const presentUpdateStatus = useCallback(
    (status: UpdateStatusView) => {
      if (status.state === "checking") {
        pushToast({ message: t("update.checking"), tone: "info" }, 3_000);
      } else if (status.state === "upToDate") {
        setUpdatePrompt(null);
        pushToast({ message: t("update.upToDate"), tone: "success" });
      } else if (status.state === "available" || status.state === "ready") {
        setUpdatePrompt(status);
      } else if (status.state === "downloading") {
        setUpdatePrompt(null);
        pushToast({
          message: t("update.downloading", { version: status.version ?? t("update.newVersion") }),
          tone: "info",
        }, 60_000);
      } else if (status.state === "installing") {
        setUpdatePrompt(null);
        pushToast({ message: t("update.installing"), tone: "info" }, 60_000);
      } else if (status.state === "installed") {
        pushToast({ message: t("update.installed"), tone: "success" }, 10_000);
      } else if (status.state === "cancelled") {
        setUpdatePrompt(null);
        pushToast({ message: t("update.cancelled"), tone: "info" }, 6_000);
      } else if (
        status.state === "unsupported" ||
        status.state === "busy" ||
        status.state === "ignored"
      ) {
        if (status.state === "ignored") setUpdatePrompt(null);
        pushToast({ message: t("update.unavailable"), tone: "info" }, 6_000);
      } else {
        pushToast({
          message: t("update.failed"),
          tone: "warning",
        }, 10_000);
      }
    },
    [pushToast, t],
  );

  const reportQueuedFiles = useCallback(
    (transfers: TransferView[], successMessage: string) => {
      if (!transfers.length) return;
      if (transfers.every((transfer) => transfer.state === "failed")) {
        pushToast({ message: t("toast.filesFailed", { count: transfers.length }), tone: "warning" });
        return;
      }
      if (transfers.some((transfer) =>
        transfer.targets.some((target) => target.state === "failed")
      )) {
        pushToast({ message: t("toast.filesPartial", { count: transfers.length }), tone: "warning" });
        return;
      }
      pushToast({ message: successMessage, tone: "success" });
    },
    [pushToast, t],
  );

  const installUpdate = useCallback(async () => {
    setUpdatePrompt(null);
    try {
      const status = await api.installUpdate();
      if (!api.isTauri) presentUpdateStatus(status);
    } catch (error) {
      reportError(error);
    }
  }, [presentUpdateStatus, reportError]);

  const ignoreUpdate = useCallback(async () => {
    const version = updatePrompt?.version;
    setUpdatePrompt(null);
    if (!version) return;
    try {
      const status = await api.ignoreUpdate(version);
      if (!api.isTauri) presentUpdateStatus(status);
    } catch (error) {
      reportError(error);
    }
  }, [presentUpdateStatus, reportError, updatePrompt?.version]);

  const refreshDevices = useCallback(async () => {
    setRefreshingDevices(true);
    try {
      const devices = await api.refreshDevices();
      deviceOfflineDebouncer.update(devices);
      const onlineCount = devices.filter(
        (device) => !device.isCurrent && device.connectionState === "online",
      ).length;
      pushToast({
        message: onlineCount
          ? t("toast.devicesRefreshedOnline", { count: onlineCount })
          : t("toast.devicesRefreshedEmpty"),
        tone: onlineCount ? "success" : "info",
      });
    } catch (error) {
      reportError(error);
    } finally {
      setRefreshingDevices(false);
    }
  }, [deviceOfflineDebouncer, pushToast, reportError, t]);

  const showNoSyncDevices = useCallback(() => {
    pushToast({ message: t("errors.noSyncDevices"), tone: "warning" }, 6_000);
  }, [pushToast, t]);

  const loadFilePage = useCallback(
    async (
      query: string,
      favoritesOnly: boolean,
      filter: TransferHistoryFilter,
      page: number,
    ) => {
      const requestId = fileRequestRef.current + 1;
      fileRequestRef.current = requestId;
      try {
        const result = await api.listFileHistory(query, favoritesOnly, filter, page);
        if (requestId !== fileRequestRef.current) return;
        setSnapshot((current) => current && {
          ...current,
          fileHistory: result.items,
          fileHistoryTotal: result.totalItems,
        });
        setFileView({
          query,
          favoritesOnly,
          filter,
          page: result.page,
          pageSize: result.pageSize,
          totalPages: result.totalPages,
        });
      } catch (error) {
        reportError(error);
      }
    },
    [reportError],
  );

  refreshFileRef.current = () => {
    const current = fileViewRef.current;
    void loadFilePage(current.query, current.favoritesOnly, current.filter, current.page);
  };

  const enqueueFilePaths = useCallback(
    async (paths: string[], requestedTargetIds?: string[]) => {
      const selectedTargetIds = requestedTargetIds ?? fileTargetIdsRef.current;
      const targetIds = selectedTargetIds?.length ? selectedTargetIds : [];
      if (!targetIds.length && !availableFileTargetIdsRef.current.length) {
        showNoSyncDevices();
        return;
      }
      try {
        const transfers = await api.enqueueFiles(paths, targetIds);
        const current = fileViewRef.current;
        await loadFilePage(current.query, current.favoritesOnly, current.filter, 1);
        reportQueuedFiles(transfers, t("toast.filesQueued", { count: transfers.length }));
      } catch (error) {
        reportError(error);
      }
    },
    [loadFilePage, reportError, reportQueuedFiles, showNoSyncDevices, t],
  );

  const pasteFileClipboard = useCallback(async () => {
    const targetIds = fileTargetIdsRef.current?.length ? fileTargetIdsRef.current : [];
    if (!targetIds.length && !availableFileTargetIdsRef.current.length) {
      showNoSyncDevices();
      return;
    }
    try {
      const transfers = await api.pasteFiles(targetIds);
      const current = fileViewRef.current;
      await loadFilePage(current.query, current.favoritesOnly, current.filter, 1);
      if (transfers.length) {
        reportQueuedFiles(transfers, t("toast.filesQueued", { count: transfers.length }));
      } else {
        pushToast({ message: t("toast.clipboardHasNoFiles"), tone: "info" });
      }
    } catch (error) {
      reportError(error);
    }
  }, [loadFilePage, pushToast, reportError, reportQueuedFiles, showNoSyncDevices, t]);

  const loadClipboardPage = useCallback(
    async (query: string, favoritesOnly: boolean, page: number) => {
      const requestId = clipboardRequestRef.current + 1;
      clipboardRequestRef.current = requestId;
      try {
        const result = await api.listClipboardHistory(query, favoritesOnly, page);
        if (requestId !== clipboardRequestRef.current) return;
        setSnapshot((current) =>
          current && {
            ...current,
            clipboardHistory: result.items,
            clipboardHistoryTotal: result.totalItems,
          },
        );
        setClipboardView({
          query,
          favoritesOnly,
          page: result.page,
          pageSize: result.pageSize,
          totalPages: result.totalPages,
        });
      } catch (error) {
        reportError(error);
      }
    },
    [reportError],
  );

  clipboardViewRef.current = clipboardView;
  refreshClipboardRef.current = () => {
    const current = clipboardViewRef.current;
    void loadClipboardPage(current.query, current.favoritesOnly, current.page);
  };
  const enqueueFilePathsRef = useRef(enqueueFilePaths);
  const presentUpdateStatusRef = useRef(presentUpdateStatus);
  const pushToastRef = useRef(pushToast);
  const reportErrorRef = useRef(reportError);
  const setPreferenceRef = useRef(setPreference);
  const tRef = useRef(t);
  enqueueFilePathsRef.current = enqueueFilePaths;
  presentUpdateStatusRef.current = presentUpdateStatus;
  pushToastRef.current = pushToast;
  reportErrorRef.current = reportError;
  setPreferenceRef.current = setPreference;
  tRef.current = t;

  useEffect(() => {
    let alive = true;
    const unlisteners: Unlisten[] = [];
    const timers = toastTimers.current;
    api
      .getAppState()
      .then((value) => {
        if (alive) {
          deviceOfflineDebouncer.initialize(value.devices);
          setPreferenceRef.current(value.settings.language);
          setSnapshot(value);
          setClipboardView((current) => ({
            ...current,
            page: 1,
            pageSize: 100,
            totalPages: Math.max(1, Math.ceil(value.clipboardHistoryTotal / 100)),
          }));
          setFileView((current) => ({
            ...current,
            page: 1,
            pageSize: 100,
            totalPages: Math.max(1, Math.ceil(value.fileHistoryTotal / 100)),
          }));
        }
      })
      .catch((error) => {
        if (alive) setFatalError(normalizeError(error));
      });
    api.getAppVersion().then((version) => {
      if (alive) setAppVersion(version);
    }).catch((error) => reportErrorRef.current(error));

    const register = async () => {
      unlisteners.push(
        await api.onClipboardAdded(() => refreshClipboardRef.current()),
        await api.onClipboardDeleted(() => refreshClipboardRef.current()),
        await api.onHistoryChanged(() => {
          refreshClipboardRef.current();
          refreshFileRef.current();
        }),
        await api.onDevicesChanged((devices) => deviceOfflineDebouncer.update(devices)),
        await api.onPairingCodeChanged((pairingCode) => setSnapshot((current) => current && { ...current, pairingCode })),
        await api.onPairingRequested((request) => {
          const translate = tRef.current;
          const platform = request.platform === "macos"
            ? "macOS"
            : request.platform === "linux"
              ? "Ubuntu / Linux"
              : translate("common.unknownPlatform");
          setConfirm({
            title: translate("pairing.requestTitle", { name: request.deviceName }),
            body: translate("pairing.requestBody", { platform }),
            confirmLabel: translate("pairing.allow"),
            onConfirm: async () => {
              try {
                await api.respondToPairing(request.requestId, true);
                pushToastRef.current({
                  message: tRef.current("toast.pairingAllowed", { name: request.deviceName }),
                  tone: "success",
                });
              } catch (error) {
                reportErrorRef.current(error);
              }
            },
            onCancel: async () => {
              try {
                await api.respondToPairing(request.requestId, false);
                pushToastRef.current({
                  message: tRef.current("toast.pairingDenied", { name: request.deviceName }),
                  tone: "info",
                });
              } catch (error) {
                reportErrorRef.current(error);
              }
            },
          });
        }),
        await api.onSettingsChanged((settings) => {
          setPreferenceRef.current(settings.language);
          setSnapshot((current) => current && { ...current, settings });
        }),
        await api.onTransferChanged(() => refreshFileRef.current()),
        await api.onSyncStatusChanged((syncStatus) => setSnapshot((current) => current && { ...current, syncStatus })),
        await api.onUserError((error) => reportErrorRef.current(error)),
        await api.onUpdateStatus((status) => presentUpdateStatusRef.current(status)),
        await api.onNavigate(setRoute),
        await api.onFileDragDrop((event) => {
          if (event.type === "enter" || event.type === "over") {
            setRoute("files");
            setNativeFileDragging(true);
          } else if (event.type === "leave") {
            setNativeFileDragging(false);
          } else {
            setRoute("files");
            setNativeFileDragging(false);
            if (event.paths.length) void enqueueFilePathsRef.current(event.paths);
          }
        }),
      );
    };
    void register();

    return () => {
      alive = false;
      unlisteners.forEach((unlisten) => unlisten());
      timers.forEach((timer) => window.clearTimeout(timer));
      deviceOfflineDebouncer.dispose();
    };
  }, [deviceOfflineDebouncer]);

  useEffect(() => {
    const preventContextMenu = (event: MouseEvent) => event.preventDefault();
    document.addEventListener("contextmenu", preventContextMenu, true);
    return () => document.removeEventListener("contextmenu", preventContextMenu, true);
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const modifier = event.metaKey || event.ctrlKey;
      if (modifier && event.key === "1") {
        event.preventDefault();
        setRoute("clipboard");
      } else if (modifier && event.key === "2") {
        event.preventDefault();
        setRoute("files");
      } else if (modifier && event.key === ",") {
        event.preventDefault();
        setRoute("settings");
      } else if (modifier && event.key.toLowerCase() === "f") {
        const search = document.querySelector<HTMLInputElement>(route === "clipboard" ? "#clipboard-search" : ".files-page .search-field input");
        if (search) {
          event.preventDefault();
          search.focus();
        }
      } else if (modifier && event.key.toLowerCase() === "v" && route === "files") {
        const target = event.target as HTMLElement | null;
        if (target?.tagName !== "INPUT" && target?.tagName !== "TEXTAREA") {
          event.preventDefault();
          void pasteFileClipboard();
        }
      } else if (event.key === "Escape") {
        void confirm?.onCancel?.();
        setConfirm(null);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [confirm, pasteFileClipboard, route]);

  const paused = snapshot?.syncStatus.state === "paused";
  const page = useMemo(() => {
    if (!snapshot) return null;
    if (route === "clipboard") {
      return (
        <ClipboardPage
          favoritesOnly={clipboardView.favoritesOnly}
          initialQuery={clipboardView.query}
          items={snapshot.clipboardHistory}
          onClear={() => setConfirm({
            title: t("clipboard.clearTitle"),
            body: t("clipboard.clearBody"),
            confirmLabel: t("clipboard.clearConfirm"),
            danger: true,
            onConfirm: async () => {
              const removed = await api.clearClipboardHistory();
              const view = clipboardViewRef.current;
              await loadClipboardPage(view.query, view.favoritesOnly, 1);
              pushToast({ message: t("toast.historyCleared", { count: removed }), tone: "success" });
            },
          })}
          onCopy={(item) => {
            void api.copyHistoryItem(item.id).then(() => pushToast({ message: t("toast.copied"), tone: "success" })).catch(reportError);
          }}
          onDelete={(item) => {
            void api.deleteClipboardItem(item.id).then(async () => {
              const view = clipboardViewRef.current;
              await loadClipboardPage(view.query, view.favoritesOnly, view.page);
              pushToast({
                message: t("toast.historyDeleted"),
                tone: "info",
                actionLabel: t("common.undo"),
                onAction: async () => {
                  await api.restoreClipboardItem(item);
                  const currentView = clipboardViewRef.current;
                  await loadClipboardPage(
                    currentView.query,
                    currentView.favoritesOnly,
                    currentView.page,
                  );
                },
              }, 5_000);
            }).catch(reportError);
          }}
          onOpenSettings={() => setRoute("settings")}
          onPause={(value) => void handlePause(value)}
          onPinnedChange={(item, value) => {
            void api.setClipboardPinned(item.id, value).then(async () => {
              const view = clipboardViewRef.current;
              await loadClipboardPage(view.query, view.favoritesOnly, view.page);
            }).catch(reportError);
          }}
          onRequestPage={(query, favoritesOnly, page) => {
            void loadClipboardPage(query, favoritesOnly, page);
          }}
          page={clipboardView.page}
          pageSize={clipboardView.pageSize}
          paused={paused}
          totalItems={snapshot.clipboardHistoryTotal}
          totalPages={clipboardView.totalPages}
        />
      );
    }
    if (route === "files") {
      return (
        <FilesPage
          devices={snapshot.devices}
          dragging={nativeFileDragging}
          favoritesOnly={fileView.favoritesOnly}
          filter={fileView.filter}
          initialQuery={fileView.query}
          onBrowserDrop={api.isTauri ? undefined : enqueueFilePaths}
          onCancel={(id) => void updateTransfer(api.cancelTransfer(id))}
          onClear={() => setConfirm({
            title: t("files.clearTitle"),
            body: t("files.clearBody"),
            confirmLabel: t("files.clearConfirm"),
            danger: true,
            onConfirm: async () => {
              const removed = await api.clearFileHistory();
              const view = fileViewRef.current;
              await loadFilePage(view.query, view.favoritesOnly, view.filter, 1);
              pushToast({ message: t("toast.recordsCleared", { count: removed }), tone: "success" });
            },
          })}
          onDelete={(id) => {
            void api.deleteTransfer(id).then(() => {
              const current = fileViewRef.current;
              return loadFilePage(
                current.query,
                current.favoritesOnly,
                current.filter,
                current.page,
              );
            }).catch(reportError);
          }}
          onCopySyncCode={() => {
            void api.copyPairingCode()
              .then(() => pushToast({ message: t("toast.syncCodeCopied"), tone: "success" }))
              .catch(reportError);
          }}
          onNoTargets={showNoSyncDevices}
          onOpen={(id) => void api.openTransfer(id).catch(reportError)}
          onPinnedChange={(transfer, pinned) => {
            void api.setTransferPinned(transfer.id, pinned)
              .then(() => {
                const current = fileViewRef.current;
                return loadFilePage(
                  current.query,
                  current.favoritesOnly,
                  current.filter,
                  current.page,
                );
              })
              .catch(reportError);
          }}
          onRequestPage={(query, favoritesOnly, filter, page) => {
            void loadFilePage(query, favoritesOnly, filter, page);
          }}
          onRefreshDevices={() => void refreshDevices()}
          onResync={async (transfer, targetIds) => {
            try {
              const transfers = await api.resyncTransfer(transfer.id, targetIds);
              const current = fileViewRef.current;
              await loadFilePage(current.query, current.favoritesOnly, current.filter, 1);
              reportQueuedFiles(transfers, t("toast.resyncQueued", { name: transfer.fileName }));
            } catch (error) {
              reportError(error);
            }
          }}
          onRetry={(id) => void updateTransfer(api.retryTransfer(id))}
          onReveal={(id) => void api.revealTransfer(id).catch(reportError)}
          onSelectFiles={async (targetIds) => {
            try {
              const transfers = await api.selectFiles(targetIds);
              const current = fileViewRef.current;
              await loadFilePage(current.query, current.favoritesOnly, current.filter, 1);
              reportQueuedFiles(transfers, t("toast.filesQueued", { count: transfers.length }));
            } catch (error) {
              reportError(error);
            }
          }}
          onShowSyncCode={async () => {
            try {
              const pairingCode = await api.generatePairingCode();
              setSnapshot((current) => current && { ...current, pairingCode });
            } catch (error) {
              reportError(error);
            }
          }}
          onTargetIdsChange={setFileTargetIds}
          page={fileView.page}
          pageSize={fileView.pageSize}
          pairingCode={snapshot.pairingCode}
          refreshingDevices={refreshingDevices}
          targetIds={fileTargetIds}
          totalItems={snapshot.fileHistoryTotal}
          totalPages={fileView.totalPages}
          transfers={snapshot.fileHistory}
        />
      );
    }
    return (
      <SettingsPage
        appVersion={appVersion}
        capabilities={snapshot.capabilities}
        devices={snapshot.devices}
        onCopyCode={() => {
          void api.copyPairingCode().then(() => {
            pushToast({ message: t("toast.syncCodeCopied"), tone: "success" });
          }).catch(reportError);
        }}
        onCheckForUpdates={async () => {
          try {
            const status = await api.checkForUpdates();
            if (!api.isTauri) presentUpdateStatus(status);
          } catch (error) {
            reportError(error);
          }
        }}
        onGenerateCode={() => void api.generatePairingCode().then((pairingCode) => {
          setSnapshot((current) => current && { ...current, pairingCode });
        }).catch(reportError)}
        onJoin={(code) => void api.joinWithCode(code).then((device) => {
          setSnapshot((current) => current
            ? withDevices(current, [
                ...current.devices.filter((entry) => entry.id !== device.id),
                device,
              ])
            : current);
          pushToast({ message: t("toast.connected", { name: device.name }), tone: "success" });
        }).catch(reportError)}
        onOpenDirectory={() => void api.openReceiveDirectory().catch(reportError)}
        onPauseDevice={(device, value) => {
          void api.setDevicePaused(device.id, value).then((updated) => {
            setSnapshot((current) => current
              ? withDevices(
                current,
                current.devices.map((entry) =>
                  entry.id === updated.id ? { ...entry, paused: updated.paused } : entry,
                ),
              )
              : current);
            pushToast({
              message: value
                ? t("toast.devicePaused", { name: device.name })
                : t("toast.deviceResumed", { name: device.name }),
              tone: "info",
            });
          }).catch(reportError);
        }}
        onRefreshDevices={() => void refreshDevices()}
        onRevoke={(device) => setConfirm({
          title: t("settings.revokeTitle", { name: device.name }),
          body: t("settings.revokeBody"),
          confirmLabel: t("settings.revokeDevice"),
          danger: true,
          onConfirm: async () => {
            await api.revokeDevice(device.id);
            setSnapshot((current) => current
              ? withDevices(
                  current,
                  current.devices.filter((entry) => entry.id !== device.id),
                )
              : current);
          },
        })}
        onSelectDirectory={() => void api.selectReceiveDirectory().then((settings) => {
          if (settings) setSnapshot((current) => current && { ...current, settings });
        }).catch(reportError)}
        onUpdate={(patch) => void updateSettings(patch)}
        pairingCode={snapshot.pairingCode}
        refreshingDevices={refreshingDevices}
        settings={snapshot.settings}
      />
    );

    async function handlePause(value: boolean) {
      try {
        const syncStatus = await api.pauseSync(value);
        setSnapshot((current) => current && { ...current, syncStatus });
      } catch (error) {
        reportError(error);
      }
    }

    async function updateTransfer(promise: Promise<TransferView>) {
      try {
        await promise;
        const current = fileViewRef.current;
        await loadFilePage(current.query, current.favoritesOnly, current.filter, current.page);
      } catch (error) {
        reportError(error);
      }
    }

    async function updateSettings(patch: SettingsPatch) {
      try {
        const settings = await api.updateSettings(patch);
        setPreference(settings.language);
        setSnapshot((current) => current && { ...current, settings });
      } catch (error) {
        reportError(error);
      }
    }
  }, [
    appVersion,
    clipboardView,
    enqueueFilePaths,
    fileTargetIds,
    fileView,
    loadClipboardPage,
    loadFilePage,
    nativeFileDragging,
    paused,
    presentUpdateStatus,
    pushToast,
    refreshDevices,
    refreshingDevices,
    reportError,
    reportQueuedFiles,
    route,
    showNoSyncDevices,
    snapshot,
    setPreference,
    t,
  ]);

  if (fatalError) {
    return (
      <div
        className="window-frame"
        data-platform={windowPlatform}
        data-window-maximized={linuxWindowMaximized}
      >
        <WindowTitlebar platform={windowPlatform} />
        <main className="fatal-screen">
          <HaloMark size={44} />
          <AlertTriangle aria-hidden="true" size={24} />
          <h1>{t("app.fatalTitle")}</h1>
          <p>{localizeError(fatalError, t)}</p>
          {fatalError.detail ? <code>{fatalError.detail}</code> : null}
          <button className="button button--primary" onClick={() => window.location.reload()} type="button">{t("app.retry")}</button>
        </main>
      </div>
    );
  }

  if (!snapshot) {
    return (
      <div
        className="window-frame"
        data-platform={windowPlatform}
        data-window-maximized={linuxWindowMaximized}
      >
        <WindowTitlebar platform={windowPlatform} />
        <main className="loading-screen">
          <HaloMark size={40} />
          <LoaderCircle className="loading-spinner" size={20} />
          <span>{t("app.loading")}</span>
        </main>
      </div>
    );
  }

  return (
    <div
      className="window-frame"
      data-platform={windowPlatform}
      data-window-maximized={linuxWindowMaximized}
    >
      <WindowTitlebar platform={windowPlatform} />
      <div className="app-shell" data-route={route}>
        <Sidebar
          onNavigate={setRoute}
          onPause={(value) => void api.pauseSync(value).then((syncStatus) => {
            setSnapshot((current) => current && { ...current, syncStatus });
          }).catch(reportError)}
          route={route}
          status={snapshot.syncStatus}
        />
        <main className="content-pane">{page}</main>
        <ToastRegion onDismiss={dismissToast} toasts={toasts} />
        <ConfirmDialog onClose={() => setConfirm(null)} state={confirm} />
        <UpdateDialog
          onDismiss={() => setUpdatePrompt(null)}
          onIgnore={() => void ignoreUpdate()}
          onInstall={() => void installUpdate()}
          status={updatePrompt}
        />
      </div>
    </div>
  );
}

function detectWindowPlatform(): DeviceView["platform"] {
  const userAgent = navigator.userAgent.toLowerCase();
  if (userAgent.includes("mac")) return "macos";
  if (userAgent.includes("linux")) return "linux";
  return "unknown";
}

function withDevices(snapshot: AppSnapshot, devices: DeviceView[]): AppSnapshot {
  return {
    ...snapshot,
    devices,
    syncStatus: {
      ...snapshot.syncStatus,
      onlineCount: devices.filter(
        (device) => device.connectionState === "online",
      ).length,
      offlineCount: devices.filter(
        (device) => !device.isCurrent && device.connectionState === "offline",
      ).length,
    },
  };
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
    message: typeof error === "string" ? error : "Operation failed",
    detail: null,
    recoverable: true,
  };
}
