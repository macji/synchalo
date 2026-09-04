import { ArrowUpCircle, CheckCircle2, Sparkles } from "lucide-react";
import { useRef } from "react";

import type { UpdateStatusView } from "../api/types";
import { useI18n } from "../i18n";
import { ModalDialog } from "./ModalDialog";

interface UpdateDialogProps {
  status: UpdateStatusView | null;
  onDismiss: () => void;
  onIgnore: () => void;
  onInstall: () => void;
}

export function UpdateDialog({ status, onDismiss, onIgnore, onInstall }: UpdateDialogProps) {
  const { t } = useI18n();
  const primaryActionRef = useRef<HTMLButtonElement>(null);

  if (!status || (status.state !== "available" && status.state !== "ready")) return null;
  const downloaded = status.state === "ready";
  const version = status.version ?? t("update.newVersion");

  return (
    <ModalDialog
      actions={
        <>
          <button
            className="button button--secondary"
            onClick={downloaded ? onDismiss : onIgnore}
            type="button"
          >
            {downloaded ? t("update.later") : t("update.ignore")}
          </button>
          <button
            className="button button--primary"
            onClick={onInstall}
            ref={primaryActionRef}
            type="button"
          >
            {downloaded ? <CheckCircle2 size={16} /> : <ArrowUpCircle size={16} />}
            {downloaded ? t("update.installRestart") : t("update.updateNow")}
          </button>
        </>
      }
      className="update-dialog"
      initialFocusRef={primaryActionRef}
      onClose={onDismiss}
      title={downloaded ? t("update.downloadedTitle") : t("update.availableTitle")}
    >
      <div className="update-hero">
        <div className="update-symbol" aria-hidden="true">
          <Sparkles size={22} />
        </div>
        <div>
          <span className="update-kicker">SYNCHALO UPDATE</span>
          <strong>SyncHalo {version}</strong>
          <p>
            {downloaded
              ? t("update.downloadedDescription")
              : t("update.availableDescription")}
          </p>
        </div>
      </div>
      <div className="update-notes">
        <span>{t("update.releaseNotes")}</span>
        <p>{status.notes ?? t("update.defaultNotes")}</p>
      </div>
    </ModalDialog>
  );
}
