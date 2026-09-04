import { X } from "lucide-react";
import { useEffect, useRef } from "react";
import type { ReactNode, RefObject } from "react";

import { IconButton } from "./IconButton";
import { useI18n } from "../i18n";

interface ModalDialogProps {
  title: string;
  children: ReactNode;
  actions?: ReactNode;
  className?: string;
  contained?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  onClose: () => void;
  strongBackdrop?: boolean;
}

export function ModalDialog({
  title,
  children,
  actions,
  className = "",
  contained = false,
  initialFocusRef,
  onClose,
  strongBackdrop = false,
}: ModalDialogProps) {
  const { t } = useI18n();
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const target = initialFocusRef?.current ?? panelRef.current;
    target?.focus();
  }, [initialFocusRef]);

  return (
    <div
      aria-label={title}
      aria-modal="true"
      className={`dialog-backdrop ${contained ? "dialog-backdrop--contained" : ""} ${strongBackdrop ? "dialog-backdrop--strong" : ""}`}
      onKeyDown={(event) => {
        if (event.key === "Escape") onClose();
      }}
      onMouseDown={(event) => {
        if (event.currentTarget === event.target) onClose();
      }}
      role="dialog"
    >
      <div className={`modal-dialog ${className}`} ref={panelRef} tabIndex={-1}>
        <div className="dialog-header">
          <h2>{title}</h2>
          <IconButton icon={<X size={17} />} label={t("common.closeDialog", { title })} onClick={onClose} />
        </div>
        <div className="dialog-content">{children}</div>
        {actions ? <div className="dialog-actions">{actions}</div> : null}
      </div>
    </div>
  );
}
