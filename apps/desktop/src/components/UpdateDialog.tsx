import { ArrowUpCircle, CheckCircle2, Sparkles } from "lucide-react";
import { useRef } from "react";

import type { UpdateStatusView } from "../api/types";
import { ModalDialog } from "./ModalDialog";

interface UpdateDialogProps {
  status: UpdateStatusView | null;
  onDismiss: () => void;
  onIgnore: () => void;
  onInstall: () => void;
}

export function UpdateDialog({ status, onDismiss, onIgnore, onInstall }: UpdateDialogProps) {
  const primaryActionRef = useRef<HTMLButtonElement>(null);

  if (!status || (status.state !== "available" && status.state !== "ready")) return null;
  const downloaded = status.state === "ready";
  const version = status.version ?? "新版";

  return (
    <ModalDialog
      actions={
        <>
          <button
            className="button button--secondary"
            onClick={downloaded ? onDismiss : onIgnore}
            type="button"
          >
            {downloaded ? "稍后" : "忽略此版本"}
          </button>
          <button
            className="button button--primary"
            onClick={onInstall}
            ref={primaryActionRef}
            type="button"
          >
            {downloaded ? <CheckCircle2 size={16} /> : <ArrowUpCircle size={16} />}
            {downloaded ? "安装并重启" : "立即更新"}
          </button>
        </>
      }
      className="update-dialog"
      initialFocusRef={primaryActionRef}
      onClose={onDismiss}
      title={downloaded ? "更新已下载" : "发现新版本"}
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
              ? "安装包已完成下载和签名验证，可以在准备好后安装。"
              : "新版本可用。更新只会在你确认后下载、验证和安装。"}
          </p>
        </div>
      </div>
      <div className="update-notes">
        <span>发布说明</span>
        <p>{status.notes ?? "本次更新包含稳定性和使用体验改进。"}</p>
      </div>
    </ModalDialog>
  );
}
