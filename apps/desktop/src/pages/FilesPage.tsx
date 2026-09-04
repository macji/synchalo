import {
  Ban,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Copy,
  File,
  FileCheck2,
  Laptop,
  LocateFixed,
  Plus,
  RefreshCw,
  RotateCw,
  Search,
  Send,
  Star,
  Trash2,
  UploadCloud,
  Wifi,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import type {
  DeviceView,
  PairingCodeView,
  TransferHistoryFilter,
  TransferState,
  TransferView,
} from "../api/types";
import { IconButton } from "../components/IconButton";
import { ModalDialog } from "../components/ModalDialog";
import { PageHeader } from "../components/PageHeader";
import { localizeTransferError, useI18n } from "../i18n";
import type { Translate } from "../i18n";
import { formatBytes, formatTime, historyGroup, transferLabel } from "../lib/format";

interface FilesPageProps {
  transfers: TransferView[];
  devices: DeviceView[];
  pairingCode: PairingCodeView | null;
  dragging: boolean;
  initialQuery: string;
  favoritesOnly: boolean;
  filter: TransferHistoryFilter;
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
  targetIds: string[] | null;
  refreshingDevices: boolean;
  onTargetIdsChange: (ids: string[]) => void;
  onNoTargets: () => void;
  onClear: () => void;
  onSelectFiles: (targetIds: string[]) => Promise<void>;
  onBrowserDrop?: (names: string[], targetIds: string[]) => void | Promise<void>;
  onShowSyncCode: () => Promise<void>;
  onCopySyncCode: () => void;
  onResync: (transfer: TransferView, targetIds: string[]) => Promise<void>;
  onPinnedChange: (transfer: TransferView, pinned: boolean) => void;
  onRequestPage: (
    query: string,
    favoritesOnly: boolean,
    filter: TransferHistoryFilter,
    page: number,
  ) => void;
  onRefreshDevices: () => void;
  onRetry: (id: string) => void;
  onCancel: (id: string) => void;
  onDelete: (id: string) => void;
  onOpen: (id: string) => void;
  onReveal: (id: string) => void;
}

export function FilesPage({
  transfers,
  devices,
  pairingCode,
  dragging,
  initialQuery,
  favoritesOnly,
  filter,
  page,
  pageSize,
  totalItems,
  totalPages,
  targetIds,
  refreshingDevices,
  onTargetIdsChange,
  onNoTargets,
  onClear,
  onSelectFiles,
  onBrowserDrop,
  onShowSyncCode,
  onCopySyncCode,
  onResync,
  onPinnedChange,
  onRequestPage,
  onRefreshDevices,
  onRetry,
  onCancel,
  onDelete,
  onOpen,
  onReveal,
}: FilesPageProps) {
  const { locale, t } = useI18n();
  const [query, setQuery] = useState(initialQuery);
  const [browserDragging, setBrowserDragging] = useState(false);
  const [selecting, setSelecting] = useState(false);
  const [syncCodeOpen, setSyncCodeOpen] = useState(false);
  const [syncCodeLoading, setSyncCodeLoading] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const scrollRef = useRef<HTMLDivElement>(null);
  const searchReadyRef = useRef(false);
  const requestRef = useRef(onRequestPage);
  const favoritesRef = useRef(favoritesOnly);
  const filterRef = useRef(filter);
  requestRef.current = onRequestPage;
  favoritesRef.current = favoritesOnly;
  filterRef.current = filter;

  useEffect(() => {
    if (!searchReadyRef.current) {
      searchReadyRef.current = true;
      return;
    }
    const timeout = window.setTimeout(
      () => requestRef.current(query, favoritesRef.current, filterRef.current, 1),
      160,
    );
    return () => window.clearTimeout(timeout);
  }, [query]);

  useEffect(() => {
    scrollElementToStart(scrollRef.current);
  }, [page]);

  const currentDevice = devices.find((device) => device.isCurrent) ?? null;
  const targetDevices = devices.filter((device) => !device.isCurrent);
  const selectableTargetIds = targetDevices
    .filter((device) => !device.paused && device.connectionState === "online")
    .map((device) => device.id);
  const onlineTargetIds = targetDevices
    .filter((device) => !device.paused && device.connectionState === "online")
    .map((device) => device.id);
  const selectedTargetIds = (targetIds ?? []).filter((id) =>
    selectableTargetIds.includes(id),
  );
  const effectiveTargetIds = selectedTargetIds.length ? selectedTargetIds : onlineTargetIds;
  const sendingDisabled = effectiveTargetIds.length === 0;
  const isDragging = dragging || browserDragging;

  const groups = useMemo(() => {
    const result = new Map<string, TransferView[]>();
    for (const transfer of transfers) {
      const label = historyGroup(transfer.createdAt, locale, t);
      result.set(label, [...(result.get(label) ?? []), transfer]);
    }
    return [...result.entries()];
  }, [locale, t, transfers]);

  return (
    <section
      aria-labelledby="files-title"
      className={`page files-page ${isDragging ? "is-dragging" : ""}`}
      onDragEnter={(event) => {
        event.preventDefault();
        if (onBrowserDrop) setBrowserDragging(true);
      }}
      onDragLeave={(event) => {
        if (onBrowserDrop && event.currentTarget === event.target) setBrowserDragging(false);
      }}
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault();
        setBrowserDragging(false);
        if (!onBrowserDrop) return;
        const names = [...event.dataTransfer.files].map((file) => file.name);
        if (!names.length) return;
        if (sendingDisabled) onNoTargets();
        else void onBrowserDrop(names, selectedTargetIds);
      }}
    >
      <PageHeader
        actions={
          <>
            <label className="search-field" htmlFor="file-history-search">
              <Search aria-hidden="true" size={16} />
              <input
                autoComplete="off"
                id="file-history-search"
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("files.search")}
                value={query}
              />
              {query ? (
                <IconButton icon={<X size={14} />} label={t("files.clearSearch")} onClick={() => setQuery("")} />
              ) : (
                <kbd>⌘F</kbd>
              )}
            </label>
            <button
              aria-label={favoritesOnly ? t("files.showAll") : t("files.favoritesOnly")}
              aria-pressed={favoritesOnly}
              className={`button button--secondary favorite-filter ${favoritesOnly ? "is-active" : ""}`}
              onClick={() => onRequestPage(query, !favoritesOnly, filter, 1)}
              type="button"
            >
              <Star fill={favoritesOnly ? "currentColor" : "none"} size={15} />
              {t("common.favorite")}
            </button>
            <button
              className="button button--quiet-danger"
              disabled={totalItems === 0 && !favoritesOnly}
              onClick={onClear}
              type="button"
            >
              {t("common.clear")}
            </button>
          </>
        }
        eyebrow="RELIABLE TRANSFER"
        title={t("files.title")}
      />

      <div className="page-scroll files-workspace" ref={scrollRef}>
        <div className="files-compose-grid">
          <section aria-labelledby="sync-devices-title" className="files-panel device-panel">
            <header className="files-panel-heading">
              <div>
                <h2 id="sync-devices-title">{t("files.myDevices")}</h2>
              </div>
              <div className="files-panel-tools">
                <small>{t("files.deviceCount", { count: devices.length })}</small>
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
              </div>
            </header>

            <div className="sync-device-list">
              {currentDevice ? (
                <div className="sync-device-row is-current">
                  <span className="device-icon"><Laptop aria-hidden="true" size={16} /></span>
                  <span className="sync-device-copy">
                    <strong>{currentDevice.name}</strong>
                    <small>{t("common.localDevice")}</small>
                  </span>
                  <button
                    className="sync-code-trigger"
                    disabled={syncCodeLoading}
                    onClick={() => void showSyncCode()}
                    type="button"
                  >
                    {syncCodeLoading ? t("files.generating") : t("files.showSyncCode")}
                  </button>
                </div>
              ) : null}

              {targetDevices.map((device) => {
                const selected = selectedTargetIds.includes(device.id);
                return (
                  <button
                    aria-pressed={selected}
                    className={`sync-device-row ${selected ? "is-selected" : ""}`}
                    disabled={device.paused || device.connectionState !== "online"}
                    key={device.id}
                    onClick={() => toggleTarget(device.id, selected)}
                    type="button"
                  >
                    <span className={`target-check ${selected ? "is-checked" : ""}`}>
                      {selected ? <Check size={12} /> : null}
                    </span>
                    <span className="sync-device-copy">
                      <strong>{device.name}</strong>
                      <small>{deviceLabel(device, t)}</small>
                    </span>
                    <span className={`status-dot status-dot--${device.connectionState}`} />
                  </button>
                );
              })}
            </div>

            {!targetDevices.length ? (
              <p className="device-panel-empty">{t("files.noDevices")}</p>
            ) : null}
          </section>

          <section aria-labelledby="send-files-title" className="files-panel send-panel">
            <header className="files-panel-heading">
              <div>
                <h2 id="send-files-title">{t("files.syncPanel")}</h2>
              </div>
              <small>
                {selectedTargetIds.length
                  ? t("files.selectedTargets", { count: selectedTargetIds.length })
                  : t("files.allOnline")}
              </small>
            </header>

            {sendingDisabled ? (
              <div className="no-sync-devices" role="status">
                <CircleAlert aria-hidden="true" size={15} />
                <span>{t("errors.noSyncDevices")}</span>
              </div>
            ) : (
              <div className="sync-target-summary">
                <Wifi aria-hidden="true" size={14} />
                {selectedTargetIds.length
                  ? t("files.willSyncSelected", { count: selectedTargetIds.length })
                  : t("files.willSyncAll")}
              </div>
            )}

            <button
              aria-label={t("files.dropLabel")}
              className={`drop-zone ${isDragging ? "is-dragging" : ""} ${selecting ? "is-selecting" : ""}`}
              onClick={() => void chooseFiles()}
              type="button"
            >
              <span className="drop-zone-icon"><UploadCloud aria-hidden="true" size={23} /></span>
              <span className="drop-zone-copy">
                <strong>{isDragging ? t("files.releaseToSend") : t("files.dropOrPaste")}</strong>
                <small>{t("files.pasteHint")}</small>
              </span>
              <span className="drop-zone-select">
                <Plus aria-hidden="true" size={14} />
                {selecting ? t("files.opening") : t("files.select")}
              </span>
            </button>
          </section>
        </div>

        <section
          aria-labelledby="file-history-title"
          className={`files-history-region ${totalPages > 1 ? "has-pagination" : ""}`}
        >
          <div className="history-region-heading">
            <div>
              <h2 id="file-history-title">{t("files.history")}</h2>
            </div>
          </div>

          {groups.length ? (
            groups.map(([label, entries]) => (
              <section className="history-group" key={label}>
                <h3>{label}</h3>
                <div className="transfer-list">
                  {entries.map((transfer) => {
                    const canExpand = transfer.targets.length > 1;
                    const isExpanded = expanded.has(transfer.id);
                    return (
                      <article className={`transfer-row ${transfer.pinned ? "is-pinned" : ""}`} key={transfer.id}>
                        <div className="file-symbol"><File aria-hidden="true" size={20} /></div>
                        <div className="transfer-body">
                          <div className="transfer-heading">
                            <button
                              className="file-name-button"
                              disabled={!canExpand}
                              onClick={() => toggleExpanded(transfer.id, canExpand)}
                              type="button"
                            >
                              {canExpand ? (isExpanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />) : null}
                              <strong>{transfer.fileName}</strong>
                            </button>
                            <TransferStatus state={transfer.state} t={t} />
                          </div>
                          <div className="row-metadata transfer-metadata">
                            <span>{formatBytes(transfer.fileSize)}</span>
                            <span aria-hidden="true">·</span>
                            <span>{directionSummary(transfer, t)}</span>
                            <span aria-hidden="true">·</span>
                            <time dateTime={transfer.createdAt}>{formatTime(transfer.createdAt, locale, t)}</time>
                          </div>

                          {isActive(transfer.state) ? (
                            <div className="transfer-progress-wrap">
                              <div
                                aria-label={t("files.progress", { percent: Math.round(transfer.progress * 100) })}
                                aria-valuemax={100}
                                aria-valuemin={0}
                                aria-valuenow={Math.round(transfer.progress * 100)}
                                className="progress-track"
                                role="progressbar"
                              >
                                <span style={{ width: `${transfer.progress * 100}%` }} />
                              </div>
                              <div className="progress-caption">
                                <span>{Math.round(transfer.progress * 100)}%</span>
                                {transfer.bytesPerSecond ? <span>{formatBytes(transfer.bytesPerSecond)}/s</span> : null}
                                {transfer.etaSeconds ? <span>{t("files.remaining", { seconds: transfer.etaSeconds })}</span> : null}
                              </div>
                            </div>
                          ) : null}

                          {transfer.error ? <p className="transfer-error">{localizeTransferError(transfer.error, t)}</p> : null}
                          {isExpanded ? (
                            <div className="target-list">
                              {transfer.targets.map((target) => (
                                <div key={target.deviceId}>
                                  <span className={`mini-state mini-state--${target.state}`} />
                                  <strong>{target.deviceName}</strong>
                                  <span>{target.state === "completed" ? null : transferLabel(target.state, t)}</span>
                                  {target.bytesPerSecond ? <span>{formatBytes(target.bytesPerSecond)}/s</span> : null}
                                </div>
                              ))}
                            </div>
                          ) : null}
                        </div>
                        <div className="transfer-actions">
                          <IconButton
                            icon={<RotateCw size={16} />}
                            label={t("files.resync")}
                            onClick={() => void resyncTransfer(transfer)}
                            tone="accent"
                          />
                          <IconButton
                            icon={<Star fill={transfer.pinned ? "currentColor" : "none"} size={16} />}
                            label={transfer.pinned ? t("files.unfavorite") : t("files.favorite")}
                            onClick={() => onPinnedChange(transfer, !transfer.pinned)}
                          />
                          {transfer.state === "completed" ? (
                            <>
                              <button className="row-action-text" onClick={() => onOpen(transfer.id)} type="button">{t("common.open")}</button>
                              <IconButton icon={<LocateFixed size={16} />} label={t("files.reveal")} onClick={() => onReveal(transfer.id)} />
                            </>
                          ) : null}
                          {transfer.state === "failed" || transfer.state === "cancelled" ? (
                            <IconButton icon={<RefreshCw size={16} />} label={t("files.retry")} onClick={() => onRetry(transfer.id)} />
                          ) : null}
                          {isActive(transfer.state) ? (
                            <IconButton icon={<Ban size={16} />} label={t("files.cancel")} onClick={() => onCancel(transfer.id)} />
                          ) : null}
                          {(["completed", "failed", "cancelled"] as TransferState[]).includes(transfer.state) ? (
                            <IconButton icon={<Trash2 size={16} />} label={t("files.delete")} onClick={() => onDelete(transfer.id)} tone="danger" />
                          ) : null}
                        </div>
                      </article>
                    );
                  })}
                </div>
              </section>
            ))
          ) : (
            <div className="empty-state file-history-empty">
              <div className="empty-symbol"><FileCheck2 size={24} /></div>
              <h2>{favoritesOnly ? t("files.noFavorites") : query || filter !== "all" ? t("files.noMatch") : t("files.empty")}</h2>
              <p>{favoritesOnly ? t("files.favoriteHint") : t("files.emptyHint")}</p>
            </div>
          )}

          {totalPages > 1 ? (
            <nav aria-label={t("files.pagination")} className="pagination file-pagination">
              <span className="pagination-summary">
                {t("common.pagination", { page, pages: totalPages, count: totalItems, size: pageSize })}
              </span>
              <div className="pagination-controls">
                <button
                  aria-label={t("files.previousPage")}
                  disabled={page <= 1}
                  onClick={() => changePage(page - 1)}
                  type="button"
                >
                  <ChevronLeft size={15} />
                </button>
                {pageRange(page, totalPages).map((pageNumber) => (
                  <button
                    aria-current={pageNumber === page ? "page" : undefined}
                    className={pageNumber === page ? "is-active" : ""}
                    key={pageNumber}
                    onClick={() => changePage(pageNumber)}
                    type="button"
                  >
                    {pageNumber}
                  </button>
                ))}
                <button
                  aria-label={t("files.nextPage")}
                  disabled={page >= totalPages}
                  onClick={() => changePage(page + 1)}
                  type="button"
                >
                  <ChevronRight size={15} />
                </button>
              </div>
            </nav>
          ) : totalItems > 0 ? (
            <p className="single-page-summary">{t("common.items", { count: totalItems })}</p>
          ) : null}
        </section>
      </div>

      {syncCodeOpen ? (
        <ModalDialog
          className="sync-code-dialog"
          contained
          onClose={() => setSyncCodeOpen(false)}
          strongBackdrop
          title={t("files.connectDevice")}
        >
          <div className="sync-code-value">{pairingCode?.code ?? "— — —"}</div>
          <p>{t("files.syncCodeHint")}</p>
          <button
            className="button button--secondary button--small"
            disabled={!pairingCode}
            onClick={onCopySyncCode}
            type="button"
          >
            <Copy size={13} />
            {t("files.copySyncCode")}
          </button>
        </ModalDialog>
      ) : null}
    </section>
  );

  function toggleTarget(id: string, selected: boolean) {
    onTargetIdsChange(
      selected ? selectedTargetIds.filter((targetId) => targetId !== id) : [...selectedTargetIds, id],
    );
  }

  function toggleExpanded(id: string, canExpand: boolean) {
    if (!canExpand) return;
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function showSyncCode() {
    setSyncCodeOpen(true);
    setSyncCodeLoading(true);
    try {
      await onShowSyncCode();
    } finally {
      setSyncCodeLoading(false);
    }
  }

  async function chooseFiles() {
    if (sendingDisabled) {
      onNoTargets();
      return;
    }
    if (selecting) return;
    setSelecting(true);
    try {
      await onSelectFiles(selectedTargetIds);
    } finally {
      setSelecting(false);
    }
  }

  async function resyncTransfer(transfer: TransferView) {
    if (sendingDisabled) {
      onNoTargets();
      return;
    }
    await onResync(transfer, selectedTargetIds);
  }

  function changePage(nextPage: number) {
    scrollElementToStart(scrollRef.current);
    onRequestPage(query, favoritesOnly, filter, nextPage);
  }
}

