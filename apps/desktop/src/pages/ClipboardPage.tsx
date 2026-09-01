import {
  ChevronLeft,
  ChevronRight,
  ClipboardCopy,
  Copy,
  Pause,
  Search,
  Star,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import type { ClipboardItemView } from "../api/types";
import { IconButton } from "../components/IconButton";
import { PageHeader } from "../components/PageHeader";
import { formatTime, historyGroup } from "../lib/format";

interface ClipboardPageProps {
  items: ClipboardItemView[];
  initialQuery: string;
  paused: boolean;
  favoritesOnly: boolean;
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
  onRequestPage: (query: string, favoritesOnly: boolean, page: number) => void;
  onCopy: (item: ClipboardItemView) => void;
  onDelete: (item: ClipboardItemView) => void;
  onPinnedChange: (item: ClipboardItemView, pinned: boolean) => void;
  onClear: () => void;
  onPause: (paused: boolean) => void;
  onOpenSettings: () => void;
}

export function ClipboardPage({
  items,
  initialQuery,
  paused,
  favoritesOnly,
  page,
  pageSize,
  totalItems,
  totalPages,
  onRequestPage,
  onCopy,
  onDelete,
  onPinnedChange,
  onClear,
  onPause,
  onOpenSettings,
}: ClipboardPageProps) {
  const [query, setQuery] = useState(initialQuery);
  const scrollRef = useRef<HTMLDivElement>(null);
  const searchReadyRef = useRef(false);
  const requestRef = useRef(onRequestPage);
  const favoritesRef = useRef(favoritesOnly);
  requestRef.current = onRequestPage;
  favoritesRef.current = favoritesOnly;

  useEffect(() => {
    if (!searchReadyRef.current) {
      searchReadyRef.current = true;
      return;
    }
    const timeout = window.setTimeout(
      () => requestRef.current(query, favoritesRef.current, 1),
      160,
    );
    return () => window.clearTimeout(timeout);
  }, [query]);

  useEffect(() => {
    scrollElementToStart(scrollRef.current);
  }, [page]);

  const groups = useMemo(() => {
    const result = new Map<string, ClipboardItemView[]>();
    for (const item of items) {
      const label = historyGroup(item.createdAt);
      result.set(label, [...(result.get(label) ?? []), item]);
    }
    return [...result.entries()];
  }, [items]);

  return (
    <section className="page clipboard-page" aria-labelledby="clipboard-title">
      <PageHeader
        actions={
          <>
            <label className="search-field" htmlFor="clipboard-search">
              <Search aria-hidden="true" size={16} />
              <input
                autoComplete="off"
                id="clipboard-search"
                onChange={(event) => setQuery(event.target.value)}
                placeholder="搜索历史"
                value={query}
              />
              {query ? (
                <IconButton icon={<X size={14} />} label="清除搜索" onClick={() => setQuery("")} />
              ) : (
                <kbd>⌘F</kbd>
              )}
            </label>
            <button
              aria-label={favoritesOnly ? "显示全部历史" : "只看收藏"}
              aria-pressed={favoritesOnly}
              className={`button button--secondary favorite-filter ${favoritesOnly ? "is-active" : ""}`}
              onClick={() => onRequestPage(query, !favoritesOnly, 1)}
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
        eyebrow="TEXT HISTORY"
        title="粘贴板历史"
      />

      {paused ? (
        <div className="inline-notice inline-notice--warning" role="status">
          <Pause aria-hidden="true" size={16} />
          <span>粘贴板自动同步已暂停，本机历史仍可查看和复制。</span>
          <button onClick={() => onPause(false)} type="button">恢复</button>
        </div>
      ) : null}

      <div
        className={`page-scroll list-reading-width ${totalPages > 1 ? "has-pagination" : ""}`}
        ref={scrollRef}
      >
        {groups.length ? (
          groups.map(([label, groupItems]) => (
            <section className="history-group" key={label}>
              <h2>{label}</h2>
              <div className="history-list">
                {groupItems.map((item) => (
                  <article
                    aria-label={`历史内容：${item.content.slice(0, 40)}`}
                    className="clipboard-row"
                    key={item.id}
                  >
                    <span
                      aria-hidden={item.pinned ? undefined : true}
                      aria-label={item.pinned ? "已收藏" : undefined}
                      className="favorite-marker"
                      role={item.pinned ? "img" : undefined}
                    >
                      {item.pinned ? <Star aria-hidden="true" fill="currentColor" size={14} /> : null}
                    </span>
                    <div className="clipboard-copy">
                      <p>{item.content}</p>
                      <div className="row-metadata">
                        <span>{item.sourceDeviceName}</span>
                        <span aria-hidden="true">·</span>
                        <span>{item.direction === "local" ? "本机" : "已接收"}</span>
                        <span aria-hidden="true">·</span>
                        <time dateTime={item.createdAt}>{formatTime(item.createdAt)}</time>
                      </div>
                    </div>
                    <div aria-label="历史操作" className="row-actions">
                      <IconButton
                        icon={<Copy size={16} />}
                        label="复制这条历史"
                        onClick={() => onCopy(item)}
                        tone="accent"
                      />
                      <IconButton
                        icon={<Star fill={item.pinned ? "currentColor" : "none"} size={16} />}
                        label={item.pinned ? "取消收藏" : "收藏"}
                        onClick={() => onPinnedChange(item, !item.pinned)}
                      />
                      <IconButton
                        icon={<Trash2 size={16} />}
                        label="删除这条历史"
                        onClick={() => onDelete(item)}
                        tone="danger"
                      />
                    </div>
                  </article>
                ))}
              </div>
            </section>
          ))
        ) : (
          <div className="empty-state">
            <div className="empty-symbol"><ClipboardCopy size={24} /></div>
            <h2>{query ? "没有匹配的历史" : favoritesOnly ? "还没有收藏内容" : "还没有粘贴板历史"}</h2>
            <p>{query ? "换一个关键词试试。" : favoritesOnly ? "将鼠标移到历史上，点击星标即可收藏。" : "在任意已连接设备复制文本后，会显示在这里。"}</p>
            {!query && !favoritesOnly ? (
              <button className="button button--secondary" onClick={onOpenSettings} type="button">
                查看我的设备
              </button>
            ) : null}
          </div>
        )}

        {totalPages > 1 ? (
          <nav aria-label="粘贴板历史分页" className="pagination">
            <span className="pagination-summary">
              第 {page} / {totalPages} 页 · 共 {totalItems} 条 · 每页 {pageSize} 条
            </span>
            <div className="pagination-controls">
              <button
                aria-label="上一页"
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
                aria-label="下一页"
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
      </div>
    </section>
  );

  function changePage(nextPage: number) {
    scrollElementToStart(scrollRef.current);
    onRequestPage(query, favoritesOnly, nextPage);
  }
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
