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
import { formatBytes, formatTime, historyGroup, transferLabel } from "../lib/format";
import { NO_SYNC_DEVICES_MESSAGE } from "../lib/messages";

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
  onRetry,
  onCancel,
  onDelete,
  onOpen,
  onReveal,
}: FilesPageProps) {
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
      const label = historyGroup(transfer.createdAt);
      result.set(label, [...(result.get(label) ?? []), transfer]);
    }
    return [...result.entries()];
  }, [transfers]);

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
                placeholder="搜索历史"
                value={query}
              />
              {query ? (
                <IconButton icon={<X size={14} />} label="清除文件搜索" onClick={() => setQuery("")} />
              ) : (
                <kbd>⌘F</kbd>
              )}
            </label>
            <button
              aria-label={favoritesOnly ? "显示全部文件历史" : "只看收藏文件"}
              aria-pressed={favoritesOnly}
              className={`button button--secondary favorite-filter ${favoritesOnly ? "is-active" : ""}`}
              onClick={() => onRequestPage(query, !favoritesOnly, filter, 1)}
              type="button"
            >
              <Star fill={favoritesOnly ? "currentColor" : "none"} size={15} />
              收藏
            </button>
            <button
              className="button button--quiet-danger"
              disabled={totalItems === 0 && !favoritesOnly}
              onClick={onClear}
              type="button"
            >
              清空
            </button>
          </>
        }
        eyebrow="RELIABLE TRANSFER"
        title="同步文件"
      />

      <div className="page-scroll files-workspace" ref={scrollRef}>
        <div className="files-compose-grid">
          <section aria-labelledby="sync-devices-title" className="files-panel device-panel">
            <header className="files-panel-heading">
              <div>
                <h2 id="sync-devices-title">我的设备</h2>
              </div>
              <small>{devices.length} 台</small>
            </header>

            <div className="sync-device-list">
              {currentDevice ? (
                <div className="sync-device-row is-current">
                  <span className="device-icon"><Laptop aria-hidden="true" size={16} /></span>
                  <span className="sync-device-copy">
                    <strong>{currentDevice.name}</strong>
                    <small>本机</small>
                  </span>
                  <button
                    className="sync-code-trigger"
                    disabled={syncCodeLoading}
                    onClick={() => void showSyncCode()}
                    type="button"
                  >
                    {syncCodeLoading ? "生成中…" : "显示同步码"}
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
                      <small>{deviceLabel(device)}</small>
                    </span>
                    <span className={`status-dot status-dot--${device.connectionState}`} />
                  </button>
                );
              })}
            </div>

            {!targetDevices.length ? (
              <p className="device-panel-empty">添加另一台设备后，可在这里选择同步目标。</p>
            ) : null}
          </section>

          <section aria-labelledby="send-files-title" className="files-panel send-panel">
            <header className="files-panel-heading">
              <div>
                <h2 id="send-files-title">文件同步</h2>
              </div>
              <small>
                {selectedTargetIds.length ? `${selectedTargetIds.length} 个指定目标` : "全部在线设备"}
              </small>
            </header>

            {sendingDisabled ? (
              <div className="no-sync-devices" role="status">
                <CircleAlert aria-hidden="true" size={15} />
                <span>{NO_SYNC_DEVICES_MESSAGE}</span>
              </div>
            ) : (
              <div className="sync-target-summary">
                <Wifi aria-hidden="true" size={14} />
                {selectedTargetIds.length
                  ? `将同步到 ${selectedTargetIds.length} 台指定设备`
                  : `未指定目标，将同步到全部 ${onlineTargetIds.length} 台在线设备`}
              </div>
            )}

            <button
              aria-label="拖入文件或选择文件"
              className={`drop-zone ${isDragging ? "is-dragging" : ""} ${selecting ? "is-selecting" : ""}`}
              onClick={() => void chooseFiles()}
              type="button"
            >
              <span className="drop-zone-icon"><UploadCloud aria-hidden="true" size={23} /></span>
              <span className="drop-zone-copy">
                <strong>{isDragging ? "松开发送文件" : "把文件拖入或者直接粘贴文件"}</strong>
                <small>复制文件后，在此页面按 Ctrl/Cmd + V 自动同步；也可使用文件选择器</small>
              </span>
              <span className="drop-zone-select">
                <Plus aria-hidden="true" size={14} />
                {selecting ? "正在打开…" : "选择文件"}
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
              <h2 id="file-history-title">同步历史</h2>
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
                            <TransferStatus state={transfer.state} />
                          </div>
                          <div className="row-metadata transfer-metadata">
                            <span>{formatBytes(transfer.fileSize)}</span>
                            <span aria-hidden="true">·</span>
                            <span>{directionSummary(transfer)}</span>
                            <span aria-hidden="true">·</span>
                            <time dateTime={transfer.createdAt}>{formatTime(transfer.createdAt)}</time>
                          </div>

                          {isActive(transfer.state) ? (
                            <div className="transfer-progress-wrap">
                              <div
                                aria-label={`传输进度 ${Math.round(transfer.progress * 100)}%`}
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
                                {transfer.etaSeconds ? <span>剩余约 {transfer.etaSeconds} 秒</span> : null}
                              </div>
                            </div>
                          ) : null}

                          {transfer.error ? <p className="transfer-error">{transfer.error}</p> : null}
                          {isExpanded ? (
                            <div className="target-list">
                              {transfer.targets.map((target) => (
                                <div key={target.deviceId}>
                                  <span className={`mini-state mini-state--${target.state}`} />
                                  <strong>{target.deviceName}</strong>
                                  <span>{target.state === "completed" ? null : transferLabel(target.state)}</span>
                                  {target.bytesPerSecond ? <span>{formatBytes(target.bytesPerSecond)}/s</span> : null}
                                </div>
                              ))}
                            </div>
                          ) : null}
                        </div>
                        <div className="transfer-actions">
                          <IconButton
                            icon={<RotateCw size={16} />}
                            label="再次同步"
                            onClick={() => void resyncTransfer(transfer)}
                            tone="accent"
                          />
                          <IconButton
                            icon={<Star fill={transfer.pinned ? "currentColor" : "none"} size={16} />}
                            label={transfer.pinned ? "取消收藏文件" : "收藏文件"}
                            onClick={() => onPinnedChange(transfer, !transfer.pinned)}
                          />
                          {transfer.state === "completed" ? (
                            <>
                              <button className="row-action-text" onClick={() => onOpen(transfer.id)} type="button">打开</button>
                              <IconButton icon={<LocateFixed size={16} />} label="定位文件" onClick={() => onReveal(transfer.id)} />
                            </>
                          ) : null}
                          {transfer.state === "failed" || transfer.state === "cancelled" ? (
                            <IconButton icon={<RefreshCw size={16} />} label="重试当前任务" onClick={() => onRetry(transfer.id)} />
                          ) : null}
                          {isActive(transfer.state) ? (
                            <IconButton icon={<Ban size={16} />} label="取消任务" onClick={() => onCancel(transfer.id)} />
                          ) : null}
                          {(["completed", "failed", "cancelled"] as TransferState[]).includes(transfer.state) ? (
                            <IconButton icon={<Trash2 size={16} />} label="删除历史" onClick={() => onDelete(transfer.id)} tone="danger" />
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
              <h2>{favoritesOnly ? "还没有收藏的文件" : query || filter !== "all" ? "没有匹配的文件任务" : "还没有同步文件"}</h2>
              <p>{favoritesOnly ? "点击历史右侧的星标即可收藏。" : "使用上方区域拖入、选择或粘贴文件。"}</p>
            </div>
          )}

          {totalPages > 1 ? (
            <nav aria-label="文件历史分页" className="pagination file-pagination">
              <span className="pagination-summary">
                第 {page} / {totalPages} 页 · 共 {totalItems} 条 · 每页 {pageSize} 条
              </span>
              <div className="pagination-controls">
                <button
                  aria-label="文件历史上一页"
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
                  aria-label="文件历史下一页"
                  disabled={page >= totalPages}
                  onClick={() => changePage(page + 1)}
                  type="button"
                >
                  <ChevronRight size={15} />
                </button>
              </div>
            </nav>
          ) : totalItems > 0 ? (
            <p className="single-page-summary">共 {totalItems} 条</p>
          ) : null}
        </section>
      </div>

      {syncCodeOpen ? (
        <ModalDialog
          className="sync-code-dialog"
          contained
          onClose={() => setSyncCodeOpen(false)}
          strongBackdrop
          title="连接另一台设备"
        >
          <div className="sync-code-value">{pairingCode?.code ?? "— — —"}</div>
          <p>在另一台设备输入，同步码 60 秒内有效。</p>
          <button
            className="button button--secondary button--small"
            disabled={!pairingCode}
            onClick={onCopySyncCode}
            type="button"
          >
            <Copy size={13} />
            复制同步码
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

function TransferStatus({ state }: { state: TransferState }) {
  if (state === "completed") return null;
  const Icon = state === "failed" ? CircleAlert : Send;
  return (
    <span className={`transfer-status transfer-status--${state}`}>
      <Icon aria-hidden="true" size={14} />
      {transferLabel(state)}
    </span>
  );
}

function directionSummary(transfer: TransferView): string {
  if (transfer.direction === "receiving") return `来自 ${transfer.sourceDeviceName ?? "其他设备"}`;
  if (!transfer.targets.length) return "等待选择设备";
  return `发送到 ${transfer.targets.map((target) => target.deviceName).join("、")}`;
}

function deviceLabel(device: DeviceView): string {
  if (device.paused) return "已暂停";
  return device.connectionState === "online" ? "在线" : "离线";
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
