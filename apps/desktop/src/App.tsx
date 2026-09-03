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
  UserFacingError,
} from "./api/types";
import { ConfirmDialog, type ConfirmState } from "./components/ConfirmDialog";
import { HaloMark } from "./components/HaloMark";
import { Sidebar } from "./components/Sidebar";
import { ToastRegion, type ToastView } from "./components/ToastRegion";
import { DeviceOfflineDebouncer } from "./lib/devicePresence";
import { NO_SYNC_DEVICES_MESSAGE } from "./lib/messages";
import { ClipboardPage } from "./pages/ClipboardPage";
import { FilesPage } from "./pages/FilesPage";
import { SettingsPage } from "./pages/SettingsPage";

export default function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [deviceOfflineDebouncer] = useState(
    () => new DeviceOfflineDebouncer((devices) => {
      setSnapshot((current) => current ? withDevices(current, devices) : current);
    }),
  );
  const [route, setRoute] = useState<Route>("clipboard");
  const [fatalError, setFatalError] = useState<UserFacingError | null>(null);
  const [toasts, setToasts] = useState<ToastView[]>([]);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);
  const [fileTargetIds, setFileTargetIds] = useState<string[] | null>(null);
  const [nativeFileDragging, setNativeFileDragging] = useState(false);
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
      pushToast({ message: normalized.message, tone: "warning" }, 6_000);
    },
    [pushToast],
  );

  const reportQueuedFiles = useCallback(
    (transfers: TransferView[], successMessage: string) => {
      if (!transfers.length) return;
      if (transfers.every((transfer) => transfer.state === "failed")) {
        pushToast({ message: `${transfers.length} 个文件同步失败`, tone: "warning" });
        return;
      }
      if (transfers.some((transfer) =>
        transfer.targets.some((target) => target.state === "failed")
      )) {
        pushToast({ message: `${transfers.length} 个文件已开始同步，部分目标失败`, tone: "warning" });
        return;
      }
      pushToast({ message: successMessage, tone: "success" });
    },
    [pushToast],
  );

  const showNoSyncDevices = useCallback(() => {
    pushToast({ message: NO_SYNC_DEVICES_MESSAGE, tone: "warning" }, 6_000);
  }, [pushToast]);

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
        reportQueuedFiles(transfers, `${transfers.length} 个文件已加入队列`);
      } catch (error) {
        reportError(error);
      }
    },
    [loadFilePage, reportError, reportQueuedFiles, showNoSyncDevices],
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
        reportQueuedFiles(transfers, `${transfers.length} 个文件已加入队列`);
      } else {
        pushToast({ message: "粘贴板中没有文件", tone: "info" });
      }
    } catch (error) {
      reportError(error);
    }
  }, [loadFilePage, pushToast, reportError, reportQueuedFiles, showNoSyncDevices]);

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

  useEffect(() => {
    let alive = true;
    const unlisteners: Unlisten[] = [];
    const timers = toastTimers.current;
    api
      .getAppState()
      .then((value) => {
        if (alive) {
          deviceOfflineDebouncer.initialize(value.devices);
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
          const platform = request.platform === "macos" ? "macOS" : request.platform === "linux" ? "Ubuntu / Linux" : "未知平台";
          setConfirm({
            title: `允许 ${request.deviceName} 加入？`,
            body: `${platform} 设备已通过当前同步码验证。允许后，它可以接收本同步空间中的粘贴板和文件。`,
            confirmLabel: "允许加入",
            onConfirm: async () => {
              try {
                await api.respondToPairing(request.requestId, true);
                pushToast({ message: `已允许 ${request.deviceName} 加入`, tone: "success" });
              } catch (error) {
                reportError(error);
              }
            },
            onCancel: async () => {
              try {
                await api.respondToPairing(request.requestId, false);
                pushToast({ message: `已拒绝 ${request.deviceName}`, tone: "info" });
              } catch (error) {
                reportError(error);
              }
            },
          });
        }),
        await api.onSettingsChanged((settings) => setSnapshot((current) => current && { ...current, settings })),
        await api.onTransferChanged(() => refreshFileRef.current()),
        await api.onSyncStatusChanged((syncStatus) => setSnapshot((current) => current && { ...current, syncStatus })),
        await api.onUserError(reportError),
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
            if (event.paths.length) void enqueueFilePaths(event.paths);
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
  }, [deviceOfflineDebouncer, enqueueFilePaths, pushToast, reportError]);

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
            title: "清空粘贴板历史？",
            body: "收藏条目会保留，其他本机历史将被永久删除。此操作不会影响其他设备。",
            confirmLabel: "清空历史",
            danger: true,
            onConfirm: async () => {
              const removed = await api.clearClipboardHistory();
              const view = clipboardViewRef.current;
              await loadClipboardPage(view.query, view.favoritesOnly, 1);
              pushToast({ message: `已清除 ${removed} 条历史`, tone: "success" });
            },
          })}
          onCopy={(item) => {
            void api.copyHistoryItem(item.id).then(() => pushToast({ message: "已复制", tone: "success" })).catch(reportError);
          }}
          onDelete={(item) => {
            void api.deleteClipboardItem(item.id).then(async () => {
              const view = clipboardViewRef.current;
              await loadClipboardPage(view.query, view.favoritesOnly, view.page);
              pushToast({
                message: "历史已删除",
                tone: "info",
                actionLabel: "撤销",
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
            title: "清空同步记录？",
            body: "收藏记录和正在进行的任务会保留，其他本机同步记录将被永久删除。已发送或接收的文件不会被删除。",
            confirmLabel: "清空记录",
            danger: true,
            onConfirm: async () => {
              const removed = await api.clearFileHistory();
              const view = fileViewRef.current;
              await loadFilePage(view.query, view.favoritesOnly, view.filter, 1);
              pushToast({ message: `已清除 ${removed} 条同步记录`, tone: "success" });
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
              .then(() => pushToast({ message: "同步码已复制", tone: "success" }))
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
          onResync={async (transfer, targetIds) => {
            try {
              const transfers = await api.resyncTransfer(transfer.id, targetIds);
              const current = fileViewRef.current;
              await loadFilePage(current.query, current.favoritesOnly, current.filter, 1);
              reportQueuedFiles(transfers, `${transfer.fileName} 已再次加入同步队列`);
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
              reportQueuedFiles(transfers, `${transfers.length} 个文件已加入队列`);
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
          targetIds={fileTargetIds}
          totalItems={snapshot.fileHistoryTotal}
          totalPages={fileView.totalPages}
          transfers={snapshot.fileHistory}
        />
      );
    }
    return (
      <SettingsPage
        capabilities={snapshot.capabilities}
        devices={snapshot.devices}
        onCopyCode={() => {
          void api.copyPairingCode().then(() => {
            pushToast({ message: "同步码已复制", tone: "success" });
          }).catch(reportError);
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
          pushToast({ message: `已连接 ${device.name}`, tone: "success" });
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
            pushToast({ message: value ? `已暂停向 ${device.name} 同步` : `已恢复向 ${device.name} 同步`, tone: "info" });
          }).catch(reportError);
        }}
        onRevoke={(device) => setConfirm({
          title: `撤销 ${device.name}？`,
          body: "该设备将立即失去同步权限；已经保存在双方设备上的文件和历史不会被删除。",
          confirmLabel: "撤销设备",
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
        setSnapshot((current) => current && { ...current, settings });
      } catch (error) {
        reportError(error);
      }
    }
  }, [
    clipboardView,
    enqueueFilePaths,
    fileTargetIds,
    fileView,
    loadClipboardPage,
    loadFilePage,
    nativeFileDragging,
    paused,
    pushToast,
    reportError,
    reportQueuedFiles,
    route,
    showNoSyncDevices,
    snapshot,
  ]);

  if (fatalError) {
    return (
      <main className="fatal-screen">
        <HaloMark size={44} />
        <AlertTriangle aria-hidden="true" size={24} />
        <h1>SyncHalo 无法启动</h1>
        <p>{fatalError.message}</p>
        {fatalError.detail ? <code>{fatalError.detail}</code> : null}
        <button className="button button--primary" onClick={() => window.location.reload()} type="button">重新尝试</button>
      </main>
    );
  }

  if (!snapshot) {
    return (
      <main className="loading-screen">
        <HaloMark size={40} />
        <LoaderCircle className="loading-spinner" size={20} />
        <span>正在启动本地同步服务…</span>
      </main>
    );
  }

  return (
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
    </div>
  );
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
    message: typeof error === "string" ? error : "操作失败",
    detail: null,
    recoverable: true,
  };
}
