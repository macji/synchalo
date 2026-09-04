import { AlertTriangle, Check, Info, X } from "lucide-react";

import { IconButton } from "./IconButton";
import { useI18n } from "../i18n";

export interface ToastView {
  id: string;
  message: string;
  tone?: "success" | "warning" | "info";
  actionLabel?: string;
  onAction?: () => void | Promise<void>;
}
interface ToastRegionProps {
  toasts: ToastView[];
  onDismiss: (id: string) => void;
}

export function ToastRegion({ toasts, onDismiss }: ToastRegionProps) {
  const { t } = useI18n();
  return (
    <div aria-live="polite" aria-relevant="additions" className="toast-region">
      {toasts.map((toast) => {
        const tone = toast.tone ?? "info";
        const Icon = tone === "success" ? Check : tone === "warning" ? AlertTriangle : Info;
        return (
          <div className={`toast toast--${tone}`} key={toast.id} role="status">
            <Icon aria-hidden="true" size={17} />
            <span>{toast.message}</span>
            {toast.actionLabel && toast.onAction ? (
              <button
                className="toast-action"
                onClick={() => {
                  void toast.onAction?.();
                  onDismiss(toast.id);
                }}
                type="button"
              >
                {toast.actionLabel}
              </button>
            ) : null}
            <IconButton
              icon={<X size={15} />}
              label={t("common.closeNotification")}
              onClick={() => onDismiss(toast.id)}
            />
          </div>
        );
      })}
    </div>
  );
}
