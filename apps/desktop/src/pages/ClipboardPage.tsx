import {
  ChevronLeft,
  ChevronRight,
  ClipboardCopy,
  Copy,
  Eye,
  Pause,
  Search,
  Star,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import type { ClipboardItemView } from "../api/types";
import { IconButton } from "../components/IconButton";
import { ModalDialog } from "../components/ModalDialog";
import { PageHeader } from "../components/PageHeader";
import { useI18n } from "../i18n";
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
  const { locale, t } = useI18n();
  const [query, setQuery] = useState(initialQuery);
  const [previewItem, setPreviewItem] = useState<ClipboardItemView | null>(null);
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
      const label = historyGroup(item.createdAt, locale, t);
      result.set(label, [...(result.get(label) ?? []), item]);
    }
    return [...result.entries()];
  }, [items, locale, t]);

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
                placeholder={t("clipboard.search")}
                value={query}
              />
              {query ? (
                <IconButton icon={<X size={14} />} label={t("clipboard.clearSearch")} onClick={() => setQuery("")} />
              ) : (
                <kbd>⌘F</kbd>
              )}
            </label>
            <button
              aria-label={favoritesOnly ? t("clipboard.showAll") : t("clipboard.favoritesOnly")}
              aria-pressed={favoritesOnly}
              className={`button button--secondary favorite-filter ${favoritesOnly ? "is-active" : ""}`}
              onClick={() => onRequestPage(query, !favoritesOnly, 1)}
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
        eyebrow="TEXT HISTORY"
        title={t("clipboard.title")}
      />

      {paused ? (
        <div className="inline-notice inline-notice--warning" role="status">
          <Pause aria-hidden="true" size={16} />
          <span>{t("clipboard.pauseNotice")}</span>
          <button onClick={() => onPause(false)} type="button">{t("clipboard.resume")}</button>
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
                    aria-label={t("clipboard.itemLabel", { content: item.content.slice(0, 40) })}
                    className="clipboard-row"
                    key={item.id}
                  >
                    <div className="clipboard-copy">
                      <p>{item.content}</p>
                      <div className="row-metadata">
                        {item.pinned ? (
                          <>
                            <span aria-label={t("clipboard.favorited")} className="favorite-marker" role="img">
                              <Star aria-hidden="true" fill="currentColor" size={13} />
                            </span>
                            <span aria-hidden="true">·</span>
                          </>
                        ) : null}
                        <span>{item.sourceDeviceName}</span>
                        <span aria-hidden="true">·</span>
                        <span>{item.direction === "local" ? t("clipboard.local") : t("clipboard.received")}</span>
                        <span aria-hidden="true">·</span>
                        <time dateTime={item.createdAt}>{formatTime(item.createdAt, locale, t)}</time>
                      </div>
                    </div>
                    <div aria-label={t("clipboard.actions")} className="row-actions">
                      <IconButton
                        icon={<Eye size={16} />}
                        label={t("clipboard.preview")}
                        onClick={() => setPreviewItem(item)}
                      />
                      <IconButton
                        icon={<Copy size={16} />}
                        label={t("clipboard.copyItem")}
                        onClick={() => onCopy(item)}
                        tone="accent"
                      />
                      <IconButton
                        icon={<Star fill={item.pinned ? "currentColor" : "none"} size={16} />}
                        label={item.pinned ? t("clipboard.unfavorite") : t("common.favorite")}
                        onClick={() => onPinnedChange(item, !item.pinned)}
                      />
                      <IconButton
                        icon={<Trash2 size={16} />}
                        label={t("clipboard.deleteItem")}
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
            <h2>{query ? t("clipboard.noMatch") : favoritesOnly ? t("clipboard.noFavorites") : t("clipboard.empty")}</h2>
            <p>{query ? t("clipboard.tryAnother") : favoritesOnly ? t("clipboard.favoriteHint") : t("clipboard.emptyHint")}</p>
            {!query && !favoritesOnly ? (
              <button className="button button--secondary" onClick={onOpenSettings} type="button">
                {t("clipboard.viewDevices")}
              </button>
            ) : null}
          </div>
        )}

        {totalPages > 1 ? (
          <nav aria-label={t("clipboard.pagination")} className="pagination">
            <span className="pagination-summary">
              {t("common.pagination", { page, pages: totalPages, count: totalItems, size: pageSize })}
            </span>
            <div className="pagination-controls">
              <button
                aria-label={t("clipboard.previousPage")}
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
                aria-label={t("clipboard.nextPage")}
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
      </div>

      {previewItem ? (
        <ModalDialog
          className="clipboard-preview-dialog"
          onClose={() => setPreviewItem(null)}
          title={t("clipboard.fullContent")}
        >
          <textarea
            aria-label={t("clipboard.fullContentLabel")}
            className="clipboard-preview-text"
            readOnly
            value={previewItem.content}
          />
        </ModalDialog>
      ) : null}
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