function TransferStatus({ state, t }: { state: TransferState; t: Translate }) {
  if (state === "completed") return null;
  const Icon = state === "failed" ? CircleAlert : Send;
  return (
    <span className={`transfer-status transfer-status--${state}`}>
      <Icon aria-hidden="true" size={14} />
      {transferLabel(state, t)}
    </span>
  );
}

function directionSummary(transfer: TransferView, t: Translate): string {
  if (transfer.direction === "receiving") {
    return t("files.fromDevice", { name: transfer.sourceDeviceName ?? t("files.otherDevice") });
  }
  if (!transfer.targets.length) return t("files.waitingForDevice");
  return t("files.sendTo", {
    names: transfer.targets.map((target) => target.deviceName).join(t("files.deviceSeparator")),
  });
}

function deviceLabel(device: DeviceView, t: Translate): string {
  if (device.paused) return t("common.paused");
  return device.connectionState === "online" ? t("common.online") : t("common.offline");
}

function isActive(state: TransferState): boolean {
  return ["queued", "transferring", "verifying"].includes(state);
}

function scrollElementToStart(element: HTMLDivElement | null) {
  if (typeof element?.scrollTo === "function") {
    element.scrollTo({ left: 0, top: 0, behavior: "auto" });
  } else if (element) {
    element.scrollTop = 0;
  }
}

function pageRange(page: number, totalPages: number): number[] {
  const start = Math.max(1, Math.min(page - 2, totalPages - 4));
  const end = Math.min(totalPages, start + 4);
  return Array.from({ length: end - start + 1 }, (_, index) => start + index);
}
